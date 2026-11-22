use super::*;
use crate::{lobby::LobbyAuthentication, rooms::RoundKey};
use liar_core::game::{AnalysisResult, Rejection};
use liar_protocol::{
    game::{AckStatus, PublicAction, PublicError},
    online::{OnlineError, RecordingStatus},
    rooms::{ROOM_PROTOCOL_VERSION, RoomResultSummary},
    rounds::{RoundEvent, RoundInput, RoundPayload},
};
use std::{
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use uuid::Uuid;

pub trait RoundLifecycle: Send + Sync {
    fn contains(&self, key: RoundKey, account: Uuid) -> bool;
    fn begin(&self, key: RoundKey, now: u64, leases: &[ConnectionAuthority]) -> bool;
    fn complete(&self, key: RoundKey, now: u64, result: Option<RoomResultSummary>);
}

pub struct RoundAdmission {
    pub authentication: LobbyAuthentication,
    pub leases: Vec<ConnectionAuthority>,
    pub lifecycle: Arc<dyn RoundLifecycle>,
    pub journal: Arc<dyn AdmissionJournal>,
}

struct SeatBinding {
    account: Uuid,
    hash: [u8; 32],
    slot: Arc<Semaphore>,
}

struct Endpoint {
    key: RoundKey,
    seats: Vec<SeatBinding>,
    outgoing: usize,
    sender: mpsc::Sender<(u64, Event)>,
    ingress_at: Mutex<u64>,
    clock: Arc<dyn MatchClock>,
    lifecycle: Arc<dyn RoundLifecycle>,
    _socket_slots: Arc<OwnedSemaphorePermit>,
}

#[derive(Clone)]
pub struct RoundHandle(Arc<Endpoint>);

pub(super) struct RoundSocketSlot {
    seat: usize,
    permit: OwnedSemaphorePermit,
}

pub struct RoundConnection {
    handle: RoundHandle,
    authority: ConnectionAuthority,
    seat: usize,
    epoch: u32,
    receiver: mpsc::Receiver<RoundEvent>,
    _permit: OwnedSemaphorePermit,
}

enum Event {
    Attach {
        seat: usize,
        authority: ConnectionAuthority,
        sender: mpsc::Sender<RoundEvent>,
        reply: oneshot::Sender<Result<u32, OnlineError>>,
    },
    Input {
        seat: usize,
        token: Uuid,
        input: RoundInput,
    },
    Detach {
        seat: usize,
        token: Uuid,
    },
    Tick,
}

impl Event {
    fn input_order(&self) -> Option<(usize, u32)> {
        match self {
            Self::Input { seat, input, .. } => Some((*seat, input.client_seq)),
            _ => None,
        }
    }
}

impl Endpoint {
    fn send(&self, event: Event) -> Result<(), OnlineError> {
        let mut previous = self
            .ingress_at
            .lock()
            .map_err(|_| OnlineError::Unavailable)?;
        let at = self.clock.now_ms().max(*previous);
        self.sender
            .try_send((at, event))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => OnlineError::Capacity,
                mpsc::error::TrySendError::Closed(_) => OnlineError::NotMatched,
            })?;
        *previous = at;
        Ok(())
    }
}

impl RoundHandle {
    pub fn key(&self) -> RoundKey {
        self.0.key
    }

    pub(super) fn reserve_socket(
        &self,
        account: Uuid,
        hash: [u8; 32],
    ) -> Result<RoundSocketSlot, OnlineError> {
        let seat = self
            .0
            .seats
            .iter()
            .position(|member| member.account == account && member.hash == hash)
            .ok_or(OnlineError::NotMatched)?;
        let permit = self.0.seats[seat]
            .slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| OnlineError::Capacity)?;
        Ok(RoundSocketSlot { seat, permit })
    }

    pub async fn connect(
        &self,
        authority: ConnectionAuthority,
    ) -> Result<RoundConnection, OnlineError> {
        let slot = self.reserve_socket(authority.account(), authority.session_hash())?;
        self.connect_reserved(authority, slot).await
    }

    pub(super) async fn connect_reserved(
        &self,
        authority: ConnectionAuthority,
        slot: RoundSocketSlot,
    ) -> Result<RoundConnection, OnlineError> {
        let seat = slot.seat;
        let (sender, receiver) = mpsc::channel(self.0.outgoing);
        let (reply, epoch) = oneshot::channel();
        self.0.send(Event::Attach {
            seat,
            authority: authority.clone(),
            sender,
            reply,
        })?;
        let epoch = tokio::time::timeout(Duration::from_secs(2), epoch)
            .await
            .map_err(|_| OnlineError::Unavailable)?
            .map_err(|_| OnlineError::Unavailable)??;
        Ok(RoundConnection {
            handle: self.clone(),
            authority,
            seat,
            epoch,
            receiver,
            _permit: slot.permit,
        })
    }
}

