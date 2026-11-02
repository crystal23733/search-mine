use super::*;
use liar_protocol::me::*;
use liar_server::{
    me::{MeSession, PgMeReader},
    online::{AuthorityRegistry, MatchClock},
};
use std::sync::atomic::AtomicU64;

struct Monotonic(AtomicU64);
impl MatchClock for Monotonic {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
pub(crate) fn sealed(
    session: DailySession,
    attempt: Uuid,
    evidence: &ReplayEvidence,
    elapsed: u32,
) -> VerifiedDailyTime {
    let authority = AuthorityRegistry::new(1).unwrap();
    let clock = Arc::new(Monotonic(AtomicU64::new(0)));
    let registry = DailyTicketRegistry::new(1, authority.clone(), clock.clone()).unwrap();
    let lease = authority
        .bind_shared(
            authority.generation().unwrap(),
            session.account,
            session.hash,
            START + 1000,
            START + 60,
        )
        .unwrap();
    let revision = Uuid::new_v4();
    let ticket = registry
        .start(&lease, revision, attempt, &definition(), START + 60)
        .unwrap();
    let finish = u64::from(definition().rules().rules.countdown_ms) + u64::from(elapsed);
    for (sequence, now) in (1..).zip((15000..finish).step_by(15000)) {
        clock.0.store(now, Ordering::SeqCst);
        registry
            .heartbeat(&lease, revision, ticket.id, sequence, START + 60)
            .unwrap()
            .unwrap();
    }
    clock.0.store(finish, Ordering::SeqCst);
    registry
        .finish(&lease, revision, ticket.id, attempt, evidence, START + 60)
        .unwrap()
        .unwrap()
}
async fn me_session(pool: &sqlx::PgPool, session: DailySession) -> MeSession {
    let revision =
        sqlx::query_scalar("SELECT id FROM auth_sessions WHERE account_id=$1 AND token_hash=$2")
            .bind(session.account)
            .bind(session.hash.as_slice())
            .fetch_one(pool)
            .await
            .unwrap();
    MeSession {
        account: session.account,
        hash: session.hash,
        revision,
    }
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn sealed_first_time_is_read_and_never_improved_by_retry_or_practice() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let evidence = evidence();
    let time = sealed(session, attempt, &evidence, 9000);
    let first = official(
        store
            .submit_timed(session, attempt, &evidence, &time)
            .await
            .unwrap(),
    );
    assert_eq!(first.verified_elapsed_ms, Some(9000));
    assert_eq!(first.replayed_elapsed_ms, 3200);
    assert_eq!(
        store.submit(session, attempt, &evidence).await.unwrap(),
        DailySubmission::Official(first.clone())
    );
    let faster = sealed(session, attempt, &evidence, 4000);
    assert_eq!(
        store
            .submit_timed(session, attempt, &evidence, &faster)
            .await
            .unwrap(),
        DailySubmission::Official(first.clone())
    );
    let practice = Uuid::new_v4();
    let practice_time = sealed(session, practice, &evidence, 3200);
    assert_eq!(
        store
            .submit_timed(session, practice, &evidence, &practice_time)
            .await
            .unwrap(),
        DailySubmission::Practice(first.clone())
    );
    assert_eq!(
        store.own(session, definition().metadata()).await.unwrap(),
        Some(first.clone())
    );
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    let owner = me_session(&pool, session).await;
    let exported = crate::personal::daily_history(&reader, owner)
        .await
        .unwrap();
    assert!(matches!(
        exported.items[0],
        MeRecord::Daily(MeDailyRecord {
            verified_elapsed_ms: Some(9000),
            ..
        })
    ));
    assert_eq!(
        serde_json::to_value(&exported).unwrap()["items"][0]["entry"]["verified_elapsed_ms"],
        9000
    );
    let other = participant(&pool, START + 60).await;
    assert!(
        crate::personal::daily_history(&reader, me_session(&pool, other).await)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    for query in [
        "UPDATE daily_completions SET verified_elapsed_ms=4000",
        "UPDATE daily_completions SET verified_elapsed_ms=NULL",
    ] {
        let error = sqlx::query(query).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    clock.0.store(START + 60 + 604800, Ordering::SeqCst);
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp($1::double precision),authenticated_at=to_timestamp($1::double precision),expires_at=to_timestamp($2::double precision) WHERE account_id=$3")
        .bind(START+60+604800).bind(START+60+604860).bind(session.account).execute(&pool).await.unwrap();
    assert_eq!(
        crate::personal::daily_history(&reader, owner)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        store
            .own(session, definition().metadata())
            .await
            .unwrap()
            .unwrap()
            .verified_elapsed_ms,
        Some(9000)
    );
    clock.0.store(START + 60 + 7776000, Ordering::SeqCst);
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp($1::double precision),authenticated_at=to_timestamp($1::double precision),expires_at=to_timestamp($2::double precision) WHERE account_id=$3")
        .bind(START+60+7776000).bind(START+60+7776060).bind(session.account).execute(&pool).await.unwrap();
    assert!(
        crate::personal::daily_history(&reader, owner)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        store
            .own(session, definition().metadata())
            .await
            .unwrap()
            .is_none()
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn unbound_time_cannot_write_and_plain_first_completion_cannot_be_upgraded() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let other = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let evidence = evidence();
    let time = sealed(session, attempt, &evidence, 9000);
    for (who, id) in [(other, attempt), (session, Uuid::new_v4())] {
        assert_eq!(
            store.submit_timed(who, id, &evidence, &time).await,
            Err(DailyStoreError::Unavailable)
        );
    }
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let mut raw = fixture["daily"]["replay"].clone();
    raw["final_time_ms"] = serde_json::json!(u32::MAX);
    let changed = NativeReplayVerifier
        .verify(&definition(), &raw.to_string())
        .unwrap();
    assert_eq!(
        store.submit_timed(session, attempt, &changed, &time).await,
        Err(DailyStoreError::Unavailable)
    );
    assert_eq!(count(&pool).await, 0);
    let first = official(store.submit(session, attempt, &evidence).await.unwrap());
    assert_eq!(first.verified_elapsed_ms, None);
    assert_eq!(
        store
            .submit_timed(session, attempt, &evidence, &time)
            .await
            .unwrap(),
        DailySubmission::Official(first.clone())
    );
    let later = Uuid::new_v4();
    let later_time = sealed(session, later, &evidence, 3200);
    assert_eq!(
        store
            .submit_timed(session, later, &evidence, &later_time)
            .await
            .unwrap(),
        DailySubmission::Practice(first)
    );
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn forward_migration_preserves_old_completion_as_explicit_null() {
    use sha2::{Digest, Sha256};
    let pool = auth_pool_through(Some(202610110001)).await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let replay: liar_protocol::daily::DailyReplay =
        serde_json::from_value(fixture["daily"]["replay"].clone()).unwrap();
    let canonical = serde_json::to_string(&replay).unwrap();
    sqlx::query("INSERT INTO daily_completions(id,account_id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at) VALUES($1,$2,$3,$4,$5,216,0,3200,to_timestamp($6::double precision),$7::text::jsonb,to_timestamp(($6+604800)::double precision),to_timestamp(($6+7776000)::double precision))")
        .bind(Uuid::new_v4()).bind(session.account).bind(definition().metadata().id()).bind(attempt).bind(Sha256::digest(canonical.as_bytes()).as_slice()).bind(START+60).bind(canonical).execute(&pool).await.unwrap();
    let migrations = Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap();
    migrations.run(&pool).await.unwrap();
    let original = store
        .own(session, definition().metadata())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.verified_elapsed_ms, None);
    let evidence = evidence();
    let time = sealed(session, attempt, &evidence, 4000);
    assert_eq!(
        store
            .submit_timed(session, attempt, &evidence, &time)
            .await
            .unwrap(),
        DailySubmission::Official(original)
    );
    let reader = PgMeReader::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    let page = crate::personal::daily_history(&reader, me_session(&pool, session).await)
        .await
        .unwrap();
    let json = serde_json::to_value(page).unwrap();
    assert!(
        json["items"][0]["entry"]
            .get("verified_elapsed_ms")
            .is_some_and(serde_json::Value::is_null)
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn timed_corruption_and_db_bounds_fail_closed_for_own_and_my_info() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let evidence = evidence();
    let attempt = Uuid::new_v4();
    let time = sealed(session, attempt, &evidence, 9000);
    let first = official(
        store
            .submit_timed(session, attempt, &evidence, &time)
            .await
            .unwrap(),
    );
    for bad in [-1i64, 4294967296] {
        let other = participant(&pool, START + 60).await;
        let err = sqlx::query("INSERT INTO daily_completions SELECT $1,$2,daily_id,$3,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at,$4 FROM daily_completions WHERE id=$5")
            .bind(Uuid::new_v4()).bind(other.account).bind(Uuid::new_v4()).bind(bad).bind(first.id).execute(&pool).await.unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    sqlx::query(
        "ALTER TABLE daily_completions DROP CONSTRAINT daily_completions_verified_elapsed_ms_check",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE daily_completions DISABLE TRIGGER daily_completion_immutable")
        .execute(&pool)
        .await
        .unwrap();
    let reader = PgMeReader::new(pool.clone(), clock.clone());
    let owner = me_session(&pool, session).await;
    for bad in [
        -1i64,
        3199,
        i64::from(definition().rules().rules.duration_ms),
        4294967296,
    ] {
        sqlx::query("UPDATE daily_completions SET verified_elapsed_ms=$1 WHERE id=$2")
            .bind(bad)
            .bind(first.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            store.own(session, definition().metadata()).await,
            Err(DailyStoreError::Unavailable)
        );
        assert_eq!(
            store.submit(session, attempt, &evidence).await,
            Err(DailyStoreError::Unavailable)
        );
        assert_eq!(
            crate::personal::daily_history(&reader, owner).await,
            Err(MeError::Unavailable)
        );
    }
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn concurrent_plain_and_timed_retry_share_one_immutable_first_receipt() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let evidence = evidence();
    let time = sealed(session, attempt, &evidence, 9000);
    let barrier = Arc::new(tokio::sync::Barrier::new(12));
    let mut tasks = vec![];
    for n in 0..12 {
        let store = store.clone();
        let evidence = evidence.clone();
        let time = time.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            if n % 2 == 0 {
                store.submit(session, attempt, &evidence).await
            } else {
                store.submit_timed(session, attempt, &evidence, &time).await
            }
        }));
    }
    let mut receipts = vec![];
    for task in tasks {
        receipts.push(official(task.await.unwrap().unwrap()));
    }
    assert!(receipts.iter().all(|receipt| receipt == &receipts[0]));
    assert!(matches!(receipts[0].verified_elapsed_ms, None | Some(9000)));
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn sealed_time_does_not_replace_current_session_auth_or_survive_account_erasure() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(pool.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let evidence = evidence();
    let time = sealed(session, attempt, &evidence, 4000);
    let wrong = DailySession {
        hash: [0; 32],
        ..session
    };
    assert_eq!(
        store.submit_timed(wrong, attempt, &evidence, &time).await,
        Err(DailyStoreError::Unauthorized)
    );
    assert_eq!(
        store
            .submit_timed(session, Uuid::nil(), &evidence, &time)
            .await,
        Err(DailyStoreError::Invalid)
    );
    store
        .submit_timed(session, attempt, &evidence, &time)
        .await
        .unwrap();
    PgAuthStore::new(pool.clone())
        .erase_authorized(SessionAuthority {
            account: session.account,
            hash: session.hash,
            now: START + 60,
        })
        .await
        .unwrap();
    assert_eq!(count(&pool).await, 0);
    assert_eq!(
        store.submit_timed(session, attempt, &evidence, &time).await,
        Err(DailyStoreError::Unauthorized)
    );
    close_auth_pool(pool).await;
}
