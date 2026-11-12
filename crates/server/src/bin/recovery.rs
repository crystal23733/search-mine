//! Owner-only recovery transport. Controlled resume requires a separate, current source.
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
    CompareSnapshot(Snapshot),
    Resume(Uuid, Snapshot),
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
    if args.as_slice() == ["inspect-snapshot"] {
        let snapshot = Snapshot::parse(&read_input()?, SystemAuthClock.now())?;
        println!(
            "{}",
            serde_json::to_string(&snapshot.inspect(SystemAuthClock.now())?)
                .map_err(|_| RecoveryError::Invalid)?
        );
        return Ok(());
    }
    let command = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["quarantine"] => Command::Quarantine,
        ["capture"] => Command::Capture,
        ["compare-snapshot"] => {
            Command::CompareSnapshot(Snapshot::parse(&read_input()?, SystemAuthClock.now())?)
        }
        ["apply"] => Command::Apply(Ledger::parse(&read_input()?, SystemAuthClock.now())?),
        [mode @ ("apply-snapshot" | "resume-from-source"), incident] => {
            let incident = Uuid::parse_str(incident).map_err(|_| RecoveryError::Invalid)?;
            if incident.is_nil() {
                return Err(RecoveryError::Invalid);
            }
            let snapshot = Snapshot::parse(&read_input()?, SystemAuthClock.now())?;
            if *mode == "resume-from-source" {
                Command::Resume(incident, snapshot)
            } else {
                Command::ApplySnapshot(incident, snapshot)
            }
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
        Command::CompareSnapshot(snapshot) => {
            let report = recovery::compare_snapshot(&pool, &SystemAuthClock, &snapshot).await?;
            println!(
                "{}",
                serde_json::to_string(&report).map_err(|_| RecoveryError::Invalid)?
            );
        }
        Command::Resume(incident, snapshot) => {
            let source_url =
                env::var("RECOVERY_SOURCE_DATABASE_URL").map_err(|_| RecoveryError::Invalid)?;
            let source = PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(1))
                .connect(&source_url)
                .await
                .map_err(|_| RecoveryError::Unavailable)?;
            let applied = recovery::resume_from_source(
                &source,
                &pool,
                std::sync::Arc::new(SystemAuthClock),
                incident,
                &snapshot,
            )
            .await?;
            println!(
                "{}",
                serde_json::json!({"state":"active","applied":applied})
            );
            source.close().await;
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
