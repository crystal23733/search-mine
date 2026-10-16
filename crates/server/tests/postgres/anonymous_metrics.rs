use super::*;
use liar_server::events::*;
struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn authoritative_snapshot_is_persisted_as_anonymous_scalars_with_original_expiry() {
    let pool = auth_pool().await;
    let week = UtcDay::parse("2026-10-05").unwrap();
    let at = week.start();
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
        .initialize()
        .await
        .unwrap();
    let store = PgAggregateStore::new(pool.clone(), Arc::new(Clock(at + 8 * 86400)));
    let saved = store
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await;
    assert!(
        saved.is_ok(),
        "A complete authoritative snapshot must be persisted: {saved:?}"
    );
    let saved = saved.unwrap();
    assert_eq!(saved.expires_at, aggregate_expires_at(week).unwrap());
    assert_eq!(
        store.read(MetricKind::WeeklyNsm, week).await.unwrap(),
        Some(saved)
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn exact_thirteen_calendar_months_is_logically_and_physically_expired_without_extension() {
    let pool = auth_pool().await;
    let cohort = UtcDay::parse("2026-01-31").unwrap();
    let at = cohort.start();
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
        .initialize()
        .await
        .unwrap();
    let store = PgAggregateStore::new(pool.clone(), Arc::new(Clock(at + 8 * 86400)));
    let original = store
        .refresh(MetricKind::D7, cohort, Duration::from_secs(2))
        .await
        .unwrap();
    let refreshed = PgAggregateStore::new(pool.clone(), Arc::new(Clock(at + 9 * 86400)))
        .refresh(MetricKind::D7, cohort, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(original.expires_at, refreshed.expires_at);
    let expiry = UtcDay::parse("2027-02-28").unwrap().start();
    assert_eq!(expiry, original.expires_at);
    assert!(
        PgAggregateStore::new(pool.clone(), Arc::new(Clock(expiry - 1)))
            .read(MetricKind::D7, cohort)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        PgAggregateStore::new(pool.clone(), Arc::new(Clock(expiry)))
            .read(MetricKind::D7, cohort)
            .await,
        Err(EventError::Expired.into())
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM product_metric_aggregates")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    let reaper = PgAggregateRetention::new(pool.clone(), Arc::new(Clock(expiry)));
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        }),
        "The original calendar expiry must physically remove the row"
    );
    close_auth_pool(pool).await;
}

async fn cli(pool: &sqlx::PgPool, args: &[&str]) -> std::process::Output {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut url = url::Url::parse(&env::var("DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_metrics"));
    command
        .env_clear()
        .env("DATABASE_URL", url.as_str())
        .args(args);
    for key in [
        "SystemRoot",
        "WINDIR",
        "PATH",
        "TEMP",
        "TMP",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn actual_operator_cli_reads_saved_reports_without_exposing_identifiers() {
    let pool = auth_pool().await;
    let now = SystemAuthClock.now();
    let week = aggregate_schedule(now, 0).unwrap()[0].1;
    PgMetricReader::new(pool.clone(), Arc::new(Clock(week.start())))
        .initialize()
        .await
        .unwrap();
    PgAggregateStore::new(pool.clone(), Arc::new(Clock(now)))
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    let date = serde_json::to_value(week)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let result = cli(&pool, &["read", "weekly_nsm", &date]).await;
    assert!(
        result.status.success(),
        "A local operator must be able to read a stored report"
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(json["status"], "available");
    assert_eq!(json["stale"], false);
    let text = String::from_utf8(result.stdout).unwrap();
    for secret in [
        "account_id",
        "match_id",
        "event_id",
        "seed",
        "nickname",
        "postgres://",
    ] {
        assert!(!text.contains(secret));
    }
    let missing = cli(&pool, &["read", "d7", &date]).await;
    assert!(missing.status.success());
    let value: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(value["status"], "missing");
    assert!(value.get("report").is_none());
    let refreshed = cli(&pool, &["refresh", "weekly_nsm", &date]).await;
    assert!(refreshed.status.success());
    let expired = cli(&pool, &["read", "d7", "2020-01-31"]).await;
    assert!(expired.status.success());
    let value: serde_json::Value = serde_json::from_slice(&expired.stdout).unwrap();
    assert_eq!(value["status"], "expired");
    assert!(value.get("report").is_none());
    let older = UtcDay::from_timestamp(now - 10 * 86400).unwrap();
    seed_reports(&pool, &[older]).await;
    let date = serde_json::to_value(older)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let stale = cli(&pool, &["read", "d7", &date]).await;
    assert!(stale.status.success());
    let value: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(value["stale"], true);
    assert!(value["d7_rate"].is_null());
    assert_eq!(value["report"]["snapshot"]["data"]["sample_size"], 0);
    let invalid = cli(&pool, &["read", "not_a_metric", &date]).await;
    assert!(!invalid.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&invalid.stderr).unwrap()["code"],
        "invalid_arguments"
    );
    close_auth_pool(pool).await;
}

fn day(value: &str) -> UtcDay {
    UtcDay::parse(value).unwrap()
}
fn store(pool: &sqlx::PgPool, at: i64) -> PgAggregateStore {
    PgAggregateStore::new(pool.clone(), Arc::new(Clock(at)))
}
async fn initialize(pool: &sqlx::PgPool, at: i64) {
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
        .initialize()
        .await
        .unwrap();
}
async fn normal(pool: &sqlx::PgPool, account: Uuid, at: i64) {
    let game = Uuid::new_v4();
    super::privacy::seed_online(pool, account, game, at).await;
    let id = |value: Uuid| EventId::parse(&value.to_string()).unwrap();
    PgEventStore::new(pool.clone(), Arc::new(Clock(at)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Online {
                account: id(account),
                match_id: id(game),
            },
        )
        .await
        .unwrap();
}
fn participants(report: &AggregateReport) -> u32 {
    let MetricSnapshot::WeeklyNsm(r) = &report.snapshot else {
        panic!()
    };
    r.participants
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn current_account_deletion_recalculates_counts_without_a_historical_personal_mapping() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    initialize(&pool, at).await;
    let owner = super::privacy::participant(&pool, at).await;
    normal(&pool, owner.account, at + 10).await;
    normal(&pool, owner.account, at + 20).await;
    normal(&pool, owner.account, at + 7 * 86400 + 10).await;
    let now = at + 8 * 86400;
    let first = store(&pool, now)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(participants(&first), 1);
    let cohort = store(&pool, now)
        .refresh(MetricKind::D7, week, Duration::from_secs(2))
        .await
        .unwrap();
    let MetricSnapshot::D7(data) = &cohort.snapshot else {
        panic!()
    };
    assert_eq!(
        (data.sample_size, data.retained, data.rate()),
        (1, Some(1), Some(1.0))
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(owner.account)
        .execute(&pool)
        .await
        .unwrap();
    let raw: i64 = sqlx::query_scalar("SELECT count(*) FROM product_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw, 0);
    assert_eq!(
        store(&pool, now)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap(),
        Some(first.clone())
    );
    assert_eq!(
        store(&pool, now)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(EventError::Conflict.into())
    );
    assert_eq!(
        store(&pool, now + 1)
            .read(MetricKind::D7, week)
            .await
            .unwrap(),
        Some(cohort)
    );
    let refreshed = store(&pool, now + 1)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(participants(&refreshed), 0);
    assert_eq!(refreshed.expires_at, first.expires_at);
    let refreshed = store(&pool, now + 1)
        .refresh(MetricKind::D7, week, Duration::from_secs(2))
        .await
        .unwrap();
    let MetricSnapshot::D7(data) = refreshed.snapshot else {
        panic!()
    };
    assert_eq!(
        (data.sample_size, data.retained, data.rate()),
        (0, Some(0), None)
    );
    let columns:Vec<String>=sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='product_metric_aggregates'").fetch_all(&pool).await.unwrap();
    assert_eq!(columns.len(), 12);
    for secret in [
        "account_id",
        "match_id",
        "event_id",
        "seed",
        "payload",
        "mapping",
    ] {
        assert!(!columns.iter().any(|s| s.contains(secret)));
    }
    close_auth_pool(pool).await;
}
struct Sequence {
    values: Vec<i64>,
    calls: std::sync::atomic::AtomicUsize,
}
impl AuthClock for Sequence {
    fn now(&self) -> i64 {
        let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.values[n.min(self.values.len() - 1)]
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn incomplete_clock_budget_and_sql_failure_never_replace_a_prior_complete_report() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    let now = at + 8 * 86400;
    initialize(&pool, at).await;
    let prior = store(&pool, now)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    for allowance in [Duration::ZERO, Duration::from_secs(3)] {
        assert_eq!(
            store(&pool, now + 1)
                .refresh(MetricKind::WeeklyNsm, week, allowance)
                .await,
            Err(EventError::Invalid.into())
        );
    }
    for values in [
        vec![-1],
        vec![253402300800],
        vec![now + 1, now],
        vec![now + 1, now + 1, now],
    ] {
        let bad = PgAggregateStore::new(
            pool.clone(),
            Arc::new(Sequence {
                values,
                calls: std::sync::atomic::AtomicUsize::new(0),
            }),
        );
        assert_eq!(
            bad.refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
                .await,
            Err(EventError::Invalid.into())
        );
        assert_eq!(
            store(&pool, now + 1)
                .read(MetricKind::WeeklyNsm, week)
                .await
                .unwrap(),
            Some(prior.clone())
        );
    }
    let crossing = PgAggregateStore::new(
        pool.clone(),
        Arc::new(Sequence {
            values: vec![
                at + RAW_EVENT_SECONDS - 1,
                at + RAW_EVENT_SECONDS - 1,
                at + RAW_EVENT_SECONDS,
            ],
            calls: std::sync::atomic::AtomicUsize::new(0),
        }),
    );
    assert_eq!(
        crossing
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(EventError::Incomplete.into())
    );
    assert_eq!(
        store(&pool, now + 1)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap(),
        Some(prior.clone())
    );
    sqlx::raw_sql("CREATE FUNCTION reject_metric_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected update failure'; END $$; CREATE TRIGGER reject_metric_write BEFORE UPDATE ON product_metric_aggregates FOR EACH ROW EXECUTE FUNCTION reject_metric_write();").execute(&pool).await.unwrap();
    assert_eq!(
        store(&pool, now + 1)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(MetricReadError::Unavailable)
    );
    sqlx::query("DROP TRIGGER reject_metric_write ON product_metric_aggregates")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE metric_collection SET coverage_since=$1")
        .bind(at + 1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store(&pool, now + 1)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(EventError::Incomplete.into())
    );
    assert_eq!(
        store(&pool, now + 1)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap(),
        Some(prior)
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn concurrent_equal_reports_retry_as_duplicates_and_original_keys_cannot_change() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    let now = at + 8 * 86400;
    initialize(&pool, at).await;
    let writer = store(&pool, now);
    let (a, b) = tokio::join!(
        writer.refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2)),
        writer.refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
    );
    assert!(a.is_ok() || b.is_ok());
    for result in [&a, &b] {
        assert!(
            result.is_ok()
                || matches!(
                    result,
                    Err(MetricReadError::Unavailable)
                        | Err(MetricReadError::Policy(EventError::Conflict))
                )
        );
    }
    let saved = writer
        .read(MetricKind::WeeklyNsm, week)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        writer
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await
            .unwrap(),
        saved
    );
    assert_eq!(
        store(&pool, now - 1)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(EventError::Conflict.into())
    );
    for sql in [
        "UPDATE product_metric_aggregates SET expires_at=expires_at+1",
        "UPDATE product_metric_aggregates SET period=period+1",
        "UPDATE product_metric_aggregates SET computed_at=computed_at-1",
        "UPDATE product_metric_aggregates SET observed_through=observed_through-1",
        "UPDATE product_metric_aggregates SET complete=false",
    ] {
        let error = sqlx::query(sql).execute(&pool).await.unwrap_err();
        assert_eq!(error.as_database_error().unwrap().code().unwrap(), "23514");
        assert!(
            error
                .as_database_error()
                .unwrap()
                .message()
                .contains("freshness cannot regress")
        );
    }
    close_auth_pool(pool).await;
}

async fn seed_reports(pool: &sqlx::PgPool, periods: &[UtcDay]) {
    let mut dates = Vec::new();
    let mut computed = Vec::new();
    let mut expires = Vec::new();
    for period in periods {
        let report = AggregateReport::new(
            MetricSnapshot::D7(D7Cohort {
                cohort: *period,
                return_day: UtcDay::from_timestamp(period.start() + 7 * 86400).unwrap(),
                sample_size: 0,
                retained: Some(0),
                mature: true,
            }),
            period.start() + 8 * 86400,
        )
        .unwrap();
        dates.push(
            serde_json::to_value(period)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
        );
        computed.push(report.computed_at);
        expires.push(report.expires_at);
    }
    sqlx::query("INSERT INTO product_metric_aggregates(metric,period,computed_at,observed_through,expires_at,complete,sample_size,retained) SELECT 'd7',x.period::date,x.computed,x.computed,x.expires,true,0,0 FROM unnest($1::text[],$2::bigint[],$3::bigint[]) x(period,computed,expires)")
        .bind(dates).bind(computed).bind(expires).execute(pool).await.unwrap();
}
async fn count_reports(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM product_metric_aggregates")
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn physical_batches_skip_locked_periods_and_concurrent_workers_never_double_delete() {
    let pool = auth_pool().await;
    let base = day("2020-01-01").start();
    let now = SystemAuthClock.now();
    let periods: Vec<_> = (0..501)
        .map(|n| UtcDay::from_timestamp(base + n * 86400).unwrap())
        .collect();
    seed_reports(&pool, &periods).await;
    let alive = UtcDay::from_timestamp(now - 10 * 86400).unwrap();
    seed_reports(&pool, &[alive]).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query(
        "SELECT metric FROM product_metric_aggregates WHERE period=$1::text::date FOR UPDATE",
    )
    .bind(
        serde_json::to_value(periods[500])
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .fetch_one(&mut *held)
    .await
    .unwrap();
    let reaper = PgAggregateRetention::new(pool.clone(), Arc::new(Clock(now)));
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 500,
            has_more: true
        })
    );
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 0,
            has_more: true
        })
    );
    held.rollback().await.unwrap();
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    assert_eq!(count_reports(&pool).await, 1);
    seed_reports(&pool, &periods).await;
    let (a, b) = tokio::join!(
        reaper.reap(Duration::from_secs(2)),
        reaper.reap(Duration::from_secs(2))
    );
    assert_eq!(
        u32::from(a.unwrap().deleted) + u32::from(b.unwrap().deleted),
        501
    );
    assert_eq!(count_reports(&pool).await, 1);
    for allowance in [Duration::ZERO, Duration::from_secs(3)] {
        assert_eq!(reaper.reap(allowance).await, Err(EventStoreError::Invalid));
    }
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn cleanup_clock_sql_failure_pool_deadline_and_cancellation_leave_no_partial_delete() {
    let pool = auth_pool().await;
    seed_reports(&pool, &[day("2020-01-01"), day("2020-01-02")]).await;
    let now = SystemAuthClock.now();
    for values in [
        vec![-1],
        vec![253402300800],
        vec![now, now - 1],
        vec![now, now, now - 1],
        vec![now, now, now, now - 1],
    ] {
        let reaper = PgAggregateRetention::new(
            pool.clone(),
            Arc::new(Sequence {
                values,
                calls: std::sync::atomic::AtomicUsize::new(0),
            }),
        );
        assert_eq!(
            reaper.reap(Duration::from_secs(2)).await,
            Err(EventStoreError::Invalid)
        );
        assert_eq!(count_reports(&pool).await, 2);
    }
    sqlx::raw_sql("CREATE TABLE metric_delete_attempts(n integer); INSERT INTO metric_delete_attempts VALUES(0); CREATE FUNCTION reject_metric_delete() RETURNS trigger LANGUAGE plpgsql AS $$ DECLARE attempt integer; BEGIN UPDATE metric_delete_attempts SET n=n+1 RETURNING n INTO attempt; IF attempt=2 THEN RAISE EXCEPTION 'injected delete failure'; END IF; RETURN OLD; END $$; CREATE TRIGGER reject_metric_delete BEFORE DELETE ON product_metric_aggregates FOR EACH ROW EXECUTE FUNCTION reject_metric_delete();").execute(&pool).await.unwrap();
    let reaper = PgAggregateRetention::new(pool.clone(), Arc::new(Clock(now)));
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Err(EventStoreError::Unavailable)
    );
    assert_eq!(count_reports(&pool).await, 2);
    sqlx::query("DROP TRIGGER reject_metric_delete ON product_metric_aggregates")
        .execute(&pool)
        .await
        .unwrap();
    let (single, _) = super::events::single_pool(&pool).await;
    let reaper = Arc::new(PgAggregateRetention::new(
        single.clone(),
        Arc::new(Clock(now)),
    ));
    let acquired = single.acquire().await.unwrap();
    assert_eq!(
        reaper.reap(Duration::from_millis(150)).await,
        Err(EventStoreError::Unavailable)
    );
    drop(acquired);
    for cancel in [false, true] {
        let mut held = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE product_metric_aggregates IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *held)
            .await
            .unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&single)
            .await
            .unwrap();
        let task = {
            let reaper = reaper.clone();
            tokio::spawn(async move { reaper.reap(Duration::from_secs(2)).await })
        };
        super::events::blocked(&pool, pid).await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert_eq!(task.await.unwrap(), Err(EventStoreError::Unavailable));
        }
        held.rollback().await.unwrap();
        assert_eq!(count_reports(&pool).await, 2);
        let setting: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&single)
            .await
            .unwrap();
        assert_eq!(setting, "0");
    }
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 2,
            has_more: false
        })
    );
    single.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capacity_rejection_keeps_the_previous_report_and_rr_deletion_preserves_one_snapshot() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    initialize(&pool, at).await;
    let prior = store(&pool, at + 1)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    let owner = super::privacy::participant(&pool, at).await;
    super::event_metrics::many_sources(&pool, owner.account, at + 2, "forfeit").await;
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) SELECT md5('event'||id::text)::uuid,$1,$2,$2,id,'human','forfeit' FROM online_match_results").bind(owner.account).bind(at+2).execute(&pool).await.unwrap();
    assert_eq!(
        store(&pool, at + 3)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(EventError::Capacity.into())
    );
    assert_eq!(
        store(&pool, at + 3)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap(),
        Some(prior)
    );
    close_auth_pool(pool).await;
    let pool = auth_pool().await;
    initialize(&pool, at).await;
    let owner = super::privacy::participant(&pool, at).await;
    normal(&pool, owner.account, at + 1).await;
    let (single, pid) = super::events::single_pool(&pool).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE online_match_results IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let task = {
        let writer = store(&single, at + 8 * 86400);
        tokio::spawn(async move {
            writer
                .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
                .await
        })
    };
    super::events::blocked(&pool, pid).await;
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(owner.account)
        .execute(&pool)
        .await
        .unwrap();
    held.rollback().await.unwrap();
    let before = task.await.unwrap().unwrap();
    assert_eq!(participants(&before), 1);
    let after = store(&pool, at + 8 * 86400 + 1)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(participants(&after), 0);
    assert_eq!(before.expires_at, after.expires_at);
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL role administration in isolated test database"]
async fn serving_role_needs_only_declared_metric_columns_and_cannot_change_period_or_expiry() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    let now = at + 8 * 86400;
    initialize(&pool, at).await;
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_metrics145_test_dml NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$").execute(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(isolated_schema_sql(
        "GRANT USAGE ON SCHEMA",
        &schema,
        "TO liar_metrics145_test_dml",
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("GRANT SELECT ON auth_accounts,online_match_results,online_match_players,product_events,metric_collection TO liar_metrics145_test_dml").execute(&pool).await.unwrap();
    sqlx::query(
        "GRANT SELECT,INSERT,DELETE ON product_metric_aggregates TO liar_metrics145_test_dml",
    )
    .execute(&pool)
    .await
    .unwrap();
    let url = env::var("DATABASE_URL").unwrap();
    let dml = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SET ROLE liar_metrics145_test_dml")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    assert!(
        store(&dml, now)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store(&dml, now)
            .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
            .await,
        Err(MetricReadError::Unavailable)
    );
    sqlx::query("GRANT UPDATE(computed_at,observed_through,complete,participants,repeat_accounts,human_participations,bot_participations,sample_size,retained) ON product_metric_aggregates TO liar_metrics145_test_dml").execute(&pool).await.unwrap();
    let saved = store(&dml, now)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(
        store(&dml, now)
            .read(MetricKind::WeeklyNsm, week)
            .await
            .unwrap(),
        Some(saved.clone())
    );
    for sql in [
        "UPDATE product_metric_aggregates SET expires_at=expires_at+1",
        "UPDATE product_metric_aggregates SET period=period+1",
        "UPDATE product_metric_aggregates SET metric='d7'",
        "CREATE TABLE forbidden_metric145(id integer)",
        "SELECT * FROM _sqlx_migrations",
    ] {
        let err = sqlx::query(sql).execute(&dml).await.unwrap_err();
        assert_eq!(err.as_database_error().unwrap().code().unwrap(), "42501");
    }
    let reaper = PgAggregateRetention::new(dml.clone(), Arc::new(Clock(saved.expires_at)));
    assert_eq!(
        reaper.reap(Duration::from_secs(2)).await,
        Ok(RetentionBatch {
            deleted: 1,
            has_more: false
        })
    );
    dml.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn actual_main_runs_anonymous_cleanup_and_materialization_after_collection_is_ready() {
    use super::result_owner::{Server, server_command, wait_ready};
    let pool = auth_pool().await;
    let now = SystemAuthClock.now();
    let week = aggregate_schedule(now, 0).unwrap()[0].1;
    initialize(&pool, week.start()).await;
    let alive = UtcDay::from_timestamp(now - 10 * 86400).unwrap();
    seed_reports(&pool, &[day("2020-01-31"), alive]).await;
    let (mut command, origin) = server_command(&pool).await;
    let mut server = Server(command.spawn().unwrap());
    wait_ready(&mut server, &origin).await;
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let expired: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM product_metric_aggregates WHERE expires_at<=$1",
            )
            .bind(now)
            .fetch_one(&pool)
            .await
            .unwrap();
            let report = store(&pool, SystemAuthClock.now())
                .read(MetricKind::WeeklyNsm, week)
                .await
                .unwrap();
            if expired == 0 && report.is_some() {
                break;
            }
            assert!(server.0.try_wait().unwrap().is_none());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Actual main must perform independent cleanup and materialization");
    assert_eq!(count_reports(&pool).await, 2);
    assert!(
        store(&pool, SystemAuthClock.now())
            .read(MetricKind::D7, alive)
            .await
            .unwrap()
            .is_some()
    );
    drop(server);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn a_concurrent_new_report_is_valid_when_the_final_clock_has_reached_its_timestamp() {
    let pool = auth_pool().await;
    let week = day("2026-10-05");
    let at = week.start();
    let now = at + 8 * 86400;
    initialize(&pool, at).await;
    let saved = store(&pool, now + 1)
        .refresh(MetricKind::WeeklyNsm, week, Duration::from_secs(2))
        .await
        .unwrap();
    let reader = PgAggregateStore::new(
        pool.clone(),
        Arc::new(Sequence {
            values: vec![now, now, now + 1],
            calls: std::sync::atomic::AtomicUsize::new(0),
        }),
    );
    assert_eq!(
        reader.read(MetricKind::WeeklyNsm, week).await,
        Ok(Some(saved.clone())),
        "The final IO clock proves the report is no longer in the future"
    );
    assert_eq!(
        store(&pool, now).read(MetricKind::WeeklyNsm, week).await,
        Err(EventError::Invalid.into())
    );
    let expired = PgAggregateStore::new(
        pool.clone(),
        Arc::new(Sequence {
            values: vec![saved.expires_at - 1, saved.expires_at - 1, saved.expires_at],
            calls: std::sync::atomic::AtomicUsize::new(0),
        }),
    );
    assert_eq!(
        expired.read(MetricKind::WeeklyNsm, week).await,
        Err(EventError::Expired.into())
    );
    close_auth_pool(pool).await;
}
