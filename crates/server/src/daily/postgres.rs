use super::*;
use crate::auth::{
    AuthClock, AuthError, Nickname, SessionAuthority, authorize_account_transaction,
};
use liar_core::daily::DailyMetadata;
use liar_core::rules::RulesSnapshot;
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction, postgres::PgRow};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const DEADLINE: Duration = Duration::from_secs(2);
#[path = "postgres_rankings.rs"]
mod rankings;
const DEFINITION_SELECT: &str = "SELECT d.id AS definition_id,to_char(d.utc_date,'YYYY-MM-DD') AS utc_date,d.seed_version,d.rules_snapshot::text AS rules_snapshot,EXTRACT(EPOCH FROM d.published_at)::bigint AS published_at,EXTRACT(EPOCH FROM d.verification_expires_at)::bigint AS verification_expires_at,EXTRACT(EPOCH FROM (d.utc_date::timestamp AT TIME ZONE 'UTC'))::bigint AS starts_at FROM daily_definitions d";

struct Publication {
    definition: PublishedDaily,
    published_at: i64,
    expires_at: i64,
}
fn unavailable(_: sqlx::Error) -> DailyStoreError {
    DailyStoreError::Unavailable
}
fn publication(row: &PgRow, now: i64) -> Result<Publication, DailyStoreError> {
    let date: String = row.try_get("utc_date").map_err(unavailable)?;
    let version: i32 = row.try_get("seed_version").map_err(unavailable)?;
    let snapshot: String = row.try_get("rules_snapshot").map_err(unavailable)?;
    let rules = serde_json::from_str(&snapshot).map_err(|_| DailyStoreError::Unavailable)?;
    let definition = PublishedDaily::new(
        &date,
        version
            .try_into()
            .map_err(|_| DailyStoreError::Unavailable)?,
        rules,
    )
    .map_err(|_| DailyStoreError::Unavailable)?;
    let id: String = row.try_get("definition_id").map_err(unavailable)?;
    let published_at: i64 = row.try_get("published_at").map_err(unavailable)?;
    let expires_at: i64 = row
        .try_get("verification_expires_at")
        .map_err(unavailable)?;
    let starts_at: i64 = row.try_get("starts_at").map_err(unavailable)?;
    if id != definition.metadata().id()
        || starts_at.checked_add(604800) != Some(expires_at)
        || published_at < starts_at
        || published_at >= expires_at
        || now < published_at
    {
        return Err(DailyStoreError::Unavailable);
    }
    Ok(Publication {
        definition,
        published_at,
        expires_at,
    })
}
async fn by_date(
    connection: &mut PgConnection,
    date: &str,
    version: u16,
    now: i64,
) -> Result<Option<Publication>, DailyStoreError> {
    let mut query = sqlx::QueryBuilder::new(DEFINITION_SELECT);
    let row = query
        .push(" WHERE d.utc_date=")
        .push_bind(date)
        .push("::text::date AND d.seed_version=")
        .push_bind(i32::from(version))
        .build()
        .fetch_optional(connection)
        .await
        .map_err(unavailable)?;
    row.as_ref().map(|row| publication(row, now)).transpose()
}
async fn bounded<T>(
    operation: impl std::future::Future<Output = Result<T, DailyStoreError>>,
) -> Result<T, DailyStoreError> {
    tokio::time::timeout(DEADLINE, operation)
        .await
        .unwrap_or(Err(DailyStoreError::Unavailable))
}

