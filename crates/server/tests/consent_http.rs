use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_protocol::{consent::MAX_CONSENT_BYTES, online::OnlineError};
use liar_server::{
    auth::*,
    consent::*,
    online::{AuthorityRegistry, PortFuture, SessionReader},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
};
use tower::ServiceExt;
use uuid::Uuid;
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Sessions {
    row: Mutex<Option<Session>>,
    reads: AtomicUsize,
    swap: AtomicBool,
    deadlines: Mutex<Vec<std::time::Instant>>,
}
impl SessionReader for Sessions {
    fn read(&self, _: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            let n = self.reads.fetch_add(1, Ordering::SeqCst);
            let mut row = self.row.lock().unwrap().clone();
            if n > 0 && self.swap.load(Ordering::SeqCst) {
                row.as_mut().unwrap().id = Uuid::new_v4();
            }
            Ok(row)
        })
    }
    fn read_until(
        &self,
        hash: [u8; 32],
        deadline: std::time::Instant,
    ) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        self.deadlines.lock().unwrap().push(deadline);
        self.read(hash)
    }
}
#[derive(Default)]
struct Store {
    current: Mutex<Option<CurrentConsent>>,
    calls: AtomicUsize,
    expected: Mutex<Option<Uuid>>,
    deadlines: Mutex<Vec<std::time::Instant>>,
    pause: AtomicBool,
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
impl Store {
    async fn touch(&self, deadline: std::time::Instant) {
        self.deadlines.lock().unwrap().push(deadline);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.pause.load(Ordering::SeqCst) {
            self.resume.notified().await;
        }
    }
}
impl ConsentStore for Store {
    async fn read(
        &self,
        _: ConsentSession,
        deadline: std::time::Instant,
    ) -> Result<Option<CurrentConsent>, ConsentError> {
        self.touch(deadline).await;
        Ok(*self.current.lock().unwrap())
    }
    async fn refresh(
        &self,
        s: ConsentSession,
        e: Option<Uuid>,
        v: &impl ConsentVerifier,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.touch(deadline).await;
        *self.expected.lock().unwrap() = e;
        v.verify(s.revision, 1500).await?;
        self.current
            .lock()
            .unwrap()
            .ok_or(ConsentError::Unavailable)
    }
    async fn withdraw(
        &self,
        _: ConsentSession,
        _: OptionalPurpose,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.touch(deadline).await;
        self.current
            .lock()
            .unwrap()
            .ok_or(ConsentError::Unavailable)
    }
    async fn withdraw_all(
        &self,
        _: ConsentSession,
        deadline: std::time::Instant,
    ) -> Result<CurrentConsent, ConsentError> {
        self.touch(deadline).await;
        self.current
            .lock()
            .unwrap()
            .ok_or(ConsentError::Unavailable)
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
        reads: AtomicUsize::new(0),
        swap: AtomicBool::new(false),
        deadlines: Mutex::new(vec![]),
    });
    let store = Arc::new(Store::default());
    let clock = Arc::new(Clock(AtomicI64::new(1500)));
    let security = Arc::new(BrowserSecurity::new("https://game.example", [51; 32]).unwrap());
    let authorities = AuthorityRegistry::new(8).unwrap();
    let router = consent_router(
        store.clone(),
        Arc::new(DisabledConsentVerifier),
        sessions.clone(),
        security.clone(),
        ConsentAuthentication {
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
impl Fixture {
    fn request(&self, path: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("origin", "https://game.example")
            .header("content-type", "application/json")
            .header(
                "x-liar-csrf",
                self.security
                    .csrf(&self.browser, Some(&self.token), 1500)
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
            .body(Body::from(body.to_owned()))
            .unwrap()
    }
    fn denied(&self) -> CurrentConsent {
        CurrentConsent {
            subject: self.sessions.row.lock().unwrap().as_ref().unwrap().id,
            revision: Uuid::new_v4(),
            checked_at: 1400,
            expires_at: 1900,
            analytics: false,
            advertising: false,
        }
    }
}
async fn snapshot(response: axum::response::Response) -> serde_json::Value {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
#[tokio::test]
async fn ordinary_current_control_needs_neither_nickname_nor_recent_reauthentication() {
    let f = fixture();
    let r = snapshot(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["revision"], serde_json::Value::Null);
    assert_eq!(r["consent"]["status"], "unknown");
    assert_eq!(r["consent"]["cmp_available"], false);
    assert_eq!(
        r["account_id"],
        f.sessions
            .row
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .account
            .id
            .to_string()
    );
    *f.store.current.lock().unwrap() = Some(f.denied());
    let r = snapshot(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent/withdraw", r#"{"v":1,"purpose":"all"}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["consent"]["analytics"], "denied");
    assert_eq!(r["consent"]["advertising"], "denied");
    let body = r.to_string();
    for secret in [
        "subject",
        "token_hash",
        "session_id",
        "csrf",
        "credential",
        "tc_string",
    ] {
        assert!(!body.contains(secret));
    }
}
#[tokio::test]
async fn malformed_or_unauthorized_requests_do_not_reach_the_store() {
    let f = fixture();
    for (path, body) in [
        ("/api/v1/me/consent", r#"{"v":1,"account_id":"foreign"}"#),
        (
            "/api/v1/me/consent/refresh",
            r#"{"v":1,"expected_revision":null,"analytics":true}"#,
        ),
        (
            "/api/v1/me/consent/withdraw",
            r#"{"v":1,"purpose":"unknown"}"#,
        ),
        ("/api/v1/me/consent?scope=all", r#"{"v":1}"#),
    ] {
        let r = f
            .router
            .clone()
            .oneshot(f.request(path, body))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
    let mut request = f.request("/api/v1/me/consent", r#"{"v":1}"#);
    request.headers_mut().remove("x-liar-csrf");
    assert_eq!(
        f.router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    *f.sessions.row.lock().unwrap() = None;
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.store.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn replaced_session_or_corrupt_subject_never_returns_a_private_snapshot() {
    let f = fixture();
    f.sessions.swap.store(true, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let f = fixture();
    let mut value = f.denied();
    value.subject = Uuid::new_v4();
    *f.store.current.lock().unwrap() = Some(value);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn disabled_app_registers_consent_control_as_unavailable_without_caching() {
    for path in [
        "/api/v1/me/consent",
        "/api/v1/me/consent/refresh",
        "/api/v1/me/consent/withdraw",
    ] {
        let r = liar_server::app(None)
            .oneshot(Request::post(path).body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
}

#[tokio::test]
async fn mime_body_limits_and_session_account_rate_apply_to_all_controls() {
    let f = fixture();
    let r = f
        .router
        .clone()
        .oneshot(f.request("/api/v1/me/consent", &" ".repeat(MAX_CONSENT_BYTES + 1)))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(r.headers()["cache-control"], "no-store");
    let mut wrong = f.request("/api/v1/me/consent", r#"{"v":1}"#);
    wrong
        .headers_mut()
        .insert("content-type", "text/plain".parse().unwrap());
    assert_eq!(
        f.router.clone().oneshot(wrong).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    for _ in 0..20 {
        assert_eq!(
            f.router
                .clone()
                .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent/withdraw", r#"{"v":1,"purpose":"all"}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    f.clock.0.store(1501, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    f.clock.0.store(1500, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn expired_views_are_unknown_and_disabled_refresh_cannot_claim_a_grant() {
    let f = fixture();
    let mut value = f.denied();
    value.analytics = true;
    value.advertising = true;
    value.expires_at = 1500;
    *f.store.current.lock().unwrap() = Some(value);
    let r = snapshot(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(r["consent"]["status"], "expired");
    assert_eq!(r["consent"]["analytics"], "unknown");
    let request = f.request(
        "/api/v1/me/consent/refresh",
        &format!(r#"{{"v":1,"expected_revision":"{}"}}"#, value.revision),
    );
    assert_eq!(
        f.router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(*f.store.expected.lock().unwrap(), Some(value.revision));
}

#[tokio::test]
async fn withdrawal_responses_are_cancelled_when_the_session_is_revoked_or_clock_rolls_back() {
    for rollback in [false, true] {
        let f = fixture();
        *f.store.current.lock().unwrap() = Some(f.denied());
        f.store.pause.store(true, Ordering::SeqCst);
        let request = f.request("/api/v1/me/consent/withdraw", r#"{"v":1,"purpose":"all"}"#);
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
async fn slow_bodies_consume_the_shared_budget_and_end_at_the_original_whole_request_deadline() {
    use futures_util::poll;
    let f = fixture();
    let mut pending = vec![];
    for _ in 0..4 {
        let mut request = f.request("/api/v1/me/consent", r#"{"v":1}"#);
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
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    tokio::time::advance(std::time::Duration::from_secs(2)).await;
    for request in pending {
        assert_eq!(
            request.await.unwrap().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    assert_eq!(f.store.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test(start_paused = true)]
async fn body_and_both_session_proofs_and_store_share_one_original_deadline() {
    use futures_util::poll;
    let f = fixture();
    let start = tokio::time::Instant::now();
    let original = (start + CONSENT_REQUEST_TIMEOUT).into_std();
    let mut request = f.request("/api/v1/me/consent", r#"{"v":1}"#);
    *request.body_mut() = Body::from_stream(futures_util::stream::once(async {
        tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(br#"{"v":1}"#))
    }));
    let mut pending = Box::pin(f.router.clone().oneshot(request));
    assert!(poll!(pending.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(750)).await;
    assert_eq!(pending.await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        *f.sessions.deadlines.lock().unwrap(),
        vec![original, original]
    );
    assert_eq!(*f.store.deadlines.lock().unwrap(), vec![original]);
    assert_eq!(
        tokio::time::Instant::from_std(original) - tokio::time::Instant::now(),
        std::time::Duration::from_millis(1250)
    );
}
