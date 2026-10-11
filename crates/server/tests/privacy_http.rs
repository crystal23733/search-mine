use axum::{Router, http::header};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_protocol::{online::OnlineError, privacy::*};
use liar_server::{
    auth::*,
    online::{AuthorityRegistry, PortFuture, SessionReader},
    privacy::*,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
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
    fail: AtomicBool,
    post_clock: Mutex<Option<(Arc<Clock>, i64)>>,
}
impl SessionReader for Sessions {
    fn read(&self, _: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            let read = self.reads.fetch_add(1, Ordering::SeqCst);
            if read > 0
                && let Some((clock, now)) = self.post_clock.lock().unwrap().as_ref()
            {
                clock.0.store(*now, Ordering::SeqCst);
            }
            if self.fail.load(Ordering::SeqCst) {
                Err(OnlineError::Unavailable)
            } else {
                Ok(self.row.lock().unwrap().clone())
            }
        })
    }
}
#[derive(Default)]
struct Records {
    hold: AtomicBool,
    entered: Notify,
    released: Notify,
    fail: AtomicBool,
    wrong: AtomicBool,
    reads: AtomicUsize,
    after_clock: Mutex<Option<(Arc<Clock>, i64)>>,
}
impl Records {
    async fn wait(&self) -> Result<(), ExportError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.hold.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.released.notified().await;
        }
        if let Some((clock, now)) = self.after_clock.lock().unwrap().as_ref() {
            clock.0.store(*now, Ordering::SeqCst);
        }
        if self.fail.load(Ordering::SeqCst) {
            Err(ExportError::Unavailable)
        } else {
            Ok(())
        }
    }
}
impl GameExportReader for Records {
    fn page(
        &self,
        session: ExportSession,
        stream: ExportStream,
        _: Option<Uuid>,
    ) -> PortFuture<'_, Result<GameExportPage, ExportError>> {
        Box::pin(async move {
            self.wait().await?;
            Ok(GameExportPage {
                v: 1,
                account_id: if self.wrong.load(Ordering::SeqCst) {
                    Uuid::new_v4().to_string()
                } else {
                    session.account.to_string()
                },
                stream,
                records: vec![],
                next: None,
            })
        })
    }
    fn replay(
        &self,
        session: ExportSession,
        completion: Uuid,
    ) -> PortFuture<'_, Result<ReplayExport, ExportError>> {
        Box::pin(async move {
            self.wait().await?;
            Ok(ReplayExport {
                v: 1,
                account_id: session.account.to_string(),
                completion_id: if self.wrong.load(Ordering::SeqCst) {
                    Uuid::new_v4().to_string()
                } else {
                    completion.to_string()
                },
                replay: None,
            })
        })
    }
}
struct Fixture {
    app: Router,
    clock: Arc<Clock>,
    sessions: Arc<Sessions>,
    records: Arc<Records>,
    authorities: Arc<AuthorityRegistry>,
    security: Arc<BrowserSecurity>,
    browser: SecretToken,
    token: SecretToken,
    account: Uuid,
    revision: Uuid,
}
impl Fixture {
    fn new(requests: usize) -> Self {
        let account = Uuid::new_v4();
        let revision = Uuid::new_v4();
        let clock = Arc::new(Clock(AtomicI64::new(18000)));
        let sessions = Arc::new(Sessions {
            row: Mutex::new(Some(Session {
                id: revision,
                account: Account {
                    id: account,
                    nickname: None,
                },
                created_at: 18000,
                authenticated_at: 18000,
                expires_at: 20000,
            })),
            reads: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            post_clock: Mutex::new(None),
        });
        let records = Arc::new(Records::default());
        let authorities = AuthorityRegistry::new(64).unwrap();
        let security = Arc::new(BrowserSecurity::new("https://game.example", [5; 32]).unwrap());
        let app = privacy_router(
            records.clone(),
            sessions.clone(),
            security.clone(),
            PrivacyAuthentication {
                authorities: authorities.clone(),
                clock: clock.clone(),
                workers: ExportWorkers::default(),
            },
            requests,
        )
        .unwrap();
        Self {
            app,
            clock,
            sessions,
            records,
            authorities,
            security,
            browser: SecretToken::generate().unwrap(),
            token: SecretToken::generate().unwrap(),
            account,
            revision,
        }
    }
    fn body(&self, replay: bool) -> Value {
        let mut body = json!({"v":1,"account_id":self.account.to_string(),"session_revision":self.revision.to_string()});
        if replay {
            body["completion_id"] = Uuid::new_v4().to_string().into();
        } else {
            body["stream"] = "online".into();
            body["after"] = Value::Null;
        }
        body
    }
    fn request(&self, replay: bool, body: Value) -> Request<Body> {
        Request::post(if replay {
            "/api/v1/me/games/replay"
        } else {
            "/api/v1/me/games/export"
        })
        .header(header::ORIGIN, "https://game.example")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!(
                "{BROWSER_COOKIE}={}; {SESSION_COOKIE}={}",
                self.browser.expose().as_str(),
                self.token.expose().as_str()
            ),
        )
        .header(
            "x-liar-csrf",
            self.security
                .csrf(&self.browser, Some(&self.token), self.clock.now())
                .unwrap(),
        )
        .body(Body::from(body.to_string()))
        .unwrap()
    }
    async fn call(&self, replay: bool, body: Value) -> (StatusCode, Value) {
        response(self.app.clone(), self.request(replay, body)).await
    }
}

