use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use futures_util::{SinkExt, StreamExt};
use liar_core::{
    board::{Cell, CellId},
    generator::{BoardGenerator, GenerationBudget},
    rules::RulesSnapshot,
};
use liar_server::{auth::*, lobby::*, online::*};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

struct Clock {
    game: AtomicU64,
    auth: AtomicI64,
}
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.game.load(Ordering::SeqCst)
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.auth.load(Ordering::SeqCst)
    }
}
struct Sessions {
    rows: Mutex<HashMap<[u8; 32], Session>>,
    reads: AtomicUsize,
    hold: AtomicBool,
    entered: Notify,
    released: Notify,
}
impl SessionReader for Sessions {
    fn read(
        &self,
        hash: [u8; 32],
    ) -> PortFuture<'_, Result<Option<Session>, liar_protocol::online::OnlineError>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let session = self.rows.lock().unwrap().get(&hash).cloned();
            if self.hold.load(Ordering::SeqCst) {
                self.entered.notify_one();
                self.released.notified().await;
            }
            Ok(session)
        })
    }
}
struct Seed(u64);
impl SeedSource for Seed {
    fn seed(&self) -> Result<[u8; 8], liar_protocol::online::OnlineError> {
        Ok(self.0.to_le_bytes())
    }
}
struct Supply {
    pool: BoardPool,
    hold: AtomicBool,
    released: Notify,
}
impl BoardSource for Supply {
    fn take(
        &self,
        wait: Duration,
    ) -> PortFuture<'_, Result<PreparedBoard, liar_protocol::online::OnlineError>> {
        Box::pin(async move {
            if self.hold.load(Ordering::SeqCst) {
                self.released.notified().await;
            }
            self.pool.take(wait).await
        })
    }
}
struct Results;
impl ResultRepository for Results {
    fn save(
        &self,
        _: FinishedMatch,
    ) -> PortFuture<'_, Result<SaveResult, liar_protocol::online::OnlineError>> {
        Box::pin(async { Ok(SaveResult::Saved) })
    }
}
struct User {
    account: Uuid,
    browser: SecretToken,
    token: SecretToken,
}
struct Fixture {
    app: Router,
    sessions: Arc<Sessions>,
    clock: Arc<Clock>,
    registry: Arc<MatchRegistry>,
    lobby: Arc<AuthorityRegistry>,
    sockets: Arc<AuthorityRegistry>,
    security: Arc<BrowserSecurity>,
}
impl Fixture {
    fn new(requests: usize, pending: bool) -> Self {
        let clock = Arc::new(Clock {
            game: AtomicU64::new(0),
            auth: AtomicI64::new(18000),
        });
        let lobby = AuthorityRegistry::new(32).unwrap();
        let sockets = AuthorityRegistry::new(32).unwrap();
        let registry = MatchRegistry::new(
            MatchLimits {
                matches: 8,
                mailbox: 16,
                outgoing: 32,
                proof_workers: 1,
            },
            clock.clone(),
            clock.clone(),
            sockets.clone(),
            Arc::new(Results),
        )
        .unwrap();
        let mut rules = RulesSnapshot::bundled().rules;
        rules.width = 3;
        rules.height = 3;
        rules.mines = 2;
        let rules = RulesSnapshot::from_rules(rules).unwrap();
        let seed = (0..64)
            .find(|&seed| {
                BoardGenerator::generate(
                    rules.rules.board_spec(),
                    seed,
                    GenerationBudget::default(),
                )
                .is_ok_and(|g| {
                    g.board.cell(CellId(5)) == Some(Cell::Mine)
                        && g.board.cell(CellId(7)) == Some(Cell::Mine)
                })
            })
            .unwrap();
        let supply = Arc::new(Supply {
            pool: BoardPool::start(rules, 2, 1, Arc::new(Seed(seed))).unwrap(),
            hold: AtomicBool::new(pending),
            released: Notify::new(),
        });
        let service = LobbyService::new(
            LobbyLimits {
                capacity: 16,
                workers: 2,
            },
            supply,
            Arc::new(CoreMatchPreparer),
            Arc::new(OsRoomCodeSource),
            registry.clone(),
            BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap(),
            LobbyAuthentication {
                authorities: lobby.clone(),
                clock: clock.clone(),
            },
        )
        .unwrap();
        let sessions = Arc::new(Sessions {
            rows: Mutex::new(HashMap::new()),
            reads: AtomicUsize::new(0),
            hold: AtomicBool::new(false),
            entered: Notify::new(),
            released: Notify::new(),
        });
        let security = Arc::new(BrowserSecurity::new("https://game.example", [5; 32]).unwrap());
        let app = lobby_router(service, sessions.clone(), security.clone(), requests)
            .unwrap()
            .merge(
                websocket_router(
                    "https://game.example",
                    sessions.clone(),
                    registry.clone(),
                    16,
                )
                .unwrap(),
            );
        Self {
            app,
            sessions,
            clock,
            registry,
            lobby,
            sockets,
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
    fn request(&self, user: &User, command: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/v1/lobby")
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
            .body(Body::from(json!({"v":1,"command":command}).to_string()))
            .unwrap()
    }
    async fn call(&self, user: &User, command: Value) -> (StatusCode, Value) {
        response(self.app.clone(), self.request(user, command)).await
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
    (
        status,
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap(),
    )
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn origin_bound_csrf_closed_input_and_body_limits_reject_before_session_storage() {
    let f = Fixture::new(2, false);
    let user = f.user(None);
    for kind in 0..9 {
        let mut request = f.request(&user, json!({"type":"status"}));
        match kind {
            0 => {
                request.headers_mut().remove(header::ORIGIN);
            }
            1 => {
                request
                    .headers_mut()
                    .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
            }
            2 => {
                request.headers_mut().remove("x-liar-csrf");
            }
            3 => {
                request.headers_mut().insert(
                    "x-liar-csrf",
                    f.security
                        .csrf(&user.browser, None, 18000)
                        .unwrap()
                        .parse()
                        .unwrap(),
                );
            }
            4 => {
                *request.uri_mut() = "/api/v1/lobby?account=forged".parse().unwrap();
            }
            5 => {
                *request.body_mut() =
                    Body::from(r#"{"v":1,"command":{"type":"status","account":"forged"}}"#);
            }
            6 => {
                request
                    .headers_mut()
                    .append(header::ORIGIN, "https://game.example".parse().unwrap());
            }
            7 => {
                request
                    .headers_mut()
                    .insert(header::CONTENT_TYPE, "text/plain".parse().unwrap());
            }
            _ => {
                *request.body_mut() = Body::from(" ".repeat(1025));
            }
        }
        let (status, _) = response(f.app.clone(), request).await;
        assert!(
            matches!(
                status,
                StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::PAYLOAD_TOO_LARGE
            ),
            "kind{kind}:{status}"
        );
        assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn absent_nickname_expiry_and_missing_sessions_never_create_membership() {
    let f = Fixture::new(2, false);
    for kind in 0..3 {
        let user = f.user(None);
        {
            let mut rows = f.sessions.rows.lock().unwrap();
            match kind {
                0 => rows.get_mut(&user.token.hash()).unwrap().account.nickname = None,
                1 => rows.get_mut(&user.token.hash()).unwrap().expires_at = 18000,
                _ => {
                    rows.remove(&user.token.hash());
                }
            }
        }
        assert_eq!(
            f.call(&user, json!({"type":"queue_join","difficulty":"easy"}))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert!(f.registry.for_account(user.account).is_err());
    }
}
#[tokio::test]
async fn revocation_between_read_and_shared_binding_rejects_the_captured_session() {
    let f = Fixture::new(2, false);
    let user = f.user(None);
    f.sessions.hold.store(true, Ordering::SeqCst);
    let task = tokio::spawn(response(
        f.app.clone(),
        f.request(&user, json!({"type":"queue_join","difficulty":"normal"})),
    ));
    tokio::time::timeout(Duration::from_secs(2), f.sessions.entered.notified())
        .await
        .unwrap();
    drop(f.lobby.session(user.token.hash()));
    f.sessions.rows.lock().unwrap().remove(&user.token.hash());
    f.sessions.released.notify_one();
    assert_eq!(task.await.unwrap().0, StatusCode::UNAUTHORIZED);
    assert!(f.registry.for_account(user.account).is_err());
}
#[tokio::test]
async fn global_read_bound_and_request_cancellation_release_the_actual_permit() {
    let f = Fixture::new(1, false);
    let one = f.user(None);
    let two = f.user(None);
    f.sessions.hold.store(true, Ordering::SeqCst);
    let task = tokio::spawn(response(
        f.app.clone(),
        f.request(&one, json!({"type":"status"})),
    ));
    tokio::time::timeout(Duration::from_secs(2), f.sessions.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        f.call(&two, json!({"type":"status"})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    f.sessions.hold.store(false, Ordering::SeqCst);
    assert_eq!(
        f.call(&two, json!({"type":"status"})).await.0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn session_read_deadline_returns_unavailable_and_allows_the_next_request() {
    let f = Fixture::new(1, false);
    let user = f.user(None);
    f.sessions.hold.store(true, Ordering::SeqCst);
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        f.call(&user, json!({"type":"status"})),
    )
    .await
    .unwrap();
    assert_eq!(result.0, StatusCode::SERVICE_UNAVAILABLE);
    f.sessions.hold.store(false, Ordering::SeqCst);
    assert_eq!(
        f.call(&user, json!({"type":"status"})).await.0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn actual_http_queue_assigns_one_board_and_cookie_sockets_apply_authoritative_input() {
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message, client::IntoClientRequest},
    };
    let f = Fixture::new(2, false);
    let a = f.user(None);
    let b = f.user(None);
    let (status, queued) = f
        .call(&a, json!({"type":"queue_join","difficulty":"normal"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queued["state"]["type"], "queued");
    assert_eq!(queued["state"]["deadline_ms"], 10000);
    assert_eq!(
        f.call(&b, json!({"type":"queue_join","difficulty":"hard"}))
            .await
            .0,
        StatusCode::OK
    );
    eventually(|| f.registry.for_account(a.account).is_ok()).await;
    let a_state = f.call(&a, json!({"type":"status"})).await.1;
    let b_state = f.call(&b, json!({"type":"status"})).await.1;
    assert_eq!(a_state["state"]["match_id"], b_state["state"]["match_id"]);
    assert_eq!(a_state["state"]["opponent"], "human");
    assert_eq!(a_state["state"]["own_seat"], 0);
    assert_eq!(b_state["state"]["own_seat"], 1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let uri = format!("ws://{}/api/v1/ws", listener.local_addr().unwrap());
    let app = f.app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let ws_request = |user: &User| {
        let mut req = uri.clone().into_client_request().unwrap();
        req.headers_mut()
            .insert(header::ORIGIN, "https://game.example".parse().unwrap());
        req.headers_mut().insert(
            header::COOKIE,
            format!("{SESSION_COOKIE}={}", user.token.expose().as_str())
                .parse()
                .unwrap(),
        );
        req
    };
    let (mut one, _) = connect_async(ws_request(&a)).await.unwrap();
    let (mut two, _) = connect_async(ws_request(&b)).await.unwrap();
    let one_initial = one.next().await.unwrap().unwrap();
    let one_initial: Value = serde_json::from_str(one_initial.to_text().unwrap()).unwrap();
    let two_initial = two.next().await.unwrap().unwrap();
    let two_initial: Value = serde_json::from_str(two_initial.to_text().unwrap()).unwrap();
    assert_eq!(
        one_initial["payload"]["view"]["own"]["cells"],
        two_initial["payload"]["view"]["own"]["cells"]
    );
    assert_eq!(one_initial["match_id"], a_state["state"]["match_id"]);
    assert_eq!(one_initial["payload"]["view"]["countdown_ms"], 3000);
    f.clock.game.store(3000, Ordering::SeqCst);
    one.send(Message::Text(json!({"v":1,"match_id":one_initial["match_id"],"command_id":Uuid::new_v4().to_string(),"client_seq":1,"session_epoch":one_initial["payload"]["session_epoch"],"known_revision":0,"action":{"type":"open","cell":8}}).to_string().into())).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = one.next().await.unwrap().unwrap();
            let event: Value = serde_json::from_str(event.to_text().unwrap()).unwrap();
            if event["payload"]["type"] == "ack" {
                assert_eq!(event["payload"]["status"], "applied");
                break;
            }
        }
    })
    .await
    .unwrap();
    let invalidator = CombinedSessionInvalidator::new(f.sockets.clone(), f.lobby.clone());
    drop(invalidator.account(a.account));
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match one.next().await {
                None | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Text(_))) => {}
                other => panic!("Expected revoked socket closure: {other:?}"),
            }
        }
    })
    .await
    .unwrap();
    server.abort();
}
#[tokio::test]
async fn http_room_readiness_and_old_preparation_cancel_cannot_remove_new_generation() {
    let f = Fixture::new(2, true);
    let host = f.user(None);
    let guest = f.user(None);
    let outsider = f.user(None);
    let (status, room) = f.call(&host, json!({"type":"room_create"})).await;
    assert_eq!(status, StatusCode::OK);
    let id = room["state"]["room_id"].clone();
    let code = room["state"]["code"].clone();
    assert_eq!(code.as_str().unwrap().len(), 8);
    assert_eq!(
        f.call(&guest, json!({"type":"room_join","code":code}))
            .await
            .1["state"]["own_seat"],
        1
    );
    let outsider_state = f
        .call(
            &outsider,
            json!({"type":"cancel","identity":{"kind":"room","room_id":id}}),
        )
        .await;
    assert_eq!(outsider_state.0, StatusCode::OK);
    assert_eq!(outsider_state.1["state"]["type"], "idle");
    assert_eq!(
        f.call(&host, json!({"type":"status"})).await.1["state"]["occupied"],
        json!([true, true])
    );
    f.call(&host, json!({"type":"ready","room_id":id,"ready":true}))
        .await;
    let old = f
        .call(&guest, json!({"type":"ready","room_id":id,"ready":true}))
        .await
        .1;
    assert_eq!(old["state"]["type"], "preparing");
    f.call(&guest, json!({"type":"ready","room_id":id,"ready":false}))
        .await;
    let new = f
        .call(&guest, json!({"type":"ready","room_id":id,"ready":true}))
        .await
        .1;
    assert_ne!(old["state"]["identity"], new["state"]["identity"]);
    let result = f
        .call(
            &guest,
            json!({"type":"cancel","identity":old["state"]["identity"]}),
        )
        .await;
    assert_eq!(result.0, StatusCode::CONFLICT);
    assert_eq!(result.1["error"], "stale");
    assert_eq!(
        f.call(&guest, json!({"type":"status"})).await.1["state"],
        new["state"]
    );
}
#[tokio::test]
async fn account_room_join_limit_survives_session_rotation_and_reopens_at_exact_minute() {
    let f = Fixture::new(2, false);
    let account = Uuid::new_v4();
    for attempt in 0..6 {
        let user = f.user(Some(account));
        let result = f
            .call(&user, json!({"type":"room_join","code":"ZZZZ9999"}))
            .await;
        assert_eq!(
            result.0,
            if attempt < 5 {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    let other = f.user(None);
    assert_eq!(
        f.call(&other, json!({"type":"room_join","code":"ZZZZ9999"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    f.clock.auth.store(18060, Ordering::SeqCst);
    let fresh = f.user(Some(account));
    assert_eq!(
        f.call(&fresh, json!({"type":"room_join","code":"ZZZZ9999"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn session_burst_blocks_before_storage_then_resets_but_clock_rollback_fails_closed() {
    let f = Fixture::new(2, false);
    let user = f.user(None);
    for _ in 0..20 {
        assert_eq!(
            f.call(&user, json!({"type":"status"})).await.0,
            StatusCode::OK
        );
    }
    assert_eq!(
        f.call(&user, json!({"type":"status"})).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 20);
    f.clock.auth.store(18001, Ordering::SeqCst);
    assert_eq!(
        f.call(&user, json!({"type":"status"})).await.0,
        StatusCode::OK
    );
    f.clock.auth.store(18000, Ordering::SeqCst);
    assert_eq!(
        f.call(&user, json!({"type":"status"})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 21);
}

#[tokio::test]
async fn account_burst_survives_distinct_valid_sessions() {
    let f = Fixture::new(2, false);
    let account = Uuid::new_v4();
    for attempt in 0..21 {
        let user = f.user(Some(account));
        assert_eq!(
            f.call(&user, json!({"type":"status"})).await.0,
            if attempt < 20 {
                StatusCode::OK
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    assert_eq!(f.sessions.reads.load(Ordering::SeqCst), 21);
}

#[tokio::test]
async fn queued_difficulty_rejects_changes_and_backfill_uses_effective_state() {
    use liar_protocol::online::OnlinePayload;
    let f = Fixture::new(2, false);
    let user = f.user(None);
    let original = f
        .call(&user, json!({"type":"queue_join","difficulty":"normal"}))
        .await;
    let changed = f
        .call(&user, json!({"type":"queue_join","difficulty":"hard"}))
        .await;
    assert_eq!(original.0, StatusCode::OK);
    assert_eq!(changed.0, StatusCode::CONFLICT);
    assert_eq!(changed.1["error"], "busy");
    let effective = f.call(&user, json!({"type":"status"})).await;
    assert_eq!(effective, original);
    assert_eq!(effective.1["state"]["difficulty"], "normal");
    f.clock.game.store(10000, Ordering::SeqCst);
    eventually(|| f.registry.for_account(user.account).is_ok()).await;
    let state = f.call(&user, json!({"type":"status"})).await.1;
    assert_eq!(state["state"]["opponent"], "bot");
    assert_eq!(state["state"]["own_seat"], 0);
    let handle = f.registry.for_account(user.account).unwrap();
    let lease = f
        .sockets
        .bind(
            f.sockets.generation().unwrap(),
            user.account,
            user.token.hash(),
            20000,
            18000,
        )
        .unwrap();
    let mut connection = handle.connect(lease).await.unwrap();
    let initial = connection.next().await.unwrap();
    let OnlinePayload::Snapshot { view, .. } = initial.payload else {
        panic!("snapshot");
    };
    assert_eq!(view.countdown_ms, 3000);
    assert_eq!(view.opponent.opened_safe, 4);
    f.clock.game.store(13000, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(event) = connection.next().await {
                if let OnlinePayload::Delta { view } = event.payload
                    && view.opponent.opened_safe == 5
                {
                    break;
                }
            } else {
                panic!("Bot match must remain live");
            }
        }
    })
    .await
    .unwrap();
}
