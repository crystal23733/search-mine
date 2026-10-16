use super::{model::EventContent, *};
use crate::auth::AuthClock;
use chrono::Days;
use liar_protocol::game::PublicEndReason;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::time::Instant;
use uuid::Uuid;

pub struct PgMetricReader {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgMetricReader {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
    pub async fn initialize(&self) -> Result<(), MetricReadError> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let work = async {
            let started = now(self.clock.as_ref(), 0)?;
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            let sampled = now(self.clock.as_ref(), started)?;
            budget(&mut tx, deadline).await?;
            sqlx::query("INSERT INTO metric_collection(singleton,started_at,coverage_since) VALUES(true,$1,$1) ON CONFLICT DO NOTHING")
                .bind(sampled).execute(&mut *tx).await.map_err(unavailable)?;
            budget(&mut tx, deadline).await?;
            let row = sqlx::query(
                "SELECT started_at,coverage_since FROM metric_collection WHERE singleton",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
            let first: i64 = row.try_get("started_at").map_err(unavailable)?;
            let coverage: i64 = row.try_get("coverage_since").map_err(unavailable)?;
            if first > sampled || coverage < first {
                return Err(EventError::Invalid.into());
            }
            budget(&mut tx, deadline).await?;
            now(self.clock.as_ref(), sampled)?;
            tx.commit().await.map_err(unavailable)
        };
        tokio::time::timeout_at(deadline, work)
            .await
            .unwrap_or(Err(MetricReadError::Unavailable))
    }
    async fn read(&self, day: UtcDay, cohort: bool) -> Result<MetricSnapshot, MetricReadError> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let work = async {
            let started = now(self.clock.as_ref(), 0)?;
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            let sampled = now(self.clock.as_ref(), started)?;
            let (report, coverage) = snapshot(&mut tx, day, cohort, sampled, deadline).await?;
            budget(&mut tx, deadline).await?;
            let finished = now(self.clock.as_ref(), sampled)?;
            window(day.start(), coverage, finished)?;
            tx.commit().await.map_err(unavailable)?;
            Ok(report)
        };
        tokio::time::timeout_at(deadline, work)
            .await
            .unwrap_or(Err(MetricReadError::Unavailable))
    }
}
impl EventMetricReader for PgMetricReader {
    fn weekly(&self, week: UtcDay) -> MetricFuture<'_, WeeklyNsm> {
        Box::pin(async move {
            match self.read(week, false).await? {
                MetricSnapshot::WeeklyNsm(r) => Ok(r),
                _ => Err(MetricReadError::Unavailable),
            }
        })
    }
    fn d7(&self, cohort: UtcDay) -> MetricFuture<'_, D7Cohort> {
        Box::pin(async move {
            match self.read(cohort, true).await? {
                MetricSnapshot::D7(r) => Ok(r),
                _ => Err(MetricReadError::Unavailable),
            }
        })
    }
}
pub(super) fn unavailable(_: sqlx::Error) -> MetricReadError {
    MetricReadError::Unavailable
}
pub(super) fn now(clock: &dyn AuthClock, previous: i64) -> Result<i64, MetricReadError> {
    let value = clock.now();
    super::valid_timestamp(value)?;
    if value < previous {
        return Err(EventError::Invalid.into());
    }
    Ok(value)
}
pub(super) async fn budget(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), MetricReadError> {
    super::retention::statement_budget(tx, deadline)
        .await
        .map_err(|_| MetricReadError::Unavailable)
}
pub(super) fn window(start: i64, coverage: i64, now: i64) -> Result<(), MetricReadError> {
    if start > now {
        return Err(EventError::Invalid.into());
    }
    if start < coverage || start <= now - RAW_EVENT_SECONDS {
        return Err(EventError::Incomplete.into());
    }
    Ok(())
}
fn event_id(id: Uuid) -> Result<EventId, MetricReadError> {
    Ok(EventId::parse(&id.to_string())?)
}
struct Normal {
    account: Uuid,
    match_id: Uuid,
    at: i64,
    opponent: OpponentKind,
    reason: PublicEndReason,
}
fn normal(e: &ProductEvent) -> Option<Normal> {
    match (e.subject, &e.content) {
        (
            EventSubject::Account(account),
            EventContent::Server(ServerFact::MatchEnd {
                match_id,
                opponent,
                reason,
            }),
        ) if matches!(reason, PublicEndReason::Clear | PublicEndReason::Timeout) => Some(Normal {
            account: account.uuid(),
            match_id: match_id.uuid(),
            at: e.occurred_at,
            opponent: *opponent,
            reason: *reason,
        }),
        _ => None,
    }
}
struct Proof {
    account: Uuid,
    match_id: Uuid,
    at: i64,
    created: i64,
    opponent: OpponentKind,
    reason: PublicEndReason,
}
async fn proofs(
    tx: &mut Transaction<'_, Postgres>,
    accounts: &[Uuid],
    matches: &[Uuid],
    history: bool,
    now: i64,
) -> Result<Vec<Proof>, MetricReadError> {
    let mut query = sqlx::QueryBuilder::new(
        "SELECT p.account_id,r.id,EXTRACT(EPOCH FROM r.recorded_at)::bigint AS at,EXTRACT(EPOCH FROM COALESCE(r.retention_started_at,r.recorded_at))::bigint AS retention,EXTRACT(EPOCH FROM a.created_at)::bigint AS created,(r.recorded_at=date_trunc('second',r.recorded_at) AND COALESCE(r.retention_started_at,r.recorded_at)=date_trunc('second',COALESCE(r.retention_started_at,r.recorded_at)) AND a.created_at=date_trunc('second',a.created_at)) AS whole,r.reason,(SELECT count(*) FROM online_match_players WHERE match_id=r.id) AS seats,(SELECT count(*) FROM online_match_players WHERE match_id=r.id AND is_bot) AS bots FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id JOIN auth_accounts a ON a.id=p.account_id WHERE NOT p.is_bot AND r.reason IN ('clear','timeout') AND p.account_id=ANY(",
    );
    query.push_bind(accounts).push(")");
    if history {
        query
            .push(" AND COALESCE(r.retention_started_at,r.recorded_at)>to_timestamp(")
            .push_bind(now - 7776000)
            .push("::double precision)");
    } else {
        query.push(" AND r.id=ANY(").push_bind(matches).push(")");
    }
    let rows = query
        .push(" ORDER BY p.account_id,r.recorded_at,r.id LIMIT ")
        .push_bind((MAX_METRIC_INPUTS + 1) as i64)
        .build()
        .fetch_all(&mut **tx)
        .await
        .map_err(unavailable)?;
    if rows.len() > MAX_METRIC_INPUTS {
        return Err(EventError::Capacity.into());
    }
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for row in rows {
        let account = row.try_get("account_id").map_err(unavailable)?;
        let match_id = row.try_get("id").map_err(unavailable)?;
        if !ids.insert((account, match_id)) {
            return Err(EventError::Conflict.into());
        }
        let at = row.try_get("at").map_err(unavailable)?;
        let retention = row.try_get("retention").map_err(unavailable)?;
        let created = row.try_get("created").map_err(unavailable)?;
        super::valid_timestamp(created)?;
        if created > retention {
            return Err(EventError::Conflict.into());
        }
        let opponent = pg_source::online_source(
            at,
            retention,
            row.try_get("whole").map_err(unavailable)?,
            row.try_get("seats").map_err(unavailable)?,
            row.try_get("bots").map_err(unavailable)?,
            now,
        )
        .map_err(|_| MetricReadError::Policy(EventError::Incomplete))?;
        let reason =
            crate::results::reason(&row.try_get::<String, _>("reason").map_err(unavailable)?)
                .map_err(|_| MetricReadError::Unavailable)?;
        result.push(Proof {
            account,
            match_id,
            at,
            created,
            opponent,
            reason,
        });
    }
    Ok(result)
}
fn verify_raw(normal: &[Normal], proofs: &[Proof]) -> Result<(), MetricReadError> {
    let map: BTreeMap<_, _> = proofs
        .iter()
        .map(|p| ((p.account, p.match_id), p))
        .collect();
    for n in normal {
        let p = map
            .get(&(n.account, n.match_id))
            .ok_or(EventError::Incomplete)?;
        if (n.at, n.opponent, n.reason) != (p.at, p.opponent, p.reason) {
            return Err(EventError::Conflict.into());
        }
    }
    Ok(())
}

