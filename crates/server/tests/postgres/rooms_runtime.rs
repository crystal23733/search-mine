use super::{
    admission_restart::{User, spawn, user},
    result_owner::client,
    *,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::{
    client_async,
    tungstenite::{Message, client::IntoClientRequest},
};

async fn rooms(http: &reqwest::Client, origin: &str, user: &User, intent: Value) -> Value {
    let response = http
        .post(format!("{origin}/api/v2/rooms"))
        .header("origin", "https://game.example")
        .header("cookie", &user.cookie)
        .header("x-liar-csrf", &user.csrf)
        .json(&json!({"v":2,"intent":intent}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let value: Value = response.json().await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "Room intent rejected with {:?}",
        value["payload"]["error"]
    );
    value["payload"]["room"].clone()
}

fn room_change(kind: &str, view: &Value) -> Value {
    json!({ "type":kind, "request_id":Uuid::new_v4(), "room_id":view["room_id"], "revision":view["revision"] })
}

type Socket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn connect(origin: &str, user: &User) -> Result<Socket, Box<dyn std::error::Error>> {
    let authority = origin.strip_prefix("http://").unwrap();
    let mut request = format!("ws://{authority}/api/v2/ws").into_client_request()?;
    request
        .headers_mut()
        .insert("origin", "https://game.example".parse()?);
    request.headers_mut().insert("cookie", user.cookie.parse()?);
    let tcp = tokio::net::TcpStream::connect(authority).await?;
    let (socket, _) = client_async(request, tcp).await?;
    Ok(socket)
}

async fn next(socket: &mut Socket, mut expected: impl FnMut(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            match socket
                .next()
                .await
                .expect("Round socket closed unexpectedly")
                .unwrap()
            {
                Message::Text(text) => {
                    let event: Value = serde_json::from_str(&text).unwrap();
                    if expected(&event) {
                        return event;
                    }
                }
                Message::Ping(data) => socket.send(Message::Pong(data)).await.unwrap(),
                Message::Close(_) => panic!("Round socket closed before expected event"),
                _ => {}
            }
        }
    })
    .await
    .expect("Expected round event within the connection budget")
}

