use liar_server::auth::*;
use liar_server::lobby::*;
use liar_server::online::*;
use liar_server::results::*;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
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
    let mut result_owner = None;
    let auth = if let Some(config) = config {
        let online = load_online_config(|name| env::var(name).ok())
            .map_err(|_| "Online limits are invalid")?;
        let lobby_config = load_lobby_config(|name| env::var(name).ok())
            .map_err(|_| "Lobby limits are invalid")?;
        let result_config = load_result_config(|name| env::var(name).ok())
            .map_err(|_| "Result limits are invalid")?;
        let result_authorities = AuthorityRegistry::new(result_config.authorities)
            .map_err(|_| "Result capacity is invalid")?;
        let authorities =
            AuthorityRegistry::new(online.connections).map_err(|_| "Online capacity is invalid")?;
        let lobby_authorities = AuthorityRegistry::new(lobby_config.authorities)
            .map_err(|_| "Lobby capacity is invalid")?;
        let clock = Arc::new(SystemMatchClock::default());
        let auth_clock = Arc::new(SystemAuthClock);
        let origin = config.security.origin().to_string();
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
        let writer = PgResultRuntime::claim(auth_pool.clone(), auth_clock.clone())
            .await
            .map_err(|_| "Online storage ownership unavailable")?;
        result_owner = Some(writer.health());
        let registry = MatchRegistry::new(
            online.limits,
            clock,
            auth_clock.clone(),
            authorities.clone(),
            Arc::new(writer),
        )
        .map_err(|_| "Online initialization failed")?;
        let runtime = config
            .initialize_with_invalidations(
                auth_pool.clone(),
                Arc::new(CombinedSessionInvalidator::new(
                    Arc::new(CombinedSessionInvalidator::new(
                        authorities,
                        lobby_authorities.clone(),
                    )),
                    result_authorities.clone(),
                )),
            )
            .map_err(|_| "Authentication initialization failed")?;
        let sessions = Arc::new(PgSessionReader::new(
            runtime.store.clone(),
            auth_clock.clone(),
        ));
        let service = LobbyService::new(
            lobby_config.limits,
            Arc::new(
                BoardPool::start(
                    liar_core::rules::RulesSnapshot::bundled(),
                    lobby_config.board_capacity,
                    lobby_config.board_workers,
                    Arc::new(OsSeedSource),
                )
                .map_err(|_| "Board pool initialization failed")?,
            ),
            Arc::new(CoreMatchPreparer),
            Arc::new(OsRoomCodeSource),
            registry.clone(),
            BotExecutor::new(lobby_config.bot_workers, Arc::new(CoreBotFactory))
                .map_err(|_| "Bot initialization failed")?,
            LobbyAuthentication {
                authorities: lobby_authorities,
                clock: auth_clock.clone(),
            },
        )
        .map_err(|_| "Lobby initialization failed")?;
        let lobby = lobby_router(
            service,
            sessions.clone(),
            runtime.security.clone(),
            lobby_config.requests,
        )
        .map_err(|_| "Lobby HTTP initialization failed")?;
        let results = result_router(
            Arc::new(PgResultReader::new(auth_pool, auth_clock.clone())),
            sessions.clone(),
            runtime.security.clone(),
            ResultAuthentication {
                authorities: result_authorities,
                clock: auth_clock,
            },
            result_config.requests,
        )
        .map_err(|_| "Result HTTP initialization failed")?;
        let ws = websocket_router(&origin, sessions, registry, online.connections)
            .map_err(|_| "WebSocket initialization failed")?;
        tokio::spawn(runtime.maintenance);
        runtime.router.merge(ws).merge(lobby).merge(results)
    } else {
        disabled_auth_router()
            .merge(disabled_lobby_router())
            .merge(disabled_result_router())
    };
    let bind = env::var("LIAR_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|_| "Server bind failed")?;
    let app = liar_server::app_with_auth_and_owner(pool, auth, result_owner.clone());
    run_with_result_owner(result_owner, async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
    })
    .await
    .map_err(|_| "Online storage ownership lost")?
    .map_err(|_| "HTTP server failed")?;
    Ok(())
}
