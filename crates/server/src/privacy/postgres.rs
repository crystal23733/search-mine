use super::*;
use crate::auth::{AuthClock, AuthError, SessionAuthority, authorize_account_transaction};
use liar_protocol::game::PROTOCOL_VERSION;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use std::{sync::Arc, time::Duration};

const DAILY: &str = "SELECT c.id,c.daily_id,c.attempt_id,c.replay_digest,c.opened_safe,c.mistakes,c.replayed_elapsed_ms,EXTRACT(EPOCH FROM c.completed_at)::bigint AS completed_at,EXTRACT(EPOCH FROM c.replay_expires_at)::bigint AS replay_expires_at,EXTRACT(EPOCH FROM c.record_expires_at)::bigint AS record_expires_at,d.id AS definition_id,to_char(d.utc_date,'YYYY-MM-DD') AS utc_date,d.seed_version,d.rules_snapshot::text AS rules_snapshot,EXTRACT(EPOCH FROM d.published_at)::bigint AS published_at,EXTRACT(EPOCH FROM d.verification_expires_at)::bigint AS verification_expires_at,EXTRACT(EPOCH FROM (d.utc_date::timestamp AT TIME ZONE 'UTC'))::bigint AS starts_at,";
#[derive(Clone)]
pub struct PgGameExportReader {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
    workers: ExportWorkers,
}
impl PgGameExportReader {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self::with_workers(pool, clock, ExportWorkers::default())
    }
    pub fn with_workers(pool: PgPool, clock: Arc<dyn AuthClock>, workers: ExportWorkers) -> Self {
        Self {
            pool,
            clock,
            workers,
        }
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session: ExportSession,
    ) -> Result<i64, ExportError> {
        if session.account.is_nil() || session.revision.is_nil() {
            return Err(ExportError::Unauthorized);
        }
        let started = self.clock.now();
        let authority = |now| SessionAuthority {
            account: session.account,
            hash: session.hash,
            now,
        };
        authorize_account_transaction(tx, authority(started), true)
            .await
            .map_err(auth_error)?;
        let now = self.clock.now();
        if now < started {
            return Err(ExportError::Unavailable);
        }
        if now != started {
            authorize_account_transaction(tx, authority(now), true)
                .await
                .map_err(auth_error)?;
        }
        let revision: Uuid = sqlx::query_scalar(
            "SELECT id FROM auth_sessions WHERE account_id=$1 AND token_hash=$2",
        )
        .bind(session.account)
        .bind(session.hash.as_slice())
        .fetch_one(&mut **tx)
        .await
        .map_err(db)?;
        if revision != session.revision {
            return Err(ExportError::Unauthorized);
        }
        Ok(now)
    }
}
fn db(_: sqlx::Error) -> ExportError {
    ExportError::Unavailable
}
fn auth_error(error: AuthError) -> ExportError {
    match error {
        AuthError::Unavailable => ExportError::Unavailable,
        AuthError::ReauthenticationRequired => ExportError::Reauth,
        _ => ExportError::Unauthorized,
    }
}
async fn bounded<T>(
    operation: impl std::future::Future<Output = Result<T, ExportError>>,
) -> Result<T, ExportError> {
    tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap_or(Err(ExportError::Unavailable))
}
fn daily_record(row: &PgRow, now: i64) -> Result<ExportRecord, ExportError> {
    let (_, receipt) =
        crate::daily::export_receipt(row, now).map_err(|_| ExportError::Unavailable)?;
    Ok(ExportRecord::Daily {
        completion_id: receipt.id.to_string(),
        attempt_id: receipt.attempt.to_string(),
        metadata: receipt.metadata,
        opened_safe: receipt.opened_safe,
        mistakes: receipt.mistakes,
        replayed_elapsed_ms: receipt.replayed_elapsed_ms,
        completed_at: receipt.completed_at,
        replay_available: row.try_get("replay_available").map_err(db)?,
    })
}
impl GameExportReader for PgGameExportReader {
    fn page(
        &self,
        session: ExportSession,
        stream: ExportStream,
        after: Option<Uuid>,
    ) -> PortFuture<'_, Result<GameExportPage, ExportError>> {
        Box::pin(bounded(async move {
            if after.is_some_and(|id| id.is_nil()) {
                return Err(ExportError::Malformed);
            }
            let mut tx = self.pool.begin().await.map_err(db)?;
            let now = self.authorize(&mut tx, session).await?;
            let records = match stream {
                ExportStream::Online => {
                    let rows = sqlx::query("SELECT r.id,p.account_id,r.rules_hash,r.end_elapsed_ms,r.reason,p.outcome,p.opened_safe,p.mistakes,p.accusations,p.correct_accusations,EXTRACT(EPOCH FROM r.recorded_at)::bigint AS recorded_at,EXTRACT(EPOCH FROM COALESCE(r.retention_started_at,r.recorded_at))::bigint AS retention_started_at,(SELECT count(*) FROM online_match_players duplicate WHERE duplicate.match_id=r.id AND duplicate.account_id=$1 AND NOT duplicate.is_bot) AS human_seats FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id WHERE p.account_id=$1 AND NOT p.is_bot AND ($2::uuid IS NULL OR r.id>$2) AND r.recorded_at>=to_timestamp(0) AND r.recorded_at<=to_timestamp($3::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>=to_timestamp(0) AND COALESCE(r.retention_started_at,r.recorded_at)<=to_timestamp($3::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>to_timestamp($3::double precision)-interval '7776000 seconds' ORDER BY r.id LIMIT 51")
                        .bind(session.account).bind(after).bind(now).fetch_all(&mut *tx).await.map_err(db)?;
                    rows.into_iter()
                        .map(|row| {
                            if row.try_get::<i64, _>("human_seats").map_err(db)? != 1 {
                                return Err(ExportError::Unavailable);
                            }
                            let recorded_at = row.try_get("recorded_at").map_err(db)?;
                            let retention_started_at =
                                row.try_get("retention_started_at").map_err(db)?;
                            let stored = crate::results::stored(row)
                                .map_err(|_| ExportError::Unavailable)?;
                            let match_id = stored.match_id;
                            let result = stored
                                .project(session.account, match_id)
                                .map_err(|_| ExportError::Unavailable)?;
                            Ok(ExportRecord::Online {
                                result,
                                recorded_at,
                                retention_started_at,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                ExportStream::Daily => {
                    let mut query = sqlx::QueryBuilder::new(DAILY);
                    let rows = query.push("(c.replay_json IS NOT NULL AND c.replay_expires_at>to_timestamp(").push_bind(now).push("::double precision)) AS replay_available FROM daily_completions c JOIN daily_definitions d ON d.id=c.daily_id WHERE c.account_id=").push_bind(session.account).push(" AND (").push_bind(after).push("::uuid IS NULL OR c.id>").push_bind(after).push(") AND c.completed_at<=to_timestamp(").push_bind(now).push("::double precision) AND c.record_expires_at>to_timestamp(").push_bind(now).push("::double precision) ORDER BY c.id LIMIT 51").build().fetch_all(&mut *tx).await.map_err(db)?;
                    rows.iter()
                        .map(|row| daily_record(row, now))
                        .collect::<Result<Vec<_>, _>>()?
                }
            };
            let result = page(session.account, stream, after, records, now)?;
            tx.commit().await.map_err(db)?;
            Ok(result)
        }))
    }
    fn replay(
        &self,
        session: ExportSession,
        completion: Uuid,
    ) -> PortFuture<'_, Result<ReplayExport, ExportError>> {
        Box::pin(bounded(async move {
            if completion.is_nil() {
                return Err(ExportError::Malformed);
            }
            let mut tx = self.pool.begin().await.map_err(db)?;
            let now = self.authorize(&mut tx, session).await?;
            let mut query = sqlx::QueryBuilder::new(DAILY);
            let row = query.push("CASE WHEN c.replay_expires_at>to_timestamp(").push_bind(now).push("::double precision) THEN c.replay_json::text ELSE NULL END AS replay_json FROM daily_completions c JOIN daily_definitions d ON d.id=c.daily_id WHERE c.account_id=").push_bind(session.account).push(" AND c.id=").push_bind(completion).push(" AND c.completed_at<=to_timestamp(").push_bind(now).push("::double precision) AND c.record_expires_at>to_timestamp(").push_bind(now).push("::double precision)").build().fetch_optional(&mut *tx).await.map_err(db)?;
            let replay = if let Some(row) = row {
                let (_, receipt) = crate::daily::export_receipt(&row, now)
                    .map_err(|_| ExportError::Unavailable)?;
                let source: Option<String> = row.try_get("replay_json").map_err(db)?;
                let digest: Vec<u8> = row.try_get("replay_digest").map_err(db)?;
                if let Some(source) = source {
                    Some(
                        self.workers
                            .run(move || raw(source, digest, receipt.metadata))
                            .await?,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            tx.commit().await.map_err(db)?;
            Ok(ReplayExport {
                v: PROTOCOL_VERSION,
                account_id: session.account.to_string(),
                completion_id: completion.to_string(),
                replay,
            })
        }))
    }
}
