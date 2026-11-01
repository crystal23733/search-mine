use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_protocol::{me::*, online::OnlineError};
use liar_server::{
    auth::*,
    me::*,
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
}
impl SessionReader for Sessions {
    fn read(&self, _: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            let count = self.reads.fetch_add(1, Ordering::SeqCst);
            let mut value = self.row.lock().unwrap().clone();
            if count > 0 && self.swap.load(Ordering::SeqCst) {
                value.as_mut().unwrap().id = Uuid::new_v4();
            }
            Ok(value)
        })
    }
}
struct Reader {
    reads: AtomicUsize,
    wrong: AtomicBool,
    pause: AtomicBool,
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
impl MeReader for Reader {
    fn read(&self, s: MeSession, q: MeQuery) -> PortFuture<'_, Result<MeRead, MeError>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            if self.pause.load(Ordering::SeqCst) {
                self.resume.notified().await;
            }
            let account = if self.wrong.load(Ordering::SeqCst) {
                Uuid::new_v4()
            } else {
                s.account
            };
            let summary = MeMatchSummary {
                wins: 0,
                losses: 0,
                draws: 0,
                aborted: 0,
                cancelled: 0,
                played: 0,
                win_rate: None,
            };
            let payload = match q {
                MeQuery::Overview => MePayload::Overview(MeOverview {
                    v: 1,
                    account: MeAccount {
                        id: account.to_string(),
                        nickname: None,
                        created_at: 1000,
                        last_seen_at: 1000,
                        identities: vec![],
                    },
                    observed_at: 1500,
                    window_started_at: 0,
                    online: summary,
                    recent_online: vec![],
                    daily_total: 0,
                    recent_daily: vec![],
                    consent: MeConsent {
                        cmp_available: false,
                        status: MeConsentStatus::Unknown,
                        analytics: MeChoice::Unknown,
                        advertising: MeChoice::Unknown,
                        checked_at: None,
                        expires_at: None,
                    },
                }),
                MeQuery::History { kind, opponent, .. } => MePayload::History(MeHistory {
                    v: 1,
                    account_id: account.to_string(),
                    kind,
                    opponent,
                    observed_at: 1500,
                    window_started_at: 0,
                    total: 0,
                    online: (kind == MeKind::Online).then_some(summary),
                    items: vec![],
                    next: None,
                }),
            };
            Ok(MeRead {
                account,
                revision: s.revision,
                observed_at: 1500,
                valid_until: 2000,
                payload,
            })
        })
    }
}
struct Fixture {
    router: axum::Router,
    security: Arc<BrowserSecurity>,
    browser: SecretToken,
    token: SecretToken,
    sessions: Arc<Sessions>,
    reader: Arc<Reader>,
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
    });
    let reader = Arc::new(Reader {
        reads: AtomicUsize::new(0),
        wrong: AtomicBool::new(false),
        pause: AtomicBool::new(false),
        entered: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let security = Arc::new(BrowserSecurity::new("https://game.example", [51; 32]).unwrap());
    let authorities = AuthorityRegistry::new(8).unwrap();
    let router = me_router(
        reader.clone(),
        sessions.clone(),
        security.clone(),
        MeAuthentication {
            authorities: authorities.clone(),
            clock: Arc::new(Clock(AtomicI64::new(1500))),
            workers: MeWorkers::default(),
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
        reader,
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
            .body(Body::from(body.to_string()))
            .unwrap()
    }
}
#[tokio::test]
async fn ordinary_self_views_need_neither_nickname_nor_recent_reauthentication() {
    let f = fixture();
    for (path, body) in [
        ("/api/v1/me/overview", r#"{"v":1}"#),
        (
            "/api/v1/me/history",
            r#"{"v":1,"kind":"online","opponent":"all","cursor":null}"#,
        ),
    ] {
        let response = f
            .router
            .clone()
            .oneshot(f.request(path, body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(value["v"], 1);
    }
    assert_eq!(f.reader.reads.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn body_selection_origin_and_mime_fail_before_the_reader() {
    let f = fixture();
    for (path, body) in [
        ("/api/v1/me/overview?account=x", r#"{"v":1}"#),
        ("/api/v1/me/overview", r#"{"v":1,"account_id":"x"}"#),
        (
            "/api/v1/me/history",
            r#"{"v":1,"kind":"daily","opponent":"human"}"#,
        ),
    ] {
        assert_eq!(
            f.router
                .clone()
                .oneshot(f.request(path, body))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let mut request = f.request("/api/v1/me/overview", r#"{"v":1}"#);
    request
        .headers_mut()
        .insert("origin", "https://other.example".parse().unwrap());
    assert_eq!(
        f.router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut request = f.request("/api/v1/me/overview", r#"{"v":1}"#);
    request
        .headers_mut()
        .insert("content-type", "text/plain".parse().unwrap());
    assert_eq!(
        f.router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(f.reader.reads.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn session_change_and_wrong_subject_cannot_return_personal_data() {
    let f = fixture();
    f.sessions.swap.store(true, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let f = fixture();
    f.reader.wrong.store(true, Ordering::SeqCst);
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn disabled_large_bodies_and_rate_limits_are_explicit_and_never_cached() {
    let response = disabled_me_router()
        .oneshot(
            Request::post("/api/v1/me/overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let f = fixture();
    let response = f
        .router
        .clone()
        .oneshot(f.request("/api/v1/me/overview", &" ".repeat(MAX_ME_BYTES + 1)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    for _ in 0..20 {
        assert_eq!(
            f.router
                .clone()
                .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    let response = f
        .router
        .clone()
        .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(f.reader.reads.load(Ordering::SeqCst), 20);
}
#[tokio::test]
async fn concurrent_shared_reads_respect_request_capacity_and_session_invalidation() {
    let f = fixture();
    f.reader.pause.store(true, Ordering::SeqCst);
    let mut tasks = vec![];
    for _ in 0..4 {
        let router = f.router.clone();
        let request = f.request("/api/v1/me/overview", r#"{"v":1}"#);
        tasks.push(tokio::spawn(async move {
            router.oneshot(request).await.unwrap()
        }));
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            f.reader.entered.notified(),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        f.router
            .clone()
            .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let barrier = f.authorities.session(f.token.hash());
    for task in tasks {
        assert_eq!(task.await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }
    drop(barrier);
}
#[tokio::test]
async fn a_hung_reader_times_out_without_a_partial_success() {
    let f = fixture();
    f.reader.pause.store(true, Ordering::SeqCst);
    let response = f
        .router
        .clone()
        .oneshot(f.request("/api/v1/me/overview", r#"{"v":1}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
