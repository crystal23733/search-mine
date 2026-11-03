use sqlx::{Postgres, Transaction};
use tokio::time::Instant;

#[derive(Debug)]
pub(crate) struct TransactionDeadlineError;

pub(crate) async fn arm(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), TransactionDeadlineError> {
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if remaining == 0 {
        return Err(TransactionDeadlineError);
    }
    // Bound backend work even after its caller drops the future.
    sqlx::query("SELECT set_config('transaction_timeout',$1,true)")
        .bind(format!("{remaining}ms"))
        .execute(&mut **tx)
        .await
        .map_err(|_| TransactionDeadlineError)?;
    Ok(())
}
