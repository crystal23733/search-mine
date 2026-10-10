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
            let mut rows=sqlx::query("SELECT r.id,p.account_id,r.rules_hash,r.end_elapsed_ms,r.reason,p.outcome,p.opened_safe,p.mistakes,p.accusations,p.correct_accusations FROM online_match_results r JOIN online_match_players p ON p.match_id=r.id WHERE r.id=$1 AND p.account_id=$2 AND NOT p.is_bot AND r.recorded_at<=to_timestamp($3::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>=to_timestamp(0) AND COALESCE(r.retention_started_at,r.recorded_at)<=to_timestamp($3::double precision) AND COALESCE(r.retention_started_at,r.recorded_at)>to_timestamp($3::double precision)-interval '7776000 seconds' LIMIT 2")
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
    Ok(StoredPersonalResult {
        account: row.try_get("account_id").map_err(db_error)?,
        match_id: row.try_get("id").map_err(db_error)?,
        rules_hash: row.try_get("rules_hash").map_err(db_error)?,
        end_elapsed_ms: row
            .try_get::<Option<i64>, _>("end_elapsed_ms")
            .map_err(db_error)?
            .map(u64::try_from)
            .transpose()
            .map_err(|_| ResultError::Unavailable)?,
        reason: reason(&row.try_get::<String, _>("reason").map_err(db_error)?)?,
        outcome: outcome(&row.try_get::<String, _>("outcome").map_err(db_error)?)?,
        own: statistics([
            row.try_get("opened_safe").map_err(db_error)?,
            row.try_get("mistakes").map_err(db_error)?,
            row.try_get("accusations").map_err(db_error)?,
            row.try_get("correct_accusations").map_err(db_error)?,
        ])?,
    })
}
fn statistics(values: [Option<i16>; 4]) -> Result<Option<ResultStats>, ResultError> {
    match values {
        [None, None, None, None] => Ok(None),
        [Some(opened), Some(mistakes), Some(attempts), Some(correct)] => {
            let number = |value| u16::try_from(value).map_err(|_| ResultError::Unavailable);
            Ok(Some(ResultStats {
                opened_safe: number(opened)?,
                mistakes: number(mistakes)?,
                accusation_attempts: number(attempts)?,
                correct_accusations: number(correct)?,
            }))
        }
        _ => Err(ResultError::Unavailable),
    }
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
