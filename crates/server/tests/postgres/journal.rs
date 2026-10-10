use super::online::{finished, participant};
use super::*;
use liar_protocol::{game::PublicEndReason, online::OnlineError};
use liar_server::{
    online::{ActiveMatch, AdmissionJournal, PgJournalRuntime, ResultRepository, SaveResult},
    results::{PgResultReader, ResultReader},
};
use std::sync::atomic::{AtomicI64, Ordering};
use std::{future::poll_fn, task::Poll};

const NOW: i64 = 1_800_000_000;
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn admission(result: &liar_server::online::FinishedMatch) -> ActiveMatch {
    ActiveMatch {
        id: result.id,
        rules_hash: result.rules_hash.clone(),
        players: result.players.clone().map(|p| p.account),
    }
}
async fn counts(pool: &sqlx::PgPool) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM online_active_matches),(SELECT count(*) FROM online_active_players),(SELECT count(*) FROM online_match_results)")
        .fetch_one(pool).await.unwrap()
}
async fn claim(pool: &sqlx::PgPool, clock: Arc<Clock>) -> PgJournalRuntime {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Ok(owner) = PgJournalRuntime::claim(pool.clone(), clock.clone()).await {
                break owner;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("The previous raw session must release ownership")
}
async fn seed_active(pool: &sqlx::PgPool, active: &ActiveMatch, at: i64) {
    // Historical active state only; public admission is integrated in a later issue.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO online_active_matches(id,rules_hash,owner_id,admitted_at) VALUES($1,$2,$3,to_timestamp($4::double precision))")
        .bind(active.id).bind(&active.rules_hash).bind(Uuid::new_v4()).bind(at).execute(&mut *tx).await.unwrap();
    for (seat, account) in active.players.iter().enumerate() {
        sqlx::query("INSERT INTO online_active_players(match_id,seat,account_id,is_bot) VALUES($1,$2,$3,$4)")
            .bind(active.id).bind(seat as i16).bind(account).bind(account.is_none()).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_schema_is_minimal_bounded_and_cascades_erasure() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let active = admission(&finished([Some(a), None]));
    seed_active(&pool, &active, NOW).await;
    for (table, expected) in [
        (
            "online_active_matches",
            vec!["admitted_at", "id", "owner_id", "rules_hash"],
        ),
        (
            "online_active_players",
            vec!["account_id", "is_bot", "match_id", "seat"],
        ),
    ] {
        let columns:Vec<String>=sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name=$1 ORDER BY column_name")
            .bind(table).fetch_all(&pool).await.unwrap();
        assert_eq!(columns, expected);
    }
    for statement in [
        "UPDATE online_active_matches SET owner_id='00000000-0000-0000-0000-000000000000'",
        "UPDATE online_active_matches SET rules_hash='invalid'",
        "UPDATE online_active_matches SET admitted_at='infinity'::timestamptz",
        "UPDATE online_active_matches SET admitted_at=to_timestamp(-1)",
        "UPDATE online_active_matches SET admitted_at=to_timestamp(253402300799::double precision)",
        "UPDATE online_active_matches SET admitted_at=to_timestamp(1800000000.5)",
        "UPDATE online_active_players SET is_bot=true WHERE seat=0",
        "UPDATE online_active_players SET seat=2 WHERE seat=0",
    ] {
        let error = sqlx::query(statement).execute(&pool).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (1, 1, 0));
    sqlx::query("DELETE FROM online_active_matches WHERE id=$1")
        .bind(active.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (0, 0, 0));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_registration_is_immutable_and_does_not_restore_erased_accounts() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = claim(&pool, clock.clone()).await;
    let active = admission(&finished([Some(a), None]));
    assert_eq!(owner.register(active.clone()).await, Ok(SaveResult::Saved));
    clock.0.store(NOW + 5, Ordering::SeqCst);
    assert_eq!(
        owner.register(active.clone()).await,
        Ok(SaveResult::Duplicate)
    );
    let at: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM admitted_at)::bigint FROM online_active_matches WHERE id=$1",
    )
    .bind(active.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(at, NOW);
    let mut conflict = active.clone();
    conflict.rules_hash = "b".repeat(64);
    assert_eq!(owner.register(conflict).await, Err(OnlineError::Malformed));
    for players in [[None, None], [Some(a), Some(a)], [Some(Uuid::nil()), None]] {
        let mut invalid = active.clone();
        invalid.id = Uuid::new_v4();
        invalid.players = players;
        assert_eq!(owner.register(invalid).await, Err(OnlineError::Malformed));
    }
    let mut absent = active.clone();
    absent.id = Uuid::new_v4();
    absent.players = [Some(Uuid::new_v4()), None];
    assert_eq!(owner.register(absent).await, Err(OnlineError::Unauthorized));
    assert_eq!(counts(&pool).await, (1, 2, 0));
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        owner.register(active.clone()).await,
        Err(OnlineError::Unauthorized)
    );
    assert_eq!(counts(&pool).await, (1, 1, 0));
    assert_eq!(owner.discard(active.id).await, Ok(()));
    assert_eq!(owner.discard(active.id).await, Ok(()));
    assert_eq!(counts(&pool).await, (0, 0, 0));
    assert!(owner.health().available());
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_partial_registration_failure_rolls_back_and_loses_owner() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    sqlx::raw_sql("CREATE FUNCTION reject_journal_second() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.seat=1 THEN RAISE EXCEPTION 'injected second seat failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_journal_second BEFORE INSERT ON online_active_players FOR EACH ROW EXECUTE FUNCTION reject_journal_second();").execute(&pool).await.unwrap();
    let owner = claim(&pool, Arc::new(Clock(AtomicI64::new(NOW)))).await;
    assert_eq!(
        owner.register(admission(&finished([Some(a), None]))).await,
        Err(OnlineError::Unavailable)
    );
    tokio::time::timeout(Duration::from_secs(3), owner.health().failed())
        .await
        .expect("A failed registration must terminate its owner");
    assert_eq!(counts(&pool).await, (0, 0, 0));
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_final_requires_admission_and_atomically_preserves_survivors_and_time() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let a = participant(&store).await;
    let b = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = claim(&pool, clock.clone()).await;
    let unregistered = finished([Some(a), None]);
    assert_eq!(owner.save(unregistered).await, Err(OnlineError::Malformed));
    let result = finished([Some(a), Some(b)]);
    owner.register(admission(&result)).await.unwrap();
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    clock.0.store(NOW - 1, Ordering::SeqCst);
    assert_eq!(
        owner.save(result.clone()).await,
        Err(OnlineError::Malformed)
    );
    clock.0.store(NOW + 30, Ordering::SeqCst);
    assert_eq!(owner.save(result.clone()).await, Ok(SaveResult::Saved));
    let times=sqlx::query_as::<_,(i64,i64,i64)>("SELECT EXTRACT(EPOCH FROM recorded_at)::bigint,EXTRACT(EPOCH FROM retention_started_at)::bigint,EXTRACT(EPOCH FROM seed_expires_at)::bigint FROM online_match_results WHERE id=$1").bind(result.id).fetch_one(&pool).await.unwrap();
    assert_eq!(times, (NOW + 30, NOW, NOW + 604800));
    assert_eq!(counts(&pool).await, (0, 0, 1));
    let seats: Vec<i16> =
        sqlx::query_scalar("SELECT seat FROM online_match_players WHERE match_id=$1 ORDER BY seat")
            .bind(result.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(seats, vec![0]);
    clock.0.store(NOW + 60, Ordering::SeqCst);
    assert_eq!(owner.save(result.clone()).await, Ok(SaveResult::Duplicate));
    assert_eq!(
        owner.register(admission(&result)).await,
        Err(OnlineError::Malformed)
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(owner.save(result.clone()).await, Ok(SaveResult::Duplicate));
    let repeated=sqlx::query_as::<_,(i64,i64,i64)>("SELECT EXTRACT(EPOCH FROM recorded_at)::bigint,EXTRACT(EPOCH FROM retention_started_at)::bigint,EXTRACT(EPOCH FROM seed_expires_at)::bigint FROM online_match_results WHERE id=$1").bind(result.id).fetch_one(&pool).await.unwrap();
    assert_eq!(times, repeated);
    let reader = PgResultReader::new(pool.clone(), clock);
    assert!(reader.read(a, result.id).await.unwrap().is_none());
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_startup_aborts_orphans_without_secrets_or_restoring_erased_players() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let a = participant(&store).await;
    let erased = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = claim(&pool, clock.clone()).await;
    let known = finished([Some(a), None]);
    owner.register(admission(&known)).await.unwrap();
    owner.save(known.clone()).await.unwrap();
    let discarded = admission(&finished([Some(a), None]));
    owner.register(discarded.clone()).await.unwrap();
    owner.discard(discarded.id).await.unwrap();
    let orphan = admission(&finished([Some(a), Some(erased)]));
    owner.register(orphan.clone()).await.unwrap();
    let empty = admission(&finished([Some(erased), None]));
    owner.register(empty.clone()).await.unwrap();
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(erased)
        .execute(&pool)
        .await
        .unwrap();
    // Delete the bot too, representing a fully unlinked historical active record.
    sqlx::query("DELETE FROM online_active_players WHERE match_id=$1")
        .bind(empty.id)
        .execute(&pool)
        .await
        .unwrap();
    drop(owner);
    clock.0.store(NOW + 10, Ordering::SeqCst);
    let replacement = claim(&pool, clock.clone()).await;
    assert!(replacement.health().available());
    assert_eq!(counts(&pool).await, (0, 0, 3));
    let unknown = PgResultReader::new(pool.clone(), clock.clone())
        .read(a, orphan.id)
        .await
        .unwrap()
        .unwrap()
        .project(a, orphan.id)
        .unwrap();
    assert_eq!(unknown.result.reason, PublicEndReason::ServerFailure);
    assert!(unknown.end_elapsed_ms.is_none() && unknown.own.is_none() && !unknown.result.completed);
    let row=sqlx::query_as::<_,(Option<Vec<u8>>,i64,i64,i64)>("SELECT secret_seed,EXTRACT(EPOCH FROM recorded_at)::bigint,EXTRACT(EPOCH FROM retention_started_at)::bigint,EXTRACT(EPOCH FROM seed_expires_at)::bigint FROM online_match_results WHERE id=$1").bind(orphan.id).fetch_one(&pool).await.unwrap();
    assert_eq!(row, (None, NOW + 10, NOW, NOW + 604800));
    let survivors: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(empty.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(survivors, 0);
    assert_eq!(replacement.save(known).await, Ok(SaveResult::Duplicate));
    assert_eq!(
        replacement.save(finished([Some(a), None])).await,
        Err(OnlineError::Malformed)
    );
    clock.0.store(NOW + 7776000, Ordering::SeqCst);
    assert!(
        PgResultReader::new(pool.clone(), clock.clone())
            .read(a, orphan.id)
            .await
            .unwrap()
            .is_none()
    );
    drop(replacement);
    let final_owner = claim(&pool, clock).await;
    assert_eq!(counts(&pool).await, (0, 0, 3));
    drop(final_owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_recovery_is_batched_and_failure_does_not_return_healthy_or_half_results() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let active = admission(&finished([Some(a), None]));
    seed_active(&pool, &active, NOW + 1).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    assert!(matches!(
        PgJournalRuntime::claim(pool.clone(), clock.clone()).await,
        Err(OnlineError::Unavailable)
    ));
    assert_eq!(counts(&pool).await, (1, 2, 0));
    clock.0.store(NOW + 2, Ordering::SeqCst);
    sqlx::raw_sql("CREATE FUNCTION reject_recovery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected recovery failure'; END $$; CREATE TRIGGER reject_recovery BEFORE INSERT ON online_match_players FOR EACH ROW EXECUTE FUNCTION reject_recovery();").execute(&pool).await.unwrap();
    assert!(matches!(
        PgJournalRuntime::claim(pool.clone(), clock.clone()).await,
        Err(OnlineError::Unavailable)
    ));
    assert_eq!(counts(&pool).await, (1, 2, 0));
    sqlx::query("DROP TRIGGER reject_recovery ON online_match_players")
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..65 {
        seed_active(&pool, &admission(&finished([Some(a), None])), NOW).await;
    }
    let owner = claim(&pool, clock).await;
    assert_eq!(counts(&pool).await, (0, 0, 66));
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_recovery_rejects_over_limit_before_writing_any_result() {
    let pool = auth_pool().await;
    sqlx::query("INSERT INTO online_active_matches(id,rules_hash,owner_id,admitted_at) SELECT gen_random_uuid(),repeat('a',64),gen_random_uuid(),to_timestamp($1::double precision) FROM generate_series(1,10001)").bind(NOW).execute(&pool).await.unwrap();
    assert!(matches!(
        PgJournalRuntime::claim(pool.clone(), Arc::new(Clock(AtomicI64::new(NOW)))).await,
        Err(OnlineError::Unavailable)
    ));
    assert_eq!(counts(&pool).await, (10001, 0, 0));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_register_caller_cancellation_after_sql_start_fails_owner_and_rolls_back() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let owner = claim(&pool, Arc::new(Clock(AtomicI64::new(NOW)))).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(a)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let other = owner.clone();
    let active = admission(&finished([Some(a), None]));
    let request = tokio::spawn(async move { other.register(active).await });
    let pid = super::result_owner::owner_pid(&pool).await;
    super::result_owner::wait_blocked(&pool, pid).await;
    request.abort();
    let _ = request.await;
    blocker.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), owner.health().failed())
        .await
        .unwrap();
    assert!(!owner.health().available());
    drop(owner);
    let replacement = claim(&pool, Arc::new(Clock(AtomicI64::new(NOW)))).await;
    assert_eq!(counts(&pool).await, (0, 0, 0));
    assert!(replacement.health().available());
    drop(replacement);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_final_failure_rolls_back_before_startup_abort_and_seed_age_is_bounded() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let a = participant(&store).await;
    let b = participant(&store).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = claim(&pool, clock.clone()).await;
    let result = finished([Some(a), Some(b)]);
    owner.register(admission(&result)).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_final_second() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.seat=1 THEN RAISE EXCEPTION 'injected final failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_final_second BEFORE INSERT ON online_match_players FOR EACH ROW EXECUTE FUNCTION reject_final_second();").execute(&pool).await.unwrap();
    assert_eq!(owner.save(result).await, Err(OnlineError::Unavailable));
    tokio::time::timeout(Duration::from_secs(3), owner.health().failed())
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (1, 2, 0));
    sqlx::query("DROP TRIGGER reject_final_second ON online_match_players")
        .execute(&pool)
        .await
        .unwrap();
    drop(owner);
    let owner = claim(&pool, clock.clone()).await;
    assert_eq!(counts(&pool).await, (0, 0, 1));
    let mut cases = Vec::new();
    for age in [604799, 604800, 604801] {
        let result = finished([Some(a), None]);
        owner.register(admission(&result)).await.unwrap();
        cases.push((age, result));
    }
    for (age, result) in cases {
        clock.0.store(NOW + age, Ordering::SeqCst);
        assert_eq!(owner.save(result.clone()).await, Ok(SaveResult::Saved));
        let seed: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT secret_seed FROM online_match_results WHERE id=$1")
                .bind(result.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(seed.is_some(), age < 604800);
        let expiry:i64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM seed_expires_at)::bigint FROM online_match_results WHERE id=$1").bind(result.id).fetch_one(&pool).await.unwrap();
        assert_eq!(expiry, NOW + 604800);
        assert_eq!(owner.save(result).await, Ok(SaveResult::Duplicate));
    }
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_corrupt_owner_or_final_collision_cannot_delete_or_overwrite_records() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let owner = claim(&pool, clock.clone()).await;
    assert!(
        liar_server::online::PgResultRuntime::claim(pool.clone(), clock.clone())
            .await
            .is_err()
    );
    let result = finished([Some(a), None]);
    let active = admission(&result);
    owner.register(active.clone()).await.unwrap();
    sqlx::query("UPDATE online_active_matches SET owner_id=$1 WHERE id=$2")
        .bind(Uuid::new_v4())
        .bind(active.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        owner.register(active.clone()).await,
        Err(OnlineError::Malformed)
    );
    assert_eq!(owner.discard(active.id).await, Err(OnlineError::Malformed));
    assert_eq!(owner.save(result).await, Err(OnlineError::Malformed));
    assert_eq!(counts(&pool).await, (1, 2, 0));
    assert!(owner.health().available());
    drop(owner);
    let owner = claim(&pool, clock.clone()).await;
    assert_eq!(counts(&pool).await, (0, 0, 1));
    drop(owner);
    // Coexisting final/active is corrupt state; startup must preserve both and fail closed.
    seed_active(&pool, &active, NOW).await;
    assert!(matches!(
        PgJournalRuntime::claim(pool.clone(), clock).await,
        Err(OnlineError::Unavailable)
    ));
    assert_eq!(counts(&pool).await, (1, 2, 1));
    let reason: String = sqlx::query_scalar("SELECT reason FROM online_match_results WHERE id=$1")
        .bind(active.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason, "server_failure");
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_queued_registration_cancellation_skips_sql_without_failing_owner() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let owner = claim(&pool, Arc::new(Clock(AtomicI64::new(NOW)))).await;
    let result = finished([Some(a), None]);
    owner.register(admission(&result)).await.unwrap();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(a)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let other = owner.clone();
    let saving = tokio::spawn(async move { other.save(result).await });
    let pid = super::result_owner::owner_pid(&pool).await;
    super::result_owner::wait_blocked(&pool, pid).await;
    let mut queued = owner.register(admission(&finished([Some(a), None])));
    let polled = poll_fn(|context| Poll::Ready(queued.as_mut().poll(context))).await;
    assert!(polled.is_pending());
    drop(queued);
    blocker.commit().await.unwrap();
    assert_eq!(saving.await.unwrap(), Ok(SaveResult::Saved));
    owner.discard(Uuid::new_v4()).await.unwrap();
    assert_eq!(counts(&pool).await, (0, 0, 1));
    assert!(owner.health().available());
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_recovery_waiting_for_erasure_uses_only_surviving_participants() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let active = admission(&finished([Some(a), None]));
    seed_active(&pool, &active, NOW).await;
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(a)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let other = pool.clone();
    let recovering = tokio::spawn(async move {
        PgJournalRuntime::claim(other, Arc::new(Clock(AtomicI64::new(NOW)))).await
    });
    let pid=tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let pid:Option<i32>=sqlx::query_scalar("SELECT pid FROM pg_locks WHERE locktype='advisory' AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND classid=1280530258::oid AND objid='online_match_results'::regclass::oid AND objsubid=1 AND granted").fetch_optional(&pool).await.unwrap();
            if let Some(pid)=pid {break pid;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    super::result_owner::wait_blocked(&pool, pid).await;
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a)
        .execute(&mut *blocker)
        .await
        .unwrap();
    blocker.commit().await.unwrap();
    let owner = recovering.await.unwrap().unwrap();
    assert_eq!(counts(&pool).await, (0, 0, 1));
    let human: i64 =
        sqlx::query_scalar("SELECT count(*) FROM online_match_players WHERE NOT is_bot")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(human, 0);
    assert!(owner.health().available());
    drop(owner);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_unobserved_committed_or_duplicate_ack_cannot_leave_a_healthy_orphan() {
    let pool = auth_pool().await;
    let a = participant(&PgAuthStore::new(pool.clone())).await;
    let clock = Arc::new(Clock(AtomicI64::new(NOW)));
    let mut owner = claim(&pool, clock.clone()).await;
    for duplicate in [false, true] {
        let active = admission(&finished([Some(a), None]));
        if duplicate {
            owner.register(active.clone()).await.unwrap();
        }
        let mut unobserved = owner.register(active);
        assert!(
            poll_fn(|context| Poll::Ready(unobserved.as_mut().poll(context)))
                .await
                .is_pending()
        );
        // A later command on the same worker proves that register committed and replied.
        owner.discard(Uuid::new_v4()).await.unwrap();
        assert_eq!(
            counts(&pool).await,
            if duplicate { (1, 2, 1) } else { (1, 2, 0) }
        );
        drop(unobserved);
        tokio::time::timeout(Duration::from_secs(3), owner.health().failed())
            .await
            .unwrap();
        assert!(!owner.health().available());
        drop(owner);
        owner = claim(&pool, clock.clone()).await;
        assert_eq!(
            counts(&pool).await,
            if duplicate { (0, 0, 2) } else { (0, 0, 1) }
        );
    }
    drop(owner);
    close_auth_pool(pool).await;
}
