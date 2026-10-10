use super::online::{finished, participant};
use super::*;
use liar_protocol::online::OnlineError;
use liar_server::online::{PgResultRuntime, ResultRepository, SaveResult};
use std::process::{Child, Command, Stdio};
use std::{future::poll_fn, task::Poll};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn server_command(pool: &sqlx::PgPool) -> (Command, String) {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut url = url::Url::parse(&env::var("DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap().to_string();
    drop(reserved);
    let mut command = Command::new(env!("CARGO_BIN_EXE_liar-server"));
    command.env_clear().env("DATABASE_URL",url.as_str()).env("LIAR_BIND",&address)
        .env("LIAR_PUBLIC_ORIGIN","https://game.example")
        .env("LIAR_AUTH_CSRF_KEY","AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE")
        .env("LIAR_AUTH_DIGEST_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI"}]}"#)
        .env("LIAR_AUTH_VAULT_KEYS",r#"{"current":1,"keys":[{"version":1,"key":"AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM"}]}"#)
        .stdout(Stdio::null()).stderr(Stdio::null());
    for name in ["SystemRoot", "WINDIR", "PATH", "TEMP", "TMP"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    (command, format!("http://{address}"))
}
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap()
}
async fn wait_ready(server: &mut Server, origin: &str) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "Owner exited before serving"
            );
            if client()
                .get(format!("{origin}/health/ready"))
                .send()
                .await
                .is_ok_and(|r| r.status() == StatusCode::OK)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Owner must become ready");
}
async fn wait_failed(server: &mut Server) {
    let status = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("A server without online storage ownership must exit");
    assert!(!status.success());
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn main_rejects_a_second_storage_owner_before_serving() {
    let pool = auth_pool().await;
    let (mut first, origin) = server_command(&pool).await;
    let mut first = Server(first.spawn().unwrap());
    wait_ready(&mut first, &origin).await;
    let (mut second, second_origin) = server_command(&pool).await;
    let mut second = Server(second.spawn().unwrap());
    wait_failed(&mut second).await;
    assert!(
        client()
            .get(format!("{second_origin}/health/live"))
            .send()
            .await
            .is_err()
    );
    assert_eq!(
        client()
            .get(format!("{origin}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    drop(first);
    close_auth_pool(pool).await;
}

async fn owner_pid(pool: &sqlx::PgPool) -> i32 {
    sqlx::query_scalar("SELECT pid FROM pg_locks WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND classid=1280530258::oid AND objid='online_match_results'::regclass::oid AND objsubid=1 AND granted")
        .fetch_one(pool).await.unwrap()
}
async fn wait_blocked(pool: &sqlx::PgPool, pid: i32) {
    tokio::time::timeout(Duration::from_secs(1),async {
        loop {
            let blocked: bool=sqlx::query_scalar("SELECT COALESCE((SELECT wait_event_type='Lock' FROM pg_stat_activity WHERE pid=$1),false)").bind(pid).fetch_one(pool).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("Actual save must reach the locked SQL boundary");
}
async fn claim(pool: &sqlx::PgPool) -> PgResultRuntime {
    PgResultRuntime::claim(pool.clone(), Arc::new(HttpClock))
        .await
        .unwrap()
}
async fn claim_after_release(pool: &sqlx::PgPool) -> PgResultRuntime {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Ok(owner) = PgResultRuntime::claim(pool.clone(), Arc::new(HttpClock)).await {
                break owner;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("A closed session must release its owner lock")
}
async fn count_results(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM online_match_results")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn owner_is_exclusive_and_drop_releases_without_pool_reuse() {
    let pool = auth_pool().await;
    let owner = claim(&pool).await;
    let health = owner.health();
    let pid = owner_pid(&pool).await;
    assert!(health.available());
    assert!(matches!(
        PgResultRuntime::claim(pool.clone(), Arc::new(HttpClock)).await,
        Err(OnlineError::Unavailable)
    ));
    let pool_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(
        pid, pool_pid,
        "The owner connection must never return to the auth pool"
    );
    drop(owner);
    tokio::time::timeout(Duration::from_secs(3), health.failed())
        .await
        .unwrap();
    let next = claim_after_release(&pool).await;
    assert_ne!(pid, owner_pid(&pool).await);
    assert!(!health.available());
    drop(next);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn owner_loss_is_permanent_and_old_handle_cannot_write_after_replacement() {
    let pool = auth_pool().await;
    let account = participant(&PgAuthStore::new(pool.clone())).await;
    let owner = claim(&pool).await;
    let health = owner.health();
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(owner_pid(&pool).await)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(terminated);
    tokio::time::timeout(Duration::from_secs(4), health.failed())
        .await
        .unwrap();
    let next = claim_after_release(&pool).await;
    assert_eq!(
        owner.save(finished([Some(account), None])).await,
        Err(OnlineError::Unavailable)
    );
    assert_eq!(count_results(&pool).await, 0);
    assert_eq!(
        next.save(finished([Some(account), None])).await,
        Ok(SaveResult::Saved)
    );
    assert_eq!(count_results(&pool).await, 1);
    assert!(!health.available());
    let response =
        liar_server::app_with_auth_and_owner(Some(pool.clone()), axum::Router::new(), Some(health))
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    drop(next);
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn started_save_timeout_discards_owner_and_rolls_back_before_replacement() {
    let pool = auth_pool().await;
    let account = participant(&PgAuthStore::new(pool.clone())).await;
    let owner = claim(&pool).await;
    let health = owner.health();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(account)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let task = {
        let owner = owner.clone();
        tokio::spawn(async move { owner.save(finished([Some(account), None])).await })
    };
    wait_blocked(&pool, owner_pid(&pool).await).await;
    assert_eq!(task.await.unwrap(), Err(OnlineError::Unavailable));
    tokio::time::timeout(Duration::from_secs(2), health.failed())
        .await
        .unwrap();
    blocker.rollback().await.unwrap();
    let next = claim_after_release(&pool).await;
    assert_eq!(
        count_results(&pool).await,
        0,
        "A timed-out transaction must not publish a partial result"
    );
    assert_eq!(
        owner.save(finished([Some(account), None])).await,
        Err(OnlineError::Unavailable)
    );
    assert_eq!(
        next.save(finished([Some(account), None])).await,
        Ok(SaveResult::Saved)
    );
    drop(next);
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn bounded_queue_skips_unstarted_cancellations_but_preserves_started_commit() {
    let pool = auth_pool().await;
    let account = participant(&PgAuthStore::new(pool.clone())).await;
    let owner = claim(&pool).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(account)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let first = finished([Some(account), None]);
    let task = {
        let owner = owner.clone();
        let result = first.clone();
        tokio::spawn(async move { owner.save(result).await })
    };
    wait_blocked(&pool, owner_pid(&pool).await).await;
    let mut queued = Vec::new();
    for _ in 0..16 {
        let mut future = owner.save(finished([Some(account), None]));
        poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        queued.push(future);
    }
    assert_eq!(
        owner.save(finished([Some(account), None])).await,
        Err(OnlineError::Unavailable)
    );
    assert!(
        owner.health().available(),
        "Queue saturation must not revoke ownership"
    );
    drop(queued);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    // Releasing the SQL lock does not synchronously drain the saturated queue.
    let repeated = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            assert!(owner.health().available());
            match owner.save(first.clone()).await {
                Err(OnlineError::Unavailable) => tokio::task::yield_now().await,
                result => break result,
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(repeated, Ok(SaveResult::Duplicate));
    assert_eq!(
        count_results(&pool).await,
        1,
        "Unstarted cancelled jobs must have no durable effects"
    );
    let mut conflict = first.clone();
    conflict.ended_ms += 1;
    assert_eq!(owner.save(conflict).await, Err(OnlineError::Malformed));
    let mut invalid = first.clone();
    invalid.rules_hash.clear();
    assert_eq!(owner.save(invalid).await, Err(OnlineError::Malformed));
    assert!(owner.health().available());
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(owner.save(first).await, Ok(SaveResult::Duplicate));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE account_id=$1")
            .bind(account)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        owner.save(finished([Some(account), None])).await,
        Ok(SaveResult::Saved)
    );
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn main_exits_and_stops_http_and_existing_socket_when_owner_is_lost() {
    let pool = auth_pool().await;
    let (mut command, origin) = server_command(&pool).await;
    let mut server = Server(command.spawn().unwrap());
    wait_ready(&mut server, &origin).await;
    let client = client();
    let denied = client
        .post(format!("{origin}/api/v1/lobby"))
        .json(&serde_json::json!({"v":1,"command":{"type":"status"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(denied.headers()["cache-control"], "no-store");
    let bootstrap = client
        .get(format!("{origin}/api/v1/auth/bootstrap"))
        .send()
        .await
        .unwrap();
    assert_eq!(bootstrap.status(), StatusCode::OK);
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
    let body: serde_json::Value = bootstrap.json().await.unwrap();
    let denied = client
        .post(format!("{origin}/api/v1/lobby"))
        .header("origin", "https://game.example")
        .header("cookie", &browser)
        .header("x-liar-csrf", body["csrf"].as_str().unwrap())
        .json(&serde_json::json!({"v":1,"command":{"type":"status"}}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        denied.status(),
        StatusCode::UNAUTHORIZED,
        "Bootstrap CSRF must pass the shared lobby gate"
    );
    use futures_util::StreamExt;
    use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
    let store = PgAuthStore::new(pool.clone());
    let mut cookies = Vec::new();
    for name in ["OwnerOne", "OwnerTwo"] {
        let token = SecretToken::generate().unwrap();
        let account = store
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
                .nickname(account, Nickname::parse(name).unwrap())
                .await
                .unwrap()
        );
        let cookie = format!("{browser}; {SESSION_COOKIE}={}", token.expose().as_str());
        let proof: serde_json::Value = client
            .get(format!("{origin}/api/v1/auth/bootstrap"))
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(proof["account"]["id"], account.to_string());
        cookies.push((cookie, proof["csrf"].as_str().unwrap().to_owned()));
    }
    for (cookie, csrf) in &cookies {
        let queued = client
            .post(format!("{origin}/api/v1/lobby"))
            .header("origin", "https://game.example")
            .header("cookie", cookie)
            .header("x-liar-csrf", csrf)
            .json(&serde_json::json!({"v":1,"command":{"type":"queue_join","difficulty":"normal"}}))
            .send()
            .await
            .unwrap();
        assert_eq!(queued.status(), StatusCode::OK);
    }
    let matched = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let state: serde_json::Value = client
                .post(format!("{origin}/api/v1/lobby"))
                .header("origin", "https://game.example")
                .header("cookie", &cookies[0].0)
                .header("x-liar-csrf", &cookies[0].1)
                .json(&serde_json::json!({"v":1,"command":{"type":"status"}}))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if state["state"]["type"] == "matched" {
                break state;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Real main must admit both human sessions");
    let mut request = format!("{}/api/v1/ws", origin.replacen("http://", "ws://", 1))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "https://game.example".parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", cookies[0].0.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();
    let initial = tokio::time::timeout(Duration::from_secs(2), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let initial: serde_json::Value = serde_json::from_str(initial.to_text().unwrap()).unwrap();
    assert_eq!(initial["match_id"], matched["state"]["match_id"]);
    assert_eq!(initial["payload"]["type"], "snapshot");

    sqlx::query("SELECT pg_terminate_backend($1)")
        .bind(owner_pid(&pool).await)
        .execute(&pool)
        .await
        .unwrap();
    wait_failed(&mut server).await;
    assert!(
        client
            .get(format!("{origin}/health/live"))
            .send()
            .await
            .is_err()
    );
    assert!(
        client
            .get(format!("{origin}/health/ready"))
            .send()
            .await
            .is_err()
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                None
                | Some(Err(_))
                | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .expect("Process exit must close an already authenticated socket");
    let next = claim_after_release(&pool).await;
    drop(next);
    drop(server);
    close_auth_pool(pool).await;
}