#[tokio::test]
async fn response_clock_cannot_roll_back_after_reader_completion() {
    for replay in [false, true] {
        let f = Fixture::new(4);
        *f.records.after_clock.lock().unwrap() = Some((f.clock.clone(), 18002));
        *f.sessions.post_clock.lock().unwrap() = Some((f.clock.clone(), 18001));
        assert_eq!(
            f.call(replay, f.body(replay)).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
async fn response(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let r = app.oneshot(request).await.unwrap();
    assert_eq!(r.headers()["cache-control"], "no-store");
    assert_eq!(r.headers()["referrer-policy"], "no-referrer");
    let status = r.status();
    (
        status,
        serde_json::from_slice(&to_bytes(r.into_body(), 3 * 1024 * 1024).await.unwrap()).unwrap(),
    )
}

#[tokio::test]
async fn rights_work_without_nickname_and_require_exact_owner_revision_and_recent_auth() {
    let f = Fixture::new(4);
    for replay in [false, true] {
        assert_eq!(f.call(replay, f.body(replay)).await.0, StatusCode::OK);
        for key in ["account_id", "session_revision"] {
            let mut body = f.body(replay);
            body[key] = Uuid::new_v4().to_string().into();
            assert_eq!(f.call(replay, body).await.0, StatusCode::UNAUTHORIZED);
        }
    }
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 8);
    f.clock.0.store(18299, Ordering::SeqCst);
    assert_eq!(f.call(false, f.body(false)).await.0, StatusCode::OK);
    f.clock.0.store(18300, Ordering::SeqCst);
    assert_eq!(
        f.call(false, f.body(false)).await,
        (StatusCode::CONFLICT, json!({"code":"reauth_required"}))
    );
}
#[tokio::test]
async fn closed_body_canonical_ids_csrf_origin_and_query_are_checked_before_storage() {
    for replay in [false, true] {
        let f = Fixture::new(4);
        for key in [
            "account_id",
            "session_revision",
            if replay { "completion_id" } else { "after" },
        ] {
            for invalid in [
                "bad",
                "00000000-0000-0000-0000-000000000000",
                "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            ] {
                let mut body = f.body(replay);
                body[key] = invalid.into();
                assert_eq!(f.call(replay, body).await.0, StatusCode::BAD_REQUEST);
            }
        }
        for (key, value) in [
            ("v", json!(2)),
            ("v", json!(1.0)),
            ("email", json!("forbidden")),
        ] {
            let mut body = f.body(replay);
            body[key] = value;
            assert_eq!(f.call(replay, body).await.0, StatusCode::BAD_REQUEST);
        }
        for key in [
            header::ORIGIN,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-liar-csrf"),
        ] {
            let mut request = f.request(replay, f.body(replay));
            request.headers_mut().remove(key);
            assert_ne!(response(f.app.clone(), request).await.0, StatusCode::OK);
        }
        let mut request = f.request(replay, f.body(replay));
        *request.uri_mut() = format!("{}?after=secret", request.uri()).parse().unwrap();
        assert_eq!(
            response(f.app.clone(), request).await.0,
            StatusCode::BAD_REQUEST
        );
        let mut request = f.request(replay, f.body(replay));
        *request.body_mut() = Body::from(" ".repeat(4097));
        assert_eq!(
            response(f.app.clone(), request).await.0,
            StatusCode::BAD_REQUEST
        );
        let mut request = f.request(replay, f.body(replay));
        *request.body_mut() = Body::from(f.body(replay).to_string().replacen(
            "\"v\":1",
            "\"v\":1,\"v\":1",
            1,
        ));
        assert_eq!(
            response(f.app.clone(), request).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(f.records.reads.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn post_read_proof_and_invalidation_prevent_stale_success() {
    for replay in [false, true] {
        for change in 0..6 {
            let f = Fixture::new(4);
            f.records.hold.store(true, Ordering::SeqCst);
            let request = f.request(replay, f.body(replay));
            let app = f.app.clone();
            let task = tokio::spawn(async move { response(app, request).await });
            f.records.entered.notified().await;
            match change {
                0 => *f.sessions.row.lock().unwrap() = None,
                1 => f.sessions.row.lock().unwrap().as_mut().unwrap().id = Uuid::new_v4(),
                2 => f.clock.0.store(18300, Ordering::SeqCst),
                3 => f.clock.0.store(17999, Ordering::SeqCst),
                4 => f.sessions.fail.store(true, Ordering::SeqCst),
                _ => {
                    let _barrier = f.authorities.account(f.account);
                }
            }
            f.records.released.notify_one();
            let (status, _) = task.await.unwrap();
            assert_ne!(status, StatusCode::OK, "change {change}");
        }
    }
}
#[tokio::test]
async fn bounded_requests_rate_timeout_and_port_failures_are_explicit() {
    for replay in [false, true] {
        let f = Fixture::new(1);
        f.records.hold.store(true, Ordering::SeqCst);
        let app = f.app.clone();
        let request = f.request(replay, f.body(replay));
        let task = tokio::spawn(async move { response(app, request).await });
        f.records.entered.notified().await;
        assert_eq!(
            f.call(replay, f.body(replay)).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap()
                .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        f.records.hold.store(false, Ordering::SeqCst);
        f.records.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            f.call(replay, f.body(replay)).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        f.records.fail.store(false, Ordering::SeqCst);
        f.records.wrong.store(true, Ordering::SeqCst);
        assert_eq!(
            f.call(replay, f.body(replay)).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        let f = Fixture::new(4);
        for _ in 0..20 {
            assert_eq!(f.call(replay, f.body(replay)).await.0, StatusCode::OK);
        }
        assert_eq!(
            f.call(replay, f.body(replay)).await.0,
            StatusCode::TOO_MANY_REQUESTS
        );
        f.clock.0.store(18001, Ordering::SeqCst);
        assert_eq!(f.call(replay, f.body(replay)).await.0, StatusCode::OK);
    }
    let f = Fixture::new(4);
    f.sessions.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        f.call(false, f.body(false)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    for key in ["LIAR_PRIVACY_REQUESTS", "LIAR_PRIVACY_AUTHORITIES"] {
        for invalid in ["0", "-1", "1.0", " ", "", "20001"] {
            assert!(load_privacy_config(|name| (name == key).then(|| invalid.into())).is_err());
        }
    }
    let config = load_privacy_config(|_| None).unwrap();
    assert_eq!((config.requests, config.authorities), (4, 64));
}
#[tokio::test]
async fn unconfigured_export_and_replay_are_closed_and_not_cacheable() {
    for path in ["/api/v1/me/games/export", "/api/v1/me/games/replay"] {
        let response = liar_server::app(None)
            .oneshot(Request::post(path).body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(
            to_bytes(response.into_body(), 4096).await.unwrap(),
            r#"{"code":"auth_unavailable"}"#
        );
    }
}
