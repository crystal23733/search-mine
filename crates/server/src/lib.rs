//! HTTP composition root. Domain logic is owned by liar-core.
pub mod auth;
pub mod lobby;
pub mod online;
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    routing::get,
};
use liar_protocol::{HealthResponse, ServiceStatus};
use sqlx::PgPool;
use std::time::Duration;

pub fn app(pool: Option<PgPool>) -> Router {
    app_with_auth(
        pool,
        auth::disabled_auth_router().merge(lobby::disabled_lobby_router()),
    )
}
pub fn app_with_auth(pool: Option<PgPool>, auth: Router) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(pool)
        .merge(auth)
}

async fn live() -> (
    [(header::HeaderName, &'static str); 1],
    Json<HealthResponse>,
) {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(HealthResponse {
            status: ServiceStatus::Ok,
        }),
    )
}

async fn ready(
    State(pool): State<Option<PgPool>>,
) -> (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<HealthResponse>,
) {
    let available = if let Some(pool) = pool {
        matches!(
            tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query("SELECT 1").execute(&pool)
            )
            .await,
            Ok(Ok(_))
        )
    } else {
        false
    };
    let (code, status) = if available {
        (StatusCode::OK, ServiceStatus::Ok)
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, ServiceStatus::Unavailable)
    };
    (
        code,
        [(header::CACHE_CONTROL, "no-store")],
        Json(HealthResponse { status }),
    )
}
