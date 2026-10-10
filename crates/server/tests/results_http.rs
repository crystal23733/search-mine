use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use liar_protocol::{
    game::{Outcome, PublicEndReason},
    online::OnlineError,
    results::*,
};
use liar_server::{
    auth::*,
    online::{AuthorityRegistry, PortFuture, SessionReader},
    results::*,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
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
#[derive(Default)]
struct Gate {
    hold: AtomicBool,
    entered: Notify,
    released: Notify,
}
impl Gate {
    async fn wait(&self) {
        if self.hold.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.released.notified().await;
        }
    }
    fn release(&self) {
        self.hold.store(false, Ordering::SeqCst);
        self.released.notify_waiters();
    }
}
#[derive(Default)]
struct Sessions {
    rows: Mutex<HashMap<[u8; 32], Session>>,
    reads: AtomicUsize,
    gate: Gate,
    fail: AtomicBool,
    delay: Mutex<Duration>,
}
impl SessionReader for Sessions {
    fn read(&self, hash: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let value = self.rows.lock().unwrap().get(&hash).cloned();
            let delay = *self.delay.lock().unwrap();
            tokio::time::sleep(delay).await;
            self.gate.wait().await;
            if self.fail.load(Ordering::SeqCst) {
                Err(OnlineError::Unavailable)
            } else {
                Ok(value)
            }
        })
    }
}
#[derive(Default)]
struct Records {
    rows: Mutex<HashMap<(Uuid, Uuid), StoredPersonalResult>>,
    reads: AtomicUsize,
    gate: Gate,
    fail: AtomicBool,
    delay: Mutex<Duration>,
}
impl ResultReader for Records {
    fn read(
        &self,
        account: Uuid,
        id: Uuid,
    ) -> PortFuture<'_, Result<Option<StoredPersonalResult>, ResultError>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let value = self.rows.lock().unwrap().get(&(account, id)).cloned();
            let delay = *self.delay.lock().unwrap();
            tokio::time::sleep(delay).await;
            self.gate.wait().await;
            if self.fail.load(Ordering::SeqCst) {
                Err(ResultError::Unavailable)
            } else {
                Ok(value)
            }
        })
    }
}
struct User {
    account: Uuid,
    browser: SecretToken,
    token: SecretToken,
}
struct Fixture {
    app: Router,
    clock: Arc<Clock>,
    sessions: Arc<Sessions>,
    records: Arc<Records>,
    authorities: Arc<AuthorityRegistry>,
    security: Arc<BrowserSecurity>,
}
impl Fixture {
    fn new(requests: usize, authorities: usize) -> Self {
        let clock = Arc::new(Clock(AtomicI64::new(18000)));
        let sessions = Arc::new(Sessions::default());
        let records = Arc::new(Records::default());
        let authorities = AuthorityRegistry::new(authorities).unwrap();
        let security = Arc::new(BrowserSecurity::new("https://game.example", [5; 32]).unwrap());
        let app = result_router(
            records.clone(),
            sessions.clone(),
            security.clone(),
            ResultAuthentication {
                clock: clock.clone(),
                authorities: authorities.clone(),
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
        }
    }
    fn user(&self, account: Option<Uuid>) -> User {
        let user = User {
            account: account.unwrap_or_else(Uuid::new_v4),
            browser: SecretToken::generate().unwrap(),
            token: SecretToken::generate().unwrap(),
        };
        self.sessions.rows.lock().unwrap().insert(
            user.token.hash(),
            Session {
                id: Uuid::new_v4(),
                account: Account {
                    id: user.account,
                    nickname: Some(Nickname::parse("Player探偵").unwrap()),
                },
                created_at: 17000,
                authenticated_at: 17000,
                expires_at: 20000,
            },
        );
        user
    }
    fn save(&self, user: &User) -> Uuid {
        let id = Uuid::new_v4();
        self.records.rows.lock().unwrap().insert(
            (user.account, id),
            StoredPersonalResult {
                account: user.account,
                match_id: id,
                rules_hash: "a".repeat(64),
                end_elapsed_ms: Some(123),
                reason: PublicEndReason::Forfeit,
                outcome: Outcome::Loss,
                own: Some(ResultStats {
                    opened_safe: 17,
                    mistakes: 2,
                    accusation_attempts: 3,
                    correct_accusations: 1,
                }),
            },
        );
        id
    }
    fn request(&self, user: &User, id: Uuid) -> Request<Body> {
        Request::post("/api/v1/results")
            .header(header::ORIGIN, "https://game.example")
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::COOKIE,
                format!(
                    "{BROWSER_COOKIE}={}; {SESSION_COOKIE}={}",
                    user.browser.expose().as_str(),
                    user.token.expose().as_str()
                ),
            )
            .header(
                "x-liar-csrf",
                self.security
                    .csrf(&user.browser, Some(&user.token), self.clock.now())
                    .unwrap(),
            )
            .body(Body::from(
                json!({"v":1,"match_id":id.to_string()}).to_string(),
            ))
            .unwrap()
    }
    async fn call(&self, user: &User, id: Uuid) -> (StatusCode, Value) {
        response(self.app.clone(), self.request(user, id)).await
    }
}

