use super::pg_metrics::{budget, now, snapshot, unavailable, window};
use super::*;
use crate::auth::AuthClock;
use chrono::Days;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
pub struct PgAggregateStore {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgAggregateStore {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl AggregateStore for PgAggregateStore {
    fn refresh(
        &self,
        kind: MetricKind,
        period: UtcDay,
        allowance: Duration,
    ) -> MetricFuture<'_, AggregateReport> {
        Box::pin(async move {
            if allowance.is_zero() || allowance > Duration::from_secs(2) {
                return Err(EventError::Invalid.into());
            }
            let deadline = Instant::now() + allowance;
            let work = async {
                let started = now(self.clock.as_ref(), 0)?;
                let mut tx = self.pool.begin().await.map_err(unavailable)?;
                sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                let sampled = now(self.clock.as_ref(), started)?;
                let (computed, coverage) =
                    snapshot(&mut tx, period, kind == MetricKind::D7, sampled, deadline).await?;
                let report = AggregateReport::new(computed, sampled)?;
                budget(&mut tx, deadline).await?;
                let stored = load(&mut tx, kind, period, true).await?;
                if let Some(prior) = &stored
                    && !prior.can_replace(&report)?
                {
                    budget(&mut tx, deadline).await?;
                    let finished = now(self.clock.as_ref(), sampled)?;
                    window(period.start(), coverage, finished)?;
                    tx.commit().await.map_err(unavailable)?;
                    return Ok(prior.clone());
                }
                budget(&mut tx, deadline).await?;
                save(&mut tx, &report).await?;
                budget(&mut tx, deadline).await?;
                let finished = now(self.clock.as_ref(), sampled)?;
                window(period.start(), coverage, finished)?;
                aggregate_alive(period, finished)?;
                tx.commit().await.map_err(unavailable)?;
                Ok(report)
            };
            tokio::time::timeout_at(deadline, work)
                .await
                .unwrap_or(Err(MetricReadError::Unavailable))
        })
    }
    fn read(&self, kind: MetricKind, period: UtcDay) -> MetricFuture<'_, Option<AggregateReport>> {
        Box::pin(async move {
            let deadline = Instant::now() + Duration::from_secs(2);
            let work = async {
                let started = now(self.clock.as_ref(), 0)?;
                aggregate_alive(period, started)?;
                let mut tx = self.pool.begin().await.map_err(unavailable)?;
                sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                let sampled = now(self.clock.as_ref(), started)?;
                aggregate_alive(period, sampled)?;
                budget(&mut tx, deadline).await?;
                let report = load(&mut tx, kind, period, false).await?;
                budget(&mut tx, deadline).await?;
                let finished = now(self.clock.as_ref(), sampled)?;
                aggregate_alive(period, finished)?;
                if let Some(report) = &report {
                    report.stale(finished)?;
                }
                tx.commit().await.map_err(unavailable)?;
                Ok(report)
            };
            tokio::time::timeout_at(deadline, work)
                .await
                .unwrap_or(Err(MetricReadError::Unavailable))
        })
    }
}
const COLUMNS: &str = "metric,to_char(period,'YYYY-MM-DD') AS period,computed_at,observed_through,expires_at,complete,participants,repeat_accounts,human_participations,bot_participations,sample_size,retained";
fn hydrate(row: &PgRow) -> Result<AggregateReport, MetricReadError> {
    let kind = MetricKind::parse(&row.try_get::<String, _>("metric").map_err(unavailable)?)?;
    let period = UtcDay::parse(&row.try_get::<String, _>("period").map_err(unavailable)?)?;
    let computed_at = row.try_get("computed_at").map_err(unavailable)?;
    let observed_through = row.try_get("observed_through").map_err(unavailable)?;
    let complete = row.try_get("complete").map_err(unavailable)?;
    let number = |key: &str| -> Result<u32, MetricReadError> {
        u32::try_from(row.try_get::<i32, _>(key).map_err(unavailable)?)
            .map_err(|_| MetricReadError::Unavailable)
    };
    let snapshot = match kind {
        MetricKind::WeeklyNsm => MetricSnapshot::WeeklyNsm(WeeklyNsm {
            week: period,
            observed_through,
            complete,
            participants: number("participants")?,
            repeat_accounts: number("repeat_accounts")?,
            human_participations: number("human_participations")?,
            bot_participations: number("bot_participations")?,
        }),
        MetricKind::D7 => MetricSnapshot::D7(D7Cohort {
            cohort: period,
            return_day: UtcDay(
                period
                    .0
                    .checked_add_days(Days::new(7))
                    .ok_or(EventError::Invalid)?,
            ),
            sample_size: number("sample_size")?,
            retained: row
                .try_get::<Option<i32>, _>("retained")
                .map_err(unavailable)?
                .map(u32::try_from)
                .transpose()
                .map_err(|_| MetricReadError::Unavailable)?,
            mature: complete,
        }),
    };
    let report = AggregateReport {
        period,
        computed_at,
        observed_through,
        expires_at: row.try_get("expires_at").map_err(unavailable)?,
        snapshot,
    };
    report.validate()?;
    Ok(report)
}
async fn load(
    tx: &mut Transaction<'_, Postgres>,
    kind: MetricKind,
    period: UtcDay,
    lock: bool,
) -> Result<Option<AggregateReport>, MetricReadError> {
    let mut query = sqlx::QueryBuilder::new("SELECT ");
    query
        .push(COLUMNS)
        .push(" FROM product_metric_aggregates WHERE metric=")
        .push_bind(kind.as_str())
        .push(" AND period=")
        .push_bind(period.0.format("%Y-%m-%d").to_string())
        .push("::text::date");
    if lock {
        query.push(" FOR UPDATE");
    }
    let row = query
        .build()
        .fetch_optional(&mut **tx)
        .await
        .map_err(unavailable)?;
    row.as_ref().map(hydrate).transpose()
}
async fn save(
    tx: &mut Transaction<'_, Postgres>,
    report: &AggregateReport,
) -> Result<(), MetricReadError> {
    report.validate()?;
    let (participants, repeat, human, bot, sample, retained) = match &report.snapshot {
        MetricSnapshot::WeeklyNsm(r) => (
            Some(r.participants),
            Some(r.repeat_accounts),
            Some(r.human_participations),
            Some(r.bot_participations),
            None,
            None,
        ),
        MetricSnapshot::D7(r) => (None, None, None, None, Some(r.sample_size), r.retained),
    };
    let count=sqlx::query("INSERT INTO product_metric_aggregates(metric,period,computed_at,observed_through,expires_at,complete,participants,repeat_accounts,human_participations,bot_participations,sample_size,retained) VALUES($1,$2::text::date,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(metric,period) DO UPDATE SET computed_at=EXCLUDED.computed_at,observed_through=EXCLUDED.observed_through,complete=EXCLUDED.complete,participants=EXCLUDED.participants,repeat_accounts=EXCLUDED.repeat_accounts,human_participations=EXCLUDED.human_participations,bot_participations=EXCLUDED.bot_participations,sample_size=EXCLUDED.sample_size,retained=EXCLUDED.retained WHERE product_metric_aggregates.computed_at<EXCLUDED.computed_at AND product_metric_aggregates.expires_at=EXCLUDED.expires_at AND product_metric_aggregates.observed_through<=EXCLUDED.observed_through AND (NOT product_metric_aggregates.complete OR EXCLUDED.complete)")
        .bind(report.snapshot.kind().as_str()).bind(report.period.0.format("%Y-%m-%d").to_string()).bind(report.computed_at).bind(report.observed_through).bind(report.expires_at).bind(report.snapshot.complete())
        .bind(participants.map(|n|n as i32)).bind(repeat.map(|n|n as i32)).bind(human.map(|n|n as i32)).bind(bot.map(|n|n as i32)).bind(sample.map(|n|n as i32)).bind(retained.map(|n|n as i32)).execute(&mut **tx).await.map_err(unavailable)?.rows_affected();
    if count != 1 {
        return Err(EventError::Conflict.into());
    }
    Ok(())
}
