use futures_util::{SinkExt, StreamExt};
use liar_core::{
    board::{Board, CellId},
    game::RuleEngine,
    rules::RulesSnapshot,
};
use liar_protocol::online::OnlineError;
use liar_server::{
    auth::{Account, AuthClock, Nickname, SecretToken, Session},
    online::*,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use uuid::Uuid;
struct Clock(AtomicU64);
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        18000
    }
}
struct Results;
impl ResultRepository for Results {
    fn save(&self, _: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async { Ok(SaveResult::Saved) })
    }
}
struct Sessions {
    hash: [u8; 32],
    account: Uuid,
    nickname: AtomicBool,
    hold: AtomicBool,
    started: tokio::sync::Notify,
    released: tokio::sync::Notify,
}
impl SessionReader for Sessions {
    fn read(&self, hash: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            let session = (hash == self.hash).then(|| Session {
                id: Uuid::from_u128(46),
                account: Account {
                    id: self.account,
                    nickname: self
                        .nickname
                        .load(Ordering::SeqCst)
                        .then(|| Nickname::parse("WS探偵").unwrap()),
                },
                created_at: 17000,
                authenticated_at: 17000,
                expires_at: 20000,
            });
            if self.hold.load(Ordering::SeqCst) {
                self.started.notify_one();
                self.released.notified().await;
            }
            Ok(session)
        })
    }
}
struct Fixture {
    uri: String,
    token: SecretToken,
    account: Uuid,
    source: Arc<Sessions>,
    authorities: Arc<AuthorityRegistry>,
    clock: Arc<Clock>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn fixture() -> Fixture {
    fixture_with_capacity(2).await
}
async fn fixture_with_capacity(capacity: usize) -> Fixture {
    fixture_with_admission(capacity, false).await
}
async fn fixture_with_admission(capacity: usize, admitted: bool) -> Fixture {
    let account = Uuid::new_v4();
    let token = SecretToken::generate().unwrap();
    let source = Arc::new(Sessions {
        hash: token.hash(),
        account,
        nickname: AtomicBool::new(true),
        hold: AtomicBool::new(false),
        started: tokio::sync::Notify::new(),
        released: tokio::sync::Notify::new(),
    });
    let authorities = AuthorityRegistry::new(2).unwrap();
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 1,
            mailbox: 64,
            outgoing: 64,
            proof_workers: 1,
        },
        clock.clone(),
        clock.clone(),
        authorities.clone(),
        Arc::new(Results),
    )
    .unwrap();
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    let peer = Uuid::new_v4();
    let state = MatchState::new(
        Uuid::new_v4(),
        RuleEngine::new(board, rules, 0).unwrap(),
        [Some(account), Some(peer)],
        [46; 8],
        0,
    )
    .unwrap();
    if admitted {
        let lobby = AuthorityRegistry::new(2).unwrap();
        let one = lobby
            .bind_shared(0, account, token.hash(), 20000, 18000)
            .unwrap();
        let two = lobby.bind_shared(0, peer, [2; 32], 20000, 18000).unwrap();
        registry
            .create_admitted(
                state,
                MatchAdmission {
                    authorities: lobby,
                    clock: clock.clone(),
                    participants: [Some(one), Some(two)],
                },
                None,
            )
            .unwrap();
    } else {
        registry.create(state).unwrap();
    }
    let app = websocket_router("https://liar.example", source.clone(), registry, capacity).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let uri = format!("ws://{}/api/v1/ws", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Fixture {
        uri,
        token,
        account,
        source,
        authorities,
        clock,
        task,
    }
}
#[tokio::test]
async fn logout_between_session_read_and_registration_rejects_a_stale_handshake() {
    use liar_server::auth::SessionInvalidator;
    let f = fixture().await;
    f.source.hold.store(true, Ordering::SeqCst);
    let handshake = tokio::spawn(connect_async(request(
        &f,
        Some("https://liar.example"),
        true,
    )));
    tokio::time::timeout(Duration::from_secs(2), f.source.started.notified())
        .await
        .unwrap();
    let barrier = f.authorities.session(f.token.hash());
    drop(barrier);
    f.source.released.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(2), handshake)
        .await
        .unwrap()
        .unwrap();
    let tokio_tungstenite::tungstenite::Error::Http(response) = result.unwrap_err() else {
        panic!("stale authority rejection")
    };
    assert_eq!(response.status().as_u16(), 401);
}
#[tokio::test]
async fn oversized_frame_closes_with_a_stable_protocol_code() {
    let f = fixture().await;
    let (mut socket, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    socket.next().await.unwrap().unwrap();
    socket
        .send(Message::Text("x".repeat(8193).into()))
        .await
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Close(Some(frame)) = message else {
        panic!("Explicit protocol close required")
    };
    assert_eq!(u16::from(frame.code), 1002);
}
#[tokio::test]
async fn forty_one_inputs_exceed_the_burst_without_applying_the_last_command() {
    let f = fixture().await;
    let (mut socket, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let initial = socket.next().await.unwrap().unwrap();
    let initial: serde_json::Value = serde_json::from_str(initial.to_text().unwrap()).unwrap();
    for seq in 1..=41 {
        let input = serde_json::json!({"v":1,"match_id":initial["match_id"],"command_id":Uuid::new_v4().to_string(),"client_seq":seq,"session_epoch":2,"known_revision":0,"action":{"type":"flag","cell":8}});
        socket
            .send(Message::Text(input.to_string().into()))
            .await
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let event: serde_json::Value = serde_json::from_str(event.to_text().unwrap()).unwrap();
        if seq <= 40 {
            assert_eq!(event["payload"]["type"], "ack");
        } else {
            assert_eq!(event["payload"]["code"], "rate_limited");
        }
    }
}
#[tokio::test]
async fn physical_capacity_rejects_before_replacing_an_existing_socket() {
    let f = fixture_with_capacity(1).await;
    let (mut first, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    first.next().await.unwrap().unwrap();
    let error = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
        panic!("capacity HTTP rejection")
    };
    assert_eq!(response.status().as_u16(), 503);
    first.send(Message::Text("{}".into())).await.unwrap();
    assert!(first.next().await.unwrap().unwrap().is_text());
}
#[tokio::test]
async fn new_socket_replaces_old_epoch_without_old_close_revoking_the_new_owner() {
    let f = fixture().await;
    let (mut first, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    first.next().await.unwrap().unwrap();
    let (mut second, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let snapshot = second.next().await.unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(snapshot.to_text().unwrap()).unwrap();
    assert_eq!(value["payload"]["session_epoch"], 3);
    let old = tokio::time::timeout(Duration::from_secs(2), first.next())
        .await
        .unwrap();
    assert!(matches!(old, None | Some(Ok(Message::Close(_)))));
    drop(first);
    second.send(Message::Text("{}".into())).await.unwrap();
    let error = second.next().await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(error.to_text().unwrap()).unwrap()["payload"]["code"],
        "malformed"
    );
}
fn request(
    f: &Fixture,
    origin: Option<&str>,
    cookie: bool,
) -> tokio_tungstenite::tungstenite::http::Request<()> {
    let mut request = f.uri.clone().into_client_request().unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("Origin", origin.parse().unwrap());
    }
    if cookie {
        request.headers_mut().insert(
            "Cookie",
            format!("__Host-liar_session={}", f.token.expose().as_str())
                .parse()
                .unwrap(),
        );
    }
    request
}

async fn receive_kind(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    kind: &str,
) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if let Message::Text(text) = message {
                let value: serde_json::Value = serde_json::from_str(text.as_str()).unwrap();
                if value["payload"]["type"] == kind {
                    break value;
                }
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn reconnect_snapshot_restores_consumed_cursor_and_retries_keep_the_original_ack() {
    let f = fixture().await;
    let (mut first, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let initial = receive_kind(&mut first, "snapshot").await;
    assert_eq!(initial["payload"]["last_client_seq"], 0);
    f.clock.0.store(3000, Ordering::SeqCst);
    let mut original = serde_json::json!({"v":1,"match_id":initial["match_id"],"command_id":Uuid::new_v4().to_string(),"client_seq":9,"session_epoch":2,"known_revision":0,"action":{"type":"flag","cell":8}});
    first
        .send(Message::Text(original.to_string().into()))
        .await
        .unwrap();
    let original_ack = receive_kind(&mut first, "ack").await;
    assert_eq!(original_ack["payload"]["status"], "applied");
    let mut later = original.clone();
    later["command_id"] = serde_json::json!(Uuid::new_v4().to_string());
    later["client_seq"] = serde_json::json!(11);
    later["action"]["cell"] = serde_json::json!(5);
    first
        .send(Message::Text(later.to_string().into()))
        .await
        .unwrap();
    assert_eq!(
        receive_kind(&mut first, "ack").await["payload"]["status"],
        "applied"
    );
    first.close(None).await.unwrap();
    drop(first);
    let (mut resumed, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let snapshot = receive_kind(&mut resumed, "snapshot").await;
    assert_eq!(snapshot["payload"]["last_client_seq"], 11);
    assert_eq!(snapshot["payload"]["session_epoch"], 3);
    assert_eq!(
        snapshot["payload"]["view"]["own"]["cells"][8]["flagged"],
        true
    );
    assert_eq!(
        snapshot["payload"]["view"]["opponent"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    original["session_epoch"] = serde_json::json!(3);
    resumed
        .send(Message::Text(original.to_string().into()))
        .await
        .unwrap();
    let repeated = receive_kind(&mut resumed, "ack").await;
    assert_eq!(repeated["payload"]["duplicate"], true);
    assert_eq!(
        repeated["payload"]["revision"],
        original_ack["payload"]["revision"]
    );
    let mut conflict = original.clone();
    conflict["action"]["type"] = serde_json::json!("open");
    resumed
        .send(Message::Text(conflict.to_string().into()))
        .await
        .unwrap();
    assert_eq!(
        receive_kind(&mut resumed, "ack").await["payload"]["error"],
        "conflict"
    );
    later["command_id"] = serde_json::json!(Uuid::new_v4().to_string());
    later["client_seq"] = serde_json::json!(12);
    later["action"]["cell"] = serde_json::json!(8);
    later["session_epoch"] = serde_json::json!(3);
    resumed
        .send(Message::Text(later.to_string().into()))
        .await
        .unwrap();
    assert_eq!(
        receive_kind(&mut resumed, "ack").await["payload"]["status"],
        "applied"
    );
    let delta = receive_kind(&mut resumed, "delta").await;
    assert_eq!(
        delta["payload"]["view"]["own"]["cells"][8]["flagged"],
        false
    );
    assert_eq!(delta["payload"]["view"]["own"]["cells"][5]["flagged"], true);
    assert_eq!(delta["payload"]["view"]["own"]["gauge"], 0);
    later["session_epoch"] = serde_json::json!(2);
    resumed
        .send(Message::Text(later.to_string().into()))
        .await
        .unwrap();
    assert_eq!(
        receive_kind(&mut resumed, "error").await["payload"]["code"],
        "invalid_epoch"
    );
}
#[tokio::test]
async fn actual_socket_authenticates_and_sends_only_public_projection() {
    let f = fixture().await;
    let (mut socket, response) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
    assert_eq!(value["payload"]["type"], "snapshot");
    assert_eq!(value["payload"]["session_epoch"], 2);
    assert_eq!(
        value["payload"]["view"]["opponent"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(value["payload"]["view"]["own"]["gauge"], 0);
    for forbidden in [
        "seed",
        "truth",
        "lies",
        "target",
        "source",
        "provider",
        "session_hash",
        "nonce",
        "token",
    ] {
        assert!(!value.to_string().contains(&format!("\"{forbidden}\"")));
    }
    f.clock.0.store(3000, Ordering::SeqCst);
    let input = serde_json::json!({"v":1,"match_id":value["match_id"],"command_id":Uuid::new_v4().to_string(),"client_seq":1,"session_epoch":2,"known_revision":0,"action":{"type":"open","cell":8}});
    socket
        .send(Message::Text(input.to_string().into()))
        .await
        .unwrap();
    let ack = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            let value: serde_json::Value =
                serde_json::from_str(message.to_text().unwrap()).unwrap();
            if value["payload"]["type"] == "ack" {
                break value;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(ack["payload"]["type"], "ack");
    assert_eq!(ack["payload"]["status"], "applied");
}
#[tokio::test]
async fn rejects_foreign_origin_absent_cookie_and_accounts_without_nickname() {
    let f = fixture().await;
    for (origin, cookie, status) in [
        (Some("https://evil.example"), true, 403),
        (None, true, 403),
        (Some("https://liar.example"), false, 401),
    ] {
        let error = connect_async(request(&f, origin, cookie))
            .await
            .unwrap_err();
        let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
            panic!("HTTP rejection")
        };
        assert_eq!(response.status().as_u16(), status);
    }
    f.source.nickname.store(false, Ordering::SeqCst);
    let error = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
        panic!("HTTP rejection")
    };
    assert_eq!(response.status().as_u16(), 401);
}
#[tokio::test]
async fn session_revocation_closes_the_actual_socket() {
    use liar_server::auth::SessionInvalidator;
    let f = fixture().await;
    let (mut socket, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    socket.next().await.unwrap().unwrap();
    let _barrier = f.authorities.account(f.account);
    let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap();
    assert!(matches!(
        message,
        Some(Ok(Message::Close(Some(frame))))
            if frame.code == tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy
    ));
}

#[tokio::test]
async fn malformed_input_error_uses_the_generated_sequenced_wire_envelope() {
    let f = fixture().await;
    let (mut socket, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let initial = socket.next().await.unwrap().unwrap();
    let initial: serde_json::Value = serde_json::from_str(initial.to_text().unwrap()).unwrap();
    socket
        .send(Message::Text("{\"seed\":46}".into()))
        .await
        .unwrap();
    let error = socket.next().await.unwrap().unwrap();
    let error: serde_json::Value = serde_json::from_str(error.to_text().unwrap()).unwrap();
    assert_eq!(error["payload"]["type"], "error");
    assert_eq!(error["payload"]["code"], "malformed");
    assert_eq!(error["match_id"], initial["match_id"]);
    assert_eq!(
        error["server_seq"].as_u64(),
        Some(initial["server_seq"].as_u64().unwrap() + 1)
    );
    assert!(error["server_time_ms"].is_u64());
}

#[tokio::test]
async fn actual_socket_observes_initial_cancel_and_exact_boundary_handshake_is_rejected() {
    let f = fixture_with_admission(2, true).await;
    f.clock.0.store(2999, Ordering::SeqCst);
    let (mut socket, _) = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap();
    let initial = socket.next().await.unwrap().unwrap();
    let initial: serde_json::Value = serde_json::from_str(initial.to_text().unwrap()).unwrap();
    assert_eq!(initial["payload"]["type"], "snapshot");
    assert_eq!(initial["payload"]["view"]["countdown_ms"], 1);
    f.clock.0.store(40000, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = socket.next().await.unwrap().unwrap();
            let event: serde_json::Value = serde_json::from_str(event.to_text().unwrap()).unwrap();
            assert_eq!(event["match_id"], initial["match_id"]);
            if event["payload"]["type"] == "match_end" && event["payload"]["recording"] == "saved" {
                assert_eq!(event["payload"]["view"]["result"]["reason"], "cancelled");
                assert_eq!(event["payload"]["view"]["result"]["completed"], false);
                break;
            }
        }
    })
    .await
    .unwrap();

    let f = fixture_with_admission(2, true).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    let error = connect_async(request(&f, Some("https://liar.example"), true))
        .await
        .unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
        panic!("HTTP rejection");
    };
    assert_eq!(response.status().as_u16(), 409);
}
