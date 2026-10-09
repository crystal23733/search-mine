use super::*;
use crate::{
    auth::{AuthError, BrowserCookies, BrowserSecurity},
    online::{ConnectionAuthority, SessionReader},
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware,
    response::{IntoResponse, Response},
    routing::post,
};
use liar_protocol::{game::PROTOCOL_VERSION, lobby::*, online::OnlineError};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;
struct Context {
    service: Arc<LobbyService>,
    authentication: LobbyAuthentication,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    requests: Arc<Semaphore>,
    rate: Mutex<super::http_rate::LobbyRate>,
}
type Failure = (StatusCode, LobbyErrorCode);
fn status(code: LobbyErrorCode) -> StatusCode {
    match code {
        LobbyErrorCode::Malformed | LobbyErrorCode::UnsupportedVersion => StatusCode::BAD_REQUEST,
        LobbyErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        LobbyErrorCode::Unavailable | LobbyErrorCode::Capacity => StatusCode::SERVICE_UNAVAILABLE,
        LobbyErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        LobbyErrorCode::NotFound => StatusCode::NOT_FOUND,
        LobbyErrorCode::Busy | LobbyErrorCode::Full | LobbyErrorCode::Stale => StatusCode::CONFLICT,
    }
}
fn failure(error: Failure) -> Response {
    (error.0, Json(serde_json::json!({"error":error.1}))).into_response()
}
fn error(code: LobbyErrorCode) -> Failure {
    (status(code), code)
}
pub fn disabled_lobby_router() -> Router {
    Router::new()
        .route(
            "/api/v1/lobby",
            post(|| async { failure(error(LobbyErrorCode::Unavailable)) }),
        )
        .layer(middleware::map_response(no_store))
}
async fn no_store(mut response: Response) -> Response {
    for (key, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ] {
        response
            .headers_mut()
            .insert(key, value.parse().expect("Constant header"));
    }
    response
}
pub fn lobby_router(
    service: Arc<LobbyService>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=256).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    let context = Arc::new(Context {
        authentication: service.authentication().clone(),
        service,
        sessions,
        security,
        requests: Arc::new(Semaphore::new(requests)),
        rate: Mutex::new(super::http_rate::LobbyRate::new(4096)),
    });
    Ok(Router::new()
        .route("/api/v1/lobby", post(handle))
        .layer(DefaultBodyLimit::max(MAX_LOBBY_BYTES))
        .layer(middleware::map_response(no_store))
        .with_state(context))
}
async fn handle(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let result = async {
        let cookies =
            BrowserCookies::parse(&headers).map_err(|_| error(LobbyErrorCode::Malformed))?;
        ctx.security
            .require(&headers, &cookies, ctx.authentication.clock.now())
            .map_err(|cause| {
                if cause == AuthError::Unavailable {
                    error(LobbyErrorCode::Unavailable)
                } else {
                    (StatusCode::FORBIDDEN, LobbyErrorCode::Unauthorized)
                }
            })?;
        let token = cookies
            .session
            .as_ref()
            .ok_or_else(|| error(LobbyErrorCode::Unauthorized))?;
        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(error(LobbyErrorCode::Malformed));
        }
        let body = body.map_err(|cause| (cause.status(), LobbyErrorCode::Malformed))?;
        let input = decode_lobby(&body).map_err(error)?;
        let _permit = ctx
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| error(LobbyErrorCode::Unavailable))?;
        ctx.rate
            .lock()
            .map_err(|_| error(LobbyErrorCode::Unavailable))?
            .session(token.hash(), ctx.authentication.clock.now())
            .map_err(error)?;
        let generation = ctx
            .authentication
            .authorities
            .generation()
            .map_err(|_| error(LobbyErrorCode::Unavailable))?;
        let session =
            match tokio::time::timeout(Duration::from_secs(2), ctx.sessions.read(token.hash()))
                .await
            {
                Ok(Ok(Some(session))) if session.account.nickname.is_some() => session,
                Ok(Ok(_)) => return Err(error(LobbyErrorCode::Unauthorized)),
                _ => return Err(error(LobbyErrorCode::Unavailable)),
            };
        let authority = ctx
            .authentication
            .authorities
            .bind_shared(
                generation,
                session.account.id,
                token.hash(),
                session.expires_at,
                ctx.authentication.clock.now(),
            )
            .map_err(|cause| error(super::http_projection::code(cause.into())))?;
        ctx.rate
            .lock()
            .map_err(|_| error(LobbyErrorCode::Unavailable))?
            .account(
                session.account.id,
                matches!(input.command, LobbyCommand::RoomJoin { .. }),
                ctx.authentication.clock.now(),
            )
            .map_err(error)?;
        let view = dispatch(&ctx.service, &authority, input.command)
            .map_err(|cause| error(super::http_projection::code(cause)))?;
        Ok::<_, Failure>(
            Json(LobbyResponse {
                v: PROTOCOL_VERSION,
                server_time_ms: ctx.service.now_ms(),
                state: super::http_projection::view(view),
            })
            .into_response(),
        )
    }
    .await;
    result.unwrap_or_else(failure)
}
fn dispatch(
    service: &LobbyService,
    authority: &ConnectionAuthority,
    command: LobbyCommand,
) -> Result<LobbyView, LobbyServiceError> {
    let uuid = |value: &str| {
        Uuid::parse_str(value).map_err(|_| LobbyServiceError::Online(OnlineError::Malformed))
    };
    match command {
        LobbyCommand::Status => service.status(authority),
        LobbyCommand::QueueJoin { difficulty } => service.join(
            authority,
            match difficulty {
                LobbyDifficulty::Easy => liar_core::bot::Difficulty::Easy,
                LobbyDifficulty::Normal => liar_core::bot::Difficulty::Normal,
                LobbyDifficulty::Hard => liar_core::bot::Difficulty::Hard,
            },
        ),
        LobbyCommand::RoomCreate => service.create_room(authority),
        LobbyCommand::RoomJoin { code } => {
            service.join_room(authority, RoomCode::parse(&code).map_err(LobbyError::from)?)
        }
        LobbyCommand::Ready { room_id, ready } => service.ready(authority, uuid(&room_id)?, ready),
        LobbyCommand::Cancel { identity } => service.cancel(
            authority,
            match identity {
                LobbyCancellation::Queue { queue_id } => LobbyIdentity::Queue(uuid(&queue_id)?),
                LobbyCancellation::Room { room_id } => LobbyIdentity::Room(uuid(&room_id)?),
                LobbyCancellation::Preparing { entity, generation } => {
                    LobbyIdentity::Preparing(ReservationKey {
                        entity: uuid(&entity)?,
                        generation: generation.parse().map_err(|_| OnlineError::Malformed)?,
                    })
                }
            },
        ),
    }
}
