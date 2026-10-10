use super::ResultReader;
use crate::{
    auth::{AuthClock, AuthError, BrowserCookies, BrowserSecurity},
    online::{AuthorityRegistry, SessionReader},
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
use liar_protocol::online::OnlineError;
use liar_protocol::{game::PROTOCOL_VERSION, results::*};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Clone)]
pub struct ResultAuthentication {
    pub authorities: Arc<AuthorityRegistry>,
    pub clock: Arc<dyn AuthClock>,
}
pub fn disabled_result_router() -> Router {
    Router::new()
        .route(
            "/api/v1/results",
            post(|| async {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error":ResultError::Unavailable})),
                )
                    .into_response()
            }),
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
struct Context {
    reader: Arc<dyn ResultReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: ResultAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<super::rate::ResultRate>,
}
type Failure = (StatusCode, ResultError);
fn error(cause: ResultError) -> Failure {
    (
        match cause {
            ResultError::Malformed | ResultError::UnsupportedVersion => StatusCode::BAD_REQUEST,
            ResultError::Unauthorized => StatusCode::UNAUTHORIZED,
            ResultError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ResultError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ResultError::NotFound => StatusCode::NOT_FOUND,
        },
        cause,
    )
}
fn failure(cause: Failure) -> Response {
    (cause.0, Json(serde_json::json!({"error":cause.1}))).into_response()
}
fn authority_error(cause: OnlineError) -> Failure {
    error(if cause == OnlineError::Unauthorized {
        ResultError::Unauthorized
    } else {
        ResultError::Unavailable
    })
}
pub fn result_router(
    reader: Arc<dyn ResultReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: ResultAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=256).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    let context = Arc::new(Context {
        reader,
        sessions,
        security,
        authentication,
        requests: Arc::new(Semaphore::new(requests)),
        rate: Mutex::new(super::rate::ResultRate::new(4096)),
    });
    Ok(Router::new()
        .route("/api/v1/results", post(handle))
        .layer(DefaultBodyLimit::max(MAX_RESULT_BYTES))
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
        let started = ctx.authentication.clock.now();
        let cookies = BrowserCookies::parse(&headers).map_err(|_| error(ResultError::Malformed))?;
        ctx.security
            .require(&headers, &cookies, started)
            .map_err(|cause| {
                if cause == AuthError::Unavailable {
                    error(ResultError::Unavailable)
                } else {
                    (StatusCode::FORBIDDEN, ResultError::Unauthorized)
                }
            })?;
        let token = cookies
            .session
            .as_ref()
            .ok_or_else(|| error(ResultError::Unauthorized))?;
        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(error(ResultError::Malformed));
        }
        let body = body.map_err(|cause| (cause.status(), ResultError::Malformed))?;
        let input = decode_result(&body).map_err(error)?;
        let id = Uuid::parse_str(&input.match_id).map_err(|_| error(ResultError::Malformed))?;
        let _permit = ctx
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| error(ResultError::Unavailable))?;
        ctx.rate
            .lock()
            .map_err(|_| error(ResultError::Unavailable))?
            .session(token.hash(), started)
            .map_err(error)?;
        let generation = ctx
            .authentication
            .authorities
            .generation()
            .map_err(authority_error)?;
        tokio::time::timeout(Duration::from_secs(2), async {
            let session = ctx
                .sessions
                .read(token.hash())
                .await
                .map_err(|_| error(ResultError::Unavailable))?
                .filter(|session| {
                    super::model::valid_session(session, ctx.authentication.clock.now())
                })
                .ok_or_else(|| error(ResultError::Unauthorized))?;
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
                .map_err(authority_error)?;
            ctx.rate
                .lock()
                .map_err(|_| error(ResultError::Unavailable))?
                .account(session.account.id, ctx.authentication.clock.now())
                .map_err(error)?;
            let mut revoked = authority.revoked();
            let read = async {
                let record = ctx
                    .reader
                    .read(session.account.id, id)
                    .await
                    .map_err(error)?;
                let projected = record
                    .map(|row| row.project(session.account.id, id))
                    .transpose()
                    .map_err(error)?;
                let last = ctx
                    .sessions
                    .read(token.hash())
                    .await
                    .map_err(|_| error(ResultError::Unavailable))?
                    .ok_or_else(|| error(ResultError::Unauthorized))?;
                let now = ctx.authentication.clock.now();
                if now < started {
                    return Err(error(ResultError::Unavailable));
                }
                if !super::model::same_session(&session, &last, now) {
                    return Err(error(ResultError::Unauthorized));
                }
                ctx.authentication
                    .authorities
                    .with_authority(&authority, now, || {
                        projected
                            .map(|result| {
                                Json(PersonalResultResponse {
                                    v: PROTOCOL_VERSION,
                                    result,
                                })
                                .into_response()
                            })
                            .ok_or_else(|| error(ResultError::NotFound))
                    })
                    .map_err(authority_error)?
            };
            tokio::select! {
                biased;
                _=revoked.changed()=>Err(error(ResultError::Unauthorized)),
                value=read=>value,
            }
        })
        .await
        .map_err(|_| error(ResultError::Unavailable))?
    }
    .await;
    result.unwrap_or_else(failure)
}