async fn response(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn missing_external_configuration_fails_closed_without_cache() {
    let request = Request::post("/api/v1/results")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        response(liar_server::app(None), request).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"unavailable"})
        )
    );
}

#[tokio::test]
async fn own_minimal_result_is_actor_independent_and_absence_is_uniform() {
    let f = Fixture::new(2, 2);
    let a = f.user(None);
    let b = f.user(None);
    let id = f.save(&a);
    assert_eq!(
        f.call(&a, id).await,
        (
            StatusCode::OK,
            json!({"v":1,"result":{
                "match_id":id.to_string(),"rules_hash":"a".repeat(64),"end_elapsed_ms":123,
                "result":{"reason":"forfeit","outcome":"loss","completed":true},
                "own":{"opened_safe":17,"mistakes":2,"accusation_attempts":3,"correct_accusations":1}
            }})
        )
    );
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 2);
    for (user, id) in [(&b, id), (&a, Uuid::new_v4())] {
        assert_eq!(
            f.call(user, id).await,
            (StatusCode::NOT_FOUND, json!({"error":"not_found"}))
        );
    }
}

#[tokio::test]
async fn unknown_abort_is_self_only_and_partial_details_fail_closed() {
    let f = Fixture::new(2, 2);
    let a = f.user(None);
    let b = f.user(None);
    let id = f.save(&a);
    {
        let mut rows = f.records.rows.lock().unwrap();
        let r = rows.get_mut(&(a.account, id)).unwrap();
        r.reason = PublicEndReason::ServerFailure;
        r.outcome = Outcome::Abort;
        r.end_elapsed_ms = None;
        r.own = None;
    }
    assert_eq!(
        f.call(&a, id).await,
        (
            StatusCode::OK,
            json!({"v":1,"result":{
                "match_id":id.to_string(),"rules_hash":"a".repeat(64),"end_elapsed_ms":null,"own":null,
                "result":{"reason":"server_failure","outcome":"abort","completed":false}
            }})
        )
    );
    assert_eq!(
        f.call(&b, id).await,
        (StatusCode::NOT_FOUND, json!({"error":"not_found"}))
    );
    f.records
        .rows
        .lock()
        .unwrap()
        .get_mut(&(a.account, id))
        .unwrap()
        .end_elapsed_ms = Some(0);
    assert_eq!(
        f.call(&a, id).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"unavailable"})
        )
    );
}

