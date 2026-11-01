use super::*;
use crate::auth::{
    AuthClock, AuthError, Nickname, Provider, SessionAuthority, authorize_account_transaction,
};
use liar_protocol::{daily::DailyTrust, me::*};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

// These projections deliberately exclude replay bodies, board truth and provider credentials.
const ONLINE: &str = "SELECT r.id,p.account_id,r.rules_hash,r.end_elapsed_ms,r.reason,p.outcome,p.opened_safe,p.mistakes,p.accusations,p.correct_accusations,(EXTRACT(EPOCH FROM r.recorded_at)*1000000)::bigint AS recorded_us,(EXTRACT(EPOCH FROM COALESCE(r.retention_started_at,r.recorded_at))*1000000)::bigint AS retention_us,CASE WHEN other.account_id=$1 AND NOT other.is_bot THEN 2 ELSE 1 END::bigint AS human_seats,other.is_bot AS opponent_bot FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id LEFT JOIN online_match_players other ON other.match_id=p.match_id AND other.seat=1-p.seat WHERE p.account_id=$1 AND NOT p.is_bot AND r.recorded_at>=to_timestamp(0) AND r.recorded_at<=to_timestamp($2::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>=to_timestamp(0) AND COALESCE(r.retention_started_at,r.recorded_at)<=to_timestamp($2::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>to_timestamp($2::double precision)-interval '7776000 seconds' ORDER BY COALESCE(r.retention_started_at,r.recorded_at) DESC,r.recorded_at DESC,r.id DESC LIMIT 10001";
const DAILY: &str = "SELECT c.id,c.daily_id,c.attempt_id,c.replay_digest,c.opened_safe,c.mistakes,c.replayed_elapsed_ms,c.verified_elapsed_ms,FLOOR(EXTRACT(EPOCH FROM c.completed_at))::bigint AS completed_at,(EXTRACT(EPOCH FROM c.completed_at)*1000000)::bigint AS completed_us,FLOOR(EXTRACT(EPOCH FROM c.replay_expires_at))::bigint AS replay_expires_at,FLOOR(EXTRACT(EPOCH FROM c.record_expires_at))::bigint AS record_expires_at,d.id AS definition_id,to_char(d.utc_date,'YYYY-MM-DD') AS utc_date,d.seed_version,d.rules_snapshot::text AS rules_snapshot,EXTRACT(EPOCH FROM d.published_at)::bigint AS published_at,EXTRACT(EPOCH FROM d.verification_expires_at)::bigint AS verification_expires_at,EXTRACT(EPOCH FROM (d.utc_date::timestamp AT TIME ZONE 'UTC'))::bigint AS starts_at FROM daily_completions c JOIN daily_definitions d ON d.id=c.daily_id WHERE c.account_id=$1 AND c.completed_at<=to_timestamp($2::double precision) AND c.completed_at>to_timestamp($2::double precision)-interval '7776000 seconds' AND c.record_expires_at>to_timestamp($2::double precision) ORDER BY c.completed_at DESC,c.id DESC LIMIT 10001";

const IDENTITIES: &str = "SELECT provider,EXTRACT(EPOCH FROM linked_at)::bigint AS linked_at FROM auth_identities WHERE account_id=$1 ORDER BY provider LIMIT 5";