impl RoundConnection {
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn authority(&self) -> &ConnectionAuthority {
        &self.authority
    }
    pub(super) fn current_member(&self) -> bool {
        self.handle
            .0
            .lifecycle
            .contains(self.handle.key(), self.authority.account())
    }
    pub async fn next(&mut self) -> Option<RoundEvent> {
        self.receiver.recv().await
    }
    pub fn send(&self, input: RoundInput) -> Result<(), OnlineError> {
        self.handle.0.send(Event::Input {
            seat: self.seat,
            token: self.authority.token(),
            input,
        })
    }
}

impl Drop for RoundConnection {
    fn drop(&mut self) {
        let _ = self.handle.0.send(Event::Detach {
            seat: self.seat,
            token: self.authority.token(),
        });
        self.authority.close();
    }
}

struct Sink {
    authority: ConnectionAuthority,
    sender: mpsc::Sender<RoundEvent>,
    epoch: u32,
    revision: u64,
    snapshot: bool,
}

struct SeatState {
    sink: Option<Sink>,
    sequence: u32,
    departed: bool,
    dirty: bool,
    proof: Option<oneshot::Receiver<Result<AnalysisResult, Rejection>>>,
}

struct Actor {
    state: RoundState,
    seats: Vec<SeatState>,
    admission: RoundAdmission,
    registry: Weak<MatchRegistry>,
    playing: bool,
    established: bool,
}

pub(super) fn spawn(
    runtime: &tokio::runtime::Handle,
    state: RoundState,
    admission: RoundAdmission,
    registry: &Arc<MatchRegistry>,
    sockets: Arc<OwnedSemaphorePermit>,
    reservation: RaceReservation,
) -> RoundHandle {
    let (sender, receiver) = mpsc::channel(registry.limits.mailbox);
    let count = state.accounts().len();
    let handle = RoundHandle(Arc::new(Endpoint {
        key: state.key(),
        seats: admission
            .leases
            .iter()
            .map(|lease| SeatBinding {
                account: lease.account(),
                hash: lease.session_hash(),
                slot: Arc::new(Semaphore::new(1)),
            })
            .collect(),
        outgoing: registry.limits.outgoing,
        sender,
        ingress_at: Mutex::new(0),
        clock: registry.clock.clone(),
        lifecycle: admission.lifecycle.clone(),
        _socket_slots: sockets,
    }));
    let ticker = Arc::downgrade(&handle.0);
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let Some(handle) = ticker.upgrade() else {
                break;
            };
            if handle.sender.is_closed() {
                break;
            }
            let _ = handle.send(Event::Tick);
        }
    });
    let actor = Actor {
        state,
        admission,
        registry: Arc::downgrade(registry),
        playing: false,
        established: false,
        seats: (0..count)
            .map(|_| SeatState {
                sink: None,
                sequence: 0,
                departed: false,
                dirty: false,
                proof: None,
            })
            .collect(),
    };
    runtime.spawn(actor.run(receiver, reservation));
    handle
}

impl Actor {
    async fn run(
        mut self,
        mut receiver: mpsc::Receiver<(u64, Event)>,
        _reservation: RaceReservation,
    ) {
        let mut deferred = None;
        loop {
            let next = match deferred.take() {
                Some(entry) => Some(entry),
                None => receiver.recv().await,
            };
            let Some((at, event)) = next else {
                break;
            };
            let Some(registry) = self.registry.upgrade() else {
                break;
            };
            let mut batch = vec![event];
            if batch[0].input_order().is_some() {
                // Seal the ingress already waiting at this instant. A connection change
                // is a boundary, so an attach/leave cannot move past a command.
                for _ in 1..registry.limits.mailbox {
                    let Ok((received, next)) = receiver.try_recv() else {
                        break;
                    };
                    if received == at && next.input_order().is_some() {
                        batch.push(next);
                    } else if received != at || !matches!(next, Event::Tick) {
                        deferred = Some((received, next));
                        break;
                    }
                }
                batch.sort_by_key(Event::input_order);
            }
            for event in batch {
                if self.step(&registry, event, at).await {
                    return;
                }
            }
        }
    }

    async fn step(&mut self, registry: &MatchRegistry, event: Event, at: u64) -> bool {
        self.check_members(registry, at);
        if !self.playing
            && at >= self.state.starts_at()
            && self.state.finished().is_none()
            && (!self.all_connected() || !registry.accepting() || !self.start(registry, at).await)
        {
            self.state.cancel(at);
        }
        if self.playing {
            let _ = self.state.advance(at);
        }
        if self.state.finished().is_none() {
            self.process(registry, event, at);
        }
        if let Some(finished) = self.state.finished() {
            self.finish(registry, finished, at).await;
            return true;
        }
        self.publish(at);
        self.proofs(registry);
        false
    }