async fn countdown(http: &reqwest::Client, origin: &str, host: &User) -> Value {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let view = rooms(http, origin, host, json!({"type":"status"})).await;
            if view["phase"] == "countdown" {
                break view["round"]["round_id"].clone();
            }
            assert_eq!(view["phase"], "preparing");
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn actual_main_four_rooms_sockets_share_one_start_and_record_one_ranked_result() {
    let pool = auth_pool().await;
    let http = client();
    let (mut server, origin) = spawn(&pool).await;
    let mut users = Vec::new();
    for index in 0..5 {
        users.push(user(&pool, &http, &origin, &format!("Race player {index}")).await);
    }
    let mut view = rooms(
        &http,
        &origin,
        &users[0],
        json!({
            "type":"create", "request_id":Uuid::new_v4(),
            "options":{"title":"Runtime race", "visibility":"public", "capacity":4}
        }),
    )
    .await;
    let room_id = view["room_id"].clone();
    for guest in &users[1..4] {
        view = rooms(&http, &origin, guest, json!({"type":"join","request_id":Uuid::new_v4(),"entrance":{"kind":"public","room_id":room_id}})).await;
    }
    for member in &users[..4] {
        let mut ready = room_change("ready", &view);
        ready["ready"] = json!(true);
        view = rooms(&http, &origin, member, ready).await;
    }
    let preparing = rooms(&http, &origin, &users[0], room_change("start", &view)).await;
    assert_eq!(preparing["phase"], "preparing");
    assert!(preparing["round"].is_null());
    let round = countdown(&http, &origin, &users[0]).await;
    assert!(connect(&origin, &users[4]).await.is_err());
    let mut sockets = Vec::new();
    for member in &users[..4] {
        sockets.push(connect(&origin, member).await.unwrap());
    }
    let mut own = Vec::new();
    for (seat, socket) in sockets.iter_mut().enumerate() {
        let state = next(socket, |event| event["payload"]["type"] == "snapshot").await;
        assert_eq!(state["round_id"], round);
        assert_eq!(state["payload"]["own_seat"], seat);
        assert_eq!(
            state["payload"]["view"]["opponents"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let text = state.to_string();
        assert_eq!(text.matches("\"cells\"").count(), 1);
        assert!(
            users
                .iter()
                .all(|user| !text.contains(&user.id.to_string()))
        );
        assert!(!text.contains("secret_seed") && !text.contains("invite_code"));
        own.push(state);
    }
    let _ = next(&mut sockets[0], |event| {
        event["payload"]["view"]["phase"] == "playing"
    })
    .await;
    let first_closed = own[0]["payload"]["view"]["own"]["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["state"] == "closed")
        .unwrap()["cell"]
        .clone();
    let input = json!({"v":2,"round_id":round,"command_id":Uuid::new_v4(),"client_seq":1,
        "session_epoch":own[0]["payload"]["session_epoch"],"known_revision":0,
        "action":{"type":"flag","cell":first_closed}});
    sockets[0]
        .send(Message::Text(input.to_string().into()))
        .await
        .unwrap();
    let ack = next(&mut sockets[0], |event| event["payload"]["type"] == "ack").await;
    assert_eq!(ack["payload"]["status"], "applied");
    let removed = rooms(&http, &origin, &users[3], json!({"type":"status"})).await;
    assert!(
        rooms(&http, &origin, &users[3], room_change("leave", &removed))
            .await
            .is_null()
    );
    let _ = next(&mut sockets[0], |event| {
        event["payload"]["view"]["opponents"]
            .as_array()
            .is_some_and(|opponents| {
                opponents
                    .iter()
                    .any(|opponent| opponent["seat"] == 3 && opponent["forfeited"] == true)
            })
    })
    .await;
    let next_round_only = rooms(&http, &origin, &users[3], json!({
        "type":"join", "request_id":Uuid::new_v4(), "entrance":{"kind":"public","room_id":room_id}
    })).await;
    assert_eq!(next_round_only["phase"], "playing");
    assert!(next_round_only["round"].is_null());
    assert!(connect(&origin, &users[3]).await.is_err());
    for member in &users[1..3] {
        let view = rooms(&http, &origin, member, json!({"type":"status"})).await;
        assert!(
            rooms(&http, &origin, member, room_change("leave", &view))
                .await
                .is_null()
        );
    }
    let end = next(&mut sockets[0], |event| event["payload"]["type"] == "end").await;
    assert_eq!(end["payload"]["recording"], "saved");
    assert_eq!(end["payload"]["view"]["result"]["winner"], 0);
    assert_eq!(end["payload"]["view"]["result"]["reason"], "forfeit");
    let round_id = Uuid::parse_str(round.as_str().unwrap()).unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 4);
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM product_events WHERE online_match_id=$1")
            .bind(round_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 4);
    let lobby = rooms(&http, &origin, &users[0], json!({"type":"status"})).await;
    assert_eq!(lobby["phase"], "results");
    assert_eq!(lobby["room_id"], room_id);
    let saved_summary = &lobby["result_summary"];
    assert_eq!(saved_summary["recording"], "saved");
    assert_eq!(saved_summary["result"]["winner"], 0);
    assert_eq!(
        saved_summary["result"]["standings"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(saved_summary["result"]["standings"][1]["nickname"].is_null());
    let spectator = rooms(&http, &origin, &users[3], json!({"type":"status"})).await;
    assert!(spectator["result_summary"].is_null());
    drop(sockets);

    let mut view = lobby;
    for member in [&users[0], &users[3]] {
        let mut ready = room_change("ready", &view);
        ready["ready"] = json!(true);
        view = rooms(&http, &origin, member, ready).await;
    }
    rooms(&http, &origin, &users[0], room_change("start", &view)).await;
    let replay = countdown(&http, &origin, &users[0]).await;
    assert_ne!(replay, round);
    let mut connection = connect(&origin, &users[0]).await.unwrap();
    let _ = next(&mut connection, |event| {
        event["payload"]["type"] == "awaiting"
    })
    .await;
    let cancelled = next(&mut connection, |event| event["payload"]["type"] == "end").await;
    assert_eq!(cancelled["payload"]["view"]["phase"], "cancelled");
    let latest = rooms(&http, &origin, &users[0], json!({"type":"status"})).await;
    assert_eq!(latest["phase"], "waiting");
    assert_eq!(latest["start_error"], "not_ready");
    assert_eq!(latest["room_id"], room_id);
    let replay_id = Uuid::parse_str(replay.as_str().unwrap()).unwrap();
    let unfinished: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM online_active_matches WHERE id=$1 UNION ALL SELECT 1 FROM online_match_results WHERE id=$1)")
        .bind(replay_id).fetch_one(&pool).await.unwrap();
    assert!(!unfinished);
    drop(connection);

    let mut view = latest;
    for member in [&users[0], &users[3]] {
        let mut ready = room_change("ready", &view);
        ready["ready"] = json!(true);
        view = rooms(&http, &origin, member, ready).await;
    }
    rooms(&http, &origin, &users[0], room_change("start", &view)).await;
    let lost = countdown(&http, &origin, &users[0]).await;
    let lost_id = Uuid::parse_str(lost.as_str().unwrap()).unwrap();
    let mut first = connect(&origin, &users[0]).await.unwrap();
    let second = connect(&origin, &users[3]).await.unwrap();
    let _ = next(&mut first, |event| {
        event["payload"]["view"]["phase"] == "playing"
    })
    .await;
    let start: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM started_at)::bigint FROM online_active_matches WHERE id=$1",
    )
    .bind(lost_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    server.0.kill().unwrap();
    assert!(!server.0.wait().unwrap().success());
    drop((first, second));
    drop(server);
    let (replacement, replacement_origin) = spawn(&pool).await;
    let recovered: (i64, String, Option<i64>, Option<Vec<u8>>) = sqlx::query_as("SELECT EXTRACT(EPOCH FROM retention_started_at)::bigint,reason,end_elapsed_ms,secret_seed FROM online_match_results WHERE id=$1")
        .bind(lost_id).fetch_one(&pool).await.unwrap();
    assert_eq!(recovered, (start, "server_failure".into(), None, None));
    let response: Value = http
        .post(format!("{replacement_origin}/api/v1/me/overview"))
        .header("origin", "https://game.example")
        .header("cookie", &users[0].cookie)
        .header("x-liar-csrf", &users[0].csrf)
        .json(&json!({"v":1}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let aborted = response["recent_online"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["result"]["match_id"] == lost)
        .unwrap();
    assert_eq!(aborted["race"]["participant_count"], 2);
    assert!(aborted["race"]["rank"].is_null());
    assert!(aborted["result"]["own"].is_null());
    assert!(
        rooms(
            &http,
            &replacement_origin,
            &users[0],
            json!({"type":"status"})
        )
        .await
        .is_null()
    );
    drop(replacement);
    close_auth_pool(pool).await;
}
