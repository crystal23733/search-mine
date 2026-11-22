use super::*;
use crate::auth::AuthClock;
use liar_protocol::{game::PublicEndReason, online::OnlineError, results::ResultStats};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct ActiveRace {
    pub id: Uuid,
    pub room_id: Uuid,
    pub rules_hash: String,
    pub accounts: Vec<Uuid>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RacePlayerResult {
    pub account: Uuid,
    pub rank: Option<u8>,
    pub forfeited: bool,
    pub stats: ResultStats,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FinishedRace {
    pub id: Uuid,
    pub room_id: Uuid,
    pub rules_hash: String,
    pub seed: [u8; 8],
    pub ended_ms: u64,
    pub reason: PublicEndReason,
    pub players: Vec<RacePlayerResult>,
}

#[derive(Clone)]
pub struct PgJournalRuntime(PgResultRuntime);
impl PgJournalRuntime {
    pub async fn claim(pool: PgPool, clock: Arc<dyn AuthClock>) -> Result<Self, OnlineError> {
        Ok(Self(PgResultRuntime::claim_journal(pool, clock).await?))
    }
    pub fn health(&self) -> PgResultHealth {
        self.0.health()
    }
    pub async fn register_race(&self, race: ActiveRace) -> Result<SaveResult, OnlineError> {
        self.0.register_race(race).await
    }
    pub async fn start_race(&self, id: Uuid) -> Result<SaveResult, OnlineError> {
        self.0.start_race(id).await
    }
    pub async fn save_race(&self, race: FinishedRace) -> Result<SaveResult, OnlineError> {
        self.0.save_race(race).await
    }
}
impl AdmissionJournal for PgJournalRuntime {
    fn fail_closed(&self) {
        self.0.fail_closed();
    }
    fn register(&self, active: ActiveMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.0.register(active)
    }
    fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        self.0.discard(id)
    }
    fn register_race(&self, active: ActiveRace) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(self.0.register_race(active))
    }
    fn start_race(&self, id: Uuid) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(self.0.start_race(id))
    }
    fn save_race(&self, result: FinishedRace) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(self.0.save_race(result))
    }
}
impl ResultRepository for PgJournalRuntime {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.0.save(result)
    }
}
