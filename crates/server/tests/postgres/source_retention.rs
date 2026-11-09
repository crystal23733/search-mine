use super::event_retention::Clock;
use super::*;
use liar_server::events::*;
const WEEK: i64 = 604800;
const RECORD: i64 = 7776000;

async fn reap(pool: &sqlx::PgPool, at: i64) -> Result<RetentionBatch, EventStoreError> {
    PgSourceRetention::new(pool.clone(), Arc::new(Clock(at)))
        .reap(Duration::from_secs(2))
        .await
}
async fn count(pool: &sqlx::PgPool, table: &str) -> i64 {
    let query = match table {
        "online" => "SELECT count(*) FROM online_match_results",
        "daily" => "SELECT count(*) FROM daily_completions",
        "definitions" => "SELECT count(*) FROM daily_definitions",
        "raw" => "SELECT count(*) FROM product_events",
        _ => panic!("Closed fixture table"),
    };
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn secrets_expire_at_exact_seven_days_then_records_and_unreferenced_definition_at_ninety() {
    let pool = auth_pool().await;
    let (owner, daily) = super::personal::completed(&pool).await;
    let at: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM completed_at)::bigint FROM daily_completions WHERE id=$1",
    )
    .bind(daily)
    .fetch_one(&pool)
    .await
    .unwrap();
    let online = Uuid::new_v4();
    super::personal::seed_online(&pool, owner.account, online, at).await;
    let before: String = sqlx::query_scalar(
        "SELECT (to_jsonb(d)-'replay_json')::text FROM daily_completions d WHERE id=$1",
    )
    .bind(daily)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reap(&pool, at + WEEK - 1).await.unwrap().deleted, 0);
    assert_eq!(
        reap(&pool, at + WEEK).await.unwrap(),
        RetentionBatch {
            deleted: 2,
            has_more: false
        }
    );
    let secrets: (bool,bool) = sqlx::query_as("SELECT (SELECT secret_seed IS NULL FROM online_match_results WHERE id=$1),(SELECT replay_json IS NULL FROM daily_completions WHERE id=$2)")
        .bind(online).bind(daily).fetch_one(&pool).await.unwrap();
    assert_eq!(secrets, (true, true));
    use liar_server::daily::*;
    let definition =
        PublishedDaily::new("2026-10-07", 1, liar_core::rules::RulesSnapshot::bundled()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap();
    let evidence = NativeReplayVerifier
        .verify(&definition, &fixture["daily"]["replay"].to_string())
        .unwrap();
    let attempt: Uuid = sqlx::query_scalar("SELECT attempt_id FROM daily_completions WHERE id=$1")
        .bind(daily)
        .fetch_one(&pool)
        .await
        .unwrap();
    PgAuthStore::new(pool.clone())
        .nickname(owner.account, Nickname::parse("Retry Player").unwrap())
        .await
        .unwrap();
    super::personal::fresh(&pool, owner, at + WEEK).await;
    let repo = PgDailyRepository::new(pool.clone(), Arc::new(Clock(at + WEEK)));
    assert!(matches!(
        repo.submit(
            DailySession {
                account: owner.account,
                hash: owner.hash
            },
            attempt,
            &evidence
        )
        .await,
        Ok(DailySubmission::Official(_))
    ));
    let cleared: bool =
        sqlx::query_scalar("SELECT replay_json IS NULL FROM daily_completions WHERE id=$1")
            .bind(daily)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        cleared,
        "A duplicate receipt must not restore an expired replay"
    );
    let after: String = sqlx::query_scalar(
        "SELECT (to_jsonb(d)-'replay_json')::text FROM daily_completions d WHERE id=$1",
    )
    .bind(daily)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        before, after,
        "Payload cleanup must preserve digest/receipt and original anchors"
    );
    assert_eq!(reap(&pool, at + RECORD - 1).await.unwrap().deleted, 0);
    assert_eq!(count(&pool, "definitions").await, 1);
    assert_eq!(
        reap(&pool, at + RECORD).await.unwrap(),
        RetentionBatch {
            deleted: 3,
            has_more: false
        }
    );
    for table in ["online", "daily", "definitions", "raw"] {
        assert_eq!(count(&pool, table).await, 0);
    }
    super::personal::fresh(&pool, owner, at + RECORD).await;
    let repo = PgDailyRepository::new(pool.clone(), Arc::new(Clock(at + RECORD)));
    assert!(
        repo.submit(
            DailySession {
                account: owner.account,
                hash: owner.hash
            },
            attempt,
            &evidence
        )
        .await
        .is_err()
    );
    assert!(repo.publish(definition).await.is_err());
    assert_eq!(count(&pool, "daily").await, 0);
    assert_eq!(count(&pool, "definitions").await, 0);
    let timeout: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(timeout, "0");
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn online_cleanup_uses_original_anchor_and_null_fallback_not_late_recorded_time() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, 1000).await;
    let now = 1000 + RECORD;
    for offset in [0, 1, 2] {
        let id = Uuid::new_v4();
        super::personal::seed_online(&pool, owner.account, id, 1000 + offset).await;
        sqlx::query("UPDATE online_match_results SET secret_seed=NULL,retention_started_at=CASE WHEN $2=2 THEN to_timestamp(1000) ELSE NULL END,recorded_at=CASE WHEN $2=2 THEN to_timestamp($3::double precision) ELSE recorded_at END WHERE id=$1")
            .bind(id).bind(offset).bind(now).execute(&pool).await.unwrap();
    }
    assert_eq!(
        reap(&pool, now).await.unwrap(),
        RetentionBatch {
            deleted: 2,
            has_more: false
        }
    );
    assert_eq!(count(&pool, "online").await, 1);
    close_auth_pool(pool).await;
}

