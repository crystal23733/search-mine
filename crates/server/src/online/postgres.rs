use super::*;
use crate::auth::AuthClock;
use liar_protocol::game::{Outcome, PublicEndReason};
use liar_protocol::online::OnlineError;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;
pub struct PgResultRepository {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgResultRepository {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl ResultRepository for PgResultRepository {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            let now = self.clock.now();
            if result.id.is_nil()
                || result.rules_hash.len() != 64
                || !result
                    .rules_hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || result.ended_ms > 9007199254740991
                || !(0..=253402300799 - 604800).contains(&now)
                || result.players.iter().all(|p| p.account.is_none())
                || result.players.iter().any(|p| {
                    p.account.is_some_and(|a| a.is_nil())
                        || p.opened_safe > 256
                        || p.mistakes > 4096
                        || p.accusations > 4096
                        || p.correct_accusations > p.accusations
                })
                || (result.players[0].account.is_some()
                    && result.players[0].account == result.players[1].account)
            {
                return Err(OnlineError::Malformed);
            }
            let reason = reason(result.reason);
            let mut tx = self.pool.begin().await.map_err(db_error)?;
            let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at) VALUES($1,$2,$3,$4,$5,to_timestamp($6::double precision),to_timestamp($6::double precision)+interval '7 days') ON CONFLICT(id) DO NOTHING RETURNING id")
                .bind(result.id).bind(&result.rules_hash).bind(result.seed.as_slice())
                .bind(result.ended_ms as i64).bind(reason).bind(now)
                .fetch_optional(&mut *tx).await.map_err(db_error)?;
            if inserted.is_none() {
                let row = sqlx::query("SELECT rules_hash,secret_seed,end_elapsed_ms,reason FROM online_match_results WHERE id=$1")
                    .bind(result.id).fetch_one(&mut *tx).await.map_err(db_error)?;
                let seed: Option<Vec<u8>> = row.try_get("secret_seed").map_err(db_error)?;
                let same = row.try_get::<String, _>("rules_hash").map_err(db_error)?
                    == result.rules_hash
                    && seed.is_none_or(|seed| seed == result.seed)
                    && row.try_get::<i64, _>("end_elapsed_ms").map_err(db_error)?
                        == result.ended_ms as i64
                    && row.try_get::<String, _>("reason").map_err(db_error)? == reason;
                if !same {
                    return Err(OnlineError::Malformed);
                }
                tx.commit().await.map_err(db_error)?;
                return Ok(SaveResult::Duplicate);
            }
            for (seat, player) in result.players.iter().enumerate() {
                if let Some(account) = player.account {
                    // Serialize with deletion without restoring a deleted participant.
                    let exists: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM auth_accounts WHERE id=$1 FOR KEY SHARE",
                    )
                    .bind(account)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db_error)?;
                    if exists.is_none() {
                        continue;
                    }
                }
                sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
                    .bind(result.id).bind(seat as i16).bind(player.account).bind(player.account.is_none())
                    .bind(outcome(player.outcome)).bind(player.opened_safe as i16).bind(player.mistakes as i16)
                    .bind(player.accusations as i16).bind(player.correct_accusations as i16)
                    .execute(&mut *tx).await.map_err(db_error)?;
            }
            tx.commit().await.map_err(db_error)?;
            Ok(SaveResult::Saved)
        })
    }
}
fn db_error(_: sqlx::Error) -> OnlineError {
    OnlineError::Unavailable
}
pub struct PgSessionReader {
    store: crate::auth::PgAuthStore,
    clock: Arc<dyn AuthClock>,
}
impl PgSessionReader {
    pub fn new(store: crate::auth::PgAuthStore, clock: Arc<dyn AuthClock>) -> Self {
        Self { store, clock }
    }
}
impl SessionReader for PgSessionReader {
    fn read(
        &self,
        hash: [u8; 32],
    ) -> PortFuture<'_, Result<Option<crate::auth::Session>, OnlineError>> {
        use crate::auth::AuthStore;
        Box::pin(async move {
            self.store
                .session(hash, self.clock.now())
                .await
                .map_err(|_| OnlineError::Unavailable)
        })
    }
}
fn reason(value: PublicEndReason) -> &'static str {
    match value {
        PublicEndReason::Clear => "clear",
        PublicEndReason::Timeout => "timeout",
        PublicEndReason::Forfeit => "forfeit",
        PublicEndReason::Abandoned => "abandoned",
        PublicEndReason::ServerFailure => "server_failure",
        PublicEndReason::Cancelled => "cancelled",
    }
}
fn outcome(value: Outcome) -> &'static str {
    match value {
        Outcome::Win => "win",
        Outcome::Loss => "loss",
        Outcome::Draw => "draw",
        Outcome::Abort => "abort",
        Outcome::Cancelled => "cancelled",
    }
}
