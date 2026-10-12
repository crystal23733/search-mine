use super::*;
use std::sync::atomic::AtomicUsize;

struct ReversingClock(AtomicUsize);
impl AuthClock for ReversingClock {
    fn now(&self) -> i64 {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            START + 60
        } else {
            START + 59
        }
    }
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn even_an_unpublished_day_rejects_clock_rollback_before_returning_empty_success() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(ReversingClock(AtomicUsize::new(0))));
    assert!(matches!(
        store.read("2026-10-07", 1).await,
        Err(DailyStoreError::Unavailable)
    ));
    close_auth_pool(pool).await;
}

async fn read(store: &PgDailyRepository, now: i64) -> liar_protocol::daily::DailyRankingsResponse {
    store
        .read("2026-10-07", 1)
        .await
        .unwrap()
        .unwrap()
        .project("2026-10-07", 1, now)
        .unwrap()
}
async fn timed(
    store: &PgDailyRepository,
    pool: &sqlx::PgPool,
    name: &str,
    evidence: &ReplayEvidence,
    elapsed: u32,
) -> DailySession {
    let session = participant(pool, START + 60).await;
    PgAuthStore::new(pool.clone())
        .nickname(session.account, Nickname::parse(name).unwrap())
        .await
        .unwrap();
    let attempt = Uuid::new_v4();
    let proof = timing::sealed(session, attempt, evidence, elapsed);
    store
        .submit_timed(session, attempt, evidence, &proof)
        .await
        .unwrap();
    session
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn native_time_errors_and_null_are_separate_and_current_nickname_erasure_are_live() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock);
    assert!(store.read("2026-10-07", 1).await.unwrap().is_none());
    let accepted = store.publish(definition()).await.unwrap();
    let clean = evidence();
    let errors = swept(&accepted);
    assert_eq!(clean.mistakes(), 0);
    assert!(errors.mistakes() > 0);
    let fast_elapsed = errors.replayed_elapsed_ms() + 1000;
    assert!(fast_elapsed < 180000);
    let slow = timed(&store, &pool, "Slow Clean", &clean, 180000).await;
    timed(&store, &pool, "Fast Errors", &errors, fast_elapsed).await;
    let offline = participant(&pool, START + 60).await;
    PgAuthStore::new(pool.clone())
        .nickname(offline.account, Nickname::parse("Offline Clean").unwrap())
        .await
        .unwrap();
    store.submit(offline, Uuid::new_v4(), &clean).await.unwrap();
    let ranked = read(&store, START + 60).await;
    assert_eq!(
        ranked
            .time
            .entries
            .iter()
            .map(|r| r.nickname.as_str())
            .collect::<Vec<_>>(),
        ["Fast Errors", "Slow Clean"]
    );
    assert_eq!(ranked.time.entries[0].elapsed_ms, fast_elapsed);
    assert_eq!(ranked.completion.entries.len(), 3);
    assert_eq!(ranked.completion.entries[0].mistakes, 0);
    assert_eq!(ranked.completion.entries[1].mistakes, 0);
    assert_eq!(ranked.completion.entries[2].nickname, "Fast Errors");
    let auth = PgAuthStore::new(pool.clone());
    auth.nickname(slow.account, Nickname::parse("Renamed Clean").unwrap())
        .await
        .unwrap();
    assert_eq!(
        read(&store, START + 60).await.time.entries[1].nickname,
        "Renamed Clean"
    );
    auth.erase_authorized(SessionAuthority {
        account: slow.account,
        hash: slow.hash,
        now: START + 60,
    })
    .await
    .unwrap();
    let ranked = read(&store, START + 60).await;
    assert_eq!(ranked.time.entries.len(), 1);
    assert_eq!(ranked.completion.entries.len(), 2);
    let security = Arc::new(BrowserSecurity::new("https://game.example", [124; 32]).unwrap());
    let router = daily_rankings_router(
        Arc::new(store),
        BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier)),
        Arc::new(Clock(AtomicI64::new(START + 60))),
        security,
    );
    let response = router
        .oneshot(
            Request::post("/api/v1/daily/rankings")
                .header("origin", "https://game.example")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"v":1,"date":"2026-10-07","seed_version":1}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(json, serde_json::to_value(ranked).unwrap());
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn indexed_ties_have_stable_positions_and_only_fifty_rows_with_a_more_marker() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let evidence = evidence();
    for index in 0..52 {
        timed(&store, &pool, &format!("Ranked {index}"), &evidence, 9000).await;
    }
    let first = read(&store, START + 60).await;
    assert_eq!(first.time.entries.len(), 50);
    assert_eq!(first.completion.entries.len(), 50);
    assert!(first.time.has_more && first.completion.has_more);
    assert_eq!(first.time.entries[49].position, 50);
    assert_eq!(first, read(&store, START + 60).await);
    let indexes:i64=sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE schemaname=current_schema() AND indexname IN ('daily_completions_time_ranking','daily_completions_completion_ranking')").fetch_one(&pool).await.unwrap();
    assert_eq!(indexes, 2);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn public_retention_and_corrupt_storage_fail_closed_without_client_time_fallback() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    timed(&store, &pool, "Stored Player", &evidence(), 9000).await;
    clock.0.store(START + 60 + 90 * 86400 - 1, Ordering::SeqCst);
    assert_eq!(
        read(&store, START + 60 + 90 * 86400 - 1)
            .await
            .time
            .entries
            .len(),
        1
    );
    clock.0.fetch_add(1, Ordering::SeqCst);
    let expired = read(&store, START + 60 + 90 * 86400).await;
    assert!(expired.time.entries.is_empty() && expired.completion.entries.is_empty());
    clock.0.store(START + 60, Ordering::SeqCst);
    sqlx::raw_sql("ALTER TABLE daily_completions DISABLE TRIGGER daily_completion_immutable; UPDATE daily_completions SET opened_safe=1;").execute(&pool).await.unwrap();
    assert!(matches!(
        store.read("2026-10-07", 1).await,
        Err(DailyStoreError::Unavailable)
    ));
    assert!(matches!(
        store.read("bad", 1).await,
        Err(DailyStoreError::Invalid)
    ));
    clock.0.store(0, Ordering::SeqCst);
    assert!(matches!(
        store.read("2026-10-07", 1).await,
        Err(DailyStoreError::Unavailable)
    ));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires actual PostgreSQL; executed in database CI"]
async fn public_reader_pool_wait_is_bounded_and_read_only_never_creates_a_definition() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    assert!(store.read("2026-10-07", 1).await.unwrap().is_none());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM daily_definitions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(4), store.read("2026-10-07", 1))
            .await
            .unwrap(),
        Err(DailyStoreError::Unavailable)
    ));
    drop(held);
    close_auth_pool(pool).await;
}