async fn old_source_with_fresh_raw(pool: &sqlx::PgPool, now: i64) -> Uuid {
    let owner = super::personal::participant(pool, 1000).await;
    let id = Uuid::new_v4();
    super::personal::seed_online(pool, owner.account, id, now - RECORD).await;
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) VALUES($1,$2,$3,$3,$4,'bot','timeout')")
        .bind(Uuid::new_v4()).bind(owner.account).bind(now-60).bind(id).execute(pool).await.unwrap();
    id
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn source_cascade_advances_coverage_atomically_without_changing_collection_start_or_aggregate_expiry()
 {
    let pool = auth_pool().await;
    let now = 10_000_000;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    old_source_with_fresh_raw(&pool, now).await;
    let original_expiry = aggregate_expires_at(UtcDay::parse("1970-01-01").unwrap()).unwrap();
    sqlx::query("INSERT INTO product_metric_aggregates(metric,period,computed_at,observed_through,expires_at,complete,sample_size,retained) VALUES('d7','1970-01-01',691200,691200,$1,true,1,1)")
        .bind(original_expiry).execute(&pool).await.unwrap();
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 1);
    assert_eq!(count(&pool, "online").await, 0);
    assert_eq!(count(&pool, "raw").await, 0);
    let boundaries:(i64,i64,i64)=sqlx::query_as("SELECT started_at,coverage_since,(SELECT expires_at FROM product_metric_aggregates) FROM metric_collection")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(boundaries, (1000, now - 59, original_expiry));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn missing_collection_or_failed_cascade_rolls_back_parent_raw_and_coverage_together() {
    let pool = auth_pool().await;
    let now = 10_000_000;
    old_source_with_fresh_raw(&pool, now).await;
    assert_eq!(reap(&pool, now).await, Err(EventStoreError::Unavailable));
    assert_eq!(
        (count(&pool, "online").await, count(&pool, "raw").await),
        (1, 1)
    );
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_source_cleanup() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture delete failure'; END; $$; CREATE TRIGGER fail_source BEFORE DELETE ON online_match_results FOR EACH ROW EXECUTE FUNCTION fail_source_cleanup();")
        .execute(&pool).await.unwrap();
    assert_eq!(reap(&pool, now).await, Err(EventStoreError::Unavailable));
    let coverage: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, 1000);
    assert_eq!(
        (count(&pool, "online").await, count(&pool, "raw").await),
        (1, 1)
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn cascade_wait_deadline_and_clock_reversal_rollback_before_a_later_success() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reversing(AtomicUsize, i64);
    impl AuthClock for Reversing {
        fn now(&self) -> i64 {
            self.1 - i64::from(self.0.fetch_add(1, Ordering::SeqCst) >= 6)
        }
    }
    let pool = auth_pool().await;
    let now = 10_000_000;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    old_source_with_fresh_raw(&pool, now).await;
    let reaper = PgSourceRetention::new(pool.clone(), Arc::new(Clock(now)));
    for budget in [Duration::ZERO, Duration::from_secs(3)] {
        assert_eq!(reaper.reap(budget).await, Err(EventStoreError::Invalid));
    }
    for invalid in [-1, 253402300800] {
        assert_eq!(reap(&pool, invalid).await, Err(EventStoreError::Invalid));
    }
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM product_events FOR UPDATE")
        .fetch_all(&mut *held)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    assert_eq!(
        reaper.reap(Duration::from_millis(100)).await,
        Err(EventStoreError::Unavailable)
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "A locked cascade must respect the caller's actual deadline"
    );
    held.rollback().await.unwrap();
    assert_eq!(
        PgSourceRetention::new(pool.clone(), Arc::new(Reversing(AtomicUsize::new(0), now)))
            .reap(Duration::from_secs(2))
            .await,
        Err(EventStoreError::Invalid)
    );
    assert_eq!(
        (count(&pool, "online").await, count(&pool, "raw").await),
        (1, 1)
    );
    let coverage: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, 1000);
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 1);
    let lock: String = sqlx::query_scalar("SELECT current_setting('lock_timeout')")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lock, "0");
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn fresh_fk_insert_locks_parent_until_commit_then_cleanup_invalidates_the_committed_raw() {
    let pool = auth_pool().await;
    let now = 10_000_000;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    let game = old_source_with_fresh_raw(&pool, now).await;
    let account: Uuid =
        sqlx::query_scalar("SELECT account_id FROM product_events WHERE online_match_id=$1")
            .bind(game)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    let mut insert = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) VALUES($1,$2,$3,$3,$4,'bot','timeout')")
        .bind(Uuid::new_v4()).bind(account).bind(now-60).bind(game).execute(&mut *insert).await.unwrap();
    assert_eq!(
        reap(&pool, now).await.unwrap(),
        RetentionBatch {
            deleted: 0,
            has_more: true
        }
    );
    insert.commit().await.unwrap();
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 1);
    let coverage: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, now - 59);
    assert_eq!(count(&pool, "raw").await, 0);
    let week = UtcDay::parse("1970-04-20").unwrap();
    assert_eq!(
        PgMetricReader::new(pool.clone(), Arc::new(Clock(now)))
            .weekly(week)
            .await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    close_auth_pool(pool).await;
}

