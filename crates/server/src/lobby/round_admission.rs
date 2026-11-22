use super::{LobbyService, rooms::RoomState};
use crate::{
    lobby::{BoardSource, admission::Receipt},
    online::{
        ActiveRace, ConnectionAuthority, RaceReservation, RoundAdmission, RoundLifecycle,
        RoundParticipant, RoundState, SaveResult,
    },
    rooms::{RoundKey, RoundReservation},
};
use liar_core::{game::multiplayer::PreparedRace, random::SeededRng};
use liar_protocol::{online::OnlineError, rooms::RoomWireError};
use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, watch};
use uuid::Uuid;

struct RoomLifecycle(Weak<LobbyService>);

impl RoundLifecycle for RoomLifecycle {
    fn contains(&self, key: RoundKey, account: Uuid) -> bool {
        self.0.upgrade().is_some_and(|service| {
            service
                .inner
                .lock()
                .is_ok_and(|inner| inner.draining || inner.rooms.policy.round_member(key, account))
        })
    }

    fn begin(&self, key: RoundKey, now: u64, leases: &[ConnectionAuthority]) -> bool {
        let Some(service) = self.0.upgrade() else {
            return false;
        };
        let Ok(mut inner) = service.inner.lock() else {
            return false;
        };
        if inner.draining
            || !leases.iter().all(|lease| {
                inner.rooms.policy.round_member(key, lease.account())
                    && inner
                        .rooms
                        .leases
                        .get(&lease.account())
                        .is_some_and(|current| current.token() == lease.token())
            })
        {
            return false;
        }
        let references = leases.iter().collect::<Vec<_>>();
        service
            .authentication
            .authorities
            .with_authorities(&references, service.authentication.clock.now(), || {
                inner
                    .rooms
                    .policy
                    .begin_round(key, service.now_ms().max(now))
                    .is_ok()
            })
            .unwrap_or(false)
    }

    fn complete(&self, key: RoundKey, now: u64, started: bool) {
        let Some(service) = self.0.upgrade() else {
            return;
        };
        let Ok(mut inner) = service.inner.lock() else {
            return;
        };
        if inner.draining {
            return;
        }
        let now = service.now_ms().max(now);
        if started {
            let _ = inner.rooms.policy.finish(key, now);
        } else if inner.rooms.policy.cancel_countdown(key, now).is_ok() {
            inner
                .rooms
                .failures
                .insert(key.room_id, RoomWireError::NotReady);
        }
    }
}

struct RoundWork {
    key: RoundKey,
    service: Weak<LobbyService>,
    seats: Vec<RoundParticipant>,
    admission: RoundAdmission,
    boards: Arc<dyn BoardSource>,
    claim: RaceReservation,
    permit: OwnedSemaphorePermit,
}

impl LobbyService {
    pub(super) fn prepare_round(
        self: &Arc<Self>,
        rooms: &mut RoomState,
        reservation: RoundReservation,
    ) -> Result<(), OnlineError> {
        let sockets = self
            .round_connections
            .as_ref()
            .ok_or(OnlineError::Unavailable)?;
        let key = reservation.key;
        let names = rooms
            .policy
            .names(&reservation)
            .ok_or(OnlineError::NotMatched)?;
        let leases = reservation
            .accounts
            .iter()
            .map(|account| {
                rooms
                    .leases
                    .get(account)
                    .cloned()
                    .ok_or(OnlineError::Unauthorized)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let references = leases.iter().collect::<Vec<_>>();
        let authentication = self.authentication.clone();
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| OnlineError::Capacity)?;
        let claim = authentication.authorities.with_authorities(
            &references,
            authentication.clock.now(),
            || {
                self.registry
                    .reserve_race(key.round_id, &reservation.accounts, sockets)
            },
        )??;
        let seats = reservation
            .accounts
            .into_iter()
            .zip(names)
            .map(|(account, nickname)| RoundParticipant { account, nickname })
            .collect();
        let (cancel, cancelled) = watch::channel(false);
        rooms.jobs.insert(key, cancel);
        let service = Arc::downgrade(self);
        let work = RoundWork {
            key,
            service: service.clone(),
            seats,
            claim,
            permit,
            boards: self.boards.clone(),
            admission: RoundAdmission {
                authentication,
                leases,
                lifecycle: Arc::new(RoomLifecycle(service)),
                journal: self.journal.clone(),
            },
        };
        tokio::spawn(work.run(cancelled));
        Ok(())
    }

    fn commit_round(
        &self,
        key: RoundKey,
        state: RoundState,
        claim: RaceReservation,
        admission: RoundAdmission,
    ) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        let now = self.now_ms();
        if inner.draining
            || inner.rooms.advance(&self.authentication, now).is_err()
            || inner.rooms.policy.admit(key, now).is_err()
        {
            return false;
        }
        if self.registry.admit_round(claim, state, admission).is_err() {
            let _ = inner.rooms.policy.cancel_countdown(key, now);
            inner
                .rooms
                .failures
                .insert(key.room_id, RoomWireError::Unavailable);
            return false;
        }
        true
    }

    fn prepare_failed(&self, key: RoundKey) {
        if let Ok(mut inner) = self.inner.lock()
            && inner
                .rooms
                .policy
                .fail_preparation(key, self.now_ms())
                .is_ok()
        {
            inner
                .rooms
                .failures
                .insert(key.room_id, RoomWireError::Unavailable);
        }
    }
}

impl RoundWork {
    async fn run(self, cancelled: watch::Receiver<bool>) {
        let service = self.service.clone();
        let key = self.key;
        if self.prepare(cancelled).await.is_err()
            && let Some(service) = service.upgrade()
        {
            service.prepare_failed(key);
        }
    }

    async fn prepare(self, mut cancelled: watch::Receiver<bool>) -> Result<(), OnlineError> {
        let deadline = Duration::from_secs(2);
        let board = tokio::select! {
            biased;
            _ = cancelled.changed() => return Ok(()),
            result = tokio::time::timeout(deadline, self.boards.take(deadline)) => result.map_err(|_| OnlineError::Unavailable)??,
        };
        let count = self.seats.len();
        let permit = self.permit;
        let (prepared, seed, random, permit) = tokio::task::spawn_blocking(move || {
            let (board, rules, seed) = board.into_parts();
            let mut random = [0; 8];
            getrandom::fill(&mut random).map_err(|_| OnlineError::Unavailable)?;
            let prepared =
                PreparedRace::prepare(board, rules, count).map_err(|_| OnlineError::Unavailable)?;
            Ok((
                prepared,
                seed,
                SeededRng::new(u64::from_le_bytes(random)),
                permit,
            ))
        })
        .await
        .map_err(|_| OnlineError::Unavailable)??;
        if cancelled.has_changed().is_err() || *cancelled.borrow() {
            return Ok(());
        }
        let key = self.key;
        let active = ActiveRace {
            id: key.round_id,
            room_id: key.room_id,
            rules_hash: prepared.rules_hash().to_owned(),
            accounts: self.seats.iter().map(|entry| entry.account).collect(),
        };
        if self.admission.journal.register_race(active).await? != SaveResult::Saved {
            return Err(OnlineError::Unavailable);
        }
        let receipt = Receipt::hold(key.round_id, self.admission.journal.clone(), permit);
        if let Some(service) = self.service.upgrade()
            && let Ok(engine) = prepared.start(service.now_ms(), random)
            && let Ok(state) = RoundState::new(key, engine, seed, self.seats)
            && service.commit_round(key, state, self.claim, self.admission)
        {
            receipt.commit();
            return Ok(());
        }
        receipt.discard().await;
        Err(OnlineError::Unavailable)
    }
}