#[tokio::test]
async fn request_boundaries_reject_without_reading_result_storage() {
    let f = Fixture::new(2, 2);
    let user = f.user(None);
    let id = f.save(&user);
    for case in 0..12 {
        let mut req = f.request(&user, id);
        let expected = match case {
            0 => {
                req.headers_mut().remove(header::ORIGIN);
                StatusCode::FORBIDDEN
            }
            1 => {
                req.headers_mut()
                    .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
                StatusCode::FORBIDDEN
            }
            2 => {
                req.headers_mut().remove("x-liar-csrf");
                StatusCode::FORBIDDEN
            }
            3 => {
                req.headers_mut().remove(header::COOKIE);
                StatusCode::FORBIDDEN
            }
            4 => {
                *req.uri_mut() = "/api/v1/results?match_id=x".parse().unwrap();
                StatusCode::BAD_REQUEST
            }
            5 => {
                req.headers_mut()
                    .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
                StatusCode::BAD_REQUEST
            }
            6 => {
                req.headers_mut()
                    .append(header::CONTENT_TYPE, "application/json".parse().unwrap());
                StatusCode::BAD_REQUEST
            }
            7 => {
                *req.body_mut() = Body::from(" ".repeat(257));
                StatusCode::PAYLOAD_TOO_LARGE
            }
            8 => {
                *req.body_mut() = Body::from(json!({"v":2,"match_id":id.to_string()}).to_string());
                StatusCode::BAD_REQUEST
            }
            9 => {
                *req.body_mut() = Body::from(
                    json!({"v":1,"match_id":id.to_string(),"account_id":user.account.to_string()})
                        .to_string(),
                );
                StatusCode::BAD_REQUEST
            }
            10 => {
                *req.body_mut() =
                    Body::from(json!({"v":1,"match_id":Uuid::nil().to_string()}).to_string());
                StatusCode::BAD_REQUEST
            }
            _ => {
                req.headers_mut()
                    .insert("x-liar-csrf", "invalid".parse().unwrap());
                StatusCode::FORBIDDEN
            }
        };
        assert_eq!(
            response(f.app.clone(), req).await.0,
            expected,
            "case {case}"
        );
    }
    assert_eq!(f.records.reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_missing_incomplete_expired_or_corrupt_inputs_never_emit_result() {
    for case in 0..7 {
        let f = Fixture::new(1, 1);
        let user = f.user(None);
        let id = f.save(&user);
        let expected = match case {
            0 => {
                f.sessions.fail.store(true, Ordering::SeqCst);
                StatusCode::SERVICE_UNAVAILABLE
            }
            1 => {
                f.records.fail.store(true, Ordering::SeqCst);
                StatusCode::SERVICE_UNAVAILABLE
            }
            2 => {
                f.sessions.rows.lock().unwrap().clear();
                StatusCode::UNAUTHORIZED
            }
            3 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .account
                    .nickname = None;
                StatusCode::UNAUTHORIZED
            }
            4 => {
                f.clock.0.store(20000, Ordering::SeqCst);
                StatusCode::UNAUTHORIZED
            }
            5 => {
                f.records
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&(user.account, id))
                    .unwrap()
                    .account = Uuid::new_v4();
                StatusCode::SERVICE_UNAVAILABLE
            }
            _ => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .authenticated_at = 18001;
                StatusCode::UNAUTHORIZED
            }
        };
        let (status, body) = f.call(&user, id).await;
        assert_eq!(status, expected, "case {case}");
        assert!(body.get("result").is_none());
    }
}

#[tokio::test]
async fn revocation_during_initial_session_read_rejects_stale_storage_reply() {
    let f = Fixture::new(1, 1);
    let user = f.user(None);
    let id = f.save(&user);
    f.sessions.gate.hold.store(true, Ordering::SeqCst);
    let task = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.sessions.gate.entered.notified().await;
    let barrier = f.authorities.session(user.token.hash());
    drop(barrier);
    f.sessions.gate.release();
    assert_eq!(task.await.unwrap().0, StatusCode::UNAUTHORIZED);
    assert_eq!(f.records.reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn post_storage_session_proof_and_revocation_close_every_response() {
    for case in 0..10 {
        let f = Fixture::new(2, 2);
        let user = f.user(None);
        let id = f.save(&user);
        f.records.gate.hold.store(true, Ordering::SeqCst);
        let task = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
        f.records.gate.entered.notified().await;
        let mut barrier = None;
        match case {
            0 => {
                barrier = Some(f.authorities.session(user.token.hash()));
            }
            1 => {
                barrier = Some(f.authorities.account(user.account));
            }
            2 => {
                f.sessions.rows.lock().unwrap().clear();
            }
            3 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .id = Uuid::new_v4();
            }
            4 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .account
                    .id = Uuid::new_v4();
            }
            5 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .expires_at += 1;
            }
            6 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .created_at -= 1;
            }
            7 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .authenticated_at += 1;
            }
            8 => {
                f.sessions
                    .rows
                    .lock()
                    .unwrap()
                    .get_mut(&user.token.hash())
                    .unwrap()
                    .account
                    .nickname = None;
            }
            _ => {
                f.clock.0.store(20000, Ordering::SeqCst);
            }
        }
        f.records.gate.release();
        assert_eq!(
            task.await.unwrap().0,
            StatusCode::UNAUTHORIZED,
            "case {case}"
        );
        drop(barrier);
    }
}

#[tokio::test]
async fn capacity_and_cancellation_release_the_request_and_subject_lease() {
    let f = Fixture::new(1, 1);
    let user = f.user(None);
    let other = f.user(None);
    let id = f.save(&user);
    f.records.gate.hold.store(true, Ordering::SeqCst);
    let task = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    assert_eq!(f.call(&other, id).await.0, StatusCode::SERVICE_UNAVAILABLE);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    f.records.gate.release();
    assert_eq!(f.call(&other, id).await.0, StatusCode::NOT_FOUND);
    assert_eq!(f.call(&user, id).await.0, StatusCode::OK);
}

