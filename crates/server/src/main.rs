use liar_server::auth::*;
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
    let config = load_auth_config(|name| env::var(name).ok(), read_private_key)
        .map_err(|_| "Authentication configuration is invalid")?;
    let auth = if let Some(config) = config {
        let auth_pool = pool.clone().ok_or("Authentication requires a database")?;
        let store = PgAuthStore::new(auth_pool.clone());
        tokio::spawn(async move {
            let clock = SystemAuthClock;
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let _ = store.cleanup(clock.now()).await;
            }
        });
        config
            .router(auth_pool)
            .map_err(|_| "Authentication initialization failed")?
    } else {
        disabled_auth_router()
    };
    let bind = env::var("LIAR_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|_| "Server bind failed")?;
    axum::serve(listener, liar_server::app_with_auth(pool, auth))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "HTTP server failed")?;
    Ok(())
}
