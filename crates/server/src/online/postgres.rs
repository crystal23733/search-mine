use super::*;
use crate::auth::AuthClock;
use liar_protocol::online::OnlineError;
use sqlx::PgPool;
use std::sync::Arc;
pub struct PgResultRepository {
    _pool: PgPool,
    _clock: Arc<dyn AuthClock>,
}
impl PgResultRepository {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            _pool: pool,
            _clock: clock,
        }
    }
}
impl ResultRepository for PgResultRepository {
    fn save(&self, _result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async { Err(OnlineError::Unavailable) })
    }
}
