//! HTTP composition root. Domain logic is owned by liar-core.
pub mod auth;
pub mod lobby;
pub mod online;
pub mod results;
pub mod room_code;
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
        auth::disabled_auth_router()
            .merge(lobby::disabled_lobby_router())
            .merge(results::disabled_result_router()),
    )
}
pub fn app_with_auth(pool: Option<PgPool>, auth: Router) -> Router {
    app_with_auth_and_owner(pool, auth, None)
}
#[derive(Clone)]
struct Readiness {
    pool: Option<PgPool>,
    owner: Option<online::PgResultHealth>,
}
impl Readiness {
    fn owns_results(&self) -> bool {
        self.owner.as_ref().is_none_or(|owner| owner.available())
    }
}
pub fn app_with_auth_and_owner(
    pool: Option<PgPool>,
    auth: Router,
    owner: Option<online::PgResultHealth>,
) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(Readiness { pool, owner })
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
    State(state): State<Readiness>,
) -> (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<HealthResponse>,
) {
    let available = if state.owns_results()
        && let Some(pool) = &state.pool
    {
        matches!(
            tokio::time::timeout(
                Duration::from_secs(1),
                sqlx::query("SELECT 1").execute(pool)
            )
            .await,
            Ok(Ok(_))
        ) && state.owns_results()
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
