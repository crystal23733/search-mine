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
}
impl SessionReader for Sessions {
    fn read(&self, hash: [u8; 32]) -> PortFuture<'_, Result<Option<Session>, OnlineError>> {
        Box::pin(async move {
            Ok((hash == self.hash).then(|| Session {
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
            }))
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
    let account = Uuid::new_v4();
    let token = SecretToken::generate().unwrap();
    let source = Arc::new(Sessions {
        hash: token.hash(),
        account,
        nickname: AtomicBool::new(true),
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
    registry
        .create(
            MatchState::new(
                Uuid::new_v4(),
                RuleEngine::new(board, rules, 0).unwrap(),
                [Some(account), Some(Uuid::new_v4())],
                [46; 8],
            )
            .unwrap(),
        )
        .unwrap();
    let app = websocket_router("https://liar.example", source.clone(), registry, 2).unwrap();
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
    let ack = socket.next().await.unwrap().unwrap();
    let ack: serde_json::Value = serde_json::from_str(ack.to_text().unwrap()).unwrap();
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
    assert!(matches!(message, None | Some(Ok(Message::Close(_)))));
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
