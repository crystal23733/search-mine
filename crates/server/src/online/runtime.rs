use super::*;
use crate::auth::AuthClock;
use liar_core::game::{AnalysisResult, Rejection, Seat};
use liar_protocol::online::{OnlineError, OnlineEvent, OnlineInput};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, oneshot};
use uuid::Uuid;

struct RegistryState {
    matches: HashMap<Uuid, MatchHandle>,
    accounts: HashMap<Uuid, Uuid>,
}
pub struct MatchRegistry {
    inner: Mutex<RegistryState>,
    pub(super) limits: MatchLimits,
    pub(super) clock: Arc<dyn MatchClock>,
    pub(super) auth_clock: Arc<dyn AuthClock>,
    pub(super) authorities: Arc<AuthorityRegistry>,
    pub(super) results: Arc<dyn ResultRepository>,
    capacity: Arc<Semaphore>,
    pub(super) proofs: Arc<Semaphore>,
}
pub(super) struct HandleInner {
    pub id: Uuid,
    players: [Option<Uuid>; 2],
    tx: mpsc::Sender<Ingress>,
    ingress: Mutex<u64>,
    clock: Arc<dyn MatchClock>,
    outgoing: usize,
}
#[derive(Clone)]
pub struct MatchHandle(pub(super) Arc<HandleInner>);
pub struct MatchConnection {
    handle: MatchHandle,
    authority: ConnectionAuthority,
    seat: Seat,
    epoch: u32,
    receiver: mpsc::Receiver<OnlineEvent>,
}
pub(super) struct Ingress {
    pub at: u64,
    pub event: Event,
}
pub(super) enum Event {
    Attach {
        authority: ConnectionAuthority,
        seat: Seat,
        outgoing: mpsc::Sender<OnlineEvent>,
        reply: oneshot::Sender<Result<u32, OnlineError>>,
    },
    Input {
        authority: ConnectionAuthority,
        seat: Seat,
        input: OnlineInput,
    },
    Reject {
        authority: ConnectionAuthority,
        seat: Seat,
        code: OnlineError,
    },
    Disconnect {
        account: Uuid,
        token: Uuid,
        seat: Seat,
        epoch: u32,
    },
    Tick,
    Proof {
        seat: Seat,
        token: Uuid,
        result: Result<AnalysisResult, Rejection>,
    },
}
impl MatchRegistry {
    pub fn new(
        limits: MatchLimits,
        clock: Arc<dyn MatchClock>,
        auth_clock: Arc<dyn AuthClock>,
        authorities: Arc<AuthorityRegistry>,
        results: Arc<dyn ResultRepository>,
    ) -> Result<Arc<Self>, OnlineError> {
        if limits.matches == 0
            || limits.matches > 10000
            || !(4..=1024).contains(&limits.mailbox)
            || !(4..=256).contains(&limits.outgoing)
            || !(1..=64).contains(&limits.proof_workers)
        {
            return Err(OnlineError::Capacity);
        }
        Ok(Arc::new(Self {
            inner: Mutex::new(RegistryState {
                matches: HashMap::new(),
                accounts: HashMap::new(),
            }),
            limits,
            clock,
            auth_clock,
            authorities,
            results,
            capacity: Arc::new(Semaphore::new(limits.matches)),
            proofs: Arc::new(Semaphore::new(limits.proof_workers)),
        }))
    }
    pub fn create(self: &Arc<Self>, state: MatchState) -> Result<MatchHandle, OnlineError> {
        self.create_inner(state, None)
    }
    fn create_inner(
        self: &Arc<Self>,
        state: MatchState,
        bot: Option<super::bot::BotDriver>,
    ) -> Result<MatchHandle, OnlineError> {
        let mut inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        let id = state.id();
        let players = state.players();
        if inner.matches.contains_key(&id)
            || players
                .iter()
                .flatten()
                .any(|account| inner.accounts.contains_key(account))
        {
            return Err(OnlineError::NotMatched);
        }
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| OnlineError::Capacity)?;
        let (tx, receiver) = mpsc::channel(self.limits.mailbox);
        let handle = MatchHandle(Arc::new(HandleInner {
            id,
            players,
            tx,
            ingress: Mutex::new(0),
            clock: self.clock.clone(),
            outgoing: self.limits.outgoing,
        }));
        inner.matches.insert(id, handle.clone());
        for account in players.iter().flatten() {
            inner.accounts.insert(*account, id);
        }
        super::actor::spawn(
            state,
            receiver,
            Arc::downgrade(&handle.0),
            self,
            permit,
            bot,
        );
        Ok(handle)
    }
    pub fn create_with_bot(
        self: &Arc<Self>,
        state: MatchState,
        difficulty: liar_core::bot::Difficulty,
        executor: Arc<BotExecutor>,
    ) -> Result<MatchHandle, OnlineError> {
        let seat = match state.players() {
            [Some(_), None] => Seat::Two,
            [None, Some(_)] => Seat::One,
            _ => return Err(OnlineError::Malformed),
        };
        self.create_inner(
            state,
            Some(super::bot::BotDriver::new(seat, difficulty, executor)),
        )
    }
    pub fn for_account(&self, account: Uuid) -> Result<MatchHandle, OnlineError> {
        let inner = self.inner.lock().map_err(|_| OnlineError::Unavailable)?;
        inner
            .accounts
            .get(&account)
            .and_then(|id| inner.matches.get(id))
            .cloned()
            .ok_or(OnlineError::NotMatched)
    }
    pub fn by_id(&self, id: Uuid) -> Result<MatchHandle, OnlineError> {
        self.inner
            .lock()
            .map_err(|_| OnlineError::Unavailable)?
            .matches
            .get(&id)
            .cloned()
            .ok_or(OnlineError::NotMatched)
    }
    pub(super) fn finish(&self, id: Uuid, remove: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.accounts.retain(|_, current| *current != id);
            if remove {
                inner.matches.remove(&id);
            }
        }
    }
}
impl HandleInner {
    pub(super) fn tx_closed(&self) -> bool {
        self.tx.is_closed()
    }
    pub(super) fn enqueue(&self, event: Event) -> Result<(), OnlineError> {
        let mut last = self.ingress.lock().map_err(|_| OnlineError::Unavailable)?;
        let at = self.clock.now_ms();
        if at < *last {
            return Err(OnlineError::Unavailable);
        }
        // The timestamp and mailbox insertion share one serial admission lock.
        self.tx
            .try_send(Ingress { at, event })
            .map_err(|_| OnlineError::Unavailable)?;
        *last = at;
        Ok(())
    }
}
impl MatchHandle {
    pub fn id(&self) -> Uuid {
        self.0.id
    }
    pub async fn connect(
        &self,
        authority: ConnectionAuthority,
    ) -> Result<MatchConnection, OnlineError> {
        let index = self
            .0
            .players
            .iter()
            .position(|p| *p == Some(authority.account()))
            .ok_or(OnlineError::NotMatched)?;
        let seat = if index == 0 { Seat::One } else { Seat::Two };
        let (tx, receiver) = mpsc::channel(self.0.outgoing);
        let (reply, response) = oneshot::channel();
        if let Err(error) = self.0.enqueue(Event::Attach {
            authority: authority.clone(),
            seat,
            outgoing: tx,
            reply,
        }) {
            authority.close();
            return Err(error);
        }
        let epoch = match tokio::time::timeout(Duration::from_secs(2), response).await {
            Ok(Ok(Ok(epoch))) => epoch,
            Ok(Ok(Err(error))) => {
                authority.close();
                return Err(error);
            }
            _ => {
                authority.close();
                return Err(OnlineError::Unavailable);
            }
        };
        Ok(MatchConnection {
            handle: self.clone(),
            authority,
            seat,
            epoch,
            receiver,
        })
    }
}
impl MatchConnection {
    pub fn reject(&self, code: OnlineError) -> Result<(), OnlineError> {
        self.handle.0.enqueue(Event::Reject {
            authority: self.authority.clone(),
            seat: self.seat,
            code,
        })
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn authority(&self) -> &ConnectionAuthority {
        &self.authority
    }
    pub async fn next(&mut self) -> Option<OnlineEvent> {
        self.receiver.recv().await
    }
    pub fn send(&self, input: OnlineInput) -> Result<(), OnlineError> {
        if input.session_epoch != self.epoch {
            return Err(OnlineError::InvalidEpoch);
        }
        self.handle.0.enqueue(Event::Input {
            authority: self.authority.clone(),
            seat: self.seat,
            input,
        })
    }
}
impl Drop for MatchConnection {
    fn drop(&mut self) {
        self.authority.close();
        let _ = self.handle.0.enqueue(Event::Disconnect {
            account: self.authority.account(),
            token: self.authority.token(),
            seat: self.seat,
            epoch: self.epoch,
        });
    }
}
pub(super) fn enqueue_weak(handle: &Weak<HandleInner>, event: Event) {
    if let Some(handle) = handle.upgrade() {
        let _ = handle.enqueue(event);
    }
}
