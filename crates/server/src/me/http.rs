use super::*;
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
use liar_protocol::{me::*, online::OnlineError, results::ResultError};
use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;
#[derive(Clone)]
pub struct MeAuthentication {
    pub authorities: Arc<AuthorityRegistry>,
    pub clock: Arc<dyn AuthClock>,
    pub workers: MeWorkers,
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
async fn unavailable() -> Response {
    failure(error(MeError::Unavailable))
}
pub fn disabled_me_router() -> Router {
    Router::new()
        .route("/api/v1/me/overview", post(unavailable))
        .route("/api/v1/me/history", post(unavailable))
        .layer(middleware::map_response(no_store))
}
struct Context {
    reader: Arc<dyn MeReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: MeAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<crate::results::ResultRate>,
}
type Failure = (StatusCode, MeError);
fn error(value: MeError) -> Failure {
    (
        match value {
            MeError::Malformed | MeError::UnsupportedVersion => StatusCode::BAD_REQUEST,
            MeError::Unauthorized => StatusCode::UNAUTHORIZED,
            MeError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            MeError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        },
        value,
    )
}
fn failure(value: Failure) -> Response {
    (value.0, Json(serde_json::json!({"error":value.1}))).into_response()
}
fn authority(value: OnlineError) -> Failure {
    error(if value == OnlineError::Unauthorized {
        MeError::Unauthorized
    } else {
        MeError::Unavailable
    })
}
fn rate(value: ResultError) -> Failure {
    error(if value == ResultError::RateLimited {
        MeError::RateLimited
    } else {
        MeError::Unavailable
    })
}
pub fn me_router(
    reader: Arc<dyn MeReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: MeAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=64).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    let context = Arc::new(Context {
        reader,
        sessions,
        security,
        authentication,
        requests: Arc::new(Semaphore::new(requests)),
        rate: Mutex::new(crate::results::ResultRate::new(4096)),
    });
    Ok(Router::new()
        .route("/api/v1/me/overview", post(overview))
        .route("/api/v1/me/history", post(history))
        .layer(DefaultBodyLimit::max(MAX_ME_BYTES))
        .layer(middleware::map_response(no_store))
        .with_state(context))
}
async fn overview(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, false).await
}
async fn history(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, true).await
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > 65536 {
            return Err(std::io::Error::other("Personal response exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn serialize(
    read: MeRead,
    session: &crate::auth::Session,
    query: &MeQuery,
    started: i64,
    now: i64,
) -> Result<Vec<u8>, MeError> {
    if read.account != session.account.id
        || read.revision != session.id
        || read.observed_at < started
        || read.observed_at > now
        || now >= read.valid_until
        || read.valid_until > session.expires_at
    {
        return Err(MeError::Unavailable);
    }
    let valid = match (&read.payload, query) {
        (MePayload::Overview(value), MeQuery::Overview) => {
            value.v == 1
                && value.account.id == session.account.id.to_string()
                && value.observed_at == read.observed_at
                && value.window_started_at == (read.observed_at - ME_RETENTION_SECONDS).max(0)
        }
        (MePayload::History(value), MeQuery::History { kind, opponent, .. }) => {
            value.v == 1
                && value.account_id == session.account.id.to_string()
                && value.kind == *kind
                && value.opponent == *opponent
                && value.observed_at == read.observed_at
                && value.window_started_at == (read.observed_at - ME_RETENTION_SECONDS).max(0)
        }
        _ => false,
    };
    if !valid {
        return Err(MeError::Unavailable);
    }
    let mut output = Limited(Vec::new());
    serde_json::to_writer(&mut output, &read.payload).map_err(|_| MeError::Unavailable)?;
    Ok(output.0)
}
async fn respond(
    ctx: Arc<Context>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
    history: bool,
) -> Response {
    let result = async {
        let started = ctx.authentication.clock.now();

        let cookies = BrowserCookies::parse(&headers).map_err(|_| error(MeError::Malformed))?;

        ctx.security
            .require(&headers, &cookies, started)
            .map_err(|cause| {
                if cause == AuthError::Unavailable {
                    error(MeError::Unavailable)
                } else {
                    (StatusCode::FORBIDDEN, MeError::Unauthorized)
                }
            })?;

        let token = cookies
            .session
            .as_ref()
            .ok_or_else(|| error(MeError::Unauthorized))?;

        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(error(MeError::Malformed));
        }

        let body = body.map_err(|cause| (cause.status(), MeError::Malformed))?;

        let query = if history {
            MeQuery::history(decode_me_history(&body).map_err(error)?).map_err(error)?
        } else {
            decode_me_overview(&body).map_err(error)?;
            MeQuery::Overview
        };

        let _permit = ctx
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| error(MeError::Unavailable))?;

        ctx.rate
            .lock()
            .map_err(|_| error(MeError::Unavailable))?
            .session(token.hash(), started)
            .map_err(rate)?;

        let generation = ctx
            .authentication
            .authorities
            .generation()
            .map_err(authority)?;

        tokio::time::timeout(Duration::from_secs(2), async {
            let session = ctx
                .sessions
                .read(token.hash())
                .await
                .map_err(|_| error(MeError::Unavailable))?
                .filter(|v| model::valid_session(v, ctx.authentication.clock.now()))
                .ok_or_else(|| error(MeError::Unauthorized))?;

            let guard = ctx
                .authentication
                .authorities
                .bind_shared(
                    generation,
                    session.account.id,
                    token.hash(),
                    session.expires_at,
                    ctx.authentication.clock.now(),
                )
                .map_err(authority)?;

            ctx.rate
                .lock()
                .map_err(|_| error(MeError::Unavailable))?
                .account(session.account.id, ctx.authentication.clock.now())
                .map_err(rate)?;

            let mut revoked = guard.revoked();

            let read = async {
                let value = ctx
                    .reader
                    .read(
                        MeSession {
                            account: session.account.id,
                            revision: session.id,
                            hash: token.hash(),
                        },
                        query.clone(),
                    )
                    .await
                    .map_err(error)?;

                let until = value.valid_until;

                let observed = value.observed_at;

                let now = ctx.authentication.clock.now();

                let proof = session.clone();

                let bytes = ctx
                    .authentication
                    .workers
                    .run(move || serialize(value, &proof, &query, started, now))
                    .await
                    .map_err(error)?;

                let last = ctx
                    .sessions
                    .read(token.hash())
                    .await
                    .map_err(|_| error(MeError::Unavailable))?
                    .ok_or_else(|| error(MeError::Unauthorized))?;

                let finish = ctx.authentication.clock.now();

                if finish < now || finish < observed || finish >= until {
                    return Err(error(MeError::Unavailable));
                }

                if !model::same_session(&session, &last, finish) {
                    return Err(error(MeError::Unauthorized));
                }

                ctx.authentication
                    .authorities
                    .with_authority(&guard, finish, || {
                        ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
                    })
                    .map_err(authority)
            };

            tokio::select! {
            biased;
            _=revoked.changed()=>Err(error(MeError::Unauthorized)),value=read=>value}
        })
        .await
        .map_err(|_| error(MeError::Unavailable))?
    }
    .await;

    result.unwrap_or_else(failure)
}
