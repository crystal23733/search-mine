use sqlx::postgres::PgPoolOptions;
use std::{env, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = env::var("DATABASE_URL")
        .ok()
        .map(|url| {
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(1))
                .connect_lazy(&url)
        })
        .transpose()
        .map_err(|_| "Database configuration is invalid")?;
    let bind = env::var("LIAR_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|_| "Server bind failed")?;
    axum::serve(listener, liar_server::app(pool))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "HTTP server failed")?;
    Ok(())
}
