use super::FinishedMatch;
use liar_protocol::online::OnlineError;
use std::{future::Future, pin::Pin, time::Instant};
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub trait MatchClock: Send + Sync {
    fn now_ms(&self) -> u64;
}
pub struct SystemMatchClock(Instant);
impl Default for SystemMatchClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl MatchClock for SystemMatchClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveResult {
    Saved,
    Duplicate,
}
pub trait ResultRepository: Send + Sync {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>>;
}
pub trait SessionReader: Send + Sync {
    fn read(
        &self,
        hash: [u8; 32],
    ) -> PortFuture<'_, Result<Option<crate::auth::Session>, OnlineError>>;
}
#[derive(Clone, Copy)]
pub struct MatchLimits {
    pub matches: usize,
    pub mailbox: usize,
    pub outgoing: usize,
    pub proof_workers: usize,
}
