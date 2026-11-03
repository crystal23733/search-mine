use super::*;
use liar_server::events::*;
struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
async fn seed(pool: &sqlx::PgPool, count: usize, at: i64) -> liar_server::me::MeSession {
    let own = super::personal::participant(pool, at).await;
    sqlx::query("INSERT INTO optional_product_events(id,owner_session_id,tab_key,consent_revision,purpose,occurred_at,received_at,locale,action) SELECT gen_random_uuid(),$1,gen_random_uuid(),gen_random_uuid(),'analytics',$2,$2,'ko','{\"type\":\"invite_open\"}'::jsonb FROM generate_series(1,$3)").bind(own.revision).bind(at).bind(count as i32).execute(pool).await.unwrap();
    own
}
async fn count(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM optional_product_events")
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn exact_thirty_day_optional_expiry_is_physical_and_never_deletes_essential_sources() {
    let pool = auth_pool().await;
    seed(&pool, 1, 1000).await;
    let source = super::personal::participant(&pool, 1000).await;
    super::personal::seed_online(&pool, source.account, Uuid::new_v4(), 1000).await;
    assert_eq!(
        PgOptionalEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS - 1)))
            .reap(Duration::from_secs(2))
            .await,
        Ok(RetentionBatch {
            deleted: 0,
            has_more: false
        })
    );
    assert_eq!(
        PgOptionalEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS)))
            .reap(Duration::from_secs(2))
            .await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    assert_eq!(count(&pool).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_results")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let timeout: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(timeout, "0");
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn optional_retention_is_bounded_skips_locked_rows_and_honors_its_deadline() {
    let pool = auth_pool().await;
    seed(&pool, 501, 1000).await;
    let store =
        PgOptionalEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS)));
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM optional_product_events ORDER BY id LIMIT 1 FOR UPDATE")
        .fetch_one(&mut *held)
        .await
        .unwrap();
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 500,
            has_more: true
        })
    );
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 0,
            has_more: true
        })
    );
    held.rollback().await.unwrap();
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    assert_eq!(
        store.reap(Duration::ZERO).await,
        Err(EventStoreError::Invalid)
    );
    assert_eq!(
        store.reap(Duration::from_secs(3)).await,
        Err(EventStoreError::Invalid)
    );
    assert_eq!(count(&pool).await, 0);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires actual PostgreSQL18 and main; executed in database CI"]
async fn actual_main_owns_the_optional_retention_cycle_without_enabling_collection() {
    let pool = auth_pool().await;
    let now = SystemAuthClock.now();
    let old = seed(&pool, 1, now - RAW_EVENT_SECONDS).await;
    let live = seed(&pool, 1, now - RAW_EVENT_SECONDS + 60).await;
    // Keep authentication cleanup independent of the raw-retention fixture's age.
    super::personal::fresh(&pool, old, now).await;
    super::personal::fresh(&pool, live, now).await;
    let (mut server, origin) = super::main_observation::ObservedServer::spawn(&pool).await;
    super::result_owner::wait_ready(&mut server, &origin).await;
    tokio::time::timeout(Duration::from_secs(4),async {
        loop {
            let complete=server.diagnostics().lines().any(|line|serde_json::from_str::<serde_json::Value>(line).is_ok_and(|v|v["component"]=="optional_event_retention" && v["code"]=="complete" && v["deleted"]==1));
            if count(&pool).await==1 && complete {break;}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("Real main must run optional cleanup immediately with the existing bounded maintenance pool");
    assert!(
        PgAuthStore::new(pool.clone())
            .session(old.hash, SystemAuthClock.now())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM product_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    drop(server);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn optional_retention_rolls_back_clock_changes_deadlines_and_caller_cancellation() {
    use std::sync::atomic::{AtomicI64, Ordering};
    struct Moving(AtomicI64);
    impl AuthClock for Moving {
        fn now(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    let pool = auth_pool().await;
    seed(&pool, 1, 1000).await;
    for change in 0..3 {
        let now = 1000 + RAW_EVENT_SECONDS;
        let clock = Arc::new(Moving(AtomicI64::new(now)));
        let (single, pid) = super::events::single_pool(&pool).await;
        let store = Arc::new(PgOptionalEventRetention::new(single.clone(), clock.clone()));
        let mut held = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE optional_product_events IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *held)
            .await
            .unwrap();
        let copy = store.clone();
        let task = tokio::spawn(async move { copy.reap(Duration::from_secs(2)).await });
        super::events::blocked(&pool, pid).await;
        match change {
            0 => {
                clock.0.store(now - 1, Ordering::SeqCst);
                held.rollback().await.unwrap();
                assert_eq!(task.await.unwrap(), Err(EventStoreError::Invalid));
            }
            1 => {
                assert_eq!(
                    tokio::time::timeout(Duration::from_secs(3), task)
                        .await
                        .unwrap()
                        .unwrap(),
                    Err(EventStoreError::Unavailable)
                );
                held.rollback().await.unwrap();
            }
            _ => {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
                held.rollback().await.unwrap();
            }
        }
        sqlx::query("SELECT 1").execute(&single).await.unwrap();
        assert_eq!(count(&pool).await, 1);
        let budget: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&single)
            .await
            .unwrap();
        assert_eq!(budget, "0");
        single.close().await;
    }
    assert_eq!(
        PgOptionalEventRetention::new(pool.clone(), Arc::new(Clock(-1)))
            .reap(Duration::from_secs(2))
            .await,
        Err(EventStoreError::Invalid)
    );
    let store =
        PgOptionalEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS)));
    assert_eq!(
        EventRetentionMaintenance::optional(Arc::new(store))
            .run_cycle()
            .await
            .unwrap()
            .deleted,
        1
    );
    assert_eq!(count(&pool).await, 0);
    close_auth_pool(pool).await;
}
