use super::state::BotIntent;
use liar_core::{
    bot::{BotPolicy, Clock, Difficulty},
    game::{Action, Projection},
    random::SeededRng,
};
use liar_protocol::online::OnlineError;
use std::sync::Arc;
use tokio::sync::{Semaphore, oneshot};
use uuid::Uuid;

pub trait BotAgent: Send {
    fn choose(&mut self, projection: &Projection, at: u64) -> Result<Option<Action>, OnlineError>;
}
pub trait BotFactory: Send + Sync {
    fn create(&self, difficulty: Difficulty) -> Result<Box<dyn BotAgent>, OnlineError>;
}
pub struct CoreBotFactory;
impl BotFactory for CoreBotFactory {
    fn create(&self, difficulty: Difficulty) -> Result<Box<dyn BotAgent>, OnlineError> {
        let mut seed = [0; 8];
        getrandom::fill(&mut seed).map_err(|_| OnlineError::Unavailable)?;
        Ok(Box::new(CoreBotAgent {
            difficulty,
            policy: None,
            random: SeededRng::new(u64::from_le_bytes(seed)),
        }))
    }
}
struct FrozenClock(u64);
impl Clock for FrozenClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}
struct CoreBotAgent {
    difficulty: Difficulty,
    policy: Option<BotPolicy>,
    random: SeededRng,
}
impl BotAgent for CoreBotAgent {
    fn choose(&mut self, projection: &Projection, at: u64) -> Result<Option<Action>, OnlineError> {
        if self.policy.is_none() {
            self.policy = Some(
                BotPolicy::new(self.difficulty, projection)
                    .map_err(|_| OnlineError::Unavailable)?,
            );
        }
        self.policy
            .as_mut()
            .expect("Initialized above")
            .choose(projection, &FrozenClock(at), &mut self.random)
            .map_err(|_| OnlineError::Unavailable)
    }
}
pub struct BotExecutor {
    capacity: Arc<Semaphore>,
    factory: Arc<dyn BotFactory>,
}
impl BotExecutor {
    pub fn new(workers: usize, factory: Arc<dyn BotFactory>) -> Result<Arc<Self>, OnlineError> {
        if !(1..=64).contains(&workers) {
            return Err(OnlineError::Capacity);
        }
        Ok(Arc::new(Self {
            capacity: Arc::new(Semaphore::new(workers)),
            factory,
        }))
    }
}
struct Ready {
    agent: Box<dyn BotAgent>,
    action: Option<Action>,
}
struct Pending {
    revision: u64,
    started: u64,
    completion: oneshot::Receiver<Result<Ready, OnlineError>>,
}
pub(super) struct BotDriver {
    pub seat: liar_core::game::Seat,
    difficulty: Difficulty,
    executor: Arc<BotExecutor>,
    agent: Option<Box<dyn BotAgent>>,
    pending: Option<Pending>,
    next_probe: u64,
    sequence: u32,
}
impl BotDriver {
    pub fn new(
        seat: liar_core::game::Seat,
        difficulty: Difficulty,
        executor: Arc<BotExecutor>,
    ) -> Self {
        Self {
            seat,
            difficulty,
            executor,
            agent: None,
            pending: None,
            next_probe: 0,
            sequence: 0,
        }
    }
    pub fn poll(&mut self, revision: u64, at: u64) -> Result<Option<BotIntent>, OnlineError> {
        let Some(pending) = self.pending.as_mut() else {
            return Ok(None);
        };
        let ready = match pending.completion.try_recv() {
            Ok(result) => result?,
            Err(oneshot::error::TryRecvError::Empty) => return Ok(None),
            Err(oneshot::error::TryRecvError::Closed) => return Err(OnlineError::Unavailable),
        };
        let pending = self.pending.take().expect("Polled above");
        self.agent = Some(ready.agent);
        self.next_probe = pending.started.saturating_add(20);
        if pending.revision != revision || at < pending.started || at - pending.started > 100 {
            return Ok(None);
        }
        let Some(action) = ready.action else {
            return Ok(None);
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(OnlineError::Unavailable)?;
        Ok(Some(BotIntent {
            id: Uuid::new_v4(),
            seq: self.sequence,
            action,
        }))
    }
    pub fn schedule(&mut self, projection: Projection, revision: u64, at: u64) {
        if self.pending.is_some()
            || at < self.next_probe
            || projection.countdown_ms > 0
            || projection.own.stun_ms > 0
            || projection.end.is_some()
        {
            return;
        }
        let Ok(permit) = self.executor.capacity.clone().try_acquire_owned() else {
            return;
        };
        let agent = self.agent.take();
        let factory = self.executor.factory.clone();
        let difficulty = self.difficulty;
        let (completed, completion) = oneshot::channel();
        self.pending = Some(Pending {
            revision,
            started: at,
            completion,
        });
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result = (|| {
                let mut agent = match agent {
                    Some(agent) => agent,
                    None => factory.create(difficulty)?,
                };
                let action = agent.choose(&projection, at)?;
                Ok(Ready { agent, action })
            })();
            let _ = completed.send(result);
        });
    }
}