async fn many(pool: &sqlx::PgPool, n: i32) {
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at) SELECT gen_random_uuid(),repeat('a',64),NULL,243000,'timeout',to_timestamp(1000),to_timestamp(605800) FROM generate_series(1,$1)")
        .bind(n).execute(pool).await.unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn bounded_batches_skip_locked_candidates_and_maintenance_stops_at_eight_batches() {
    let pool = auth_pool().await;
    let now = 1000 + RECORD;
    many(&pool, 1001).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM online_match_results ORDER BY id LIMIT 1 FOR UPDATE")
        .fetch_all(&mut *held)
        .await
        .unwrap();
    assert_eq!(
        reap(&pool, now).await.unwrap(),
        RetentionBatch {
            deleted: 100,
            has_more: true
        }
    );
    let maintenance = EventRetentionMaintenance::source(Arc::new(PgSourceRetention::new(
        pool.clone(),
        Arc::new(Clock(now)),
    )));
    assert_eq!(
        maintenance.run_cycle().await.unwrap(),
        RetentionCycle {
            deleted: 800,
            has_more: true,
            batches: 8
        }
    );
    assert_eq!(count(&pool, "online").await, 101);
    held.rollback().await.unwrap();
    assert_eq!(
        maintenance.run_cycle().await.unwrap(),
        RetentionCycle {
            deleted: 101,
            has_more: false,
            batches: 2
        }
    );
    assert_eq!(count(&pool, "online").await, 0);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn journal_duplicate_cannot_restore_cleared_seed_or_recreate_a_removed_final() {
    use liar_server::online::{
        ActiveMatch, AdmissionJournal, PgJournalRuntime, ResultRepository, SaveResult,
    };
    use std::sync::atomic::{AtomicI64, Ordering};
    struct Moving(AtomicI64);
    impl AuthClock for Moving {
        fn now(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    let pool = auth_pool().await;
    let at = 18000;
    let clock = Arc::new(Moving(AtomicI64::new(at)));
    let account = super::online::participant(&PgAuthStore::new(pool.clone())).await;
    let runtime = PgJournalRuntime::claim(pool.clone(), clock.clone())
        .await
        .unwrap();
    let result = super::online::finished([Some(account), None]);
    runtime
        .register(ActiveMatch {
            id: result.id,
            rules_hash: result.rules_hash.clone(),
            players: [Some(account), None],
        })
        .await
        .unwrap();
    runtime.save(result.clone()).await.unwrap();
    clock.0.store(at + WEEK, Ordering::SeqCst);
    assert_eq!(reap(&pool, at + WEEK).await.unwrap().deleted, 1);
    assert_eq!(
        runtime.save(result.clone()).await,
        Ok(SaveResult::Duplicate)
    );
    let missing: bool =
        sqlx::query_scalar("SELECT secret_seed IS NULL FROM online_match_results WHERE id=$1")
            .bind(result.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(missing);
    clock.0.store(at + RECORD, Ordering::SeqCst);
    assert_eq!(reap(&pool, at + RECORD).await.unwrap().deleted, 1);
    assert!(runtime.save(result).await.is_err());
    assert_eq!(count(&pool, "online").await, 0);
    drop(runtime);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn real_main_runs_source_retention_from_the_existing_maintenance_pool() {
    let pool = auth_pool().await;
    many(&pool, 1).await;
    let (server, _) = super::main_observation::ObservedServer::spawn(&pool).await;
    tokio::time::timeout(Duration::from_secs(4), async {
        while count(&pool, "online").await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Real main must run the first bounded retention cycle");
    tokio::time::timeout(Duration::from_secs(1), async {
        while !server.diagnostics().contains("source_retention") {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Source cleanup must retain its bounded operational diagnostic");
    drop(server);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn daily_source_cascade_marks_fresh_loss_but_never_regresses_a_later_coverage_boundary() {
    let pool = auth_pool().await;
    let (owner, daily) = super::personal::completed(&pool).await;
    let at: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM completed_at)::bigint FROM daily_completions WHERE id=$1",
    )
    .bind(daily)
    .fetch_one(&pool)
    .await
    .unwrap();
    let now = at + RECORD;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
        .initialize()
        .await
        .unwrap();
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,daily_completion_id,utc_date,mistakes,trust) VALUES($1,$2,$3,$3,$4,'2026-10-07',0,'verified_completion')")
        .bind(Uuid::new_v4()).bind(owner.account).bind(now-60).bind(daily).execute(&pool).await.unwrap();
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 2);
    let boundary: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(boundary, now - 59);
    sqlx::query("UPDATE metric_collection SET coverage_since=$1")
        .bind(now + 1)
        .execute(&pool)
        .await
        .unwrap();
    old_source_with_fresh_raw(&pool, now).await;
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 1);
    let boundary: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(boundary, now + 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn expired_raw_does_not_invalidate_coverage_and_invalid_future_loss_cannot_commit() {
    let pool = auth_pool().await;
    let now = 10_000_000;
    let game = old_source_with_fresh_raw(&pool, now).await;
    let account: Uuid = sqlx::query_scalar("SELECT account_id FROM product_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) VALUES($1,$2,$3,$3,$4,'bot','timeout')")
        .bind(Uuid::new_v4()).bind(account).bind(now-RAW_EVENT_SECONDS).bind(game).execute(&pool).await.unwrap();
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    assert_eq!(reap(&pool, now).await.unwrap().deleted, 1);
    let coverage: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, 1000);
    let game = old_source_with_fresh_raw(&pool, now).await;
    sqlx::query("DELETE FROM product_events")
        .execute(&pool)
        .await
        .unwrap();
    let account: Uuid = sqlx::query_scalar(
        "SELECT account_id FROM online_match_players WHERE match_id=$1 AND account_id IS NOT NULL",
    )
    .bind(game)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) VALUES($1,$2,253402300799,253402300799,$3,'bot','timeout')")
        .bind(Uuid::new_v4()).bind(account).bind(game).execute(&pool).await.unwrap();
    assert_eq!(reap(&pool, now).await, Err(EventStoreError::Invalid));
    assert_eq!(
        (count(&pool, "online").await, count(&pool, "raw").await),
        (1, 1)
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn minimal_serving_dml_can_reap_without_ddl_or_retention_anchor_update_permissions() {
    let pool = auth_pool().await;
    let now = 10_000_000;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(1000)))
        .initialize()
        .await
        .unwrap();
    old_source_with_fresh_raw(&pool, now).await;
    let owner = super::personal::participant(&pool, 1000).await;
    super::personal::seed_online(&pool, owner.account, Uuid::new_v4(), now - WEEK).await;
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_source215_test_dml NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$")
        .execute(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(isolated_schema_sql(
        "GRANT USAGE ON SCHEMA",
        &schema,
        "TO liar_source215_test_dml",
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql("GRANT SELECT,DELETE ON online_match_results,daily_completions,daily_definitions TO liar_source215_test_dml;
GRANT UPDATE(secret_seed) ON online_match_results TO liar_source215_test_dml;
GRANT UPDATE(replay_json) ON daily_completions TO liar_source215_test_dml;
GRANT UPDATE(id) ON daily_definitions TO liar_source215_test_dml;
GRANT SELECT ON product_events,metric_collection TO liar_source215_test_dml;
GRANT UPDATE(coverage_since) ON metric_collection TO liar_source215_test_dml;")
        .execute(&pool).await.unwrap();
    let serving = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SET ROLE liar_source215_test_dml")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query(
                    "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL REPEATABLE READ",
                )
                .execute(&mut *conn)
                .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert_eq!(reap(&serving, now).await.unwrap().deleted, 2);
    assert_eq!(count(&pool, "online").await, 1);
    assert_eq!(count(&pool, "raw").await, 0);
    let coverage: i64 = sqlx::query_scalar("SELECT coverage_since FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, now - 59);
    for query in [
        "CREATE TABLE forbidden_source_write(id integer)",
        "UPDATE metric_collection SET started_at=started_at",
        "UPDATE online_match_results SET retention_started_at=retention_started_at",
        "UPDATE daily_completions SET record_expires_at=record_expires_at",
        "UPDATE product_events SET occurred_at=occurred_at",
    ] {
        assert!(
            sqlx::query(query).execute(&serving).await.is_err(),
            "Serving must not acquire schema/period/payload privileges"
        );
    }
    let default: String = sqlx::query_scalar("SHOW default_transaction_isolation")
        .fetch_one(&serving)
        .await
        .unwrap();
    assert_eq!(
        default, "repeatable read",
        "Reaper isolation must remain transaction local"
    );
    serving.close().await;
    close_auth_pool(pool).await;
}
