use super::*;
use liar_protocol::daily::*;
use liar_server::{daily::*, online::*, privacy::*};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicI64, Ordering},
};
const NOW: i64 = 1_791_331_260;
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Client {
    account: Uuid,
    revision: Uuid,
    token: SecretToken,
    browser: SecretToken,
}
async fn participant(store: &PgAuthStore) -> Client {
    let token = SecretToken::generate().unwrap();
    let login = store
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            NOW,
        ))
        .await
        .unwrap();
    store
        .nickname(login.account.id, Nickname::parse("Daily Player").unwrap())
        .await
        .unwrap();
    let revision = store.session(token.hash(), NOW).await.unwrap().unwrap().id;
    Client {
        account: login.account.id,
        revision,
        token,
        browser: SecretToken::generate().unwrap(),
    }
}
fn replay() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    fixture["daily"]["replay"].clone()
}
fn body(client: &Client, operation: &str) -> Value {
    let mut body = json!({"v":1,"account_id":client.account.to_string(),"session_revision":client.revision.to_string()});
    match operation {
        "definition" => {
            body["date"] = json!("2026-10-07");
            body["seed_version"] = json!(1);
        }
        "submit" => {
            body["attempt_id"] = json!(Uuid::new_v4().to_string());
            body["replay"] = replay();
        }
        "own" => body["metadata"] = replay()["metadata"].clone(),
        _ => {
            body["stream"] = json!("daily");
            body["after"] = Value::Null;
        }
    };
    body
}
fn request(
    client: &Client,
    security: &BrowserSecurity,
    path: &str,
    body: Value,
    now: i64,
) -> Request<Body> {
    Request::post(path)
        .header("origin", "https://game.example")
        .header("content-type", "application/json")
        .header(
            "cookie",
            format!(
                "{BROWSER_COOKIE}={}; {SESSION_COOKIE}={}",
                client.browser.expose().as_str(),
                client.token.expose().as_str()
            ),
        )
        .header(
            "x-liar-csrf",
            security
                .csrf(&client.browser, Some(&client.token), now)
                .unwrap(),
        )
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn response(app: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let result = app.oneshot(request).await.unwrap();
    assert_eq!(result.headers()["cache-control"], "no-store");
    (
        result.status(),
        serde_json::from_slice(
            &to_bytes(result.into_body(), MAX_DAILY_REPLAY_BYTES + 4096)
                .await
                .unwrap(),
        )
        .unwrap(),
    )
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn production_http_native_verifier_pg_first_retry_practice_export_and_erasure_are_connected()
{
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let authorities = AuthorityRegistry::new(64).unwrap();
    let rights = AuthorityRegistry::new(64).unwrap();
    let store = PgAuthStore::new(pool.clone()).with_invalidations(Arc::new(
        CombinedSessionInvalidator::new(authorities.clone(), rights.clone()),
    ));
    let client = participant(&store).await;
    let other = participant(&store).await;
    let security = Arc::new(BrowserSecurity::new("https://game.example", [116; 32]).unwrap());
    let sessions = Arc::new(PgSessionReader::new(store.clone(), clock.clone()));
    let app = daily_router(
        Arc::new(PgDailyRepository::new(pool.clone(), clock.clone())),
        BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier)),
        sessions.clone(),
        security.clone(),
        DailyAuthentication {
            authorities,
            clock: clock.clone(),
        },
        4,
    )
    .unwrap()
    .merge(
        privacy_router(
            Arc::new(PgGameExportReader::new(pool.clone(), clock.clone())),
            sessions,
            security.clone(),
            PrivacyAuthentication {
                authorities: rights,
                clock: clock.clone(),
                workers: ExportWorkers::default(),
            },
            4,
        )
        .unwrap(),
    );
    let call = |client: &Client, operation: &str, body: Value| {
        request(
            client,
            &security,
            &format!("/api/v1/daily/{operation}"),
            body,
            clock.now(),
        )
    };
    let (status, definition) = response(
        app.clone(),
        call(&client, "definition", body(&client, "definition")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(definition["metadata"], replay()["metadata"]);
    assert!(
        response(app.clone(), call(&client, "own", body(&client, "own")))
            .await
            .1["receipt"]
            .is_null()
    );
    let submission = body(&client, "submit");
    let (status, first) = response(app.clone(), call(&client, "submit", submission.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["disposition"], "official");
    assert_eq!(first["trust"], "verified_completion");
    assert_eq!(
        response(app.clone(), call(&client, "submit", submission.clone())).await,
        (StatusCode::OK, first.clone())
    );
    let mut conflict = submission.clone();
    conflict["replay"]["final_time_ms"] = json!(u32::MAX);
    assert_eq!(
        response(app.clone(), call(&client, "submit", conflict))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let practice = response(
        app.clone(),
        call(&client, "submit", body(&client, "submit")),
    )
    .await;
    assert_eq!(practice.0, StatusCode::OK);
    assert_eq!(practice.1["receipt"], first["receipt"]);
    assert_eq!(practice.1["disposition"], "practice");
    assert_eq!(
        response(app.clone(), call(&client, "own", body(&client, "own")))
            .await
            .1["receipt"],
        first["receipt"]
    );
    assert!(
        response(app.clone(), call(&other, "own", body(&other, "own")))
            .await
            .1["receipt"]
            .is_null()
    );
    let page = response(
        app.clone(),
        request(
            &client,
            &security,
            "/api/v1/me/games/export",
            body(&client, "export"),
            clock.now(),
        ),
    )
    .await;
    assert_eq!(page.0, StatusCode::OK);
    assert_eq!(page.1["records"].as_array().unwrap().len(), 1);
    assert_eq!(
        page.1["records"][0]["completion_id"],
        first["receipt"]["completion_id"]
    );
    let raw = json!({"v":1,"account_id":client.account.to_string(),"session_revision":client.revision.to_string(),"completion_id":first["receipt"]["completion_id"]});
    let raw = response(
        app.clone(),
        request(
            &client,
            &security,
            "/api/v1/me/games/replay",
            raw,
            clock.now(),
        ),
    )
    .await;
    assert_eq!(raw.0, StatusCode::OK);
    assert_eq!(raw.1["replay"], replay());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM daily_completions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    store
        .erase_authorized(SessionAuthority {
            account: client.account,
            hash: client.token.hash(),
            now: NOW,
        })
        .await
        .unwrap();
    assert_eq!(
        response(app.clone(), call(&client, "submit", submission))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM daily_completions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_accounts WHERE id=$1")
            .bind(client.account)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
struct GatedVerifier {
    entered: tokio::sync::mpsc::UnboundedSender<()>,
    released: Mutex<std::sync::mpsc::Receiver<()>>,
}
impl ReplayVerifier for GatedVerifier {
    fn verify(
        &self,
        definition: &PublishedDaily,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError> {
        self.entered.send(()).unwrap();
        self.released
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        NativeReplayVerifier.verify(definition, source)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn actual_logout_or_erasure_while_native_worker_is_live_cannot_restore_a_completion() {
    let pool = auth_pool().await;
    for erase in [false, true] {
        let clock = Arc::new(Clock(AtomicI64::new(NOW)));
        let authorities = AuthorityRegistry::new(64).unwrap();
        let store = PgAuthStore::new(pool.clone()).with_invalidations(authorities.clone());
        let client = participant(&store).await;
        let security = Arc::new(BrowserSecurity::new("https://game.example", [116; 32]).unwrap());
        let (entered, mut started) = tokio::sync::mpsc::unbounded_channel();
        let (released, receiver) = std::sync::mpsc::channel();
        let app = daily_router(
            Arc::new(PgDailyRepository::new(pool.clone(), clock.clone())),
            BoundedReplayVerifier::new(Arc::new(GatedVerifier {
                entered,
                released: Mutex::new(receiver),
            })),
            Arc::new(PgSessionReader::new(store.clone(), clock.clone())),
            security.clone(),
            DailyAuthentication { authorities, clock },
            4,
        )
        .unwrap();
        let req = request(
            &client,
            &security,
            "/api/v1/daily/submit",
            body(&client, "submit"),
            NOW,
        );
        let task = tokio::spawn(response(app.clone(), req));
        tokio::time::timeout(Duration::from_secs(2), started.recv())
            .await
            .unwrap()
            .unwrap();
        if erase {
            store
                .erase_authorized(SessionAuthority {
                    account: client.account,
                    hash: client.token.hash(),
                    now: NOW,
                })
                .await
                .unwrap();
        } else {
            store.logout(client.token.hash()).await.unwrap();
        }
        released.send(()).unwrap();
        assert_eq!(task.await.unwrap().0, StatusCode::UNAUTHORIZED);
        // A second request sees the actual PG session absence even after the worker exits.
        assert_eq!(
            response(
                app,
                request(
                    &client,
                    &security,
                    "/api/v1/daily/submit",
                    body(&client, "submit"),
                    NOW
                )
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM daily_completions WHERE account_id=$1"
            )
            .bind(client.account)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }
    close_auth_pool(pool).await;
}
