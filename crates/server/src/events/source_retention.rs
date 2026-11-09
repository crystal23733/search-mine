//! Bounded removal of expired source data, with atomic metric coverage invalidation.
use super::postgres::{checked_now, db, policy};
use super::{
    EventRetentionStore, EventStoreError, RAW_EVENT_SECONDS, RetentionBatch, RetentionFuture,
};
use crate::auth::AuthClock;
use sqlx::{PgPool, Postgres, Transaction};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

const STAGE_LIMIT: i64 = 100;
const RECORD_SECONDS: i64 = 7_776_000;

pub struct PgSourceRetention {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgSourceRetention {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl EventRetentionStore for PgSourceRetention {
    fn reap(&self, allowance: Duration) -> RetentionFuture<'_> {
        Box::pin(async move {
            if allowance.is_zero() || allowance > Duration::from_secs(2) {
                return Err(EventStoreError::Invalid);
            }
            let deadline = tokio::time::Instant::now() + allowance;
            tokio::time::timeout_at(deadline, async {
                let mut guard = Budget {
                    deadline,
                    clock: self.clock.as_ref(),
                    last: checked_now(self.clock.as_ref(), 0)?,
                };
                let mut tx = self.pool.begin().await.map_err(db)?;
                // A new statement snapshot must see raw rows committed before our parent lock.
                sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
                guard.check(&mut tx).await?;
                let at = guard.last;
                let mut deleted = 0;
                for table in [SourceTable::Online, SourceTable::Daily] {
                    deleted += delete_records(&mut tx, &mut guard, table, at).await?;
                }
                for query in [CLEAR_SEEDS, CLEAR_REPLAYS, DELETE_DEFINITIONS] {
                    guard.check(&mut tx).await?;
                    let changed: i64 = sqlx::query_scalar(query)
                        .bind(at)
                        .bind(STAGE_LIMIT)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(db)?;
                    deleted += bounded(changed)?;
                }
                guard.check(&mut tx).await?;
                let has_more = sqlx::query_scalar(HAS_MORE)
                    .bind(guard.last)
                    .bind(guard.last - RECORD_SECONDS)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?;
                guard.check(&mut tx).await?;
                tx.commit().await.map_err(db)?;
                Ok(RetentionBatch { deleted, has_more })
            })
            .await
            .unwrap_or(Err(EventStoreError::Unavailable))
        })
    }
}
struct Budget<'a> {
    deadline: tokio::time::Instant,
    clock: &'a dyn AuthClock,
    last: i64,
}
impl Budget<'_> {
    async fn check(&mut self, tx: &mut Transaction<'_, Postgres>) -> Result<(), EventStoreError> {
        self.last = checked_now(self.clock, self.last)?;
        let millis = self
            .deadline
            .saturating_duration_since(tokio::time::Instant::now())
            .as_millis();
        if millis == 0 {
            return Err(EventStoreError::Unavailable);
        }
        sqlx::query(
            "SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
        )
        .bind(format!("{millis}ms"))
        .execute(&mut **tx)
        .await
        .map_err(db)?;
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum SourceTable {
    Online,
    Daily,
}
impl SourceTable {
    fn cutoff(self, at: i64) -> i64 {
        match self {
            Self::Online => at - RECORD_SECONDS,
            Self::Daily => at,
        }
    }
    fn select(self) -> &'static str {
        match self {
            Self::Online => {
                "SELECT id FROM online_match_results WHERE COALESCE(retention_started_at,recorded_at)<=to_timestamp($1::double precision) ORDER BY COALESCE(retention_started_at,recorded_at),id LIMIT $2 FOR UPDATE SKIP LOCKED"
            }
            Self::Daily => {
                "SELECT id FROM daily_completions WHERE record_expires_at<=to_timestamp($1::double precision) ORDER BY record_expires_at,id LIMIT $2 FOR UPDATE SKIP LOCKED"
            }
        }
    }
    fn raw(self) -> &'static str {
        match self {
            Self::Online => {
                "SELECT max(occurred_at) FROM product_events WHERE online_match_id=ANY($1) AND occurred_at>$2"
            }
            Self::Daily => {
                "SELECT max(occurred_at) FROM product_events WHERE daily_completion_id=ANY($1) AND occurred_at>$2"
            }
        }
    }
    fn delete(self) -> &'static str {
        match self {
            Self::Online => "DELETE FROM online_match_results WHERE id=ANY($1)",
            Self::Daily => "DELETE FROM daily_completions WHERE id=ANY($1)",
        }
    }
}
fn bounded(changed: i64) -> Result<u16, EventStoreError> {
    u16::try_from(changed)
        .ok()
        .filter(|n| i64::from(*n) <= STAGE_LIMIT)
        .ok_or(EventStoreError::Unavailable)
}
async fn delete_records(
    tx: &mut Transaction<'_, Postgres>,
    guard: &mut Budget<'_>,
    table: SourceTable,
    at: i64,
) -> Result<u16, EventStoreError> {
    guard.check(tx).await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(table.select())
        .bind(table.cutoff(at))
        .bind(STAGE_LIMIT)
        .fetch_all(&mut **tx)
        .await
        .map_err(db)?;
    if ids.is_empty() {
        return Ok(0);
    }
    guard.check(tx).await?;
    let affected: Option<i64> = sqlx::query_scalar(table.raw())
        .bind(&ids)
        .bind(at - RAW_EVENT_SECONDS)
        .fetch_one(&mut **tx)
        .await
        .map_err(db)?;
    if let Some(occurred) = affected {
        guard.check(tx).await?;
        if occurred > guard.last {
            return Err(EventStoreError::Invalid);
        }
        let boundary = occurred.checked_add(1).ok_or(EventStoreError::Invalid)?;
        super::valid_timestamp(boundary).map_err(policy)?;
        let updated=sqlx::query("UPDATE metric_collection SET coverage_since=GREATEST(coverage_since,$1) WHERE singleton")
            .bind(boundary).execute(&mut **tx).await.map_err(db)?;
        if updated.rows_affected() != 1 {
            return Err(EventStoreError::Unavailable);
        }
    }
    guard.check(tx).await?;
    let changed = sqlx::query(table.delete())
        .bind(&ids)
        .execute(&mut **tx)
        .await
        .map_err(db)?
        .rows_affected();
    bounded(i64::try_from(changed).map_err(|_| EventStoreError::Unavailable)?)
}
const CLEAR_SEEDS: &str = "WITH expired AS (SELECT id FROM online_match_results WHERE secret_seed IS NOT NULL AND seed_expires_at<=to_timestamp($1::double precision) ORDER BY seed_expires_at,id LIMIT $2 FOR UPDATE SKIP LOCKED), changed AS (UPDATE online_match_results r SET secret_seed=NULL FROM expired x WHERE r.id=x.id RETURNING r.id) SELECT count(*) FROM changed";
const CLEAR_REPLAYS: &str = "WITH expired AS (SELECT id FROM daily_completions WHERE replay_json IS NOT NULL AND replay_expires_at<=to_timestamp($1::double precision) ORDER BY replay_expires_at,id LIMIT $2 FOR UPDATE SKIP LOCKED), changed AS (UPDATE daily_completions r SET replay_json=NULL FROM expired x WHERE r.id=x.id RETURNING r.id) SELECT count(*) FROM changed";
const DELETE_DEFINITIONS: &str = "WITH expired AS (SELECT d.id FROM daily_definitions d WHERE d.verification_expires_at<=to_timestamp($1::double precision) AND NOT EXISTS(SELECT 1 FROM daily_completions c WHERE c.daily_id=d.id) ORDER BY d.verification_expires_at,d.id LIMIT $2 FOR UPDATE OF d SKIP LOCKED), changed AS (DELETE FROM daily_definitions d USING expired x WHERE d.id=x.id RETURNING d.id) SELECT count(*) FROM changed";
const HAS_MORE: &str = "SELECT EXISTS(SELECT 1 FROM online_match_results WHERE COALESCE(retention_started_at,recorded_at)<=to_timestamp($2::double precision)) OR EXISTS(SELECT 1 FROM daily_completions WHERE record_expires_at<=to_timestamp($1::double precision)) OR EXISTS(SELECT 1 FROM online_match_results WHERE secret_seed IS NOT NULL AND seed_expires_at<=to_timestamp($1::double precision)) OR EXISTS(SELECT 1 FROM daily_completions WHERE replay_json IS NOT NULL AND replay_expires_at<=to_timestamp($1::double precision)) OR EXISTS(SELECT 1 FROM daily_definitions d WHERE d.verification_expires_at<=to_timestamp($1::double precision) AND NOT EXISTS(SELECT 1 FROM daily_completions c WHERE c.daily_id=d.id))";
