use super::{ResultReader, StoredPersonalResult};
use crate::{auth::AuthClock, online::PortFuture};
use liar_protocol::{
    game::{Outcome, PublicEndReason},
    results::{ResultError, ResultStats},
};
use sqlx::{PgPool, Row, postgres::PgRow};
use std::sync::Arc;
use uuid::Uuid;
pub struct PgResultReader {
    pool: PgPool,
    clock: Arc<dyn AuthClock>,
}
impl PgResultReader {
    pub fn new(pool: PgPool, clock: Arc<dyn AuthClock>) -> Self {
        Self { pool, clock }
    }
}
impl ResultReader for PgResultReader {
    fn read(
        &self,
        account: Uuid,
        match_id: Uuid,
    ) -> PortFuture<'_, Result<Option<StoredPersonalResult>, ResultError>> {
        Box::pin(async move {
            let now = self.clock.now();
            if account.is_nil() || match_id.is_nil() || !(0..=253402300799).contains(&now) {
                return Err(ResultError::Unavailable);
            }
            // Select only the authorized participant projection; never select board truth.
            let mut rows=sqlx::query("SELECT r.id,p.account_id,r.rules_hash,r.end_elapsed_ms,r.reason,p.outcome,p.opened_safe,p.mistakes,p.accusations,p.correct_accusations FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id WHERE r.id=$1 AND p.account_id=$2 AND NOT p.is_bot AND r.recorded_at<=to_timestamp($3::double precision) AND r.recorded_at>to_timestamp($3::double precision)-interval '7776000 seconds' LIMIT 2")
                .bind(match_id).bind(account).bind(now).fetch_all(&self.pool).await.map_err(db_error)?;
            if rows.len() > 1 {
                return Err(ResultError::Unavailable);
            }
            let Some(row) = rows.pop() else {
                return Ok(None);
            };
            let result = stored(row)?;
            result.clone().project(account, match_id)?;
            Ok(Some(result))
        })
    }
}
fn db_error(_: sqlx::Error) -> ResultError {
    ResultError::Unavailable
}
fn stored(row: PgRow) -> Result<StoredPersonalResult, ResultError> {
    let number = |key| {
        u16::try_from(row.try_get::<i16, _>(key).map_err(db_error)?)
            .map_err(|_| ResultError::Unavailable)
    };
    Ok(StoredPersonalResult {
        account: row.try_get("account_id").map_err(db_error)?,
        match_id: row.try_get("id").map_err(db_error)?,
        rules_hash: row.try_get("rules_hash").map_err(db_error)?,
        end_elapsed_ms: u64::try_from(row.try_get::<i64, _>("end_elapsed_ms").map_err(db_error)?)
            .map_err(|_| ResultError::Unavailable)?,
        reason: reason(&row.try_get::<String, _>("reason").map_err(db_error)?)?,
        outcome: outcome(&row.try_get::<String, _>("outcome").map_err(db_error)?)?,
        own: ResultStats {
            opened_safe: number("opened_safe")?,
            mistakes: number("mistakes")?,
            accusation_attempts: number("accusations")?,
            correct_accusations: number("correct_accusations")?,
        },
    })
}
fn reason(value: &str) -> Result<PublicEndReason, ResultError> {
    Ok(match value {
        "clear" => PublicEndReason::Clear,
        "timeout" => PublicEndReason::Timeout,
        "forfeit" => PublicEndReason::Forfeit,
        "abandoned" => PublicEndReason::Abandoned,
        "server_failure" => PublicEndReason::ServerFailure,
        "cancelled" => PublicEndReason::Cancelled,
        _ => return Err(ResultError::Unavailable),
    })
}
fn outcome(value: &str) -> Result<Outcome, ResultError> {
    Ok(match value {
        "win" => Outcome::Win,
        "loss" => Outcome::Loss,
        "draw" => Outcome::Draw,
        "abort" => Outcome::Abort,
        "cancelled" => Outcome::Cancelled,
        _ => return Err(ResultError::Unavailable),
    })
}
#[cfg(test)]
#[path = "../../tests/unit/result_postgres.rs"]
mod tests;
