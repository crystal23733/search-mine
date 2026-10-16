use super::*;
use crate::auth::AuthClock;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
pub struct PgAggregateRetention {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgAggregateRetention {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl EventRetentionStore for PgAggregateRetention {
    fn reap(&self, allowance: Duration) -> RetentionFuture<'_> {
        Box::pin(async move {
            if allowance.is_zero() || allowance > Duration::from_secs(2) {
                return Err(EventStoreError::Invalid);
            }
            let deadline = tokio::time::Instant::now() + allowance;
            let work = async {
                let started = postgres::checked_now(self.clock.as_ref(), 0)?;
                let mut tx = self.pool.begin().await.map_err(postgres::db)?;
                let now = postgres::checked_now(self.clock.as_ref(), started)?;
                retention::statement_budget(&mut tx, deadline).await?;
                let count:i64=sqlx::query_scalar("WITH expired AS (SELECT metric,period FROM product_metric_aggregates WHERE expires_at<=$1 ORDER BY expires_at,metric,period LIMIT $2 FOR UPDATE SKIP LOCKED), removed AS (DELETE FROM product_metric_aggregates a USING expired x WHERE a.metric=x.metric AND a.period=x.period RETURNING a.metric) SELECT count(*) FROM removed")
                    .bind(now).bind(i64::from(RETENTION_BATCH_SIZE)).fetch_one(&mut *tx).await.map_err(postgres::db)?;
                let after = postgres::checked_now(self.clock.as_ref(), now)?;
                retention::statement_budget(&mut tx, deadline).await?;
                let has_more = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM product_metric_aggregates WHERE expires_at<=$1)",
                )
                .bind(after)
                .fetch_one(&mut *tx)
                .await
                .map_err(postgres::db)?;
                let deleted = u16::try_from(count)
                    .ok()
                    .filter(|n| *n <= RETENTION_BATCH_SIZE)
                    .ok_or(EventStoreError::Unavailable)?;
                retention::statement_budget(&mut tx, deadline).await?;
                postgres::checked_now(self.clock.as_ref(), after)?;
                tx.commit().await.map_err(postgres::db)?;
                Ok(RetentionBatch { deleted, has_more })
            };
            tokio::time::timeout_at(deadline, work)
                .await
                .unwrap_or(Err(EventStoreError::Unavailable))
        })
    }
}
