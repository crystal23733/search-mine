use super::*;
use crate::auth::AuthClock;
use liar_protocol::online::OnlineError;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgJournalRuntime(PgResultRuntime);
impl PgJournalRuntime {
    pub async fn claim(pool: PgPool, clock: Arc<dyn AuthClock>) -> Result<Self, OnlineError> {
        Ok(Self(PgResultRuntime::claim_journal(pool, clock).await?))
    }
    pub fn health(&self) -> PgResultHealth {
        self.0.health()
    }
}
impl AdmissionJournal for PgJournalRuntime {
    fn register(&self, active: ActiveMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.0.register(active)
    }
    fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        self.0.discard(id)
    }
}
impl ResultRepository for PgJournalRuntime {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.0.save(result)
    }
}
