//! Owner-only recovery transport. No activation/release command is provided.
use liar_server::{
    auth::{AuthClock, SystemAuthClock},
    recovery::{self, Ledger, MAX_BYTES, RecoveryError, Snapshot},
};
use sqlx::postgres::PgPoolOptions;
use std::{
    env,
    io::{self, Read, Write},
    time::Duration,
};
use uuid::Uuid;
use zeroize::Zeroizing;
#[tokio::main]
async fn main() {
    if run().await.is_err() {
        println!("{{\"status\":\"failed\",\"code\":\"recovery_failed\"}}");
        std::process::exit(1);
    }
}
enum Command {
    Quarantine,
    Capture,
    Apply(Ledger),
    ApplySnapshot(Uuid, Snapshot),
}
fn read_input() -> Result<Zeroizing<Vec<u8>>, RecoveryError> {
    let mut bytes = Zeroizing::new(Vec::new());
    io::stdin()
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| RecoveryError::Invalid)?;
    Ok(bytes)
}
async fn run() -> Result<(), RecoveryError> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let command = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["quarantine"] => Command::Quarantine,
        ["capture"] => Command::Capture,
        ["apply"] => Command::Apply(Ledger::parse(&read_input()?, SystemAuthClock.now())?),
        ["apply-snapshot", incident] => {
            let incident = Uuid::parse_str(incident).map_err(|_| RecoveryError::Invalid)?;
            if incident.is_nil() {
                return Err(RecoveryError::Invalid);
            }
            Command::ApplySnapshot(
                incident,
                Snapshot::parse(&read_input()?, SystemAuthClock.now())?,
            )
        }
        _ => return Err(RecoveryError::Invalid),
    };
    let url = env::var("MIGRATION_DATABASE_URL").map_err(|_| RecoveryError::Invalid)?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(1))
        .connect(&url)
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    match command {
        Command::Quarantine => {
            let id = recovery::quarantine(&pool, &SystemAuthClock).await?;
            println!(
                "{}",
                serde_json::json!({"state":"quarantined","recovery_id":id})
            );
        }
        Command::Capture => {
            let snapshot = recovery::capture(&pool, &SystemAuthClock).await?;
            let bytes = Zeroizing::new(snapshot.to_bytes(SystemAuthClock.now())?);
            io::stdout()
                .write_all(&bytes)
                .map_err(|_| RecoveryError::Unavailable)?;
        }
        Command::Apply(ledger) => {
            let applied = recovery::apply(&pool, &SystemAuthClock, &ledger).await?;
            println!(
                "{}",
                serde_json::json!({"state":"quarantined","applied":applied})
            );
        }
        Command::ApplySnapshot(incident, snapshot) => {
            let applied =
                recovery::apply_snapshot(&pool, &SystemAuthClock, incident, &snapshot).await?;
            println!(
                "{}",
                serde_json::json!({"state":"quarantined","applied":applied})
            );
        }
    }
    pool.close().await;
    Ok(())
}