    fn all_connected(&self) -> bool {
        self.seats.iter().all(|seat| seat.sink.is_some())
    }

    fn check_members(&mut self, registry: &MatchRegistry, at: u64) {
        let key = self.state.key();
        let mut departed = Vec::new();
        for seat in 0..self.seats.len() {
            let member = &self.admission.leases[seat];
            let auth = &self.admission.authentication;
            if self.seats[seat].departed {
                continue;
            }
            if !self.admission.lifecycle.contains(key, member.account())
                || auth
                    .authorities
                    .with_authority(member, auth.clock.now(), || ())
                    .is_err()
            {
                self.seats[seat].departed = true;
                self.seats[seat].sink.take();
                self.state.anonymize(seat);
                if self.playing {
                    registry.release_round_account(key.round_id, member.account());
                }
                departed.push(seat);
            } else if self.seats[seat].sink.as_ref().is_some_and(|sink| {
                sink.sender.is_closed()
                    || registry
                        .authorities
                        .with_authority(&sink.authority, registry.auth_clock.now(), || ())
                        .is_err()
            }) {
                self.detach(seat, at);
            }
        }
        if !departed.is_empty() {
            if self.playing {
                self.state.forfeit_many(&departed, at);
            } else {
                self.state.cancel(at);
            }
        }
    }

    async fn start(&mut self, registry: &MatchRegistry, at: u64) -> bool {
        let key = self.state.key();
        let leases: Vec<_> = self.admission.leases.iter().collect();
        let auth = &self.admission.authentication;
        if auth
            .authorities
            .with_authorities(&leases, auth.clock.now(), || ())
            .is_err()
        {
            return false;
        }
        if !matches!(
            tokio::time::timeout(
                Duration::from_secs(2),
                self.admission.journal.start_race(key.round_id)
            )
            .await,
            Ok(Ok(SaveResult::Saved))
        ) {
            return false;
        }
        self.check_members(registry, at);
        if !self.all_connected()
            || !self
                .admission
                .lifecycle
                .begin(key, at, &self.admission.leases)
        {
            return false;
        }
        self.playing = true;
        for seat in &mut self.seats {
            if let Some(sink) = &mut seat.sink {
                sink.revision = u64::MAX;
            }
        }
        true
    }

    fn process(&mut self, registry: &MatchRegistry, event: Event, at: u64) {
        match event {
            Event::Tick => {}
            Event::Detach { seat, token } => {
                if self.matches_socket(seat, token) {
                    self.detach(seat, at);
                }
            }
            Event::Attach {
                seat,
                authority,
                sender,
                reply,
            } => {
                let member = &self.admission.leases[seat];
                let auth = &self.admission.authentication;
                let epoch = if self.seats[seat].departed
                    || authority.session_hash() != member.session_hash()
                    || authority.account() != member.account()
                {
                    Err(OnlineError::Unauthorized)
                } else {
                    auth.authorities
                        .with_authority(member, auth.clock.now(), || {
                            registry.authorities.with_authority(
                                &authority,
                                registry.auth_clock.now(),
                                || self.state.attach(seat, at),
                            )?
                        })
                        .and_then(|result| result)
                };
                if let Ok(epoch) = epoch {
                    self.seats[seat].sink = Some(Sink {
                        authority,
                        sender,
                        epoch,
                        revision: 0,
                        snapshot: false,
                    });
                    if reply.send(Ok(epoch)).is_err() {
                        self.detach(seat, at);
                        return;
                    }
                    if !self.established {
                        let connected =
                            self.seats.iter().filter(|seat| seat.sink.is_some()).count() as u8;
                        self.emit(
                            seat,
                            RoundPayload::Awaiting {
                                own_seat: seat as u8,
                                connected,
                                participant_count: self.seats.len() as u8,
                            },
                            at,
                        );
                        self.established = self.all_connected();
                    }
                } else {
                    let _ = reply.send(epoch);
                }
            }
            Event::Input { seat, token, input } => {
                if !self.matches_socket(seat, token) {
                    return;
                }
                let observation = matches!(
                    input.action,
                    PublicAction::Open { .. } | PublicAction::Accuse { .. }
                );
                let Some(sink) = self.seats[seat].sink.as_ref() else {
                    return;
                };
                let member = &self.admission.leases[seat];
                let auth = &self.admission.authentication;
                let applied = auth
                    .authorities
                    .with_authority(member, auth.clock.now(), || {
                        registry.authorities.with_authority(
                            &sink.authority,
                            registry.auth_clock.now(),
                            || {
                                if self.playing {
                                    self.state.apply(seat, input, at)
                                } else {
                                    RoundPayload::Ack {
                                        command_id: input.command_id,
                                        revision: self.state.revision(seat),
                                        status: AckStatus::Rejected,
                                        error: Some(PublicError::NotStarted),
                                        duplicate: false,
                                    }
                                }
                            },
                        )
                    })
                    .and_then(|result| result);
                match applied {
                    Ok(payload) => {
                        if observation
                            && matches!(
                                payload,
                                RoundPayload::Ack {
                                    status: AckStatus::Applied,
                                    duplicate: false,
                                    ..
                                }
                            )
                        {
                            self.seats[seat].dirty = true;
                        }
                        self.emit(seat, payload, at);
                    }
                    Err(_) => self.detach(seat, at),
                }
            }
        }
    }

