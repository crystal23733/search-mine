use super::*;
use crate::{
    auth::{AuthClock, AuthError, BrowserCookies, BrowserSecurity},
    online::{AuthorityRegistry, SessionReader},
    results::ResultRate,
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
use liar_protocol::{game::PROTOCOL_VERSION, online::OnlineError, results::ResultError};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct PrivacyAuthentication {
    pub authorities: Arc<AuthorityRegistry>,
    pub clock: Arc<dyn AuthClock>,
    pub workers: ExportWorkers,
}
struct Context {
    reader: Arc<dyn GameExportReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: PrivacyAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<ResultRate>,
}
fn failure(cause: ExportError) -> Response {
    let (status, code) = match cause {
        ExportError::Malformed => (StatusCode::BAD_REQUEST, "auth_invalid"),
        ExportError::Unauthorized => (StatusCode::UNAUTHORIZED, "auth_required"),
        ExportError::Reauth => (StatusCode::CONFLICT, "reauth_required"),
        ExportError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "auth_unavailable"),
        ExportError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "auth_rate_limited"),
    };
    (status, Json(serde_json::json!({"code":code}))).into_response()
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
pub fn disabled_privacy_router() -> Router {
    Router::new()
        .route(
            "/api/v1/me/games/export",
            post(|| async { failure(ExportError::Unavailable) }),
        )
        .route(
            "/api/v1/me/games/replay",
            post(|| async { failure(ExportError::Unavailable) }),
        )
        .layer(middleware::map_response(no_store))
}
pub fn privacy_router(
    reader: Arc<dyn GameExportReader>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: PrivacyAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=64).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    Ok(Router::new()
        .route("/api/v1/me/games/export", post(handle))
        .route("/api/v1/me/games/replay", post(replay_handle))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::map_response(no_store))
        .with_state(Arc::new(Context {
            reader,
            sessions,
            security,
            authentication,
            requests: Arc::new(Semaphore::new(requests)),
            rate: Mutex::new(ResultRate::new(4096)),
        })))
}
async fn handle(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, false).await
}
async fn replay_handle(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, true).await
}
enum Selection {
    Page(ExportStream, Option<Uuid>),
    Replay(Uuid),
}
fn selection(
    body: &[u8],
    replay: bool,
    hash: [u8; 32],
) -> Result<(ExportSession, Selection), ExportError> {
    if body.len() > 4096 {
        return Err(ExportError::Malformed);
    }
    let (version, account, revision, selection) = if replay {
        let input: ReplayExportInput =
            serde_json::from_slice(body).map_err(|_| ExportError::Malformed)?;
        (
            input.v,
            input.account_id,
            input.session_revision,
            Selection::Replay(id(&input.completion_id)?),
        )
    } else {
        let input: GameExportInput =
            serde_json::from_slice(body).map_err(|_| ExportError::Malformed)?;
        (
            input.v,
            input.account_id,
            input.session_revision,
            Selection::Page(input.stream, input.after.as_deref().map(id).transpose()?),
        )
    };
    if version != PROTOCOL_VERSION {
        return Err(ExportError::Malformed);
    }
    Ok((
        ExportSession {
            account: id(&account)?,
            revision: id(&revision)?,
            hash,
        },
        selection,
    ))
}
fn rate_error(error: ResultError) -> ExportError {
    if error == ResultError::RateLimited {
        ExportError::RateLimited
    } else {
        ExportError::Unavailable
    }
}
fn authority_error(error: OnlineError) -> ExportError {
    if error == OnlineError::Unauthorized {
        ExportError::Unauthorized
    } else {
        ExportError::Unavailable
    }
}
async fn respond(
    ctx: Arc<Context>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
    replay: bool,
) -> Response {
    let result = async {
        let started = ctx.authentication.clock.now();
        let cookies = BrowserCookies::parse(&headers).map_err(|_| ExportError::Malformed)?;
        ctx.security.require(&headers, &cookies, started).map_err(|cause| if cause==AuthError::Unavailable { ExportError::Unavailable } else { ExportError::Unauthorized })?;
        let token = cookies.session.as_ref().ok_or(ExportError::Unauthorized)?;
        if uri.query().is_some() || headers.get_all(header::CONTENT_TYPE).iter().count()!=1 || headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::trim)!=Some("application/json") { return Err(ExportError::Malformed); }
        let (expected, selection) = selection(&body.map_err(|_| ExportError::Malformed)?, replay, token.hash())?;
        let _permit = ctx.requests.clone().try_acquire_owned().map_err(|_| ExportError::Unavailable)?;
        ctx.rate.lock().map_err(|_| ExportError::Unavailable)?.session(token.hash(), started).map_err(rate_error)?;
        let generation = ctx.authentication.authorities.generation().map_err(authority_error)?;
        tokio::time::timeout(Duration::from_secs(2), async {
            let first = ctx.sessions.read(token.hash()).await.map_err(|_| ExportError::Unavailable)?.ok_or(ExportError::Unauthorized)?;
            let observed = ctx.authentication.clock.now();
            if observed < started { return Err(ExportError::Unavailable); }
            proof(&first, expected, observed)?;
            let authority = ctx.authentication.authorities.bind_shared(generation, expected.account, token.hash(), first.expires_at, observed).map_err(authority_error)?;
            ctx.rate.lock().map_err(|_| ExportError::Unavailable)?.account(expected.account, observed).map_err(rate_error)?;
            let mut revoked = authority.revoked();
            let read = async {
                let (bytes, read_at) = match selection {
                    Selection::Page(stream, after) => {
                        let value = ctx.reader.page(expected, stream, after).await?;
                        let now = ctx.authentication.clock.now();
                        if now < observed { return Err(ExportError::Unavailable); }
                        let bytes = ctx.authentication.workers.run(move || {
                            if value.v!=PROTOCOL_VERSION || value.account_id!=expected.account.to_string() || value.stream!=stream || value.records.len()>50 { return Err(ExportError::Unavailable); }
                            let mut previous = after.unwrap_or(Uuid::nil());
                            for record in &value.records { let current = record_id(record, stream, now)?; if current<=previous { return Err(ExportError::Unavailable); } previous=current; }
                            if value.next.is_some() && (value.records.len()!=50 || value.next.as_deref()!=Some(previous.to_string().as_str())) { return Err(ExportError::Unavailable); }
                            let bytes = serde_json::to_vec(&value).map_err(|_| ExportError::Unavailable)?;
                            if bytes.len()>65536 { return Err(ExportError::Unavailable); } Ok(bytes)
                        }).await?
                        ; (bytes, now)
                    }
                    Selection::Replay(completion) => {
                        let value = ctx.reader.replay(expected, completion).await?;
                        let now = ctx.authentication.clock.now();
                        if now < observed { return Err(ExportError::Unavailable); }
                        let bytes = ctx.authentication.workers.run(move || {
                            if value.v!=PROTOCOL_VERSION || value.account_id!=expected.account.to_string() || value.completion_id!=completion.to_string() { return Err(ExportError::Unavailable); }
                            let bytes=serde_json::to_vec(&value).map_err(|_| ExportError::Unavailable)?;
                            if bytes.len()>liar_protocol::daily::MAX_DAILY_REPLAY_BYTES+4096 { return Err(ExportError::Unavailable); } Ok(bytes)
                        }).await?
                        ; (bytes, now)
                    }
                };
                let last = ctx.sessions.read(token.hash()).await.map_err(|_| ExportError::Unavailable)?.ok_or(ExportError::Unauthorized)?;
                let now = ctx.authentication.clock.now();
                if now<read_at { return Err(ExportError::Unavailable); }
                same(&first, &last, expected, now)?;
                ctx.authentication.authorities.with_authority(&authority, now, || ([(header::CONTENT_TYPE,"application/json")],bytes).into_response()).map_err(authority_error)
            };
            tokio::select! { biased; _=revoked.changed()=>Err(ExportError::Unauthorized), value=read=>value }
        }).await.map_err(|_| ExportError::Unavailable)?
    }.await;
    result.unwrap_or_else(failure)
}