pub(super) async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    day: UtcDay,
    cohort: bool,
    sampled: i64,
    deadline: Instant,
) -> Result<(MetricSnapshot, i64), MetricReadError> {
    budget(tx, deadline).await?;
    let coverage = sqlx::query_scalar::<_, i64>(
        "SELECT coverage_since FROM metric_collection WHERE singleton",
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(unavailable)?
    .ok_or(EventError::Incomplete)?;
    window(day.start(), coverage, sampled)?;
    let end = UtcDay(
        day.0
            .checked_add_days(Days::new(if cohort { 8 } else { 7 }))
            .ok_or(EventError::Invalid)?,
    )
    .start();
    super::valid_timestamp(end)?;
    budget(tx, deadline).await?;
    let events = pg_records::metric_events(tx, day.start(), end, sampled).await?;
    budget(tx, deadline).await?;
    let normal: Vec<_> = events.iter().filter_map(normal).collect();
    let accounts: Vec<_> = normal
        .iter()
        .map(|n| n.account)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let matches: Vec<_> = normal
        .iter()
        .map(|n| n.match_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let proofs = proofs(tx, &accounts, &matches, cohort, sampled).await?;
    verify_raw(&normal, &proofs)?;
    let report = if cohort {
        let mut first = Vec::new();
        let mut members = BTreeSet::new();
        let cohort_end = day.start() + 86400;
        let mut earliest_by_account = BTreeMap::<Uuid, &Proof>::new();
        for proof in &proofs {
            earliest_by_account
                .entry(proof.account)
                .and_modify(|earliest| {
                    if proof.at < earliest.at {
                        *earliest = proof;
                    }
                })
                .or_insert(proof);
        }
        for account in normal
            .iter()
            .filter(|n| n.at < cohort_end)
            .map(|n| n.account)
            .collect::<BTreeSet<_>>()
        {
            let earliest = earliest_by_account
                .get(&account)
                .ok_or(EventError::Incomplete)?;
            if earliest.at < day.start() {
                continue;
            }
            if earliest.created <= sampled - 7776000 {
                return Err(EventError::Incomplete.into());
            }
            first.push(FirstNormalCompletion {
                account: event_id(account)?,
                day: UtcDay::from_timestamp(earliest.at)?,
            });
            members.insert(account);
        }
        let selected: Vec<_> = events
            .into_iter()
            .filter(|e| matches!(e.subject,EventSubject::Account(a) if members.contains(&a.uuid())))
            .collect();
        MetricSnapshot::D7(d7_cohort(&first, &selected, day, sampled, true)?)
    } else {
        MetricSnapshot::WeeklyNsm(weekly_nsm(&events, day, sampled, true)?)
    };

    Ok((report, coverage))
}
