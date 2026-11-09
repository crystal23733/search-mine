//! Owner-only offline sanitation. No activation/release command is provided.
use liar_server::{
    auth::SystemAuthClock,
    recovery::{self, Ledger, MAX_BYTES, RecoveryError},
};
use sqlx::postgres::PgPoolOptions;
use std::{
    env,
    io::{self, Read},
    time::Duration,
};
use zeroize::Zeroizing;
#[tokio::main]
async fn main() {
    if run().await.is_err() {
        println!("{{\"status\":\"failed\",\"code\":\"recovery_failed\"}}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), RecoveryError> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 1 || !matches!(args[0].as_str(), "quarantine" | "apply") {
        return Err(RecoveryError::Invalid);
    }
    let ledger = if args[0] == "apply" {
        let mut bytes = Zeroizing::new(Vec::new());
        io::stdin()
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RecoveryError::Invalid)?;
        use liar_server::auth::AuthClock;
        Some(Ledger::parse(&bytes, SystemAuthClock.now())?)
    } else {
        None
    };
    let url = env::var("MIGRATION_DATABASE_URL").map_err(|_| RecoveryError::Invalid)?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(1))
        .connect(&url)
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    if let Some(ledger) = ledger {
        let applied = recovery::apply(&pool, &SystemAuthClock, &ledger).await?;
        println!(
            "{}",
            serde_json::json!({"state":"quarantined","applied":applied})
        );
    } else {
        let id = recovery::quarantine(&pool, &SystemAuthClock).await?;
        println!(
            "{}",
            serde_json::json!({"state":"quarantined","recovery_id":id})
        );
    }
    pool.close().await;
    Ok(())
}