pub struct PgMeReader {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
    workers: MeWorkers,
}
impl PgMeReader {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self::with_workers(pool, clock, MeWorkers::default())
    }
    pub fn with_workers(pool: PgPool, clock: Arc<dyn AuthClock>, workers: MeWorkers) -> Self {
        Self {
            pool,
            clock,
            workers,
        }
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session: MeSession,
        previous: i64,
    ) -> Result<(PgRow, i64, i64), MeError> {
        if session.account.is_nil() || session.revision.is_nil() {
            return Err(MeError::Unauthorized);
        }
        let started = self.clock.now();
        if started < previous || !(0..=MAX_ME_TIMESTAMP).contains(&started) {
            return Err(MeError::Unavailable);
        }
        let account = authorize_account_transaction(
            tx,
            SessionAuthority {
                account: session.account,
                hash: session.hash,
                now: started,
            },
            false,
        )
        .await
        .map_err(auth)?;
        let row=sqlx::query("SELECT id,EXTRACT(EPOCH FROM created_at)::bigint AS created_at,EXTRACT(EPOCH FROM authenticated_at)::bigint AS authenticated_at,EXTRACT(EPOCH FROM expires_at)::bigint AS expires_at FROM auth_sessions WHERE account_id=$1 AND token_hash=$2")
            .bind(session.account).bind(session.hash.as_slice()).fetch_optional(&mut **tx).await.map_err(db)?.ok_or(MeError::Unauthorized)?;
        let now = self.clock.now();
        if now < started || now > MAX_ME_TIMESTAMP {
            return Err(MeError::Unavailable);
        }
        let id: Uuid = row.try_get("id").map_err(db)?;
        let created: i64 = row.try_get("created_at").map_err(db)?;
        let authenticated: i64 = row.try_get("authenticated_at").map_err(db)?;
        let expiry: i64 = row.try_get("expires_at").map_err(db)?;
        if id != session.revision
            || created < 0
            || authenticated < created
            || authenticated > now
            || now >= expiry
            || expiry > MAX_ME_TIMESTAMP
        {
            return Err(MeError::Unauthorized);
        }
        Ok((account, now, expiry))
    }
}
fn db(_: sqlx::Error) -> MeError {
    MeError::Unavailable
}
fn auth(value: AuthError) -> MeError {
    if value == AuthError::Unavailable {
        MeError::Unavailable
    } else {
        MeError::Unauthorized
    }
}
fn online(row: PgRow, account: Uuid) -> Result<MeOnlineRow, MeError> {
    if row.try_get::<i64, _>("human_seats").map_err(db)? != 1 {
        return Err(MeError::Unavailable);
    }
    let primary: i64 = row.try_get("retention_us").map_err(db)?;
    let secondary: i64 = row.try_get("recorded_us").map_err(db)?;
    let opponent = match row.try_get::<Option<bool>, _>("opponent_bot").map_err(db)? {
        Some(true) => MeOpponent::Bot,
        Some(false) => MeOpponent::Human,
        None => MeOpponent::Unknown,
    };
    let saved = crate::results::stored(row).map_err(|_| MeError::Unavailable)?;
    let id = saved.match_id;
    let result = saved
        .project(account, id)
        .map_err(|_| MeError::Unavailable)?;
    Ok(MeOnlineRow {
        key: MeKey {
            primary,
            secondary,
            id,
        },
        entry: MeOnlineRecord {
            result,
            opponent,
            retention_started_at: primary / 1000000,
            recorded_at: secondary / 1000000,
        },
    })
}
fn daily(row: PgRow, now: i64) -> Result<MeDailyRow, MeError> {
    let primary = row.try_get("completed_us").map_err(db)?;
    let (_, receipt) = crate::daily::export_receipt(&row, now).map_err(|_| MeError::Unavailable)?;
    Ok(MeDailyRow {
        key: MeKey {
            primary,
            secondary: primary,
            id: receipt.id,
        },
        entry: MeDailyRecord {
            completion_id: receipt.id.to_string(),
            date: receipt.metadata.date,
            opened_safe: receipt.opened_safe,
            mistakes: receipt.mistakes,
            replayed_elapsed_ms: receipt.replayed_elapsed_ms,
            verified_elapsed_ms: receipt.verified_elapsed_ms,
            trust: if receipt.verified_elapsed_ms.is_some() {
                DailyTrust::VerifiedTime
            } else {
                DailyTrust::VerifiedCompletion
            },
            completed_at: receipt.completed_at,
        },
    })
}
impl MeReader for PgMeReader {
    fn read(
        &self,
        session: MeSession,
        query: MeQuery,
    ) -> crate::online::PortFuture<'_, Result<MeRead, MeError>> {
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(2), async move {
                let started = self.clock.now();
                let mut tx = self.pool.begin().await.map_err(db)?;

                let (account, now, expiry) = self.authorize(&mut tx, session, started).await?;

                let nickname: Option<String> = account.try_get("nickname").map_err(db)?;

                let nickname = nickname
                    .map(|v| {
                        Nickname::parse(&v)
                            .map(|v| v.as_str().to_owned())
                            .map_err(|_| MeError::Unavailable)
                    })
                    .transpose()?;

                let identities = sqlx::query(IDENTITIES)
                    .bind(session.account)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(db)?;

                let identities = identities
                    .into_iter()
                    .map(|row| {
                        let provider: String = row.try_get("provider").map_err(db)?;

                        Provider::parse(&provider).map_err(|_| MeError::Unavailable)?;

                        let linked_at = row.try_get("linked_at").map_err(db)?;

                        if !(0..=now).contains(&linked_at) {
                            return Err(MeError::Unavailable);
                        }

                        Ok(MeIdentity {
                            provider,
                            linked_at,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                if identities.len() > 4 {
                    return Err(MeError::Unavailable);
                }

                let account = MeAccount {
                    id: session.account.to_string(),
                    nickname,
                    created_at: account.try_get("created_at").map_err(db)?,
                    last_seen_at: account.try_get("last_seen_at").map_err(db)?,
                    identities,
                };

                let overview = matches!(query, MeQuery::Overview);

                let online_rows = if overview
                    || matches!(
                        query,
                        MeQuery::History {
                            kind: MeKind::Online,
                            ..
                        }
                    ) {
                    sqlx::query(ONLINE)
                        .bind(session.account)
                        .bind(now)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(db)?
                } else {
                    vec![]
                };

                let daily_rows = if overview
                    || matches!(
                        query,
                        MeQuery::History {
                            kind: MeKind::Daily,
                            ..
                        }
                    ) {
                    sqlx::query(DAILY)
                        .bind(session.account)
                        .bind(now)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(db)?
                } else {
                    vec![]
                };

                if online_rows.len() > MAX_ME_RECORDS || daily_rows.len() > MAX_ME_RECORDS {
                    return Err(MeError::Unavailable);
                }

                let consent = if overview {
                    crate::consent::current_in_transaction(&mut tx, session.revision, now, expiry)
                        .await
                        .map_err(|_| MeError::Unavailable)?
                } else {
                    None
                };

                let (payload, until) = self
                    .workers
                    .run(move || {
                        let online = online_rows
                            .into_iter()
                            .map(|row| online(row, session.account))
                            .collect::<Result<Vec<_>, _>>()?;

                        let daily = daily_rows
                            .into_iter()
                            .map(|row| daily(row, now))
                            .collect::<Result<Vec<_>, _>>()?;

                        model::build(
                            MeData {
                                account,
                                online,
                                daily,
                                consent,
                            },
                            query,
                            now,
                        )
                    })
                    .await?;

                let (_, finish, expiry) = self.authorize(&mut tx, session, now).await?;

                let valid_until = until.min(expiry);

                if finish >= valid_until {
                    return Err(MeError::Unavailable);
                }

                tx.commit().await.map_err(db)?;

                Ok(MeRead {
                    account: session.account,
                    revision: session.revision,
                    observed_at: now,
                    valid_until,
                    payload,
                })
            })
            .await
            .unwrap_or(Err(MeError::Unavailable))
        })
    }
}
