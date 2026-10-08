use super::{
    runtime::{Event, HandleInner, Ingress, enqueue_weak},
    *,
};
use crate::auth::AuthClock;
use liar_core::game::Seat;
use liar_protocol::{
    game::{AckStatus, PROTOCOL_VERSION, PublicAction},
    online::{OnlineError, OnlineEvent, OnlinePayload, RecordingStatus},
};
use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use uuid::Uuid;

struct Sink {
    authority: ConnectionAuthority,
    epoch: u32,
    outgoing: mpsc::Sender<OnlineEvent>,
}
struct Work {
    token: Uuid,
    started: u64,
}
struct Actor {
    state: MatchState,
    handle: Weak<HandleInner>,
    owner: Weak<MatchRegistry>,
    authorities: Arc<AuthorityRegistry>,
    auth_clock: Arc<dyn AuthClock>,
    proofs: Arc<Semaphore>,
    results: Arc<dyn ResultRepository>,
    sinks: [Option<Sink>; 2],
    delayed_disconnect: [Option<(u32, u64)>; 2],
    sequences: [u32; 2],
    work: [Option<Work>; 2],
    dirty: [bool; 2],
    recording: Option<RecordingStatus>,
    finished_at: Option<u64>,
}
pub(super) fn spawn(
    state: MatchState,
    mut receiver: mpsc::Receiver<Ingress>,
    handle: Weak<HandleInner>,
    registry: &Arc<MatchRegistry>,
    permit: OwnedSemaphorePermit,
) {
    let ticker = handle.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let Some(handle) = ticker.upgrade() else {
                break;
            };
            if handle.tx_closed() {
                break;
            }
            let _ = handle.enqueue(Event::Tick);
        }
    });
    let mut actor = Actor {
        state,
        handle,
        owner: Arc::downgrade(registry),
        authorities: registry.authorities.clone(),
        auth_clock: registry.auth_clock.clone(),
        proofs: registry.proofs.clone(),
        results: registry.results.clone(),
        sinks: [None, None],
        delayed_disconnect: [None, None],
        sequences: [0; 2],
        work: [None, None],
        dirty: [false; 2],
        recording: None,
        finished_at: None,
    };
    tokio::spawn(async move {
        let _permit = permit;
        while let Some(ingress) = receiver.recv().await {
            let at = ingress.at;
            actor.process(ingress);
            actor.publish(at);
            actor.schedule(at);
            if actor
                .finished_at
                .is_some_and(|end| at.saturating_sub(end) >= 30000)
            {
                break;
            }
        }
        if let Some(owner) = actor.owner.upgrade() {
            owner.finish(actor.state.id(), true);
        }
        for sink in actor.sinks.iter().flatten() {
            sink.authority.close();
        }
    });
}
impl Actor {
    fn process(&mut self, ingress: Ingress) {
        let at = ingress.at;
        match ingress.event {
            Event::Attach {
                authority,
                seat,
                outgoing,
                reply,
            } => {
                let result =
                    self.authorities
                        .with_authority(&authority, self.auth_clock.now(), || {
                            self.state.attach(seat, at)
                        });
                match result {
                    Ok(Ok(epoch)) => {
                        self.sinks[seat.index()] = Some(Sink {
                            authority,
                            epoch,
                            outgoing,
                        });
                        self.delayed_disconnect[seat.index()] = None;
                        if reply.send(Ok(epoch)).is_err() {
                            self.retire(seat, at);
                        } else {
                            self.emit(
                                seat,
                                OnlinePayload::Snapshot {
                                    session_epoch: epoch,
                                    view: self.state.view(seat),
                                },
                                at,
                            );
                        }
                    }
                    Ok(Err(error)) | Err(error) => {
                        authority.close();
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Event::Reject {
                authority,
                seat,
                code,
            } => {
                if self.sinks[seat.index()]
                    .as_ref()
                    .is_some_and(|s| s.authority.token() == authority.token())
                {
                    self.emit(seat, OnlinePayload::Error { code }, at);
                }
            }
            Event::Input {
                authority,
                seat,
                input,
            } => {
                if !self.sinks[seat.index()]
                    .as_ref()
                    .is_some_and(|s| s.authority.token() == authority.token())
                {
                    return;
                }
                let analysis = matches!(
                    input.action,
                    PublicAction::Open { .. } | PublicAction::Accuse { .. }
                );
                let ack =
                    self.authorities
                        .with_authority(&authority, self.auth_clock.now(), || {
                            self.state.apply(seat, input, at)
                        });
                match ack {
                    Ok(ack) => {
                        if analysis
                            && matches!(
                                ack,
                                OnlinePayload::Ack {
                                    status: AckStatus::Applied,
                                    duplicate: false,
                                    ..
                                }
                            )
                        {
                            self.dirty[seat.index()] = true;
                        }
                        self.emit(seat, ack, at);
                    }
                    Err(_) => self.retire(seat, at),
                }
            }
            Event::Disconnect {
                account,
                token,
                seat,
                epoch,
            } => {
                if self.sinks[seat.index()].as_ref().is_some_and(|s| {
                    s.authority.token() == token
                        && s.authority.account() == account
                        && s.epoch == epoch
                }) {
                    self.retire(seat, at);
                }
            }
            Event::Tick => {
                for seat in [Seat::One, Seat::Two] {
                    let i = seat.index();
                    if self.sinks[i].as_ref().is_some_and(|s| {
                        self.authorities
                            .with_authority(&s.authority, self.auth_clock.now(), || ())
                            .is_err()
                    }) {
                        self.retire(seat, at);
                    }
                    if let Some((epoch, deadline)) = self.delayed_disconnect[i]
                        && at >= deadline
                    {
                        self.state.disconnect(seat, epoch, at);
                        self.delayed_disconnect[i] = None;
                    }
                    if self.work[i]
                        .as_ref()
                        .is_some_and(|work| at.saturating_sub(work.started) > 100)
                    {
                        self.work[i] = None;
                        self.dirty[i] = true;
                    }
                }
                self.state.advance(at);
            }
            Event::Proof {
                seat,
                token,
                result,
            } => {
                let i = seat.index();
                if self.work[i].as_ref().is_some_and(|work| {
                    work.token == token && at.saturating_sub(work.started) <= 100
                }) {
                    self.work[i] = None;
                    self.dirty[i] = match result {
                        Ok(result) => !self.state.commit_proof(result),
                        Err(_) => false,
                    };
                }
            }
            Event::Stored { result } => {
                self.recording = Some(if result.is_ok() {
                    RecordingStatus::Saved
                } else {
                    RecordingStatus::Failed
                });
                for seat in [Seat::One, Seat::Two] {
                    self.emit(
                        seat,
                        OnlinePayload::MatchEnd {
                            view: self.state.view(seat),
                            recording: self.recording.expect("Set above"),
                        },
                        at,
                    );
                }
            }
        }
    }
    fn retire(&mut self, seat: Seat, at: u64) {
        if let Some(sink) = self.sinks[seat.index()].take() {
            sink.authority.close();
            if self.authorities.is_replacement(&sink.authority) {
                self.delayed_disconnect[seat.index()] = Some((sink.epoch, at.saturating_add(2000)));
            } else {
                self.state.disconnect(seat, sink.epoch, at);
            }
        }
    }
    fn emit(&mut self, seat: Seat, payload: OnlinePayload, at: u64) {
        let i = seat.index();
        let Some(sink) = &self.sinks[i] else {
            return;
        };
        let Some(seq) = self.sequences[i].checked_add(1) else {
            self.retire(seat, at);
            return;
        };
        let event = OnlineEvent {
            v: PROTOCOL_VERSION,
            match_id: self.state.id().to_string(),
            server_seq: seq,
            server_time_ms: at,
            payload,
        };
        if matches!(
            self.authorities
                .with_authority(&sink.authority, self.auth_clock.now(), || sink
                    .outgoing
                    .try_send(event)
                    .is_ok()),
            Ok(true)
        ) {
            self.sequences[i] = seq;
        } else {
            self.retire(seat, at);
        }
    }
    fn publish(&mut self, at: u64) {
        let finished = self.state.finished();
        if let Some(result) = finished
            && self.recording.is_none()
        {
            self.finished_at = Some(at);
            self.recording = Some(RecordingStatus::Pending);
            if let Some(owner) = self.owner.upgrade() {
                owner.finish(self.state.id(), false);
            }
            let repository = self.results.clone();
            let handle = self.handle.clone();
            tokio::spawn(async move {
                let mut saved = Err(OnlineError::Unavailable);
                for retry in 0..3 {
                    saved = tokio::time::timeout(
                        Duration::from_secs(2),
                        repository.save(result.clone()),
                    )
                    .await
                    .unwrap_or(Err(OnlineError::Unavailable));
                    if saved.is_ok() {
                        break;
                    }
                    if retry < 2 {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                enqueue_weak(&handle, Event::Stored { result: saved });
            });
        }
        for (seat, view) in self.state.take_changes() {
            let payload = if let Some(recording) = self.recording {
                OnlinePayload::MatchEnd { view, recording }
            } else {
                OnlinePayload::Delta { view }
            };
            self.emit(seat, payload, at);
        }
    }
    fn schedule(&mut self, at: u64) {
        if self.recording.is_some() {
            return;
        }
        for seat in [Seat::One, Seat::Two] {
            let i = seat.index();
            if !self.dirty[i] || self.work[i].is_some() {
                continue;
            }
            let Ok(permit) = self.proofs.clone().try_acquire_owned() else {
                continue;
            };
            let token = Uuid::new_v4();
            let job = self.state.proof_work(seat);
            self.work[i] = Some(Work { token, started: at });
            self.dirty[i] = false;
            let handle = self.handle.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    job.run()
                })
                .await
                .unwrap_or(Err(liar_core::game::Rejection::InvalidBoard));
                enqueue_weak(
                    &handle,
                    Event::Proof {
                        seat,
                        token,
                        result,
                    },
                );
            });
        }
    }
}
