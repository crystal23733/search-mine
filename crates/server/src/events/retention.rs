use super::{
    EventStoreError, RAW_EVENT_SECONDS,
    postgres::{checked_now, db},
};
use crate::auth::AuthClock;
use sqlx::PgPool;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

pub const RETENTION_BATCH_SIZE: u16 = 500;

pub type RetentionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RetentionBatch, EventStoreError>> + Send + 'a>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionBatch {
    pub deleted: u16,
    pub has_more: bool,
}
pub trait EventRetentionStore: Send + Sync {
    fn reap(&self, budget: Duration) -> RetentionFuture<'_>;
}
pub struct PgEventRetention {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgEventRetention {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl EventRetentionStore for PgEventRetention {
    fn reap(&self, budget: Duration) -> RetentionFuture<'_> {
        Box::pin(async move {
            if budget.is_zero() || budget > Duration::from_secs(2) {
                return Err(EventStoreError::Invalid);
            }
            let deadline = tokio::time::Instant::now() + budget;
            tokio::time::timeout_at(deadline,async {
                let started=checked_now(self.clock.as_ref(),0)?;
                let mut tx=self.pool.begin().await.map_err(db)?;
                let now=checked_now(self.clock.as_ref(),started)?;
                statement_budget(&mut tx,deadline).await?;
                let count:i64=sqlx::query_scalar("WITH expired AS (SELECT id FROM product_events WHERE occurred_at<=$1 ORDER BY occurred_at LIMIT $2 FOR UPDATE SKIP LOCKED), removed AS (DELETE FROM product_events e USING expired x WHERE e.id=x.id RETURNING e.id) SELECT count(*) FROM removed")
                    .bind(now-RAW_EVENT_SECONDS).bind(i64::from(RETENTION_BATCH_SIZE)).fetch_one(&mut *tx).await.map_err(db)?;
                let after=checked_now(self.clock.as_ref(),now)?;
                statement_budget(&mut tx,deadline).await?;
                let has_more=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_events WHERE occurred_at<=$1)")
                    .bind(after-RAW_EVENT_SECONDS).fetch_one(&mut *tx).await.map_err(db)?;
                let deleted=u16::try_from(count).ok().filter(|n|*n<=RETENTION_BATCH_SIZE).ok_or(EventStoreError::Unavailable)?;
                statement_budget(&mut tx,deadline).await?;
                let _=checked_now(self.clock.as_ref(),after)?;
                tx.commit().await.map_err(db)?;
                Ok(RetentionBatch{deleted,has_more})
            }).await.unwrap_or(Err(EventStoreError::Unavailable))
        })
    }
}
pub(super) async fn statement_budget(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    deadline: tokio::time::Instant,
) -> Result<(), EventStoreError> {
    let millis = deadline
        .saturating_duration_since(tokio::time::Instant::now())
        .as_millis();
    if millis == 0 {
        return Err(EventStoreError::Unavailable);
    }
    sqlx::query("SELECT set_config('statement_timeout',$1,true)")
        .bind(format!("{millis}ms"))
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    Ok(())
}
