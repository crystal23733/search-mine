use super::*;
use liar_server::events::*;
use std::sync::atomic::{AtomicUsize, Ordering};
pub(super) struct Clock(pub(super) i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
fn id(value: Uuid) -> EventId {
    EventId::parse(&value.to_string()).unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn expired_raw_is_physically_deleted_at_exact_thirty_days_without_deleting_source() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    let game = Uuid::new_v4();
    super::personal::seed_online(&pool, owner.account, game, 1000).await;
    PgEventStore::new(pool.clone(), Arc::new(Clock(1001)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Online {
                account: id(owner.account),
                match_id: id(game),
            },
        )
        .await
        .unwrap();
    let before = PgEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS - 1)));
    assert_eq!(
        before.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 0,
            has_more: false
        })
    );
    let now = 1000 + RAW_EVENT_SECONDS;
    super::personal::fresh(&pool, owner, now).await;
    assert_eq!(
        PgEventStore::new(pool.clone(), Arc::new(Clock(now)))
            .capture(
                id(Uuid::new_v4()),
                EventSource::Online {
                    account: id(owner.account),
                    match_id: id(game)
                }
            )
            .await,
        Err(EventStoreError::Expired)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM product_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "Logical expiry is immediate before physical cleanup"
    );
    let retention = PgEventRetention::new(pool.clone(), Arc::new(Clock(now)));
    let result = retention.reap(Duration::from_secs(2)).await;
    assert_eq!(
        result,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        }),
        "Expired raw must be physically removed while its source survives"
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM product_events),(SELECT count(*) FROM online_match_results)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 1));
    let reader = liar_server::me::PgMeReader::new(pool.clone(), Arc::new(Clock(now)));
    let read = liar_server::me::MeReader::read(&reader, owner, liar_server::me::MeQuery::Overview)
        .await
        .unwrap();
    let liar_server::me::MePayload::Overview(view) = read.payload else {
        panic!("overview")
    };
    assert_eq!(view.online.played, 1);
    assert_eq!(
        PgEventStore::new(pool.clone(), Arc::new(Clock(now)))
            .capture(
                id(Uuid::new_v4()),
                EventSource::Online {
                    account: id(owner.account),
                    match_id: id(game)
                }
            )
            .await,
        Err(EventStoreError::Expired)
    );
    let timeout: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        timeout, "0",
        "Transaction timeout must not leak into the shared pool"
    );
    close_auth_pool(pool).await;
}
async fn seed(
    pool: &sqlx::PgPool,
    account: Uuid,
    count: usize,
    at: i64,
    received: i64,
) -> Vec<Uuid> {
    let ids: Vec<_> = (0..count).map(|_| Uuid::new_v4()).collect();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,reason,end_elapsed_ms,recorded_at,seed_expires_at) SELECT id,$2,'timeout',243000,to_timestamp($3::double precision),to_timestamp($3::double precision)+interval '7 days' FROM UNNEST($1::uuid[]) AS id")
        .bind(&ids).bind(liar_core::rules::RulesSnapshot::bundled().hash).bind(at).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) SELECT id,0,$2,false,'draw',4,0,0,0 FROM UNNEST($1::uuid[]) AS id")
        .bind(&ids).bind(account).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) SELECT id,1,NULL,true,'draw',4,0,0,0 FROM UNNEST($1::uuid[]) AS id")
        .bind(&ids).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) SELECT id,$2,$3,$4,id,'bot','timeout' FROM UNNEST($1::uuid[]) AS id")
        .bind(&ids).bind(account).bind(at).bind(received).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    ids
}
async fn count(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM product_events")
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn each_batch_stops_at_five_hundred_and_preserves_unexpired_and_future_rows() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    seed(
        &pool,
        owner.account,
        501,
        1000,
        1000 + RAW_EVENT_SECONDS - 1,
    )
    .await;
    seed(&pool, owner.account, 1, 1001, 1001).await;
    seed(
        &pool,
        owner.account,
        1,
        1000 + RAW_EVENT_SECONDS + 1,
        1000 + RAW_EVENT_SECONDS + 1,
    )
    .await;
    let store = PgEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS)));
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 500,
            has_more: true
        })
    );
    assert_eq!(count(&pool).await, 3);
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    assert_eq!(count(&pool).await, 2);
    for budget in [Duration::ZERO, Duration::from_secs(3)] {
        assert_eq!(store.reap(budget).await, Err(EventStoreError::Invalid));
    }
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn locked_rows_are_skipped_and_concurrent_workers_never_double_count() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    let ids = seed(&pool, owner.account, 501, 1000, 1000).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM product_events WHERE id=$1 FOR UPDATE")
        .bind(ids[0])
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let store = PgEventRetention::new(pool.clone(), Arc::new(Clock(1000 + RAW_EVENT_SECONDS)));
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
    seed(&pool, owner.account, 501, 1000, 1000).await;
    let (a, b) = tokio::join!(
        store.reap(Duration::from_secs(2)),
        store.reap(Duration::from_secs(2))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(u32::from(a.deleted) + u32::from(b.deleted), 501);
    assert!(a.deleted <= 500 && b.deleted <= 500);
    assert_eq!(count(&pool).await, 0);
    close_auth_pool(pool).await;
}
struct Sequence {
    values: Vec<i64>,
    calls: AtomicUsize,
}
impl AuthClock for Sequence {
    fn now(&self) -> i64 {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        self.values[n.min(self.values.len() - 1)]
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn clock_rollback_and_delete_failure_roll_back_the_whole_batch() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    seed(&pool, owner.account, 2, 1000, 1000).await;
    let now = 1000 + RAW_EVENT_SECONDS;
    for values in [
        vec![-1],
        vec![253402300800],
        vec![now, now - 1],
        vec![now, now, now - 1],
        vec![now, now, now, now - 1],
    ] {
        let store = PgEventRetention::new(
            pool.clone(),
            Arc::new(Sequence {
                values,
                calls: AtomicUsize::new(0),
            }),
        );
        assert_eq!(
            store.reap(Duration::from_secs(2)).await,
            Err(EventStoreError::Invalid)
        );
        assert_eq!(count(&pool).await, 2);
    }
    sqlx::raw_sql("CREATE TABLE delete_attempts(n integer); INSERT INTO delete_attempts VALUES(0); CREATE FUNCTION reject_raw_delete() RETURNS trigger LANGUAGE plpgsql AS $$ DECLARE attempt integer; BEGIN UPDATE delete_attempts SET n=n+1 RETURNING n INTO attempt; IF attempt=2 THEN RAISE EXCEPTION 'injected second delete failure'; END IF; RETURN OLD; END $$; CREATE TRIGGER reject_raw_delete BEFORE DELETE ON product_events FOR EACH ROW EXECUTE FUNCTION reject_raw_delete();").execute(&pool).await.unwrap();
    let store = PgEventRetention::new(pool.clone(), Arc::new(Clock(now)));
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Err(EventStoreError::Unavailable)
    );
    assert_eq!(count(&pool).await, 2);
    sqlx::query("DROP TRIGGER reject_raw_delete ON product_events")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 2,
            has_more: false
        })
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn pool_sql_deadline_and_cancellation_leave_no_partial_delete_or_session_timeout() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    seed(&pool, owner.account, 1, 1000, 1000).await;
    let now = 1000 + RAW_EVENT_SECONDS;
    let (single, _) = super::events::single_pool(&pool).await;
    let store = Arc::new(PgEventRetention::new(single.clone(), Arc::new(Clock(now))));
    let acquired = single.acquire().await.unwrap();
    let started = std::time::Instant::now();
    assert_eq!(
        store.reap(Duration::from_millis(150)).await,
        Err(EventStoreError::Unavailable)
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(acquired);
    let mut held = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE product_events IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&single)
        .await
        .unwrap();
    let running = {
        let store = store.clone();
        tokio::spawn(async move { store.reap(Duration::from_secs(2)).await })
    };
    super::events::blocked(&pool, pid).await;
    assert_eq!(running.await.unwrap(), Err(EventStoreError::Unavailable));
    held.rollback().await.unwrap();
    assert_eq!(count(&pool).await, 1);
    let timeout: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(timeout, "0");
    let mut held = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE product_events IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&single)
        .await
        .unwrap();
    let running = {
        let store = store.clone();
        tokio::spawn(async move { store.reap(Duration::from_secs(2)).await })
    };
    super::events::blocked(&pool, pid).await;
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    held.rollback().await.unwrap();
    assert_eq!(count(&pool).await, 1);
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    single.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn actual_main_startup_physically_reaps_expired_raw_and_keeps_source_and_rights() {
    use super::result_owner::{Server, client, server_command, wait_ready};
    let pool = auth_pool().await;
    let now = SystemAuthClock.now();
    let owner = super::personal::participant(&pool, now - RAW_EVENT_SECONDS).await;
    seed(
        &pool,
        owner.account,
        1,
        now - RAW_EVENT_SECONDS,
        now - RAW_EVENT_SECONDS + 1,
    )
    .await;
    seed(
        &pool,
        owner.account,
        1,
        now - RAW_EVENT_SECONDS + 60,
        now - RAW_EVENT_SECONDS + 60,
    )
    .await;
    super::personal::fresh(&pool, owner, now).await;
    let (mut command, origin) = server_command(&pool).await;
    let mut server = Server(command.spawn().unwrap());
    wait_ready(&mut server, &origin).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while count(&pool).await != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Immediate background cycle must physically remove only the expired raw");
    assert_eq!(
        client()
            .get(format!("{origin}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let source: i64 = sqlx::query_scalar("SELECT count(*) FROM online_match_results")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source, 2);
    super::personal::fresh(&pool, owner, SystemAuthClock.now()).await;
    let identities = PgAuthStore::new(pool.clone())
        .identities(SessionAuthority {
            account: owner.account,
            hash: owner.hash,
            now: SystemAuthClock.now(),
        })
        .await
        .unwrap();
    assert_eq!(identities.len(), 1);
    let reader = liar_server::me::PgMeReader::new(pool.clone(), Arc::new(SystemAuthClock));
    let view = liar_server::me::MeReader::read(&reader, owner, liar_server::me::MeQuery::Overview)
        .await
        .unwrap();
    let liar_server::me::MePayload::Overview(view) = view.payload else {
        panic!("overview")
    };
    assert_eq!(view.online.played, 2);
    drop(server);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn account_cascade_and_cleanup_skip_each_others_locked_rows_without_restoration() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    let other = super::personal::participant(&pool, 1000).await;
    let owned = seed(&pool, owner.account, 1, 1000, 1000).await;
    seed(&pool, other.account, 1, 1000, 1000).await;
    let mut erasure = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(owner.account)
        .execute(&mut *erasure)
        .await
        .unwrap();
    let now = 1000 + RAW_EVENT_SECONDS;
    let store = PgEventRetention::new(pool.clone(), Arc::new(Clock(now)));
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: true
        })
    );
    erasure.commit().await.unwrap();
    assert_eq!(count(&pool).await, 0);
    assert_eq!(
        store.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 0,
            has_more: false
        })
    );
    assert_eq!(
        PgEventStore::new(pool.clone(), Arc::new(Clock(now)))
            .capture(
                id(Uuid::new_v4()),
                EventSource::Online {
                    account: id(owner.account),
                    match_id: id(owned[0])
                }
            )
            .await,
        Err(EventStoreError::Missing)
    );
    close_auth_pool(pool).await;
}