async fn completion(
    connection: &mut PgConnection,
    account: Uuid,
    attempt: Option<Uuid>,
    daily: &str,
) -> Result<Option<PgRow>, DailyStoreError> {
    sqlx::query("SELECT id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,verified_elapsed_ms,EXTRACT(EPOCH FROM completed_at)::bigint AS completed_at,EXTRACT(EPOCH FROM replay_expires_at)::bigint AS replay_expires_at,EXTRACT(EPOCH FROM record_expires_at)::bigint AS record_expires_at FROM daily_completions WHERE account_id=$1 AND (($2::uuid IS NOT NULL AND attempt_id=$2) OR ($2::uuid IS NULL AND daily_id=$3))")
        .bind(account).bind(attempt).bind(daily).fetch_optional(connection).await.map_err(unavailable)
}
fn receipt(
    row: &PgRow,
    publication: &Publication,
    now: i64,
) -> Result<DailyReceipt, DailyStoreError> {
    let published = &publication.definition;
    let value = DailyReceipt {
        id: row.try_get("id").map_err(unavailable)?,
        attempt: row.try_get("attempt_id").map_err(unavailable)?,
        metadata: published.metadata().clone(),
        opened_safe: row
            .try_get::<i32, _>("opened_safe")
            .map_err(unavailable)?
            .try_into()
            .map_err(|_| DailyStoreError::Unavailable)?,
        mistakes: row
            .try_get::<i32, _>("mistakes")
            .map_err(unavailable)?
            .try_into()
            .map_err(|_| DailyStoreError::Unavailable)?,
        replayed_elapsed_ms: row
            .try_get::<i64, _>("replayed_elapsed_ms")
            .map_err(unavailable)?
            .try_into()
            .map_err(|_| DailyStoreError::Unavailable)?,
        verified_elapsed_ms: row
            .try_get::<Option<i64>, _>("verified_elapsed_ms")
            .map_err(unavailable)?
            .map(u32::try_from)
            .transpose()
            .map_err(|_| DailyStoreError::Unavailable)?,
        completed_at: row.try_get("completed_at").map_err(unavailable)?,
    };
    value.validate(published, now)?;
    let daily: String = row.try_get("daily_id").map_err(unavailable)?;
    let digest: Vec<u8> = row.try_get("replay_digest").map_err(unavailable)?;
    let replay_expiry: i64 = row.try_get("replay_expires_at").map_err(unavailable)?;
    let record_expiry: i64 = row.try_get("record_expires_at").map_err(unavailable)?;
    if value.completed_at < publication.published_at
        || value.completed_at >= publication.expires_at
        || daily != published.metadata().id()
        || digest.len() != 32
        || value.completed_at.checked_add(604800) != Some(replay_expiry)
        || value.completed_at.checked_add(7776000) != Some(record_expiry)
    {
        return Err(DailyStoreError::Unavailable);
    }
    Ok(value)
}

pub(crate) fn export_receipt(
    row: &PgRow,
    now: i64,
) -> Result<(PublishedDaily, DailyReceipt), DailyStoreError> {
    let accepted = publication(row, now)?;
    let saved = receipt(row, &accepted, now)?;
    Ok((accepted.definition, saved))
}

