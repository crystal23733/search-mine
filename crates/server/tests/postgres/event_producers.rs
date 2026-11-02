use super::*;
use liar_protocol::online::OnlineError;
use liar_server::daily::*;
use liar_server::online::{ActiveMatch, AdmissionJournal, PgJournalRuntime};
use liar_server::online::{PgResultRepository, ResultRepository, SaveResult};
use std::sync::atomic::{AtomicI64, Ordering};

struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
async fn event_rows(pool: &sqlx::PgPool) -> Vec<(Uuid, Uuid, i64, i64)> {
    sqlx::query_as(
        "SELECT id,account_id,occurred_at,received_at FROM product_events ORDER BY account_id,id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}
async fn reject_events(pool: &sqlx::PgPool) {
    sqlx::raw_sql("CREATE FUNCTION reject_product_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected event persistence failure'; END $$; CREATE TRIGGER reject_product_event BEFORE INSERT ON product_events FOR EACH ROW EXECUTE FUNCTION reject_product_event();")
        .execute(pool).await.unwrap();
}
async fn allow_events(pool: &sqlx::PgPool) {
    sqlx::query("DROP TRIGGER reject_product_event ON product_events")
        .execute(pool)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn event_failure_rolls_back_both_humans_and_the_result_before_a_successful_retry() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let b = super::personal::participant(&pool, 1000).await;
    let result = super::online::finished([Some(a.account), Some(b.account)]);
    // Fail after the first fact was inserted, proving the partial batch also rolls back.
    sqlx::raw_sql("CREATE FUNCTION reject_second_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF (SELECT count(*) FROM product_events)>0 THEN RAISE EXCEPTION 'injected second event failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_second_event BEFORE INSERT ON product_events FOR EACH ROW EXECUTE FUNCTION reject_second_event();").execute(&pool).await.unwrap();
    let repo = PgResultRepository::new(pool.clone(), Arc::new(Clock(1001)));
    assert_eq!(
        repo.save(result.clone()).await,
        Err(OnlineError::Unavailable)
    );
    let counts: (i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM online_match_results),(SELECT count(*) FROM online_match_players),(SELECT count(*) FROM product_events)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
    sqlx::query("DROP TRIGGER reject_second_event ON product_events")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(repo.save(result).await, Ok(SaveResult::Saved));
    assert_eq!(event_rows(&pool).await.len(), 2);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn opposite_seats_and_shared_matches_serialize_before_fk_locks_without_deadlock() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let b = super::personal::participant(&pool, 1000).await;
    let repo = PgResultRepository::new(pool.clone(), Arc::new(Clock(1001)));
    let one = super::online::finished([Some(a.account), Some(b.account)]);
    let two = super::online::finished([Some(b.account), Some(a.account)]);
    let (x, y, z) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(repo.save(one.clone()), repo.save(two), repo.save(one))
    })
    .await
    .expect("Account order must not deadlock opposing seats");
    assert_eq!(y, Ok(SaveResult::Saved));
    assert!(matches!(
        (x, z),
        (Ok(SaveResult::Saved), Ok(SaveResult::Duplicate))
            | (Ok(SaveResult::Duplicate), Ok(SaveResult::Saved))
    ));
    assert_eq!(event_rows(&pool).await.len(), 4);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn result_retries_after_raw_expiry_and_account_erasure_never_recreate_events() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let b = super::personal::participant(&pool, 1000).await;
    let result = super::online::finished([Some(a.account), Some(b.account)]);
    let repo = PgResultRepository::new(pool.clone(), Arc::new(Clock(1001)));
    assert_eq!(repo.save(result.clone()).await, Ok(SaveResult::Saved));
    assert_eq!(event_rows(&pool).await.len(), 2);
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    let expired = PgResultRepository::new(pool.clone(), Arc::new(Clock(1001 + 2_592_000)));
    assert_eq!(
        expired.save(result.clone()).await,
        Ok(SaveResult::Duplicate)
    );
    assert!(event_rows(&pool).await.is_empty());
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(a.account)
        .execute(&pool)
        .await
        .unwrap();
    let another = super::online::finished([Some(a.account), Some(b.account)]);
    assert_eq!(repo.save(another).await, Ok(SaveResult::Saved));
    assert_eq!(
        event_rows(&pool)
            .await
            .iter()
            .map(|r| r.1)
            .collect::<Vec<_>>(),
        vec![b.account]
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(b.account)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(repo.save(result).await, Ok(SaveResult::Duplicate));
    let empty = super::online::finished([Some(a.account), None]);
    assert_eq!(repo.save(empty).await, Ok(SaveResult::Saved));
    assert!(event_rows(&pool).await.is_empty());
    close_auth_pool(pool).await;
}
struct MovingClock(AtomicI64);
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn daily_first_and_verified_time_collect_automatically_but_retries_and_practice_do_not() {
    let pool = auth_pool().await;
    let at = 1_791_331_260;
    let session = super::daily::participant(&pool, at).await;
    let clock = Arc::new(MovingClock(AtomicI64::new(at)));
    let repo = PgDailyRepository::new(pool.clone(), clock.clone());
    repo.publish(super::daily::definition()).await.unwrap();
    let evidence = super::daily::evidence();
    let attempt = Uuid::new_v4();
    let DailySubmission::Official(first) = repo.submit(session, attempt, &evidence).await.unwrap()
    else {
        panic!()
    };
    let original = event_rows(&pool).await;
    assert_eq!(original.len(), 1);
    assert_eq!(
        (original[0].1, original[0].2, original[0].3),
        (session.account, at, at)
    );
    let trust: String = sqlx::query_scalar("SELECT trust FROM product_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(trust, "verified_completion");
    clock.0.store(at + 60, Ordering::SeqCst);
    assert_eq!(
        repo.submit(session, attempt, &evidence).await.unwrap(),
        DailySubmission::Official(first.clone())
    );
    assert_eq!(
        repo.submit(session, Uuid::new_v4(), &evidence)
            .await
            .unwrap(),
        DailySubmission::Practice(first.clone())
    );
    assert_eq!(event_rows(&pool).await, original);
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    // The original daily remains valid for 90 days, independently from raw's 30 days.
    let later = at + 2_592_000;
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp($2::double precision),authenticated_at=to_timestamp($2::double precision),expires_at=to_timestamp($2::double precision)+interval '30 days' WHERE account_id=$1").bind(session.account).bind(later).execute(&pool).await.unwrap();
    clock.0.store(later, Ordering::SeqCst);
    assert_eq!(
        repo.submit(session, attempt, &evidence).await.unwrap(),
        DailySubmission::Official(first)
    );
    assert!(event_rows(&pool).await.is_empty());
    let second = super::daily::participant(&pool, at).await;
    clock.0.store(at, Ordering::SeqCst);
    let second_attempt = Uuid::new_v4();
    let time = super::daily::timing::sealed(second, second_attempt, &evidence, 4000);
    let DailySubmission::Official(saved) = repo
        .submit_timed(second, second_attempt, &evidence, &time)
        .await
        .unwrap()
    else {
        panic!()
    };
    let row: (Uuid, String, i64) =
        sqlx::query_as("SELECT daily_completion_id,trust,occurred_at FROM product_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, (saved.id, "verified_time".into(), at));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn daily_event_failure_rolls_back_completion_and_replay_then_retry_is_official() {
    let pool = auth_pool().await;
    let at = 1_791_331_260;
    let session = super::daily::participant(&pool, at).await;
    let repo = PgDailyRepository::new(pool.clone(), Arc::new(Clock(at)));
    repo.publish(super::daily::definition()).await.unwrap();
    let evidence = super::daily::evidence();
    let attempt = Uuid::new_v4();
    reject_events(&pool).await;
    assert_eq!(
        repo.submit(session, attempt, &evidence).await,
        Err(DailyStoreError::Unavailable)
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM daily_completions),(SELECT count(*) FROM product_events)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
    allow_events(&pool).await;
    assert!(matches!(
        repo.submit(session, attempt, &evidence).await,
        Ok(DailySubmission::Official(_))
    ));
    assert_eq!(event_rows(&pool).await.len(), 1);
    close_auth_pool(pool).await;
}
impl AuthClock for MovingClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
async fn claim(
    pool: &sqlx::PgPool,
    clock: Arc<dyn AuthClock>,
) -> Result<PgJournalRuntime, OnlineError> {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            match PgJournalRuntime::claim(pool.clone(), clock.clone()).await {
                Err(OnlineError::Unavailable) => {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                value => break value,
            }
        }
    })
    .await
    .unwrap_or(Err(OnlineError::Unavailable))
}
fn admission(result: &liar_server::online::FinishedMatch) -> ActiveMatch {
    ActiveMatch {
        id: result.id,
        rules_hash: result.rules_hash.clone(),
        players: result.players.clone().map(|p| p.account),
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_event_failure_rolls_back_finish_and_startup_recovery_until_atomic_success() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let b = super::personal::participant(&pool, 1000).await;
    let clock = Arc::new(Clock(1001));
    let owner = claim(&pool, clock.clone()).await.unwrap();
    let result = super::online::finished([Some(a.account), Some(b.account)]);
    owner.register(admission(&result)).await.unwrap();
    reject_events(&pool).await;
    assert_eq!(
        owner.save(result.clone()).await,
        Err(OnlineError::Unavailable)
    );
    drop(owner);
    let counts: (i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM online_active_matches),(SELECT count(*) FROM online_match_results),(SELECT count(*) FROM product_events)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 0, 0));
    assert!(claim(&pool, clock.clone()).await.is_err());
    let counts: (i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM online_active_matches),(SELECT count(*) FROM online_match_results),(SELECT count(*) FROM product_events)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 0, 0));
    allow_events(&pool).await;
    let replacement = claim(&pool, clock.clone()).await.unwrap();
    let facts: Vec<(String, String)> =
        sqlx::query_as("SELECT end_reason,opponent_type FROM product_events ORDER BY account_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(facts, vec![("server_failure".into(), "human".into()); 2]);
    let accepted = event_rows(&pool).await;
    drop(replacement);
    let replacement = claim(&pool, clock).await.unwrap();
    assert_eq!(event_rows(&pool).await, accepted);
    drop(replacement);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn original_ninety_day_journal_expiry_closes_finish_and_recovery_without_new_raw() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let clock = Arc::new(MovingClock(AtomicI64::new(1001)));
    let owner = claim(&pool, clock.clone()).await.unwrap();
    let finish = super::online::finished([Some(a.account), None]);
    let orphan = super::online::finished([Some(a.account), None]);
    owner.register(admission(&finish)).await.unwrap();
    owner.register(admission(&orphan)).await.unwrap();
    clock.0.store(1001 + 7_776_000, Ordering::SeqCst);
    assert_eq!(owner.save(finish).await, Ok(SaveResult::Saved));
    assert!(event_rows(&pool).await.is_empty());
    drop(owner);
    let replacement = claim(&pool, clock).await.unwrap();
    assert!(event_rows(&pool).await.is_empty());
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM online_active_matches),(SELECT count(*) FROM online_match_results)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 2));
    drop(replacement);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn direct_results_automatically_commit_one_essential_fact_per_live_human() {
    let pool = auth_pool().await;
    let a = super::personal::participant(&pool, 1000).await;
    let b = super::personal::participant(&pool, 1000).await;
    let result = super::online::finished([Some(a.account), Some(b.account)]);
    let repo = PgResultRepository::new(pool.clone(), Arc::new(Clock(1001)));
    assert_eq!(repo.save(result.clone()).await, Ok(SaveResult::Saved));
    let rows: Vec<(Uuid, Uuid, i64, i64, String, String)> = sqlx::query_as("SELECT id,account_id,occurred_at,received_at,opponent_type,end_reason FROM product_events WHERE online_match_id=$1 ORDER BY account_id")
        .bind(result.id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        rows.len(),
        2,
        "The result ACK requires both automatically collected facts"
    );
    for row in &rows {
        assert_eq!(
            (row.2, row.3, row.4.as_str(), row.5.as_str()),
            (1001, 1001, "human", "timeout")
        );
    }
    assert_ne!(rows[0].0, rows[1].0);
    assert_eq!(repo.save(result.clone()).await, Ok(SaveResult::Duplicate));
    let retry: Vec<(Uuid, Uuid, i64, i64, String, String)> = sqlx::query_as("SELECT id,account_id,occurred_at,received_at,opponent_type,end_reason FROM product_events WHERE online_match_id=$1 ORDER BY account_id")
        .bind(result.id).fetch_all(&pool).await.unwrap();
    assert_eq!(rows, retry);
    close_auth_pool(pool).await;
}
