use sqlx::{migrate::Migrator, postgres::PgPoolOptions};
use std::{env, path::Path, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url =
        env::var("MIGRATION_DATABASE_URL").map_err(|_| "MIGRATION_DATABASE_URL is required")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .map_err(|_| "Migration database connection failed")?;
    let folder = env::var("LIAR_MIGRATIONS").unwrap_or_else(|_| "migrations".into());
    Migrator::new(Path::new(&folder))
        .await
        .map_err(|_| "Migration files are invalid")?
        .run(&pool)
        .await
        .map_err(|_| "Migration failed")?;
    pool.close().await;
    Ok(())
}
