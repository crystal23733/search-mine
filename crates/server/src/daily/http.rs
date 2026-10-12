use super::{boundary::*, service::DailyService, *};
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
use liar_protocol::{daily::MAX_DAILY_REPLAY_BYTES, online::OnlineError, results::ResultError};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct DailyAuthentication {
    authorities: Arc<AuthorityRegistry>,
    clock: Arc<dyn AuthClock>,
    tickets: Arc<DailyTicketRegistry>,
}
impl DailyAuthentication {
    pub fn new(tickets: Arc<DailyTicketRegistry>, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            authorities: tickets.authorities(),
            clock,
            tickets,
        }
    }
}
struct Context {
    service: DailyService,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: DailyAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<ResultRate>,
}
fn failure(error: Failure) -> Response {
    let (status, code) = match error {
        Failure::Invalid => (StatusCode::BAD_REQUEST, "daily_invalid"),
        Failure::Unauthorized => (StatusCode::UNAUTHORIZED, "auth_required"),
        Failure::Conflict => (StatusCode::CONFLICT, "daily_conflict"),
        Failure::Expired => (StatusCode::GONE, "daily_expired"),
        Failure::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "daily_unavailable"),
        Failure::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "daily_rate_limited"),
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
pub fn disabled_daily_router() -> Router {
    let mut router = Router::new();
    for path in [
        "/api/v1/daily/definition",
        "/api/v1/daily/submit",
        "/api/v1/daily/own",
        "/api/v1/daily/start",
        "/api/v1/daily/heartbeat",
        "/api/v1/daily/submit-timed",
    ] {
        router = router.route(path, post(|| async { failure(Failure::Unavailable) }));
    }
    router.layer(middleware::map_response(no_store))
}
pub fn daily_router(
    repository: Arc<dyn DailyRepository>,
    verifier: BoundedReplayVerifier,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: DailyAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=64).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    Ok(Router::new()
        .route("/api/v1/daily/definition", post(definition))
        .route("/api/v1/daily/submit", post(submit))
        .route("/api/v1/daily/own", post(own))
        .route("/api/v1/daily/start", post(start))
        .route("/api/v1/daily/heartbeat", post(heartbeat))
        .route("/api/v1/daily/submit-timed", post(timed_submit))
        .layer(DefaultBodyLimit::max(MAX_DAILY_REPLAY_BYTES + 4096))
        .layer(middleware::map_response(no_store))
        .with_state(Arc::new(Context {
            service: DailyService {
                repository,
                verifier,
                clock: authentication.clock.clone(),
                tickets: authentication.tickets.clone(),
            },
            sessions,
            security,
            authentication,
            requests: Arc::new(Semaphore::new(requests)),
            rate: Mutex::new(ResultRate::new(4096)),
        })))
}
async fn definition(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Definition).await
}
async fn submit(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Submit).await
}
async fn own(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Own).await
}
async fn start(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Start).await
}
async fn heartbeat(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Heartbeat).await
}
async fn timed_submit(
    State(ctx): State<Arc<Context>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::TimedSubmit).await
}
fn authority(error: OnlineError) -> Failure {
    if error == OnlineError::Unauthorized {
        Failure::Unauthorized
    } else {
        Failure::Unavailable
    }
}
fn rate(error: ResultError) -> Failure {
    if error == ResultError::RateLimited {
        Failure::RateLimited
    } else {
        Failure::Unavailable
    }
}
async fn respond(
    ctx: Arc<Context>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
    operation: Operation,
) -> Response {
    let result = async {
        let mut watermark = ctx.authentication.clock.now();
        let cookies = BrowserCookies::parse(&headers).map_err(|_| Failure::Invalid)?;
        ctx.security.require(&headers, &cookies, watermark).map_err(|error| if error == AuthError::Unavailable { Failure::Unavailable } else { Failure::Unauthorized })?;
        let token = cookies.session.as_ref().ok_or(Failure::Unauthorized)?;
        if uri.query().is_some() || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).and_then(|v| v.split(';').next()).map(str::trim) != Some("application/json") { return Err(Failure::Invalid); }
        let body = body.map_err(|_| Failure::Invalid)?;
        let limit = if matches!(operation, Operation::Submit | Operation::TimedSubmit) { MAX_DAILY_REPLAY_BYTES + 4096 } else { 4096 };
        if body.len() > limit { return Err(Failure::Invalid); }
        let _permit = ctx.requests.clone().try_acquire_owned().map_err(|_| Failure::Unavailable)?;
        ctx.rate.lock().map_err(|_| Failure::Unavailable)?.session(token.hash(), watermark).map_err(rate)?;
        ctx.authentication.tickets.reap()?;
        let generation = ctx.authentication.authorities.generation().map_err(authority)?;
        tokio::time::timeout(Duration::from_secs(2), async {
            let first = ctx.sessions.read(token.hash()).await.map_err(|_| Failure::Unavailable)?.ok_or(Failure::Unauthorized)?;
            valid(&first, observe(ctx.authentication.clock.as_ref(), &mut watermark)?)?;
            ctx.rate.lock().map_err(|_| Failure::Unavailable)?.account(first.account.id, watermark).map_err(rate)?;
            let source = body.to_vec();
            let expected = Arc::new(ctx.service.verifier.run(move || decode(source, operation)).await.map_err(Failure::from)?);
            proof(&first, &expected, observe(ctx.authentication.clock.as_ref(), &mut watermark)?)?;
            let lease = ctx.authentication.authorities.bind_shared(generation, expected.account, token.hash(), first.expires_at, watermark).map_err(authority)?;
            let mut revoked = lease.revoked();
            let work = async {
                let reply = ctx.service.execute(DailySession { account: expected.account, hash: token.hash() }, &lease, expected.revision, &expected.input, &mut watermark).await?;
                let now = observe(ctx.authentication.clock.as_ref(), &mut watermark)?;
                let encoding = expected.clone();
                let bytes = ctx.service.verifier.run(move || reply.encode(&encoding, now).map_err(|_| ReplayError::Unavailable)).await.map_err(Failure::from)?;
                observe(ctx.authentication.clock.as_ref(), &mut watermark)?;
                let last = ctx.sessions.read(token.hash()).await.map_err(|_| Failure::Unavailable)?.ok_or(Failure::Unauthorized)?;
                let now = observe(ctx.authentication.clock.as_ref(), &mut watermark)?;
                same(&first, &last, &expected, now)?;
                ctx.authentication.authorities.with_authority(&lease, now, || ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()).map_err(authority)
            };
            tokio::select! { biased; _ = revoked.changed() => Err(Failure::Unauthorized), value = work => value }
        }).await.map_err(|_| Failure::Unavailable)?
    }.await;
    result.unwrap_or_else(failure)
}
