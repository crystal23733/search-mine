//! HTTP composition root. Domain logic is owned by liar-core.
pub mod auth;
pub mod consent;
pub mod daily;
pub mod events;
pub mod lobby;
pub mod me;
pub mod online;
mod postgres_deadline;
pub mod recovery;
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
            .merge(results::disabled_result_router())
            .merge(me::disabled_me_router())
            .merge(consent::disabled_consent_router())
            .merge(events::disabled_optional_event_router())
            .merge(daily::disabled_daily_router()),
    )
}
pub fn app_with_auth(pool: Option<PgPool>, auth: Router) -> Router {
    app_with_auth_and_owner(pool, auth, None)
}
#[derive(Clone)]
struct Readiness {
    pool: Option<PgPool>,
    owner: Option<online::PgResultHealth>,
    registry: Option<std::sync::Arc<online::MatchRegistry>>,
}
impl Readiness {
    fn available(&self) -> bool {
        self.owner.as_ref().is_none_or(|owner| owner.available())
            && self
                .registry
                .as_ref()
                .is_none_or(|registry| registry.accepting())
    }
}
pub fn app_with_auth_and_owner(
    pool: Option<PgPool>,
    auth: Router,
    owner: Option<online::PgResultHealth>,
) -> Router {
    app_with_lifecycle(pool, auth, owner, None)
}
pub fn app_with_lifecycle(
    pool: Option<PgPool>,
    auth: Router,
    owner: Option<online::PgResultHealth>,
    registry: Option<std::sync::Arc<online::MatchRegistry>>,
) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(Readiness {
            pool,
            owner,
            registry,
        })
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
    let available = if state.available()
        && let Some(pool) = &state.pool
    {
        matches!(
            tokio::time::timeout(Duration::from_secs(1), recovery::ensure_serving(pool)).await,
            Ok(Ok(_))
        ) && state.available()
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
