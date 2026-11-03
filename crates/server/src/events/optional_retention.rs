use super::*;
use crate::auth::AuthClock;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
pub struct PgOptionalEventRetention {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgOptionalEventRetention {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl EventRetentionStore for PgOptionalEventRetention {
    fn reap(&self, budget: Duration) -> RetentionFuture<'_> {
        super::retention::reap_raw(
            &self.pool,
            &self.clock,
            budget,
            super::retention::RawEventTable::Optional,
        )
    }
}
