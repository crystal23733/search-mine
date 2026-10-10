use super::rights::account_write;
use super::*;
use liar_protocol::{
    game::{Outcome, PublicEndReason},
    results::ResultError,
};
use liar_server::{
    online::{PgResultRepository, PgSessionReader, PortFuture, ResultRepository, SaveResult},
    results::*,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicI64, Ordering};

struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL and DATABASE_URL"]
async fn unknown_abort_details_are_explicit_and_do_not_restore_deleted_players() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(1_800_000_000)));
    let (account, _) = participant(&PgAuthStore::new(pool.clone())).await;
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let reader = PgResultReader::new(pool.clone(), clock);
    let finished = super::online::finished([Some(account), None]);
    writer.save(finished.clone()).await.unwrap();
    sqlx::query("UPDATE online_match_results SET end_elapsed_ms=NULL,reason='server_failure',secret_seed=NULL WHERE id=$1")
        .bind(finished.id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE online_match_players SET outcome='abort',opened_safe=NULL,mistakes=NULL,accusations=NULL,correct_accusations=NULL WHERE match_id=$1")
        .bind(finished.id).execute(&pool).await.unwrap();
    let view = reader
        .read(account, finished.id)
        .await
        .unwrap()
        .unwrap()
        .project(account, finished.id)
        .unwrap();
    let json = serde_json::to_value(view).unwrap();
    assert_eq!(
        json,
        json!({"match_id":finished.id.to_string(),"rules_hash":finished.rules_hash,"end_elapsed_ms":null,"own":null,"result":{"reason":"server_failure","outcome":"abort","completed":false}})
    );
    assert!(
        reader
            .read(Uuid::new_v4(), finished.id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        writer.save(finished.clone()).await,
        Err(liar_protocol::online::OnlineError::Malformed)
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        writer.save(finished.clone()).await,
        Err(liar_protocol::online::OnlineError::Malformed)
    );
    assert!(reader.read(account, finished.id).await.unwrap().is_none());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM online_match_players WHERE match_id=$1 AND NOT is_bot",
    )
    .bind(finished.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL and DATABASE_URL"]
async fn forward_migration_preserves_known_results_and_rejects_partial_nulls() {
    let pool = auth_pool_through(Some(202610090002)).await;
    let store = PgAuthStore::new(pool.clone());
    let (a, _) = participant(&store).await;
    let (b, _) = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(19000)));
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let reader = PgResultReader::new(pool.clone(), clock);
    let finished = super::online::finished([Some(a), Some(b)]);
    writer.save(finished.clone()).await.unwrap();
    let before = serde_json::to_value(
        reader
            .read(a, finished.id)
            .await
            .unwrap()
            .unwrap()
            .project(a, finished.id)
            .unwrap(),
    )
    .unwrap();
    Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap()
    .run(&pool)
    .await
    .unwrap();
    let after = serde_json::to_value(
        reader
            .read(a, finished.id)
            .await
            .unwrap()
            .unwrap()
            .project(a, finished.id)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(before, after);
    assert!(before["own"].is_object());
    assert!(before["end_elapsed_ms"].is_number());
    sqlx::query("UPDATE online_match_results SET reason='server_failure' WHERE id=$1")
        .bind(finished.id)
        .execute(&pool)
        .await
        .unwrap();
    for mask in 0..16 {
        let values: [Option<i16>; 4] =
            std::array::from_fn(|i| if mask & (1 << i) == 0 { Some(1) } else { None });
        let change = sqlx::query("UPDATE online_match_players SET outcome='abort',opened_safe=$2,mistakes=$3,accusations=$4,correct_accusations=$5 WHERE match_id=$1 AND seat=0")
            .bind(finished.id).bind(values[0]).bind(values[1]).bind(values[2]).bind(values[3]).execute(&pool).await;
        if mask == 0 || mask == 15 {
            change.unwrap();
        } else {
            assert_eq!(
                change
                    .unwrap_err()
                    .as_database_error()
                    .unwrap()
                    .code()
                    .as_deref(),
                Some("23514")
            );
        }
    }
    assert!(
        matches!(
            reader.read(a, finished.id).await,
            Err(ResultError::Unavailable)
        ),
        "null own with known elapsed must not project"
    );
    sqlx::query("UPDATE online_match_results SET end_elapsed_ms=NULL WHERE id=$1")
        .bind(finished.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(reader.read(a, finished.id).await.unwrap().is_some());
    assert!(
        matches!(
            reader.read(b, finished.id).await,
            Err(ResultError::Unavailable)
        ),
        "known own with null elapsed must not project"
    );
    for reason in ["clear", "timeout", "forfeit", "abandoned", "cancelled"] {
        assert!(
            sqlx::query("UPDATE online_match_results SET reason=$2 WHERE id=$1")
                .bind(finished.id)
                .bind(reason)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    for outcome in ["win", "loss", "draw", "cancelled"] {
        assert!(
            sqlx::query("UPDATE online_match_players SET outcome=$2 WHERE match_id=$1 AND seat=0")
                .bind(finished.id)
                .bind(outcome)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    sqlx::query("UPDATE online_match_players SET outcome='abort',opened_safe=NULL,mistakes=NULL,accusations=NULL,correct_accusations=NULL WHERE match_id=$1 AND seat=1").bind(finished.id).execute(&pool).await.unwrap();
    assert!(reader.read(b, finished.id).await.unwrap().is_some());
    assert_eq!(
        writer.save(finished).await,
        Err(liar_protocol::online::OnlineError::Malformed)
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn ninety_day_retention_is_elapsed_seconds_even_across_database_dst() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let (a, _) = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(1770000000)));
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let result = super::online::finished([Some(a), None]);
    writer.save(result.clone()).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let read_pool=PgPoolOptions::new().max_connections(1).after_connect(move |conn,_| {
        let schema=schema.clone();Box::pin(async move {
            sqlx::query("SELECT set_config('search_path',$1,false),set_config('TimeZone','America/New_York',false)").bind(schema).execute(conn).await?;Ok(())
        })
    }).connect(&env::var("DATABASE_URL").unwrap()).await.unwrap();
    let reader = PgResultReader::new(read_pool.clone(), clock.clone());
    clock
        .0
        .store(1770000000 + 90 * 86400 - 1800, Ordering::SeqCst);
    assert!(
        reader.read(a, result.id).await.unwrap().is_some(),
        "DST must not expire a result thirty minutes early"
    );
    clock.0.store(1770000000 + 90 * 86400, Ordering::SeqCst);
    assert!(reader.read(a, result.id).await.unwrap().is_none());
    read_pool.close().await;
    close_auth_pool(pool).await;
}
async fn participant(store: &PgAuthStore) -> (Uuid, SecretToken) {
    let token = SecretToken::generate().unwrap();
    let account = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            19000,
        ))
        .await
        .unwrap()
        .account
        .id;
    store
        .nickname(account, Nickname::parse("Player探偵").unwrap())
        .await
        .unwrap();
    (account, token)
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn persisted_results_are_self_only_historical_bounded_and_never_restored_by_retry() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let (a, _) = participant(&store).await;
    let (b, _) = participant(&store).await;
    let (other, _) = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(19000)));
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let reader = PgResultReader::new(pool.clone(), clock.clone());
    let mut result = super::online::finished([Some(a), Some(b)]);
    result.rules_hash = "a".repeat(64);
    result.reason = PublicEndReason::Forfeit;
    result.players[0].outcome = Outcome::Win;
    result.players[1].outcome = Outcome::Loss;
    result.players[1].opened_safe = 17;
    result.players[1].mistakes = 2;
    assert_eq!(
        writer.save(result.clone()).await.unwrap(),
        SaveResult::Saved
    );
    for (account, outcome, opened) in [(a, Outcome::Win, 4), (b, Outcome::Loss, 17)] {
        let view = reader
            .read(account, result.id)
            .await
            .unwrap()
            .unwrap()
            .project(account, result.id)
            .unwrap();
        assert_eq!(view.rules_hash, "a".repeat(64));
        assert_eq!(view.result.outcome, outcome);
        assert_eq!(view.own.as_ref().unwrap().opened_safe, opened);
        assert_eq!(
            serde_json::to_value(view)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            5
        );
    }
    assert!(reader.read(other, result.id).await.unwrap().is_none());
    assert!(reader.read(a, Uuid::new_v4()).await.unwrap().is_none());
    let bot = super::online::finished([None, Some(a)]);
    writer.save(bot.clone()).await.unwrap();
    assert!(reader.read(a, bot.id).await.unwrap().is_some());
    sqlx::query("UPDATE online_match_results SET secret_seed=NULL WHERE id=$1")
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        writer.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    assert!(reader.read(a, result.id).await.unwrap().is_some());
    clock.0.store(19000 + 90 * 86400 - 1, Ordering::SeqCst);
    assert!(reader.read(a, result.id).await.unwrap().is_some());
    clock.0.store(19000 + 90 * 86400, Ordering::SeqCst);
    assert!(reader.read(a, result.id).await.unwrap().is_none());
    clock.0.store(19000, Ordering::SeqCst);
    sqlx::query("DELETE FROM online_match_players WHERE account_id=$1 AND match_id=$2")
        .bind(a)
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        writer.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    assert!(reader.read(a, result.id).await.unwrap().is_none());
    assert!(reader.read(b, result.id).await.unwrap().is_some());
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        writer.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    assert!(reader.read(b, result.id).await.unwrap().is_none());
    assert!(matches!(
        reader.read(Uuid::nil(), result.id).await,
        Err(ResultError::Unavailable)
    ));
    assert!(matches!(
        reader.read(a, Uuid::nil()).await,
        Err(ResultError::Unavailable)
    ));
    clock.0.store(-1, Ordering::SeqCst);
    assert!(matches!(
        reader.read(a, result.id).await,
        Err(ResultError::Unavailable)
    ));
    clock.0.store(253402300800, Ordering::SeqCst);
    assert!(matches!(
        reader.read(a, result.id).await,
        Err(ResultError::Unavailable)
    ));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn ambiguous_subject_and_corrupt_reason_outcome_fail_closed() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let (a, _) = participant(&store).await;
    let (b, _) = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(19000)));
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let reader = PgResultReader::new(pool.clone(), clock);
    let result = super::online::finished([Some(a), Some(b)]);
    writer.save(result.clone()).await.unwrap();
    sqlx::query("UPDATE online_match_players SET account_id=$1 WHERE match_id=$2")
        .bind(a)
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.read(a, result.id).await,
        Err(ResultError::Unavailable)
    ));
    sqlx::query("UPDATE online_match_players SET account_id=$1 WHERE match_id=$2 AND seat=1")
        .bind(b)
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE online_match_players SET outcome='abort' WHERE match_id=$1 AND seat=0")
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        reader.read(a, result.id).await,
        Err(ResultError::Unavailable)
    ));
    sqlx::query("UPDATE online_match_results SET reason='server_failure' WHERE id=$1")
        .bind(result.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        !reader
            .read(a, result.id)
            .await
            .unwrap()
            .unwrap()
            .project(a, result.id)
            .unwrap()
            .result
            .completed
    );
    assert!(matches!(
        reader.read(b, result.id).await,
        Err(ResultError::Unavailable)
    ));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn actual_http_sessions_and_delete_barrier_reject_a_result_captured_before_deletion() {
    use liar_server::online::AuthorityRegistry;
    use tokio::sync::Notify;
    struct Held {
        reader: PgResultReader,
        entered: Notify,
        released: Notify,
    }
    impl ResultReader for Held {
        fn read(
            &self,
            a: Uuid,
            id: Uuid,
        ) -> PortFuture<'_, Result<Option<StoredPersonalResult>, ResultError>> {
            Box::pin(async move {
                let row = self.reader.read(a, id).await?;
                self.entered.notify_one();
                self.released.notified().await;
                Ok(row)
            })
        }
    }
    let pool = auth_pool().await;
    let authorities = AuthorityRegistry::new(4).unwrap();
    let sockets = AuthorityRegistry::new(4).unwrap();
    let lobby = AuthorityRegistry::new(4).unwrap();
    let runtime = RuntimeAuthConfig {
        security: BrowserSecurity::new("https://game.example", [5; 32]).unwrap(),
        vault: AeadVault::new(1, vec![(1, [6; 32])]).unwrap(),
        digests: DigestKeys::new(1, vec![(1, [7; 32])]).unwrap(),
        providers: vec![],
        notification_audience: None,
    }
    .initialize_with_invalidations(
        pool.clone(),
        Arc::new(CombinedSessionInvalidator::new(
            Arc::new(CombinedSessionInvalidator::new(
                sockets.clone(),
                lobby.clone(),
            )),
            authorities.clone(),
        )),
    )
    .unwrap();
    let (a, token) = participant(&runtime.store).await;
    let (b, token_b) = participant(&runtime.store).await;
    let (_, token_other) = participant(&runtime.store).await;
    let socket = sockets.bind(0, a, token.hash(), 20000, 19000).unwrap();
    let lobby_lease = lobby.bind_shared(0, a, token.hash(), 20000, 19000).unwrap();
    let clock = Arc::new(Clock(AtomicI64::new(19000)));
    let writer = PgResultRepository::new(pool.clone(), clock.clone());
    let mut result = super::online::finished([Some(a), Some(b)]);
    result.players[1].opened_safe = 17;
    writer.save(result.clone()).await.unwrap();
    let reader = Arc::new(Held {
        reader: PgResultReader::new(pool.clone(), clock.clone()),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let browser = SecretToken::generate().unwrap();
    let request = |token: &SecretToken, match_id: Uuid| {
        Request::post("/api/v1/results")
            .header("origin", "https://game.example")
            .header("content-type", "application/json")
            .header(
                "cookie",
                format!(
                    "{BROWSER_COOKIE}={}; {SESSION_COOKIE}={}",
                    browser.expose().as_str(),
                    token.expose().as_str()
                ),
            )
            .header(
                "x-liar-csrf",
                runtime.security.csrf(&browser, Some(token), 19000).unwrap(),
            )
            .body(Body::from(
                json!({"v":1,"match_id":match_id.to_string()}).to_string(),
            ))
            .unwrap()
    };
    let auth = ResultAuthentication {
        clock: clock.clone(),
        authorities,
    };
    let sessions = Arc::new(PgSessionReader::new(runtime.store.clone(), clock));
    let regular = result_router(
        Arc::new(PgResultReader::new(
            pool.clone(),
            Arc::new(Clock(AtomicI64::new(19000))),
        )),
        sessions.clone(),
        runtime.security.clone(),
        auth.clone(),
        2,
    )
    .unwrap();
    let response = regular
        .clone()
        .oneshot(request(&token, result.id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["result"]["match_id"], result.id.to_string());
    assert!(body.to_string().find("secret").is_none());
    let response = regular
        .clone()
        .oneshot(request(&token_b, result.id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["result"]["own"]["opened_safe"], 17);
    for id in [result.id, Uuid::new_v4()] {
        let response = regular
            .clone()
            .oneshot(request(&token_other, id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body, json!({"error":"not_found"}));
    }
    let bot = super::online::finished([None, Some(a)]);
    writer.save(bot.clone()).await.unwrap();
    assert_eq!(
        regular
            .clone()
            .oneshot(request(&token, bot.id))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    sockets.with_authority(&socket, 19000, || ()).unwrap();
    lobby.with_authority(&lobby_lease, 19000, || ()).unwrap();
    let held = result_router(reader.clone(), sessions, runtime.security.clone(), auth, 2).unwrap();
    let task = tokio::spawn(held.oneshot(request(&token, result.id)));
    reader.entered.notified().await;
    runtime
        .store
        .erase_authorized(SessionAuthority {
            account: a,
            hash: token.hash(),
            now: 19000,
        })
        .await
        .unwrap();
    reader.released.notify_waiters();
    assert_eq!(
        task.await.unwrap().unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        writer.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE account_id=$1")
            .bind(a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert!(sockets.with_authority(&socket, 19000, || ()).is_err());
    assert!(lobby.with_authority(&lobby_lease, 19000, || ()).is_err());
    assert_eq!(
        regular
            .clone()
            .oneshot(request(&token_b, result.id))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        regular
            .oneshot(request(&token, result.id))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    close_auth_pool(pool).await;
}
