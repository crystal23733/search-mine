use super::*;
use crate::consent::{CONSENT_REQUEST_TIMEOUT, ConsentSession};
use crate::{
    auth::{AuthClock, AuthError, BrowserCookies, BrowserSecurity},
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
use liar_protocol::{events::*, online::OnlineError, results::ResultError};
use std::sync::{Arc, Mutex};
use tokio::{sync::Semaphore, time::Instant};
#[derive(Clone, Copy)]
struct RequestDeadline(Instant);
use uuid::Uuid;

#[derive(Clone)]
pub struct OptionalEventAuthentication {
    pub authorities: Arc<AuthorityRegistry>,
    pub clock: Arc<dyn AuthClock>,
}
struct Context<S> {
    store: Arc<S>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: OptionalEventAuthentication,
    requests: Arc<Semaphore>,
    rate: Mutex<crate::results::ResultRate>,
}
type Failure = (StatusCode, OptionalEventApiError);
fn error(value: OptionalEventApiError) -> Failure {
    (
        match value {
            OptionalEventApiError::Malformed | OptionalEventApiError::UnsupportedVersion => {
                StatusCode::BAD_REQUEST
            }
            OptionalEventApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            OptionalEventApiError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            OptionalEventApiError::Consent => StatusCode::FORBIDDEN,
            OptionalEventApiError::Conflict => StatusCode::CONFLICT,
            OptionalEventApiError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        },
        value,
    )
}
fn domain(value: OptionalEventError) -> Failure {
    error(match value {
        OptionalEventError::Unauthorized => OptionalEventApiError::Unauthorized,
        OptionalEventError::Conflict => OptionalEventApiError::Conflict,
        OptionalEventError::Invalid => OptionalEventApiError::Malformed,
        OptionalEventError::Consent | OptionalEventError::Expired => OptionalEventApiError::Consent,
        OptionalEventError::Unavailable => OptionalEventApiError::Unavailable,
    })
}
fn authority(value: OnlineError) -> Failure {
    error(if value == OnlineError::Unauthorized {
        OptionalEventApiError::Unauthorized
    } else {
        OptionalEventApiError::Unavailable
    })
}
fn rate(value: ResultError) -> Failure {
    error(if value == ResultError::RateLimited {
        OptionalEventApiError::RateLimited
    } else {
        OptionalEventApiError::Unavailable
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
    failure(error(OptionalEventApiError::Unavailable))
}
pub fn disabled_optional_event_router() -> Router {
    Router::new()
        .route("/api/v1/events/optional", post(unavailable))
        .layer(middleware::map_response(no_store))
}
pub fn optional_event_router<S: OptionalEventStore + 'static>(
    store: Arc<S>,
    sessions: Arc<dyn SessionReader>,
    security: Arc<BrowserSecurity>,
    authentication: OptionalEventAuthentication,
    requests: usize,
) -> Result<Router, OnlineError> {
    if !(1..=64).contains(&requests) {
        return Err(OnlineError::Capacity);
    }
    let ctx = Arc::new(Context {
        store,
        sessions,
        security,
        authentication,
        requests: Arc::new(Semaphore::new(requests)),
        rate: Mutex::new(crate::results::ResultRate::new(4096)),
    });
    Ok(Router::new()
        .route("/api/v1/events/optional", post(capture::<S>))
        .layer(DefaultBodyLimit::max(MAX_OPTIONAL_EVENT_BYTES))
        .layer(middleware::from_fn_with_state(ctx.clone(), bound::<S>))
        .layer(middleware::map_response(no_store))
        .with_state(ctx))
}
async fn bound<S: OptionalEventStore + 'static>(
    State(ctx): State<Arc<Context<S>>>,
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
        .unwrap_or_else(|_| failure(error(OptionalEventApiError::Unavailable)))
}
async fn capture<S: OptionalEventStore + 'static>(
    State(ctx): State<Arc<Context<S>>>,
    Extension(RequestDeadline(deadline)): Extension<RequestDeadline>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let result = async {
        let started = ctx.authentication.clock.now();
        let cookies =
            BrowserCookies::parse(&headers).map_err(|_| error(OptionalEventApiError::Malformed))?;
        ctx.security
            .require(&headers, &cookies, started)
            .map_err(|e| {
                if e == AuthError::Unavailable {
                    error(OptionalEventApiError::Unavailable)
                } else {
                    (StatusCode::FORBIDDEN, OptionalEventApiError::Unauthorized)
                }
            })?;
        let token = cookies
            .session
            .as_ref()
            .ok_or_else(|| error(OptionalEventApiError::Unauthorized))?;
        if uri.query().is_some()
            || headers.get_all(header::CONTENT_TYPE).iter().count() != 1
            || headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .map(str::trim)
                != Some("application/json")
        {
            return Err(error(OptionalEventApiError::Malformed));
        }
        let body = body.map_err(|e| (e.status(), OptionalEventApiError::Malformed))?;
        let value = decode_optional_capture(&body).map_err(error)?;
        let expected_revision = Uuid::parse_str(&value.expected_revision)
            .map_err(|_| error(OptionalEventApiError::Malformed))?;
        let draft = decode_client(
            &serde_json::to_vec(&value.draft)
                .map_err(|_| error(OptionalEventApiError::Malformed))?,
        )
        .map_err(|e| {
            error(if e == EventError::Unsupported {
                OptionalEventApiError::UnsupportedVersion
            } else {
                OptionalEventApiError::Malformed
            })
        })?;
        let event_id = value.draft.event_id;
        ctx.rate
            .lock()
            .map_err(|_| error(OptionalEventApiError::Unavailable))?
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
            .ok_or_else(|| error(OptionalEventApiError::Unauthorized))?;
        let before = ctx.authentication.clock.now();
        if before < started {
            return Err(error(OptionalEventApiError::Unavailable));
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
            .map_err(|_| error(OptionalEventApiError::Unavailable))?
            .account(session.account.id, before)
            .map_err(rate)?;
        let mut revoked = guard.revoked();
        let work = async {
            let owner = ConsentSession {
                account: session.account.id,
                revision: session.id,
                hash: token.hash(),
            };
            let stored = ctx
                .store
                .capture_until(
                    OptionalCapture {
                        session: owner,
                        expected_revision,
                        draft,
                    },
                    deadline.into_std(),
                )
                .await
                .map_err(domain)?;
            let observed = ctx.authentication.clock.now();
            let last = ctx
                .sessions
                .read_until(token.hash(), deadline.into_std())
                .await
                .map_err(authority)?
                .ok_or_else(|| error(OptionalEventApiError::Unauthorized))?;
            let finish = ctx.authentication.clock.now();
            if observed < before || finish < observed {
                return Err(error(OptionalEventApiError::Unavailable));
            }
            if !session.same_authority(&last, finish) {
                return Err(error(OptionalEventApiError::Unauthorized));
            }
            if stored.event.id().uuid().to_string() != event_id
                || stored.received_at < stored.event.occurred_at()
                || stored.received_at > finish
                || stored.event.purpose() == Purpose::Essential
            {
                return Err(error(OptionalEventApiError::Unavailable));
            }
            let receipt = OptionalEventReceipt {
                v: 1,
                event_id,
                received_at: stored.received_at,
            };
            ctx.authentication
                .authorities
                .with_authority(&guard, finish, || Json(receipt).into_response())
                .map_err(authority)
        };
        tokio::select! {
            biased;
            _ = revoked.changed() => Err(error(OptionalEventApiError::Unauthorized)),
            value = work => value,
        }
    }
    .await;
    result.unwrap_or_else(failure)
}
