use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use axum::{Router, body::to_bytes, http::header};
use liar_core::rules::RulesSnapshot;
use liar_protocol::online::OnlineError;
use liar_server::{
    auth::*,
    daily::*,
    online::{AuthorityRegistry, PortFuture, SessionReader},
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
use uuid::Uuid;
const NOW: i64 = 1_791_331_260;
struct Clock(AtomicI64);
impl liar_server::online::MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        ((self.0.load(Ordering::SeqCst) - NOW).max(0) as u64) * 1000
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Sessions {
    row: Mutex<Option<Session>>,
    reads: AtomicUsize,
    fail: AtomicBool,
    post: Mutex<Option<(Arc<Clock>, i64)>>,
}
impl SessionReader for Sessions {
    fn read(&self, _: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            if self.reads.fetch_add(1, Ordering::SeqCst) > 0
                && let Some((clock, now)) = self.post.lock().unwrap().as_ref()
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
struct Records {
    accepted: Mutex<Option<PublishedDaily>>,
    saved: Mutex<Option<DailyReceipt>>,
    error: Mutex<Option<DailyStoreError>>,
    calls: AtomicUsize,
    writes: AtomicUsize,
    wrong: AtomicUsize,
    hold: AtomicBool,
    entered: Notify,
    released: Notify,
    after: Mutex<Option<(Arc<Clock>, i64)>>,
}
impl Records {
    fn new() -> Self {
        Self {
            accepted: Mutex::new(None),
            saved: Mutex::new(None),
            error: Mutex::new(None),
            calls: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            wrong: AtomicUsize::new(0),
            hold: AtomicBool::new(false),
            entered: Notify::new(),
            released: Notify::new(),
            after: Mutex::new(None),
        }
    }
    async fn check(&self) -> Result<(), DailyStoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hold.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.released.notified().await;
        }
        if let Some((clock, now)) = self.after.lock().unwrap().as_ref() {
            clock.0.store(*now, Ordering::SeqCst);
        }
        match *self.error.lock().unwrap() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
impl DailyRepository for Records {
    fn publish(
        &self,
        candidate: PublishedDaily,
    ) -> DailyFuture<'_, Result<PublishedDaily, DailyStoreError>> {
        Box::pin(async move {
            self.check().await?;
            let mut accepted = self.accepted.lock().unwrap();
            Ok(accepted.get_or_insert(candidate).clone())
        })
    }
    fn definition<'a>(
        &'a self,
        _: &'a str,
        _: u16,
    ) -> DailyFuture<'a, Result<Option<PublishedDaily>, DailyStoreError>> {
        Box::pin(async move {
            self.check().await?;
            Ok(self.accepted.lock().unwrap().clone())
        })
    }
    fn submit<'a>(
        &'a self,
        _: DailySession,
        attempt: Uuid,
        evidence: &'a ReplayEvidence,
    ) -> DailyFuture<'a, Result<DailySubmission, DailyStoreError>> {
        Box::pin(async move {
            self.check().await?;
            let mut saved = self.saved.lock().unwrap();
            let row = saved
                .get_or_insert_with(|| {
                    self.writes.fetch_add(1, Ordering::SeqCst);
                    DailyReceipt {
                        id: Uuid::new_v4(),
                        attempt,
                        metadata: evidence.metadata().clone(),
                        opened_safe: evidence.opened_safe() as u32,
                        mistakes: evidence.mistakes(),
                        replayed_elapsed_ms: evidence.replayed_elapsed_ms(),
                        verified_elapsed_ms: None,
                        completed_at: NOW,
                    }
                })
                .clone();
            let mut row = row;
            let wrong = self.wrong.load(Ordering::SeqCst);
            match wrong {
                1 => row.attempt = Uuid::new_v4(),
                2 => row.opened_safe -= 1,
                3 => row.mistakes += 1,
                4 => row.replayed_elapsed_ms += 1,
                5 => row.metadata.seed = "9".into(),
                6 => row.id = Uuid::nil(),
                7 => row.completed_at = NOW + 100,
                8 => return Ok(DailySubmission::Practice(row)),
                _ => {}
            }
            Ok(if row.attempt == attempt || wrong > 0 {
                DailySubmission::Official(row)
            } else {
                DailySubmission::Practice(row)
            })
        })
    }
    fn own<'a>(
        &'a self,
        _: DailySession,
        _: &'a liar_core::daily::DailyMetadata,
    ) -> DailyFuture<'a, Result<Option<DailyReceipt>, DailyStoreError>> {
        Box::pin(async move {
            self.check().await?;
            Ok(self.saved.lock().unwrap().clone())
        })
    }
}
struct Fixture {
    app: Router,
    records: Arc<Records>,
    sessions: Arc<Sessions>,
    clock: Arc<Clock>,
    authorities: Arc<AuthorityRegistry>,
    security: Arc<BrowserSecurity>,
    browser: SecretToken,
    token: SecretToken,
    account: Uuid,
    revision: Uuid,
}
fn replay() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    fixture["daily"]["replay"].clone()
}
impl Fixture {
    fn new(requests: usize) -> Self {
        Self::with_verifier(requests, Arc::new(NativeReplayVerifier))
    }
    fn with_verifier(requests: usize, verifier: Arc<dyn ReplayVerifier>) -> Self {
        let account = Uuid::new_v4();
        let revision = Uuid::new_v4();
        let clock = Arc::new(Clock(AtomicI64::new(NOW)));
        let sessions = Arc::new(Sessions {
            row: Mutex::new(Some(Session {
                id: revision,
                account: Account {
                    id: account,
                    nickname: Some(Nickname::parse("Daily Player").unwrap()),
                },
                created_at: NOW - 1000,
                authenticated_at: NOW - 1000,
                expires_at: NOW + 2000,
            })),
            reads: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            post: Mutex::new(None),
        });
        let records = Arc::new(Records::new());
        let authorities = AuthorityRegistry::new(64).unwrap();
        let security = Arc::new(BrowserSecurity::new("https://game.example", [5; 32]).unwrap());
        let app = daily_router(
            records.clone(),
            BoundedReplayVerifier::new(verifier),
            sessions.clone(),
            security.clone(),
            DailyAuthentication::new(
                Arc::new(DailyTicketRegistry::new(64, authorities.clone(), clock.clone()).unwrap()),
                clock.clone(),
            ),
            requests,
        )
        .unwrap();
        Self {
            app,
            records,
            sessions,
            clock,
            authorities,
            security,
            browser: SecretToken::generate().unwrap(),
            token: SecretToken::generate().unwrap(),
            account,
            revision,
        }
    }
    fn body(&self, operation: &str) -> Value {
        let mut value = json!({"v":1,"account_id":self.account.to_string(),"session_revision":self.revision.to_string()});
        match operation {
            "definition" => {
                value["date"] = json!("2026-10-07");
                value["seed_version"] = json!(1);
            }
            "submit" => {
                value["attempt_id"] = json!(Uuid::new_v4().to_string());
                value["replay"] = replay();
            }
            _ => value["metadata"] = replay()["metadata"].clone(),
        };
        value
    }
    fn request(&self, operation: &str, body: Value) -> Request<Body> {
        Request::post(format!("/api/v1/daily/{operation}"))
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
    async fn call(&self, operation: &str, body: Value) -> (StatusCode, Value) {
        response(self.app.clone(), self.request(operation, body)).await
    }
}
struct CachedEvidence(ReplayEvidence);
impl ReplayVerifier for CachedEvidence {
    fn verify(&self, _: &PublishedDaily, _: &str) -> Result<ReplayEvidence, ReplayError> {
        Ok(self.0.clone())
    }
}
#[tokio::test]
async fn a_verifier_result_for_another_payload_or_definition_is_rejected_before_any_write() {
    let published = PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap();
    let cached = NativeReplayVerifier
        .verify(&published, &replay().to_string())
        .unwrap();
    for other_definition in [false, true] {
        let f = Fixture::with_verifier(4, Arc::new(CachedEvidence(cached.clone())));
        let mut body = f.body("submit");
        if other_definition {
            body["replay"]["metadata"] = serde_json::to_value(
                PublishedDaily::new("2026-10-08", 1, RulesSnapshot::bundled())
                    .unwrap()
                    .metadata(),
            )
            .unwrap();
        } else {
            body["replay"]["final_time_ms"] = json!(u32::MAX);
        }
        assert_eq!(
            f.call("submit", body).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(f.records.writes.load(Ordering::SeqCst), 0);
    }
}
async fn response(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let result = app.oneshot(request).await.unwrap();
    assert_eq!(result.headers()["cache-control"], "no-store");
    assert_eq!(result.headers()["referrer-policy"], "no-referrer");
    assert_eq!(result.headers()["x-content-type-options"], "nosniff");
    (
        result.status(),
        serde_json::from_slice(&to_bytes(result.into_body(), 65536).await.unwrap()).unwrap(),
    )
}
#[tokio::test]
async fn authenticated_completion_retry_practice_and_own_preserve_original_receipt_and_trust() {
    let f = Fixture::new(4);
    let (status, definition) = f.call("definition", f.body("definition")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(definition["metadata"], replay()["metadata"]);
    let (status, empty) = f.call("own", f.body("own")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(empty["receipt"].is_null());
    let body = f.body("submit");
    let (status, first) = f.call("submit", body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["trust"], "verified_completion");
    assert_eq!(first["disposition"], "official");
    assert_eq!(first["receipt"]["replayed_elapsed_ms"], 3200);
    assert_eq!(first["attempt_id"], first["receipt"]["attempt_id"]);
    assert_eq!(
        f.call("submit", body).await,
        (StatusCode::OK, first.clone())
    );
    let (status, practice) = f.call("submit", f.body("submit")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(practice["disposition"], "practice");
    assert_eq!(practice["receipt"], first["receipt"]);
    assert_ne!(practice["attempt_id"], practice["receipt"]["attempt_id"]);
    let (status, own) = f.call("own", f.body("own")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(own["receipt"], first["receipt"]);
    assert_eq!(f.records.writes.load(Ordering::SeqCst), 1);
    let text = first.to_string();
    for secret in [
        "nickname",
        "email",
        "subject",
        "token",
        "official_elapsed_ms",
        "verified_time",
        "inputs",
    ] {
        assert!(!text.contains(secret));
    }
}

#[tokio::test]
async fn stored_time_controls_retry_practice_and_own_trust_and_corruption_never_downgrades() {
    let f = Fixture::new(4);
    let body = f.body("submit");
    assert_eq!(f.call("submit", body.clone()).await.0, StatusCode::OK);
    f.records
        .saved
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .verified_elapsed_ms = Some(9000);
    for (operation, input) in [
        ("submit", body),
        ("submit", f.body("submit")),
        ("own", f.body("own")),
    ] {
        let (status, value) = f.call(operation, input).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["trust"], "verified_time");
        assert_eq!(value["receipt"]["verified_elapsed_ms"], 9000);
    }
    f.records
        .saved
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .verified_elapsed_ms = Some(3199);
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn strict_headers_closed_body_identity_metadata_and_incomplete_replay_never_write() {
    let f = Fixture::new(4);
    for field in ["account_id", "session_revision"] {
        let mut body = f.body("submit");
        body[field] = json!(Uuid::new_v4().to_string());
        assert_eq!(f.call("submit", body).await.0, StatusCode::UNAUTHORIZED);
    }
    for i in 0..7 {
        let mut request = f.request("submit", f.body("submit"));
        match i {
            0 => {
                request.headers_mut().remove(header::ORIGIN);
            }
            1 => {
                request.headers_mut().remove("x-liar-csrf");
            }
            2 => {
                request.headers_mut().remove(header::COOKIE);
            }
            3 => {
                request
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
            }
            4 => {
                request
                    .headers_mut()
                    .append(header::CONTENT_TYPE, "application/json".parse().unwrap());
            }
            5 => *request.uri_mut() = "/api/v1/daily/submit?account_id=secret".parse().unwrap(),
            _ => *request.body_mut() = Body::from(vec![b' '; 2 * 1024 * 1024 + 4097]),
        };
        assert_ne!(response(f.app.clone(), request).await.0, StatusCode::OK);
    }
    for i in 0..5 {
        let mut body = f.body("submit");
        match i {
            0 => body["unknown"] = json!(1),
            1 => body["replay"]["inputs"] = json!([]),
            2 => body["replay"]["metadata"]["seed"] = json!("9"),
            3 => body["replay"]["inputs"][0]["input"]["unknown"] = json!(1),
            _ => body["attempt_id"] = json!(Uuid::nil().to_string()),
        };
        assert_eq!(f.call("submit", body).await.0, StatusCode::BAD_REQUEST);
    }
    f.sessions
        .row
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .account
        .nickname = None;
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.records.writes.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn accepted_definition_wins_and_bad_receipts_or_store_errors_are_never_empty_success() {
    let f = Fixture::new(4);
    let mut rules = RulesSnapshot::bundled().rules;
    rules.duration_ms += 1;
    let accepted =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    *f.records.accepted.lock().unwrap() = Some(accepted.clone());
    assert_eq!(
        f.call("definition", f.body("definition")).await.1["metadata"],
        serde_json::to_value(accepted.metadata()).unwrap()
    );
    assert_eq!(
        f.call("submit", f.body("submit")).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut body = f.body("own");
    body["metadata"]["seed"] = json!("9");
    assert_eq!(f.call("own", body).await.0, StatusCode::BAD_REQUEST);
    for (error, status) in [
        (DailyStoreError::Invalid, StatusCode::BAD_REQUEST),
        (DailyStoreError::Conflict, StatusCode::CONFLICT),
        (DailyStoreError::Expired, StatusCode::GONE),
        (DailyStoreError::Unauthorized, StatusCode::UNAUTHORIZED),
        (
            DailyStoreError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let f = Fixture::new(4);
        *f.records.error.lock().unwrap() = Some(error);
        assert_eq!(f.call("own", f.body("own")).await.0, status);
    }
    for wrong in 1..=8 {
        let f = Fixture::new(4);
        f.records.wrong.store(wrong, Ordering::SeqCst);
        assert_eq!(
            f.call("submit", f.body("submit")).await.0,
            StatusCode::SERVICE_UNAVAILABLE,
            "wrong mode {wrong}"
        );
    }
    let f = Fixture::new(4);
    *f.records.accepted.lock().unwrap() =
        Some(PublishedDaily::new("2026-10-08", 1, RulesSnapshot::bundled()).unwrap());
    assert_eq!(
        f.call("definition", f.body("definition")).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn post_proof_rotation_revocation_failure_and_clock_rollback_cannot_commit_response() {
    for mode in 0..7 {
        let f = Fixture::new(4);
        f.records.hold.store(true, Ordering::SeqCst);
        let app = f.app.clone();
        let request = f.request("own", f.body("own"));
        let pending = tokio::spawn(response(app, request));
        f.records.entered.notified().await;
        match mode {
            0 => f.sessions.row.lock().unwrap().as_mut().unwrap().id = Uuid::new_v4(),
            1 => *f.sessions.row.lock().unwrap() = None,
            2 => {
                let _barrier = f.authorities.account(f.account);
            }
            3 => f.sessions.fail.store(true, Ordering::SeqCst),
            4 => {
                f.clock.0.store(NOW + 2, Ordering::SeqCst);
                *f.sessions.post.lock().unwrap() = Some((f.clock.clone(), NOW + 1));
            }
            5 => f.clock.0.store(NOW - 1, Ordering::SeqCst),
            _ => f.sessions.row.lock().unwrap().as_mut().unwrap().expires_at += 1,
        };
        f.records.hold.store(false, Ordering::SeqCst);
        f.records.released.notify_one();
        let status = tokio::time::timeout(Duration::from_secs(1), pending)
            .await
            .unwrap()
            .unwrap()
            .0;
        assert_ne!(status, StatusCode::OK, "mode {mode}");
    }
    let f = Fixture::new(4);
    f.sessions.row.lock().unwrap().as_mut().unwrap().expires_at = NOW;
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::UNAUTHORIZED
    );
    let f = Fixture::new(4);
    f.sessions.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn bounded_requests_timeout_and_twenty_per_second_rate_do_not_wait_unboundedly() {
    let f = Fixture::new(1);
    f.records.hold.store(true, Ordering::SeqCst);
    let pending = tokio::spawn(response(f.app.clone(), f.request("own", f.body("own"))));
    f.records.entered.notified().await;
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap()
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let f = Fixture::new(4);
    for _ in 0..20 {
        assert_eq!(f.call("own", f.body("own")).await.0, StatusCode::OK);
    }
    assert_eq!(
        f.call("own", f.body("own")).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    f.clock.0.fetch_add(1, Ordering::SeqCst);
    assert_eq!(f.call("own", f.body("own")).await.0, StatusCode::OK);
    assert!(
        daily_router(
            f.records,
            BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier)),
            f.sessions,
            f.security,
            DailyAuthentication::new(
                Arc::new(DailyTicketRegistry::new(64, f.authorities, f.clock.clone()).unwrap()),
                f.clock
            ),
            0
        )
        .is_err()
    );
}

#[tokio::test]
async fn disabled_daily_routes_are_explicit_unavailable_and_never_cached() {
    for path in [
        "definition",
        "submit",
        "own",
        "start",
        "heartbeat",
        "submit-timed",
    ] {
        let response = liar_server::app(None)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/daily/{path}"))
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}

#[tokio::test]
async fn official_start_selects_server_utc_today_and_reuses_original_ticket() {
    let f = Fixture::new(4);
    let mut body = f.body("definition");
    body.as_object_mut().unwrap().remove("date");
    body.as_object_mut().unwrap().remove("seed_version");
    body["attempt_id"] = json!(Uuid::new_v4().to_string());
    let result = f
        .app
        .clone()
        .oneshot(f.request("start", body.clone()))
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::OK);
    let first: Value =
        serde_json::from_slice(&to_bytes(result.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(first["metadata"]["date"], "2026-10-07");
    assert_eq!(first["attempt_id"], body["attempt_id"]);
    assert_eq!(first["heartbeat_interval_ms"], 15000);
    f.clock.0.fetch_add(1, Ordering::SeqCst);
    assert_eq!(f.call("start", body).await.1, first);
}

#[tokio::test]
async fn timing_requests_are_closed_and_missing_ticket_is_explicit_null() {
    let f = Fixture::new(4);
    let mut start = json!({"v":1,"account_id":f.account.to_string(),"session_revision":f.revision.to_string(),"attempt_id":Uuid::new_v4().to_string()});
    for key in ["date", "seed", "rules", "verified_elapsed_ms"] {
        let mut invalid = start.clone();
        invalid[key] = json!(1);
        assert_eq!(f.call("start", invalid).await.0, StatusCode::BAD_REQUEST);
    }
    let (status, grant) = f.call("start", start.clone()).await;
    assert_eq!(status, StatusCode::OK);
    start.as_object_mut().unwrap().remove("attempt_id");
    start["ticket_id"] = grant["ticket_id"].clone();
    start["sequence"] = json!(0);
    assert_eq!(
        f.call("heartbeat", start.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    start["sequence"] = json!(2);
    assert_eq!(
        f.call("heartbeat", start.clone()).await.0,
        StatusCode::CONFLICT
    );
    start["sequence"] = json!(1);
    assert_eq!(f.call("heartbeat", start).await.0, StatusCode::OK);
    let mut timed = f.body("submit");
    assert_eq!(
        f.call("submit-timed", timed.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    timed["ticket_id"] = json!(Uuid::nil().to_string());
    assert_eq!(
        f.call("submit-timed", timed.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    timed["ticket_id"] = Value::Null;
    let (status, saved) = f.call("submit-timed", timed).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["trust"], "verified_completion");
    assert!(saved["receipt"]["verified_elapsed_ms"].is_null());
}

#[tokio::test]
async fn official_start_uses_accepted_snapshot_and_current_utc_date_not_client_local_date() {
    let f = Fixture::new(4);
    let mut rules = RulesSnapshot::bundled().rules;
    rules.countdown_ms = 5000;
    let accepted =
        PublishedDaily::new("2026-10-07", 1, RulesSnapshot::from_rules(rules).unwrap()).unwrap();
    *f.records.accepted.lock().unwrap() = Some(accepted.clone());
    let mut input = json!({"v":1,"account_id":f.account.to_string(),"session_revision":f.revision.to_string(),"attempt_id":Uuid::new_v4().to_string()});
    let (status, first) = f.call("start", input.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        first["rules"],
        serde_json::to_value(accepted.rules()).unwrap()
    );
    assert_eq!(
        first["ttl_ms"],
        u64::from(accepted.rules().rules.countdown_ms)
            + u64::from(accepted.rules().rules.duration_ms)
    );
    f.clock.0.fetch_add(86400, Ordering::SeqCst);
    f.sessions.row.lock().unwrap().as_mut().unwrap().expires_at += 86400;
    *f.records.accepted.lock().unwrap() = None;
    input["attempt_id"] = json!(Uuid::new_v4().to_string());
    let (status, next) = f.call("start", input).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(next["metadata"]["date"], "2026-10-08");
}

#[tokio::test]
async fn utc_midnight_during_snapshot_io_requires_a_fresh_start_without_creating_a_ticket() {
    let f = Fixture::new(4);
    let midnight = chrono::NaiveDate::from_ymd_opt(2026, 10, 8)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    f.clock.0.store(midnight - 1, Ordering::SeqCst);
    f.sessions.row.lock().unwrap().as_mut().unwrap().expires_at = midnight + 1000;
    *f.records.after.lock().unwrap() = Some((f.clock.clone(), midnight));
    let input = json!({"v":1,"account_id":f.account.to_string(),"session_revision":f.revision.to_string(),"attempt_id":Uuid::new_v4().to_string()});
    assert_eq!(f.call("start", input.clone()).await.0, StatusCode::CONFLICT);
    *f.records.after.lock().unwrap() = None;
    *f.records.accepted.lock().unwrap() = None;
    assert_eq!(f.call("start", input).await.0, StatusCode::OK);
    assert_eq!(f.records.writes.load(Ordering::SeqCst), 0);
}
