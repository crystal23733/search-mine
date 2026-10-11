use super::result_owner::{Server, client, server_command, wait_failed, wait_ready};
use super::*;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

struct User {
    id: Uuid,
    cookie: String,
    csrf: String,
}
async fn user(pool: &sqlx::PgPool, client: &Client, origin: &str, name: &str) -> User {
    let bootstrap = client
        .get(format!("{origin}/api/v1/auth/bootstrap"))
        .send()
        .await
        .unwrap();
    let browser = bootstrap
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .find(|h| h.starts_with("__Host-liar_browser="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let token = SecretToken::generate().unwrap();
    let store = PgAuthStore::new(pool.clone());
    let id = store
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            SystemAuthClock.now(),
        ))
        .await
        .unwrap()
        .account
        .id;
    assert!(
        store
            .nickname(id, Nickname::parse(name).unwrap())
            .await
            .unwrap()
    );
    let cookie = format!("{browser}; {SESSION_COOKIE}={}", token.expose().as_str());
    let proof: Value = client
        .get(format!("{origin}/api/v1/auth/bootstrap"))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(proof["account"]["id"], id.to_string());
    User {
        id,
        cookie,
        csrf: proof["csrf"].as_str().unwrap().to_owned(),
    }
}
async fn lobby(client: &Client, origin: &str, user: &User, command: Value) -> Value {
    let response = client
        .post(format!("{origin}/api/v1/lobby"))
        .header("origin", "https://game.example")
        .header("cookie", &user.cookie)
        .header("x-liar-csrf", &user.csrf)
        .json(&json!({"v":1,"command":command}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}
async fn pair(pool: &sqlx::PgPool, client: &Client, origin: &str) -> ([User; 2], Uuid) {
    let users = [
        user(pool, client, origin, "RestartOne").await,
        user(pool, client, origin, "RestartTwo").await,
    ];
    for user in &users {
        lobby(
            client,
            origin,
            user,
            json!({"type":"queue_join","difficulty":"normal"}),
        )
        .await;
    }
    let id = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let state = lobby(client, origin, &users[0], json!({"type":"status"})).await;
            if state["state"]["type"] == "matched" {
                break Uuid::parse_str(state["state"]["match_id"].as_str().unwrap()).unwrap();
            }
            // Observe preparation below the production 20 requests/second limit.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Real main must publish a durable human match");
    (users, id)
}
async fn result(client: &Client, origin: &str, user: &User, id: Uuid) -> reqwest::Response {
    client
        .post(format!("{origin}/api/v1/results"))
        .header("origin", "https://game.example")
        .header("cookie", &user.cookie)
        .header("x-liar-csrf", &user.csrf)
        .json(&json!({"v":1,"match_id":id.to_string()}))
        .send()
        .await
        .unwrap()
}
async fn spawn(pool: &sqlx::PgPool) -> (Server, String) {
    let (mut command, origin) = server_command(pool).await;
    let mut server = Server(command.spawn().unwrap());
    wait_ready(&mut server, &origin).await;
    (server, origin)
}
fn kill(mut server: Server) {
    // std::process::Child::kill is SIGKILL on Unix and TerminateProcess on Windows.
    server.0.kill().unwrap();
    assert!(!server.0.wait().unwrap().success());
}
async fn active(pool: &sqlx::PgPool, id: Uuid) -> (String, i64) {
    sqlx::query_as("SELECT rules_hash,EXTRACT(EPOCH FROM admitted_at)::bigint FROM online_active_matches WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap()
}
async fn stored(pool: &sqlx::PgPool, id: Uuid) -> Value {
    let body: String = sqlx::query_scalar("SELECT jsonb_build_object('result',to_jsonb(r),'players',(SELECT jsonb_agg(to_jsonb(p) ORDER BY seat) FROM online_match_players p WHERE p.match_id=r.id))::text FROM online_match_results r WHERE r.id=$1")
        .bind(id).fetch_one(pool).await.unwrap();
    serde_json::from_str(&body).unwrap()
}
async fn assert_unknown(
    pool: &sqlx::PgPool,
    client: &Client,
    origin: &str,
    user: &User,
    id: Uuid,
    hash: &str,
    at: i64,
) {
    let response = result(client, origin, user, id).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["result"],
        json!({"match_id":id.to_string(),"rules_hash":hash,"end_elapsed_ms":null,"own":null,"result":{"outcome":"abort","reason":"server_failure","completed":false}})
    );
    let latest = client
        .post(format!("{origin}/api/v1/results/latest"))
        .header("origin", "https://game.example")
        .header("cookie", &user.cookie)
        .header("x-liar-csrf", &user.csrf)
        .json(&json!({"v":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(latest.status(), StatusCode::OK);
    assert_eq!(latest.json::<Value>().await.unwrap(), body);
    let final_row = stored(pool, id).await;
    assert!(final_row["result"]["secret_seed"].is_null());
    assert!(final_row["result"]["end_elapsed_ms"].is_null());
    let anchor:i64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM retention_started_at)::bigint FROM online_match_results WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap();
    assert_eq!(anchor, at);
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_active_matches WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn real_main_os_kill_recovers_authenticated_ws_match_once_with_original_anchor() {
    let pool = auth_pool().await;
    let client = client();
    let (server, origin) = spawn(&pool).await;
    let (users, id) = pair(&pool, &client, &origin).await;
    let (hash, at) = active(&pool, id).await;
    let mut request = format!("{}/api/v1/ws", origin.replacen("http://", "ws://", 1))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "https://game.example".parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", users[0].cookie.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let first: Value = serde_json::from_str(first.to_text().unwrap()).unwrap();
    assert_eq!(first["match_id"], id.to_string());
    assert_eq!(first["payload"]["type"], "snapshot");
    assert_eq!(first["payload"]["view"]["rules"]["hash"], hash);
    kill(server);
    assert!(
        client
            .get(format!("{origin}/health/ready"))
            .send()
            .await
            .is_err()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(Ok(message)) = socket.next().await {
            if message.is_close() {
                break;
            }
        }
    })
    .await
    .unwrap();
    let (server, origin) = spawn(&pool).await;
    for user in &users {
        assert_unknown(&pool, &client, &origin, user, id, &hash, at).await;
    }
    let outsider = user(&pool, &client, &origin, "RestartOther").await;
    assert_eq!(
        result(&client, &origin, &outsider, id).await.status(),
        StatusCode::NOT_FOUND
    );
    let original = stored(&pool, id).await;
    kill(server);
    let (server, origin) = spawn(&pool).await;
    assert_unknown(&pool, &client, &origin, &users[0], id, &hash, at).await;
    assert_eq!(
        stored(&pool, id).await,
        original,
        "Recovery must not rewrite a committed abort"
    );
    drop(server);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn real_main_known_start_cancellation_is_immutable_across_os_kill_and_restart() {
    let pool = auth_pool().await;
    let client = client();
    let (server, origin) = spawn(&pool).await;
    let (users, id) = pair(&pool, &client, &origin).await;
    let (_, at) = active(&pool, id).await;
    // Neither participant attaches: the real core/actor saves a known start cancellation.
    let before = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let response = result(&client, &origin, &users[0], id).await;
            if response.status() == StatusCode::OK {
                break response.json::<Value>().await.unwrap();
            }
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(before["result"]["result"]["reason"], "cancelled");
    assert!(before["result"]["end_elapsed_ms"].is_number());
    assert!(before["result"]["own"].is_object());
    let original = stored(&pool, id).await;
    let anchor:i64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM retention_started_at)::bigint FROM online_match_results WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(anchor, at);
    kill(server);
    let (server, origin) = spawn(&pool).await;
    let after: Value = result(&client, &origin, &users[0], id)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(stored(&pool, id).await, original);
    drop(server);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn real_main_failed_recovery_never_serves_or_partially_saves_and_erasure_is_not_restored() {
    let pool = auth_pool().await;
    let client = client();
    let (server, origin) = spawn(&pool).await;
    let (users, id) = pair(&pool, &client, &origin).await;
    let (hash, at) = active(&pool, id).await;
    kill(server);
    sqlx::raw_sql("CREATE FUNCTION reject_recovery_seat() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.seat=1 THEN RAISE EXCEPTION 'controlled recovery failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_recovery_seat BEFORE INSERT ON online_match_players FOR EACH ROW EXECUTE FUNCTION reject_recovery_seat();").execute(&pool).await.unwrap();
    let (mut command, failed_origin) = server_command(&pool).await;
    let mut failed = Server(command.spawn().unwrap());
    wait_failed(&mut failed).await;
    assert!(
        client
            .get(format!("{failed_origin}/health/live"))
            .send()
            .await
            .is_err()
    );
    assert_eq!(active(&pool, id).await, (hash.clone(), at));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_results WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER reject_recovery_seat ON online_match_players; DROP FUNCTION reject_recovery_seat();").execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(users[0].id)
        .execute(&pool)
        .await
        .unwrap();
    let (server, origin) = spawn(&pool).await;
    assert_unknown(&pool, &client, &origin, &users[1], id, &hash, at).await;
    assert_eq!(
        result(&client, &origin, &users[0], id).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let rows: Vec<Uuid> =
        sqlx::query_scalar("SELECT account_id FROM online_match_players WHERE match_id=$1")
            .bind(id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows, vec![users[1].id]);
    drop(server);
    drop(failed);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn real_main_cancel_during_started_register_discards_after_ack_and_keeps_owner_healthy() {
    let pool = auth_pool().await;
    let client = client();
    let (server, origin) = spawn(&pool).await;
    sqlx::raw_sql("CREATE TABLE admission_test_events(id uuid,event text); CREATE FUNCTION observe_admission() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP='INSERT' THEN INSERT INTO admission_test_events VALUES(NEW.id,'registered'); RETURN NEW; ELSE INSERT INTO admission_test_events VALUES(OLD.id,'discarded'); RETURN OLD; END IF; END $$; CREATE TRIGGER observe_admission AFTER INSERT OR DELETE ON online_active_matches FOR EACH ROW EXECUTE FUNCTION observe_admission();").execute(&pool).await.unwrap();
    let users = [
        user(&pool, &client, &origin, "CancelOne").await,
        user(&pool, &client, &origin, "CancelTwo").await,
    ];
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(users[0].id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    for user in &users {
        lobby(
            &client,
            &origin,
            user,
            json!({"type":"queue_join","difficulty":"normal"}),
        )
        .await;
    }
    let pid = super::result_owner::owner_pid(&pool).await;
    super::result_owner::wait_blocked(&pool, pid).await;
    let before = lobby(&client, &origin, &users[0], json!({"type":"status"})).await;
    assert_eq!(before["state"]["type"], "preparing");
    let canceled = lobby(
        &client,
        &origin,
        &users[0],
        json!({"type":"cancel","identity":before["state"]["identity"]}),
    )
    .await;
    assert_eq!(canceled["state"]["type"], "idle");
    blocker.rollback().await.unwrap();
    // Stop the peer from backfilling into a new unrelated bot match during observation.
    let peer = lobby(&client, &origin, &users[1], json!({"type":"status"})).await;
    if peer["state"]["type"] == "queued" {
        lobby(&client,&origin,&users[1],json!({"type":"cancel","identity":{"kind":"queue","queue_id":peer["state"]["queue_id"]}})).await;
    }
    // Both committed trigger events prove registration and explicit deletion happened.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM admission_test_events")
                .fetch_one(&pool)
                .await
                .unwrap();
            if count == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let events: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id,event FROM admission_test_events ORDER BY event")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(events[0].0, events[1].0);
    assert_eq!((&*events[0].1, &*events[1].1), ("discarded", "registered"));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM online_active_matches")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(super::result_owner::owner_pid(&pool).await, pid);
    assert_eq!(
        client
            .get(format!("{origin}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_results")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "Ordinary cancel must discard, never become a recovered abort"
    );
    drop(server);
    close_auth_pool(pool).await;
}
