use super::*;
use crate::online::{BotExecutor, MatchRegistry, MatchState};
use liar_core::{
    bot::Difficulty,
    game::{PreparedEngine, RuleEngine},
};
use liar_protocol::online::OnlineError;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Semaphore, oneshot, watch};
use uuid::Uuid;

pub struct PreparedMatch {
    engine: PreparedEngine,
    seed: [u8; 8],
}
pub trait MatchPreparer: Send + Sync {
    fn prepare(&self, board: PreparedBoard) -> Result<PreparedMatch, OnlineError>;
}
pub struct CoreMatchPreparer;
impl MatchPreparer for CoreMatchPreparer {
    fn prepare(&self, board: PreparedBoard) -> Result<PreparedMatch, OnlineError> {
        let (board, rules, seed) = board.into_parts();
        let engine = RuleEngine::prepare(board, rules).map_err(|_| OnlineError::Unavailable)?;
        Ok(PreparedMatch { engine, seed })
    }
}
pub trait RoomCodeSource: Send + Sync {
    fn code(&self) -> Result<RoomCode, OnlineError>;
}
pub struct OsRoomCodeSource;
impl RoomCodeSource for OsRoomCodeSource {
    fn code(&self) -> Result<RoomCode, OnlineError> {
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).map_err(|_| OnlineError::Unavailable)?;
        for byte in &mut bytes {
            *byte = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"[usize::from(*byte & 31)];
        }
        RoomCode::parse(std::str::from_utf8(&bytes).expect("ASCII alphabet"))
            .map_err(|_| OnlineError::Malformed)
    }
}
#[derive(Clone, Copy)]
pub struct LobbyLimits {
    pub capacity: usize,
    pub workers: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyServiceError {
    Policy(LobbyError),
    Online(OnlineError),
}
impl From<LobbyError> for LobbyServiceError {
    fn from(error: LobbyError) -> Self {
        Self::Policy(error)
    }
}
impl From<OnlineError> for LobbyServiceError {
    fn from(error: OnlineError) -> Self {
        Self::Online(error)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyIdentity {
    Queue(Uuid),
    Room(Uuid),
    Preparing(ReservationKey),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LobbyView {
    Idle,
    Waiting(LobbyStatus),
    Matched {
        match_id: Uuid,
        own_seat: usize,
        opponent: OpponentKind,
    },
    Failed(OnlineError),
}
impl LobbyView {
    pub fn identity(&self) -> Option<LobbyIdentity> {
        match self {
            Self::Waiting(LobbyStatus::Queued { id, .. }) => Some(LobbyIdentity::Queue(*id)),
            Self::Waiting(LobbyStatus::Room { id, .. }) => Some(LobbyIdentity::Room(*id)),
            Self::Waiting(LobbyStatus::Preparing { key, .. }) => {
                Some(LobbyIdentity::Preparing(*key))
            }
            _ => None,
        }
    }
}
struct Job {
    result: oneshot::Receiver<Result<PreparedMatch, OnlineError>>,
    cancel: watch::Sender<bool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}
struct Failure {
    error: OnlineError,
    expires: u64,
}
struct ServiceState {
    policy: LobbyState,
    jobs: HashMap<ReservationKey, Job>,
    failures: HashMap<Uuid, Failure>,
    failure_order: VecDeque<Uuid>,
}
pub struct LobbyService {
    inner: Mutex<ServiceState>,
    limits: LobbyLimits,
    capacity: Arc<Semaphore>,
    boards: Arc<dyn BoardSource>,
    preparer: Arc<dyn MatchPreparer>,
    codes: Arc<dyn RoomCodeSource>,
    registry: Arc<MatchRegistry>,
    bots: Arc<BotExecutor>,
}
impl LobbyService {
    pub fn new(
        limits: LobbyLimits,
        boards: Arc<dyn BoardSource>,
        preparer: Arc<dyn MatchPreparer>,
        codes: Arc<dyn RoomCodeSource>,
        registry: Arc<MatchRegistry>,
        bots: Arc<BotExecutor>,
    ) -> Result<Arc<Self>, LobbyServiceError> {
        if !(1..=4096).contains(&limits.capacity)
            || !(1..=64).contains(&limits.workers)
            || limits.workers > limits.capacity
        {
            return Err(OnlineError::Capacity.into());
        }
        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| OnlineError::Unavailable)?;
        let service = Arc::new(Self {
            inner: Mutex::new(ServiceState {
                policy: LobbyState::new(limits.capacity)?,
                jobs: HashMap::new(),
                failures: HashMap::new(),
                failure_order: VecDeque::new(),
            }),
            limits,
            capacity: Arc::new(Semaphore::new(limits.workers)),
            boards,
            preparer,
            codes,
            registry,
            bots,
        });
        let weak = Arc::downgrade(&service);
        runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(20));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let Some(service) = weak.upgrade() else {
                    break;
                };
                let _ = service.tick();
            }
        });
        Ok(service)
    }
    pub fn status(&self, account: Uuid) -> Result<LobbyView, LobbyServiceError> {
        if account.is_nil() {
            return Err(OnlineError::Malformed.into());
        }
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        self.advance(&mut inner, self.registry.now_ms())?;
        self.dispatch(&mut inner);
        self.view(&inner, account)
    }
    pub fn join(
        &self,
        account: Uuid,
        difficulty: Difficulty,
    ) -> Result<LobbyView, LobbyServiceError> {
        self.change(account, |policy, now| {
            let id = match policy.status(account) {
                Some(LobbyStatus::Queued { id, .. }) => id,
                _ => Uuid::new_v4(),
            };
            policy.join(account, id, difficulty, now)?;
            Ok(())
        })
    }
    pub fn create_room(&self, account: Uuid) -> Result<LobbyView, LobbyServiceError> {
        self.change(account, |policy, now| {
            if matches!(
                policy.status(account),
                Some(LobbyStatus::Room { own_seat: 0, .. })
            ) {
                return Ok(());
            }
            let id = Uuid::new_v4();
            for _ in 0..8 {
                match policy.create_room(account, id, self.codes.code()?, now) {
                    Ok(_) => return Ok(()),
                    Err(LobbyError::Collision) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            Err(LobbyError::Collision.into())
        })
    }
    pub fn join_room(&self, account: Uuid, code: RoomCode) -> Result<LobbyView, LobbyServiceError> {
        self.change(account, |policy, now| {
            policy.join_room(account, code, now)?;
            Ok(())
        })
    }
    pub fn ready(
        &self,
        account: Uuid,
        room: Uuid,
        ready: bool,
    ) -> Result<LobbyView, LobbyServiceError> {
        self.change(account, |policy, now| {
            policy.ready(account, room, ready, now)?;
            Ok(())
        })
    }
    pub fn cancel(
        &self,
        account: Uuid,
        identity: LobbyIdentity,
    ) -> Result<LobbyView, LobbyServiceError> {
        self.change(account, |policy, now| {
            if let Some(status) = policy.status(account) {
                let current = LobbyView::Waiting(status).identity();
                if current != Some(identity) {
                    return Err(LobbyError::Stale.into());
                }
            }
            policy.cancel(account, now)?;
            Ok(())
        })
    }
    fn change(
        &self,
        account: Uuid,
        apply: impl FnOnce(&mut LobbyState, u64) -> Result<(), LobbyServiceError>,
    ) -> Result<LobbyView, LobbyServiceError> {
        if account.is_nil() {
            return Err(OnlineError::Malformed.into());
        }
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        let now = self.registry.now_ms();
        self.advance(&mut inner, now)?;
        match self.registry.for_account(account) {
            Ok(_) => return Err(LobbyError::Busy.into()),
            Err(OnlineError::NotMatched) => {}
            Err(error) => return Err(error.into()),
        }
        apply(&mut inner.policy, now)?;
        inner.failures.remove(&account);
        inner.failure_order.retain(|id| *id != account);
        self.dispatch(&mut inner);
        self.view(&inner, account)
    }
    fn view(&self, inner: &ServiceState, account: Uuid) -> Result<LobbyView, LobbyServiceError> {
        match self.registry.for_account(account) {
            Ok(handle) => {
                let seat = handle.seat(account)?;
                Ok(LobbyView::Matched {
                    match_id: handle.id(),
                    own_seat: seat.index(),
                    opponent: if handle.is_bot(seat.other()) {
                        OpponentKind::Bot
                    } else {
                        OpponentKind::Human
                    },
                })
            }
            Err(OnlineError::NotMatched) => {
                Ok(if let Some(status) = inner.policy.status(account) {
                    LobbyView::Waiting(status)
                } else if let Some(failure) = inner.failures.get(&account) {
                    LobbyView::Failed(failure.error)
                } else {
                    LobbyView::Idle
                })
            }
            Err(error) => Err(error.into()),
        }
    }
    fn tick(&self) -> Result<(), LobbyServiceError> {
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        self.advance(&mut inner, self.registry.now_ms())?;
        self.dispatch(&mut inner);
        Ok(())
    }
    fn advance(&self, inner: &mut ServiceState, now: u64) -> Result<(), LobbyServiceError> {
        inner.policy.tick(now)?;
        inner.failures.retain(|_, failure| now < failure.expires);
        inner
            .failure_order
            .retain(|id| inner.failures.contains_key(id));
        let live: HashSet<_> = inner
            .policy
            .reservations()
            .iter()
            .map(|res| res.key)
            .collect();
        inner.jobs.retain(|key, _| live.contains(key));
        let mut completed = Vec::new();
        for (key, job) in &mut inner.jobs {
            match job.result.try_recv() {
                Ok(result) => completed.push((*key, result)),
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => {
                    completed.push((*key, Err(OnlineError::Unavailable)))
                }
            }
        }
        for (key, result) in completed {
            inner.jobs.remove(&key);
            let Some(reservation) = inner
                .policy
                .reservations()
                .into_iter()
                .find(|res| res.key == key)
            else {
                continue;
            };
            let started = result.and_then(|prepared| {
                let engine = prepared
                    .engine
                    .start(now)
                    .map_err(|_| OnlineError::Unavailable)?;
                let state = MatchState::new(
                    Uuid::new_v4(),
                    engine,
                    reservation.players,
                    prepared.seed,
                    now,
                )?;
                match reservation.opponent {
                    OpponentKind::Human => self.registry.create(state),
                    OpponentKind::Bot => self.registry.create_with_bot(
                        state,
                        reservation.difficulty,
                        self.bots.clone(),
                    ),
                }
            });
            // The reservation remains live under this lock at the same captured timestamp.
            inner.policy.commit(key, now)?;
            if let Err(error) = started {
                for account in reservation.players.into_iter().flatten() {
                    self.record_failure(inner, account, error, now);
                }
            }
        }
        Ok(())
    }
    fn record_failure(
        &self,
        inner: &mut ServiceState,
        account: Uuid,
        error: OnlineError,
        now: u64,
    ) {
        inner.failure_order.retain(|id| *id != account);
        if inner.failures.len() >= self.limits.capacity
            && !inner.failures.contains_key(&account)
            && let Some(oldest) = inner.failure_order.pop_front()
        {
            inner.failures.remove(&oldest);
        }
        inner.failure_order.push_back(account);
        inner.failures.insert(
            account,
            Failure {
                error,
                expires: now.saturating_add(30000),
            },
        );
    }
    fn dispatch(&self, inner: &mut ServiceState) {
        let mut reservations = inner.policy.reservations();
        reservations.sort_by_key(|res| res.key.generation);
        let live: HashSet<_> = reservations.iter().map(|res| res.key).collect();
        inner.jobs.retain(|key, _| live.contains(key));
        for reservation in reservations {
            if inner.jobs.contains_key(&reservation.key) {
                continue;
            }
            let Ok(permit) = self.capacity.clone().try_acquire_owned() else {
                break;
            };
            let (cancel, mut canceled) = watch::channel(false);
            let (completed, result) = oneshot::channel();
            inner.jobs.insert(reservation.key, Job { result, cancel });
            let boards = self.boards.clone();
            let preparer = self.preparer.clone();
            tokio::spawn(async move {
                if *canceled.borrow() {
                    return;
                }
                let taken = tokio::select! {
                    biased;
                    _ = canceled.changed() => return,
                    result = tokio::time::timeout(Duration::from_secs(2), boards.take(Duration::from_secs(2))) => result.unwrap_or(Err(OnlineError::Unavailable)),
                };
                let ready = match taken {
                    Ok(board) => tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        if *canceled.borrow() {
                            return Err(OnlineError::Unavailable);
                        }
                        preparer.prepare(board)
                    })
                    .await
                    .unwrap_or(Err(OnlineError::Unavailable)),
                    Err(error) => Err(error),
                };
                let _ = completed.send(ready);
            });
        }
    }
}