#[tokio::test(start_paused = true)]
async fn all_three_io_phases_share_one_deadline_and_timeout_frees_capacity() {
    let f = Fixture::new(1, 1);
    let user = f.user(None);
    let id = f.save(&user);
    *f.sessions.delay.lock().unwrap() = Duration::from_millis(700);
    *f.records.delay.lock().unwrap() = Duration::from_millis(700);
    let start = tokio::time::Instant::now();
    assert_eq!(f.call(&user, id).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(start.elapsed(), Duration::from_secs(2));
    *f.sessions.delay.lock().unwrap() = Duration::ZERO;
    *f.records.delay.lock().unwrap() = Duration::ZERO;
    assert_eq!(f.call(&user, id).await.0, StatusCode::OK);
}

#[tokio::test]
async fn session_and_account_rate_limits_survive_token_rotation_and_reset() {
    let f = Fixture::new(1, 1);
    let user = f.user(None);
    let id = f.save(&user);
    for _ in 0..20 {
        assert_eq!(f.call(&user, id).await.0, StatusCode::OK);
    }
    assert_eq!(f.call(&user, id).await.0, StatusCode::TOO_MANY_REQUESTS);
    let rotated = f.user(Some(user.account));
    assert_eq!(f.call(&rotated, id).await.0, StatusCode::TOO_MANY_REQUESTS);
    f.clock.0.store(18001, Ordering::SeqCst);
    assert_eq!(f.call(&rotated, id).await.0, StatusCode::OK);
}

#[tokio::test]
async fn a_later_verified_session_replaces_only_result_ownership_and_rejects_the_old_reply() {
    let f = Fixture::new(2, 1);
    let user = f.user(None);
    let next = f.user(Some(user.account));
    let id = f.save(&user);
    let sockets = AuthorityRegistry::new(1).unwrap();
    let socket = sockets
        .bind(0, user.account, user.token.hash(), 20000, 18000)
        .unwrap();
    f.records.gate.hold.store(true, Ordering::SeqCst);
    let old = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    let new = tokio::spawn(response(f.app.clone(), f.request(&next, id)));
    f.records.gate.entered.notified().await;
    assert_eq!(old.await.unwrap().0, StatusCode::UNAUTHORIZED);
    sockets.with_authority(&socket, 18000, || ()).unwrap();
    f.records.gate.release();
    assert_eq!(new.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn cancellation_of_one_shared_session_read_does_not_cancel_the_other() {
    let f = Fixture::new(2, 1);
    let user = f.user(None);
    let id = f.save(&user);
    f.records.gate.hold.store(true, Ordering::SeqCst);
    let a = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    let b = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    a.abort();
    assert!(a.await.unwrap_err().is_cancelled());
    f.records.gate.release();
    assert_eq!(b.await.unwrap().0, StatusCode::OK);
}

#[tokio::test]
async fn authority_saturation_and_post_read_failure_or_clock_rollback_fail_closed() {
    let f = Fixture::new(2, 1);
    let user = f.user(None);
    let other = f.user(None);
    let id = f.save(&user);
    f.records.gate.hold.store(true, Ordering::SeqCst);
    let held = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    assert_eq!(f.call(&other, id).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(f.records.reads.load(Ordering::SeqCst), 1);
    f.sessions.fail.store(true, Ordering::SeqCst);
    f.records.gate.release();
    assert_eq!(held.await.unwrap().0, StatusCode::SERVICE_UNAVAILABLE);
    f.sessions.fail.store(false, Ordering::SeqCst);
    f.records.gate.hold.store(true, Ordering::SeqCst);
    let held = tokio::spawn(response(f.app.clone(), f.request(&user, id)));
    f.records.gate.entered.notified().await;
    f.clock.0.store(17999, Ordering::SeqCst);
    f.records.gate.release();
    assert_eq!(held.await.unwrap().0, StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn public_router_rejects_invalid_concurrency_configuration() {
    let f = Fixture::new(1, 1);
    for n in [0, 257, usize::MAX] {
        assert!(
            result_router(
                f.records.clone(),
                f.sessions.clone(),
                f.security.clone(),
                ResultAuthentication {
                    clock: f.clock.clone(),
                    authorities: f.authorities.clone()
                },
                n
            )
            .is_err()
        );
    }
}