    fn matches_socket(&self, seat: usize, token: Uuid) -> bool {
        self.seats[seat]
            .sink
            .as_ref()
            .is_some_and(|sink| sink.authority.token() == token)
    }

    fn detach(&mut self, seat: usize, at: u64) {
        self.seats[seat].sink.take();
        if self.playing {
            self.state.disconnect(seat, at);
        } else {
            self.state.cancel(at);
        }
    }

    fn emit(&mut self, seat: usize, payload: RoundPayload, at: u64) {
        let member = &mut self.seats[seat];
        if let Some(sink) = &member.sink {
            member.sequence = member.sequence.saturating_add(1);
            let key = self.state.key();
            if sink
                .sender
                .try_send(RoundEvent {
                    v: ROOM_PROTOCOL_VERSION,
                    room_id: key.room_id.to_string(),
                    round_id: key.round_id.to_string(),
                    server_seq: member.sequence,
                    server_time_ms: at,
                    payload,
                })
                .is_err()
            {
                self.detach(seat, at);
            }
        }
    }

    fn publish(&mut self, at: u64) {
        if !self.established {
            return;
        }
        for seat in 0..self.seats.len() {
            let Some(sink) = &mut self.seats[seat].sink else {
                continue;
            };
            let revision = self.state.revision(seat);
            if sink.snapshot && sink.revision == revision {
                continue;
            }
            let Some(view) = self.state.view(seat) else {
                continue;
            };
            let payload = if sink.snapshot {
                RoundPayload::Delta { view }
            } else {
                RoundPayload::Snapshot {
                    own_seat: seat as u8,
                    session_epoch: sink.epoch,
                    last_client_seq: self.state.last_seq(seat),
                    view,
                }
            };
            sink.snapshot = true;
            sink.revision = revision;
            self.emit(seat, payload, at);
        }
    }

    fn proofs(&mut self, registry: &MatchRegistry) {
        for (index, seat) in self.seats.iter_mut().enumerate() {
            if let Some(work) = &mut seat.proof {
                match work.try_recv() {
                    Err(oneshot::error::TryRecvError::Empty) => continue,
                    Ok(Ok(result)) => {
                        seat.dirty |= !self.state.commit_proof(result);
                    }
                    _ => {}
                }
                seat.proof = None;
            }
            if !seat.dirty || seat.departed {
                continue;
            }
            let (Ok(permit), Ok(job)) = (
                registry.proofs.clone().try_acquire_owned(),
                self.state.proof(index),
            ) else {
                continue;
            };
            let (sender, receiver) = oneshot::channel();
            seat.dirty = false;
            seat.proof = Some(receiver);
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _ = sender.send(job.run());
            });
        }
    }

    async fn finish(&mut self, registry: &MatchRegistry, result: FinishedRace, at: u64) {
        let key = self.state.key();
        let stored = if self.playing {
            matches!(
                tokio::time::timeout(
                    Duration::from_secs(2),
                    self.admission.journal.save_race(result)
                )
                .await,
                Ok(Ok(_))
            )
        } else {
            matches!(
                tokio::time::timeout(
                    Duration::from_secs(2),
                    self.admission.journal.discard(key.round_id)
                )
                .await,
                Ok(Ok(()))
            )
        };
        registry.recorded(key.round_id, stored);
        if !stored {
            self.admission.journal.fail_closed();
        }
        let auth = &self.admission.authentication;
        for (seat, lease) in self.admission.leases.iter().enumerate() {
            if auth
                .authorities
                .with_authority(lease, auth.clock.now(), || ())
                .is_err()
            {
                self.state.anonymize(seat);
            }
        }
        let recording = if stored {
            RecordingStatus::Saved
        } else {
            RecordingStatus::Failed
        };
        let summary = self
            .playing
            .then(|| self.state.view(0).and_then(|view| view.result))
            .flatten()
            .map(|result| RoomResultSummary { result, recording });
        self.admission.lifecycle.complete(key, at, summary);
        for seat in 0..self.seats.len() {
            if let Some(view) = self.state.view(seat) {
                self.emit(seat, RoundPayload::End { view, recording }, at);
            }
        }
    }
}
