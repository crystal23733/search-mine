use super::*;
use crate::{
    auth::{AuthClock, AuthError, BrowserCookies, BrowserSecurity, Session},
    online::{AuthorityRegistry, SessionReader},
};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Request, State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use liar_protocol::{consent::*, online::OnlineError, results::ResultError};
use std::sync::{Arc, Mutex};
use tokio::{sync::Semaphore, time::Instant};
#[derive(Clone, Copy)]
struct RequestDeadline(Instant);
use uuid::Uuid;

#[derive(Clone)]
pub struct ConsentAuthentication {
    pub authorities: Arc<AuthorityRegistry>,
    pub clock: Arc<dyn AuthClock>,
}
struct Context<S, V> {
    store: Arc<S>,
    verifier: Arc<V>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: ConsentAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<crate::results::ResultRate>,
}
#[derive(Clone, Copy)]
enum Operation {
    Read,
    Refresh,
    Withdraw,
}
type Failure = (StatusCode, ConsentApiError);
fn error(value: ConsentApiError) -> Failure {
    (
        match value {
            ConsentApiError::Malformed | ConsentApiError::UnsupportedVersion => {
                StatusCode::BAD_REQUEST
            }
            ConsentApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ConsentApiError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ConsentApiError::Conflict => StatusCode::CONFLICT,
            ConsentApiError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        },
        value,
    )
}
fn domain(value: ConsentError) -> Failure {
    error(match value {
        ConsentError::Unauthorized => ConsentApiError::Unauthorized,
        ConsentError::Conflict => ConsentApiError::Conflict,
        ConsentError::Invalid | ConsentError::Unavailable => ConsentApiError::Unavailable,
    })
}
fn authority(value: OnlineError) -> Failure {
    error(if value == OnlineError::Unauthorized {
        ConsentApiError::Unauthorized
    } else {
        ConsentApiError::Unavailable
    })
}
fn rate(value: ResultError) -> Failure {
    error(if value == ResultError::RateLimited {
        ConsentApiError::RateLimited
    } else {
        ConsentApiError::Unavailable
    })
}
fn failure(value: Failure) -> Response {
    (value.0, Json(serde_json::json!({"error":value.1}))).into_response()
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
    failure(error(ConsentApiError::Unavailable))
}
pub fn disabled_consent_router() -> Router {
    Router::new()
        .route("/api/v1/me/consent", post(unavailable))
        .route("/api/v1/me/consent/refresh", post(unavailable))
        .route("/api/v1/me/consent/withdraw", post(unavailable))
        .layer(middleware::map_response(no_store))
}
pub fn consent_router<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    store: Arc<S>,
    verifier: Arc<V>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: ConsentAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=64).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    let ctx = Arc::new(Context {
        store,
        verifier,
        sessions,
        security,
        authentication,
        requests: Arc::new(Semaphore::new(requests)),
        rate: Mutex::new(crate::results::ResultRate::new(4096)),
    });
    Ok(Router::new()
        .route("/api/v1/me/consent", post(read::<S, V>))
        .route("/api/v1/me/consent/refresh", post(refresh::<S, V>))
        .route("/api/v1/me/consent/withdraw", post(withdraw::<S, V>))
        .layer(DefaultBodyLimit::max(MAX_CONSENT_BYTES))
        .layer(middleware::from_fn_with_state(ctx.clone(), bound::<S, V>))
        .layer(middleware::map_response(no_store))
        .with_state(ctx))
}
async fn bound<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    State(ctx): State<Arc<Context<S, V>>>,
    mut request: Request,
    next: Next,
) -> Response {
    let Ok(_permit) = ctx.requests.clone().try_acquire_owned() else {
        return unavailable().await;
    };
    let deadline = Instant::now() + CONSENT_REQUEST_TIMEOUT;
    request.extensions_mut().insert(RequestDeadline(deadline));
    tokio::time::timeout_at(deadline, next.run(request))
        .await
        .unwrap_or_else(|_| failure(error(ConsentApiError::Unavailable)))
}
async fn read<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    State(ctx): State<Arc<Context<S, V>>>,
    Extension(RequestDeadline(deadline)): Extension<RequestDeadline>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Read, deadline).await
}
async fn refresh<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    State(ctx): State<Arc<Context<S, V>>>,
    Extension(RequestDeadline(deadline)): Extension<RequestDeadline>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Refresh, deadline).await
}
async fn withdraw<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    State(ctx): State<Arc<Context<S, V>>>,
    Extension(RequestDeadline(deadline)): Extension<RequestDeadline>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    respond(ctx, uri, headers, body, Operation::Withdraw, deadline).await
}
struct Submission<'a, V> {
    verifier: &'a V,
    evidence: Option<&'a liar_protocol::consent::ConsentEvidence>,
}
impl<V: ConsentVerifier> ConsentVerifier for Submission<'_, V> {
    async fn verify(&self, subject: Uuid, now: i64) -> Result<VerifiedConsent, ConsentError> {
        self.verifier
            .verify_submitted(subject, now, self.evidence)
            .await
    }
}
enum Command {
    Read,
    Refresh(
        Option<Uuid>,
        Option<liar_protocol::consent::ConsentEvidence>,
    ),
    Withdraw(ConsentWithdrawal),
}
fn command(operation: Operation, body: &[u8]) -> Result<Command, Failure> {
    Ok(match operation {
        Operation::Read => {
            decode_consent_read(body).map_err(error)?;
            Command::Read
        }
        Operation::Refresh => {
            let input = decode_consent_refresh(body).map_err(error)?;
            Command::Refresh(
                input
                    .expected_revision
                    .map(|id| Uuid::parse_str(&id).map_err(|_| error(ConsentApiError::Malformed)))
                    .transpose()?,
                input.evidence,
            )
        }
        Operation::Withdraw => {
            Command::Withdraw(decode_consent_withdraw(body).map_err(error)?.purpose)
        }
    })
}
fn snapshot(
    session: &Session,
    current: Option<CurrentConsent>,
    now: i64,
) -> Result<Vec<u8>, Failure> {
    if current.is_some_and(|c| c.subject != session.id || c.expires_at > session.expires_at) {
        return Err(error(ConsentApiError::Unavailable));
    }
    let value = ConsentSnapshot {
        v: 1,
        account_id: session.account.id.to_string(),
        observed_at: now,
        revision: current.map(|c| c.revision.to_string()),
        consent: super::view::project_current(current, now).map_err(domain)?,
    };
    let bytes = serde_json::to_vec(&value).map_err(|_| error(ConsentApiError::Unavailable))?;
    if bytes.len() > MAX_CONSENT_BYTES {
        return Err(error(ConsentApiError::Unavailable));
    }
    Ok(bytes)
}
async fn respond<S: ConsentStore + 'static, V: ConsentVerifier + 'static>(
    ctx: Arc<Context<S, V>>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
    operation: Operation,
    deadline: Instant,
) -> Response {
    let result = async {
        let started = ctx.authentication.clock.now();
        let cookies =
            BrowserCookies::parse(&headers).map_err(|_| error(ConsentApiError::Malformed))?;
        ctx.security
            .require(&headers, &cookies, started)
            .map_err(|e| {
                if e == AuthError::Unavailable {
                    error(ConsentApiError::Unavailable)
                } else {
                    (StatusCode::FORBIDDEN, ConsentApiError::Unauthorized)
                }
            })?;
        let token = cookies
            .session
            .as_ref()
            .ok_or_else(|| error(ConsentApiError::Unauthorized))?;
        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(error(ConsentApiError::Malformed));
        }
        let body = body.map_err(|e| (e.status(), ConsentApiError::Malformed))?;
        let command = command(operation, &body)?;
        ctx.rate
            .lock()
            .map_err(|_| error(ConsentApiError::Unavailable))?
            .session(token.hash(), started)
            .map_err(rate)?;
        let generation = ctx
            .authentication
            .authorities
            .generation()
            .map_err(authority)?;
        let session = ctx
            .sessions
            .read_until(token.hash(), deadline.into_std())
            .await
            .map_err(authority)?
            .filter(|v| v.current_at(ctx.authentication.clock.now()))
            .ok_or_else(|| error(ConsentApiError::Unauthorized))?;
        let before = ctx.authentication.clock.now();
        if before < started {
            return Err(error(ConsentApiError::Unavailable));
        }
        let guard = ctx
            .authentication
            .authorities
            .bind_shared(
                generation,
                session.account.id,
                token.hash(),
                session.expires_at,
                before,
            )
            .map_err(authority)?;
        ctx.rate
            .lock()
            .map_err(|_| error(ConsentApiError::Unavailable))?
            .account(session.account.id, before)
            .map_err(rate)?;
        let mut revoked = guard.revoked();
        let work = async {
            let owner = ConsentSession {
                account: session.account.id,
                revision: session.id,
                hash: token.hash(),
            };
            let current = match command {
                Command::Read => ctx
                    .store
                    .read(owner, deadline.into_std())
                    .await
                    .map_err(domain)?,
                Command::Refresh(expected, ref evidence) => Some(
                    ctx.store
                        .refresh(
                            owner,
                            expected,
                            &Submission {
                                verifier: ctx.verifier.as_ref(),
                                evidence: evidence.as_ref(),
                            },
                            deadline.into_std(),
                        )
                        .await
                        .map_err(domain)?,
                ),
                Command::Withdraw(ConsentWithdrawal::Analytics) => Some(
                    ctx.store
                        .withdraw(owner, OptionalPurpose::Analytics, deadline.into_std())
                        .await
                        .map_err(domain)?,
                ),
                Command::Withdraw(ConsentWithdrawal::Advertising) => Some(
                    ctx.store
                        .withdraw(owner, OptionalPurpose::Advertising, deadline.into_std())
                        .await
                        .map_err(domain)?,
                ),
                Command::Withdraw(ConsentWithdrawal::All) => Some(
                    ctx.store
                        .withdraw_all(owner, deadline.into_std())
                        .await
                        .map_err(domain)?,
                ),
            };
            let observed = ctx.authentication.clock.now();
            let last = ctx
                .sessions
                .read_until(token.hash(), deadline.into_std())
                .await
                .map_err(authority)?
                .ok_or_else(|| error(ConsentApiError::Unauthorized))?;
            let finish = ctx.authentication.clock.now();
            if observed < before || finish < observed {
                return Err(error(ConsentApiError::Unavailable));
            }
            if !session.same_authority(&last, finish) {
                return Err(error(ConsentApiError::Unauthorized));
            }
            if matches!(operation, Operation::Refresh)
                && current.is_some_and(|c| finish >= c.expires_at)
            {
                return Err(error(ConsentApiError::Unavailable));
            }
            let bytes = snapshot(&session, current, finish)?;
            ctx.authentication
                .authorities
                .with_authority(&guard, finish, || {
                    ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
                })
                .map_err(authority)
        };
        tokio::select! {
            biased;
            _ = revoked.changed() => Err(error(ConsentApiError::Unauthorized)),
            value = work => value,
        }
    }
    .await;
    result.unwrap_or_else(failure)
}
