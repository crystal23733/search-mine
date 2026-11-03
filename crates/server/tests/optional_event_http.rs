use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
#[tokio::test]
async fn unconfigured_collector_has_a_closed_unavailable_response() {
    let response = liar_server::app(None)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/events/optional")
                .body(Body::from(r#"{"v":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value, serde_json::json!({"error":"unavailable"}));
}

use liar_protocol::online::OnlineError;
use liar_server::{
    auth::*,
    consent::ConsentSession,
    events::*,
    online::{AuthorityRegistry, PortFuture, SessionReader},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::Instant,
};
use uuid::Uuid;
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Sessions {
    row: Mutex<Option<Session>>,
    deadlines: Mutex<Vec<Instant>>,
    swap: Mutex<bool>,
}
impl SessionReader for Sessions {
    fn read(&self, _: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            let mut row = self.row.lock().unwrap();
            if *self.swap.lock().unwrap() && self.deadlines.lock().unwrap().len() > 1 {
                row.as_mut().unwrap().id = Uuid::new_v4();
            }
            Ok(row.clone())
        })
    }
    fn read_until(
        &self,
        hash: [u8; 32],
        deadline: Instant,
    ) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        self.deadlines.lock().unwrap().push(deadline);
        self.read(hash)
    }
}
struct Store {
    calls: Mutex<Vec<(ConsentSession, Instant)>>,
    failure: Mutex<Option<OptionalEventError>>,
    pause: Mutex<bool>,
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
impl OptionalEventStore for Store {
    async fn capture_until(
        &self,
        input: OptionalCapture,
        deadline: Instant,
    ) -> Result<StoredEvent, OptionalEventError> {
        self.calls.lock().unwrap().push((input.session, deadline));
        self.entered.notify_one();
        if *self.pause.lock().unwrap() {
            self.resume.notified().await;
        }
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        let event = admit_client(
            input.draft,
            ConsentSnapshot {
                subject: Some(EventId::parse("00000000-0000-0000-0000-000000000002").unwrap()),
                analytics: ConsentState::Granted {
                    revision: input.expected_revision,
                    valid_from: 1000,
                    expires_at: 2000,
                },
                advertising: ConsentState::Denied,
            },
            1500,
        )
        .unwrap();
        Ok(StoredEvent {
            event,
            received_at: 1500,
            inserted: true,
        })
    }
}
struct Fixture {
    router: axum::Router,
    security: Arc<BrowserSecurity>,
    browser: SecretToken,
    token: SecretToken,
    sessions: Arc<Sessions>,
    store: Arc<Store>,
    clock: Arc<Clock>,
    authorities: Arc<AuthorityRegistry>,
}
fn fixture() -> Fixture {
    let sessions = Arc::new(Sessions {
        row: Mutex::new(Some(Session {
            id: Uuid::new_v4(),
            account: Account {
                id: Uuid::new_v4(),
                nickname: None,
            },
            created_at: 1000,
            authenticated_at: 1000,
            expires_at: 2000,
        })),
        deadlines: Mutex::new(vec![]),
        swap: Mutex::new(false),
    });
    let store = Arc::new(Store {
        calls: Mutex::new(vec![]),
        failure: Mutex::new(None),
        pause: Mutex::new(false),
        entered: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let clock = Arc::new(Clock(AtomicI64::new(1500)));
    let security = Arc::new(BrowserSecurity::new("https://game.example", [51; 32]).unwrap());
    let authorities = AuthorityRegistry::new(8).unwrap();
    let router = optional_event_router(
        store.clone(),
        sessions.clone(),
        security.clone(),
        OptionalEventAuthentication {
            authorities: authorities.clone(),
            clock: clock.clone(),
        },
        4,
    )
    .unwrap();
    Fixture {
        router,
        security,
        browser: SecretToken::generate().unwrap(),
        token: SecretToken::generate().unwrap(),
        sessions,
        store,
        clock,
        authorities,
    }
}
fn body() -> serde_json::Value {
    serde_json::json!({"v":1,"expected_revision":"00000000-0000-0000-0000-000000000005","draft":{"event_version":1,"event_id":"00000000-0000-0000-0000-000000000001","session_key":"00000000-0000-0000-0000-000000000002","occurred_at":1100,"locale":"ko","event":{"type":"landing_view","route":"home"}}})
}
impl Fixture {
    fn request(&self, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/v1/events/optional")
            .header("origin", "https://game.example")
            .header("content-type", "application/json")
            .header(
                "x-liar-csrf",
                self.security
                    .csrf(&self.browser, Some(&self.token), self.clock.now())
                    .unwrap(),
            )
            .header(
                "cookie",
                format!(
                    "__Host-liar_browser={}; __Host-liar_session={}",
                    self.browser.expose().as_str(),
                    self.token.expose().as_str()
                ),
            )
            .body(Body::from(value.to_string()))
            .unwrap()
    }
}
#[tokio::test]
async fn current_cookie_owns_the_event_and_receipt_has_no_private_payload() {
    let f = fixture();
    let own = f.sessions.row.lock().unwrap().clone().unwrap();
    let r = f.router.clone().oneshot(f.request(body())).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["cache-control"], "no-store");
    let v: serde_json::Value =
        serde_json::from_slice(&to_bytes(r.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"v":1,"event_id":"00000000-0000-0000-0000-000000000001","received_at":1500})
    );
    let calls = f.store.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].0.account, calls[0].0.revision, calls[0].0.hash),
        (own.account.id, own.id, f.token.hash())
    );
}
#[tokio::test]
async fn missing_cookie_is_unauthorized_and_current_denial_never_becomes_success() {
    let f = fixture();
    let mut req = f.request(body());
    req.headers_mut().remove("cookie");
    assert_eq!(
        f.router.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert!(f.store.calls.lock().unwrap().is_empty());
    *f.store.failure.lock().unwrap() = Some(OptionalEventError::Consent);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn closed_inputs_and_browser_proofs_are_rejected_before_storage() {
    let f = fixture();
    for mode in 0..11 {
        let mut value = body();
        match mode {
            0 => {
                value["account_id"] = serde_json::json!(Uuid::new_v4().to_string());
            }
            1 => {
                value["draft"]["event"]["email"] = serde_json::json!("forbidden");
            }
            2 => {
                value["v"] = serde_json::json!(2);
            }
            3 => {
                value["expected_revision"] = serde_json::json!(Uuid::nil().to_string());
            }
            4 => {
                value["draft"]["locale"] = serde_json::json!("xx");
            }
            5 => {
                value["draft"]["event"] =
                    serde_json::json!({"type":"match_end","match_id":Uuid::new_v4().to_string()});
            }
            _ => {}
        }
        let mut request = f.request(value);
        match mode {
            6 => {
                request
                    .headers_mut()
                    .insert("origin", "https://other.example".parse().unwrap());
            }
            7 => {
                request.headers_mut().remove("x-liar-csrf");
            }
            8 => {
                *request.uri_mut() = "/api/v1/events/optional?account=x".parse().unwrap();
            }
            9 => {
                request
                    .headers_mut()
                    .append("content-type", "application/json".parse().unwrap());
            }
            10 => {
                *request.body_mut() = Body::from(vec![b' '; 2049]);
            }
            _ => {}
        }
        let r = f.router.clone().oneshot(request).await.unwrap();
        assert_eq!(
            r.status(),
            match mode {
                6 | 7 => StatusCode::FORBIDDEN,
                10 => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::BAD_REQUEST,
            },
            "case {mode}"
        );
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
    assert!(f.store.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn domain_errors_are_closed_and_never_reveal_a_payload() {
    let f = fixture();
    for (failure, status, name) in [
        (OptionalEventError::Invalid, 400, "malformed"),
        (OptionalEventError::Unauthorized, 401, "unauthorized"),
        (OptionalEventError::Consent, 403, "consent"),
        (OptionalEventError::Expired, 403, "consent"),
        (OptionalEventError::Conflict, 409, "conflict"),
        (OptionalEventError::Unavailable, 503, "unavailable"),
    ] {
        *f.store.failure.lock().unwrap() = Some(failure);
        let r = f.router.clone().oneshot(f.request(body())).await.unwrap();
        assert_eq!(r.status().as_u16(), status);
        let v: serde_json::Value =
            serde_json::from_slice(&to_bytes(r.into_body(), 2048).await.unwrap()).unwrap();
        assert_eq!(v, serde_json::json!({"error":name}));
    }
}
#[tokio::test]
async fn missing_expired_or_replaced_sessions_cannot_receive_a_success() {
    for mode in 0..3 {
        let f = fixture();
        match mode {
            0 => *f.sessions.row.lock().unwrap() = None,
            1 => f.sessions.row.lock().unwrap().as_mut().unwrap().expires_at = 1500,
            _ => *f.sessions.swap.lock().unwrap() = true,
        }
        let r = f.router.clone().oneshot(f.request(body())).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(f.store.calls.lock().unwrap().len(), usize::from(mode == 2));
    }
}
#[tokio::test]
async fn rate_is_bounded_and_clock_rollback_fails_closed() {
    let f = fixture();
    for _ in 0..20 {
        assert_eq!(
            f.router
                .clone()
                .oneshot(f.request(body()))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    f.clock.0.store(1501, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    f.clock.0.store(1500, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn revocation_or_clock_rollback_cancels_the_receipt_while_store_is_waiting() {
    for rollback in [false, true] {
        let f = fixture();
        *f.store.pause.lock().unwrap() = true;
        let request = f.request(body());
        let router = f.router.clone();
        let task = tokio::spawn(async move { router.oneshot(request).await.unwrap() });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            f.store.entered.notified(),
        )
        .await
        .unwrap();
        if rollback {
            f.clock.0.store(1499, Ordering::SeqCst);
            f.store.resume.notify_one();
            assert_eq!(
                task.await.unwrap().status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        } else {
            let barrier = f.authorities.session(f.token.hash());
            assert_eq!(task.await.unwrap().status(), StatusCode::UNAUTHORIZED);
            drop(barrier);
        }
    }
}
#[tokio::test(start_paused = true)]
async fn slow_body_and_both_session_proofs_and_storage_share_the_original_deadline() {
    use futures_util::poll;
    let f = fixture();
    let start = tokio::time::Instant::now();
    let deadline = (start + std::time::Duration::from_secs(2)).into_std();
    let mut request = f.request(body());
    let bytes = body().to_string();
    *request.body_mut() = Body::from_stream(futures_util::stream::once(async move {
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        Ok::<_, std::io::Error>(axum::body::Bytes::from(bytes))
    }));
    let mut pending = Box::pin(f.router.clone().oneshot(request));
    assert!(poll!(pending.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(750)).await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        *f.sessions.deadlines.lock().unwrap(),
        vec![deadline, deadline]
    );
    assert_eq!(f.store.calls.lock().unwrap()[0].1, deadline);
}
#[tokio::test(start_paused = true)]
async fn four_slow_requests_consume_capacity_and_release_it_at_the_whole_request_deadline() {
    use futures_util::poll;
    let f = fixture();
    let mut pending = vec![];
    for _ in 0..4 {
        let mut request = f.request(body());
        *request.body_mut() = Body::from_stream(futures_util::stream::pending::<
            Result<axum::body::Bytes, std::io::Error>,
        >());
        let mut request = Box::pin(f.router.clone().oneshot(request));
        assert!(poll!(request.as_mut()).is_pending());
        pending.push(request);
    }
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    tokio::time::advance(std::time::Duration::from_secs(2)).await;
    for request in pending {
        let r = request.await.unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request(body()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