#[derive(Clone)]
pub struct PgDailyRepository {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgDailyRepository {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
    fn submit_completion<'a>(
        &'a self,
        session: DailySession,
        attempt: Uuid,
        evidence: &'a ReplayEvidence,
        time: Option<&'a VerifiedDailyTime>,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>> {
        Box::pin(bounded(async move {
            if attempt.is_nil() {
                return Err(DailyStoreError::Invalid);
            }
            if time.is_some_and(|time| !time.matches(session.account, attempt, evidence)) {
                return Err(DailyStoreError::Unavailable);
            }
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            let now = self.authorize(&mut tx, session).await?;
            let metadata = evidence.metadata();
            let published = by_date(&mut tx, &metadata.date, metadata.seed_version, now)
                .await?
                .ok_or(DailyStoreError::Invalid)?;
            if published.definition.metadata() != metadata {
                return Err(DailyStoreError::Invalid);
            }
            let daily = metadata.id();
            if let Some(row) = completion(&mut tx, session.account, Some(attempt), &daily).await? {
                let saved_daily: String = row.try_get("daily_id").map_err(unavailable)?;
                let saved_digest: Vec<u8> = row.try_get("replay_digest").map_err(unavailable)?;
                if saved_daily != daily || saved_digest != evidence.fingerprint() {
                    return Err(DailyStoreError::Conflict);
                }
                let saved = receipt(&row, &published, now)?;
                if saved.expired(now) {
                    return Err(DailyStoreError::Expired);
                }
                if saved.opened_safe as usize != evidence.opened_safe()
                    || saved.mistakes != evidence.mistakes()
                    || saved.replayed_elapsed_ms != evidence.replayed_elapsed_ms()
                {
                    return Err(DailyStoreError::Unavailable);
                }
                tx.commit().await.map_err(unavailable)?;
                return Ok(DailySubmission::Official(saved));
            }
            if now >= published.expires_at {
                return Err(DailyStoreError::Expired);
            }
            if let Some(row) = completion(&mut tx, session.account, None, &daily).await? {
                let saved = receipt(&row, &published, now)?;
                tx.commit().await.map_err(unavailable)?;
                return Ok(DailySubmission::Practice(saved));
            }
            let saved = DailyReceipt {
                id: Uuid::new_v4(),
                attempt,
                metadata: metadata.clone(),
                opened_safe: evidence
                    .opened_safe()
                    .try_into()
                    .map_err(|_| DailyStoreError::Invalid)?,
                mistakes: evidence.mistakes(),
                replayed_elapsed_ms: evidence.replayed_elapsed_ms(),
                verified_elapsed_ms: time.map(VerifiedDailyTime::elapsed_ms),
                completed_at: now,
            };
            saved.validate(&published.definition, now)?;
            let replay_expiry = now.checked_add(604800).ok_or(DailyStoreError::Invalid)?;
            let record_expiry = now.checked_add(7776000).ok_or(DailyStoreError::Invalid)?;
            sqlx::query("INSERT INTO daily_completions(id,account_id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at,verified_elapsed_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,to_timestamp($9::double precision),$10::text::jsonb,to_timestamp($11::double precision),to_timestamp($12::double precision),$13)")
                .bind(saved.id).bind(session.account).bind(daily).bind(attempt).bind(evidence.fingerprint().as_slice()).bind(i32::try_from(saved.opened_safe).map_err(|_| DailyStoreError::Invalid)?).bind(i32::from(saved.mistakes)).bind(i64::from(saved.replayed_elapsed_ms)).bind(now).bind(evidence.canonical_replay()).bind(replay_expiry).bind(record_expiry).bind(saved.verified_elapsed_ms.map(i64::from)).execute(&mut *tx).await.map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            Ok(DailySubmission::Official(saved))
        }))
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session: DailySession,
    ) -> Result<i64, DailyStoreError> {
        if session.account.is_nil() {
            return Err(DailyStoreError::Unauthorized);
        }
        let initial = self.clock.now();
        let auth = |now| SessionAuthority {
            account: session.account,
            hash: session.hash,
            now,
        };
        let row = authorize_account_transaction(tx, auth(initial), false)
            .await
            .map_err(authority_error)?;
        let now = self.clock.now();
        if now < initial {
            return Err(DailyStoreError::Unavailable);
        }
        if now != initial {
            authorize_account_transaction(tx, auth(now), false)
                .await
                .map_err(authority_error)?;
        }
        let nickname: Option<String> = row.try_get("nickname").map_err(unavailable)?;
        Nickname::parse(nickname.as_deref().ok_or(DailyStoreError::Unauthorized)?)
            .map_err(|_| DailyStoreError::Unauthorized)?;
        Ok(now)
    }
}
fn authority_error(error: AuthError) -> DailyStoreError {
    match error {
        AuthError::Unavailable => DailyStoreError::Unavailable,
        _ => DailyStoreError::Unauthorized,
    }
}
impl DailyRepository for PgDailyRepository {
    fn publish(
        &self,
        candidate: PublishedDaily,
    ) -> DailyFuture<'_, Result<PublishedDaily, DailyStoreError>> {
        Box::pin(bounded(async move {
            let now = self.clock.now();
            if now <= 0 {
                return Err(DailyStoreError::Invalid);
            }
            let checked = PublishedDaily::new(
                &candidate.metadata().date,
                candidate.metadata().seed_version,
                candidate.rules().clone(),
            )
            .map_err(|_| DailyStoreError::Invalid)?;
            if checked.metadata() != candidate.metadata() {
                return Err(DailyStoreError::Invalid);
            }
            let snapshot =
                serde_json::to_string(candidate.rules()).map_err(|_| DailyStoreError::Invalid)?;
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            sqlx::query("INSERT INTO daily_definitions(id,utc_date,seed_version,rules_snapshot,published_at,verification_expires_at) SELECT $1,$2::text::date,$3,$4::text::jsonb,to_timestamp($5::double precision),($2::text::date::timestamp AT TIME ZONE 'UTC')+604800*interval '1 second' WHERE to_timestamp($5::double precision)>=($2::text::date::timestamp AT TIME ZONE 'UTC') AND to_timestamp($5::double precision)<($2::text::date::timestamp AT TIME ZONE 'UTC')+604800*interval '1 second' ON CONFLICT(utc_date,seed_version) DO NOTHING")
                .bind(candidate.metadata().id()).bind(&candidate.metadata().date).bind(i32::from(candidate.metadata().seed_version)).bind(snapshot).bind(now).execute(&mut *tx).await.map_err(unavailable)?;
            let accepted = by_date(
                &mut tx,
                &candidate.metadata().date,
                candidate.metadata().seed_version,
                now,
            )
            .await?
            .ok_or(DailyStoreError::Expired)?;
            tx.commit().await.map_err(unavailable)?;
            Ok(accepted.definition)
        }))
    }
    fn definition<'a>(
        &'a self,
        date: &'a str,
        version: u16,
    ) -> DailyFuture<'a, Result<Option<PublishedDaily>, DailyStoreError>> {
        Box::pin(bounded(async move {
            DailyMetadata::derive(date, version, &RulesSnapshot::bundled())
                .map_err(|_| DailyStoreError::Invalid)?;
            let now = self.clock.now();
            if now <= 0 {
                return Err(DailyStoreError::Invalid);
            }
            let mut connection = self.pool.acquire().await.map_err(unavailable)?;
            Ok(by_date(&mut connection, date, version, now)
                .await?
                .map(|published| published.definition))
        }))
    }
    fn submit<'a>(
        &'a self,
        session: DailySession,
        attempt: Uuid,
        evidence: &'a ReplayEvidence,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>> {
        self.submit_completion(session, attempt, evidence, None)
    }
    fn submit_timed<'a>(
        &'a self,
        session: DailySession,
        attempt: Uuid,
        evidence: &'a ReplayEvidence,
        time: &'a VerifiedDailyTime,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>> {
        self.submit_completion(session, attempt, evidence, Some(time))
    }
    fn own<'a>(
        &'a self,
        session: DailySession,
        metadata: &'a DailyMetadata,
    ) -> DailyFuture<'a, Result<Option<DailyReceipt>, DailyStoreError>> {
        Box::pin(bounded(async move {
            DailyMetadata::derive(
                &metadata.date,
                metadata.seed_version,
                &RulesSnapshot::bundled(),
            )
            .map_err(|_| DailyStoreError::Invalid)?;
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            let now = self.authorize(&mut tx, session).await?;
            let published = by_date(&mut tx, &metadata.date, metadata.seed_version, now)
                .await?
                .ok_or(DailyStoreError::Invalid)?;
            if published.definition.metadata() != metadata {
                return Err(DailyStoreError::Invalid);
            }
            let value = completion(&mut tx, session.account, None, &metadata.id())
                .await?
                .map(|row| receipt(&row, &published, now))
                .transpose()?
                .filter(|saved| !saved.expired(now));
            tx.commit().await.map_err(unavailable)?;
            Ok(value)
        }))
    }
}
