use super::*;
use liar_server::events::*;

struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
fn id(value: Uuid) -> EventId {
    EventId::parse(&value.to_string()).unwrap()
}
async fn normal(pool: &sqlx::PgPool, account: Uuid, at: i64) {
    let game = Uuid::new_v4();
    super::privacy::seed_online(pool, account, game, at).await;
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
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn authoritative_week_and_d7_use_the_same_normal_history() {
    let pool = auth_pool().await;
    let week = UtcDay::parse("2026-10-05").unwrap();
    let start = week.start();
    PgMetricReader::new(pool.clone(), Arc::new(Clock(start)))
        .initialize()
        .await
        .unwrap();
    let owner = super::privacy::participant(&pool, start).await;
    normal(&pool, owner.account, start + 10).await;
    normal(&pool, owner.account, start + 20).await;
    normal(&pool, owner.account, start + 7 * 86400 + 10).await;
    let reader = PgMetricReader::new(pool.clone(), Arc::new(Clock(start + 8 * 86400)));
    let report = reader.weekly(week).await;
    assert!(
        report.is_ok(),
        "Committed normal sources must yield a weekly snapshot: {report:?}"
    );
    let report = report.unwrap();
    assert_eq!(
        (
            report.participants,
            report.repeat_accounts,
            report.bot_participations,
            report.human_participations
        ),
        (1, 1, 2, 0)
    );
    assert!(report.complete);
    let report = reader.d7(week).await.unwrap();
    assert_eq!(
        (report.sample_size, report.retained, report.rate()),
        (1, Some(1), Some(1.0))
    );
    close_auth_pool(pool).await;
}

async fn initialized(pool: &sqlx::PgPool, at: i64) {
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
        .initialize()
        .await
        .unwrap();
}
fn reader(pool: &sqlx::PgPool, at: i64) -> PgMetricReader {
    PgMetricReader::new(pool.clone(), Arc::new(Clock(at)))
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn old_history_gap_is_incomplete_and_an_earlier_source_proves_nonmembership() {
    let pool = auth_pool().await;
    let cohort = UtcDay::parse("2026-10-05").unwrap();
    let at = cohort.start();
    initialized(&pool, at).await;
    let old = super::privacy::participant(&pool, at - 100 * 86400).await;
    normal(&pool, old.account, at + 10).await;
    let now = at + 8 * 86400;
    assert_eq!(
        reader(&pool, now).d7(cohort).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    // The old retained source is evidence of nonmembership even when its raw has expired.
    let prior = Uuid::new_v4();
    super::privacy::seed_online(&pool, old.account, prior, at - 40 * 86400).await;
    let report = reader(&pool, now).d7(cohort).await.unwrap();
    assert_eq!(
        (report.sample_size, report.retained, report.rate()),
        (0, Some(0), None)
    );
    sqlx::query("UPDATE online_match_results SET retention_started_at=to_timestamp($2::double precision) WHERE id=$1")
        .bind(prior).bind(now-7776000).execute(&pool).await.unwrap();
    assert_eq!(
        reader(&pool, now).d7(cohort).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn collection_and_original_raw_boundaries_are_strict_and_restart_does_not_extend() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    assert_eq!(
        reader(&pool, at).weekly(day).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    initialized(&pool, at + 1).await;
    assert_eq!(
        reader(&pool, at + 100).weekly(day).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    initialized(&pool, at + 100).await;
    let started: i64 = sqlx::query_scalar("SELECT started_at FROM metric_collection")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(started, at + 1);
    assert_eq!(
        reader(&pool, at).initialize().await,
        Err(MetricReadError::Policy(EventError::Invalid))
    );
    for sql in [
        "UPDATE metric_collection SET started_at=started_at+1",
        "UPDATE metric_collection SET coverage_since=coverage_since-1",
    ] {
        let err = sqlx::query(sql).execute(&pool).await.unwrap_err();
        assert_eq!(err.as_database_error().unwrap().code().unwrap(), "23514");
    }
    let next = UtcDay::parse("2026-10-12").unwrap();
    let start = next.start();
    assert!(!reader(&pool, start).weekly(next).await.unwrap().complete);
    assert!(
        reader(&pool, start + RAW_EVENT_SECONDS - 1)
            .weekly(next)
            .await
            .unwrap()
            .complete
    );
    assert_eq!(
        reader(&pool, start + RAW_EVENT_SECONDS).weekly(next).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    assert_eq!(
        reader(&pool, at).weekly(next).await,
        Err(MetricReadError::Policy(EventError::Invalid))
    );
    assert_eq!(
        reader(&pool, start)
            .weekly(UtcDay::parse("2026-10-13").unwrap())
            .await,
        Err(MetricReadError::Policy(EventError::Invalid))
    );
    sqlx::query("UPDATE metric_collection SET coverage_since=$1")
        .bind(start + 1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader(&pool, start + 2).d7(next).await,
        Err(MetricReadError::Policy(EventError::Incomplete))
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn only_normal_human_and_bot_games_count_and_d7_waits_for_the_entire_return_day() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    initialized(&pool, at).await;
    let a = super::privacy::participant(&pool, at).await;
    let b = super::privacy::participant(&pool, at).await;
    let game = Uuid::new_v4();
    super::privacy::seed_online(&pool, a.account, game, at + 1).await;
    sqlx::query(
        "UPDATE online_match_players SET account_id=$2,is_bot=false WHERE match_id=$1 AND seat=1",
    )
    .bind(game)
    .bind(b.account)
    .execute(&pool)
    .await
    .unwrap();
    for account in [a.account, b.account] {
        PgEventStore::new(pool.clone(), Arc::new(Clock(at + 1)))
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
    normal(&pool, a.account, at + 2).await;
    let aborted = Uuid::new_v4();
    super::privacy::seed_online(&pool, b.account, aborted, at + 3).await;
    sqlx::query("UPDATE online_match_results SET reason='forfeit' WHERE id=$1")
        .bind(aborted)
        .execute(&pool)
        .await
        .unwrap();
    PgEventStore::new(pool.clone(), Arc::new(Clock(at + 3)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Online {
                account: id(b.account),
                match_id: id(aborted),
            },
        )
        .await
        .unwrap();
    normal(&pool, a.account, at + 7 * 86400).await;
    let early = reader(&pool, at + 8 * 86400 - 1).d7(day).await.unwrap();
    assert_eq!(
        (
            early.sample_size,
            early.retained,
            early.rate(),
            early.mature
        ),
        (2, None, None, false)
    );
    normal(&pool, b.account, at + 8 * 86400).await; // The following day is not D7.
    let full = reader(&pool, at + 8 * 86400).d7(day).await.unwrap();
    assert_eq!(
        (full.sample_size, full.retained, full.rate(), full.mature),
        (2, Some(1), Some(0.5), true)
    );
    let week = reader(&pool, at + 8 * 86400).weekly(day).await.unwrap();
    assert_eq!(
        (
            week.participants,
            week.repeat_accounts,
            week.human_participations,
            week.bot_participations
        ),
        (2, 1, 2, 1)
    );
    let json = serde_json::to_string(&week).unwrap() + &serde_json::to_string(&full).unwrap();
    for secret in [
        a.account.to_string(),
        b.account.to_string(),
        game.to_string(),
        "seed".into(),
        "nickname".into(),
        "event_id".into(),
    ] {
        assert!(!json.contains(&secret));
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
fn sequence(pool: &sqlx::PgPool, values: Vec<i64>) -> PgMetricReader {
    PgMetricReader::new(
        pool.clone(),
        Arc::new(Sequence {
            values,
            calls: std::sync::atomic::AtomicUsize::new(0),
        }),
    )
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn clocks_and_corrupt_sources_are_never_turned_into_successful_zero() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    for values in [
        vec![-1],
        vec![253402300800],
        vec![at, at - 1],
        vec![at, at, at - 1],
    ] {
        assert_eq!(
            sequence(&pool, values).initialize().await,
            Err(MetricReadError::Policy(EventError::Invalid))
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM metric_collection")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    initialized(&pool, at).await;
    for values in [
        vec![-1],
        vec![253402300800],
        vec![at, at - 1],
        vec![at, at, at - 1],
    ] {
        assert_eq!(
            sequence(&pool, values).d7(day).await,
            Err(MetricReadError::Policy(EventError::Invalid))
        );
    }
    let owner = super::privacy::participant(&pool, at).await;
    normal(&pool, owner.account, at + 10).await;
    assert_eq!(
        reader(&pool, at + 9).weekly(day).await,
        Err(MetricReadError::Policy(EventError::Invalid))
    );
    sqlx::query("UPDATE online_match_results SET recorded_at=recorded_at+interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader(&pool, at + 20).d7(day).await,
        Err(MetricReadError::Policy(EventError::Conflict))
    );
    sqlx::query("UPDATE online_match_results SET recorded_at=recorded_at-interval '1 second',retention_started_at=to_timestamp($1::double precision)").bind(at-1).execute(&pool).await.unwrap();
    assert_eq!(
        reader(&pool, at + 20).weekly(day).await,
        Err(MetricReadError::Policy(EventError::Conflict))
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn repeatable_read_keeps_raw_and_first_proof_consistent_during_account_deletion() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    initialized(&pool, at).await;
    let owner = super::privacy::participant(&pool, at).await;
    normal(&pool, owner.account, at + 1).await;
    let (single, pid) = super::events::single_pool(&pool).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE online_match_results IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *held)
        .await
        .unwrap();
    let task = {
        let r = reader(&single, at + 8 * 86400);
        tokio::spawn(async move { r.d7(day).await })
    };
    super::events::blocked(&pool, pid).await;
    tokio::time::timeout(
        Duration::from_secs(1),
        sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
            .bind(owner.account)
            .execute(&pool),
    )
    .await
    .unwrap()
    .unwrap();
    held.rollback().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap().sample_size, 1);
    assert_eq!(
        reader(&pool, at + 8 * 86400)
            .d7(day)
            .await
            .unwrap()
            .sample_size,
        0
    );
    single.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn shared_pool_and_sql_deadline_and_cancellation_do_not_leak_transaction_settings() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    initialized(&pool, at).await;
    let (single, _) = super::events::single_pool(&pool).await;
    let r = Arc::new(reader(&single, at));
    let acquired = single.acquire().await.unwrap();
    let started = std::time::Instant::now();
    assert_eq!(r.weekly(day).await, Err(MetricReadError::Unavailable));
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(acquired);
    for cancel in [false, true] {
        let mut held = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE product_events IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *held)
            .await
            .unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&single)
            .await
            .unwrap();
        let task = {
            let r = r.clone();
            tokio::spawn(async move { r.weekly(day).await })
        };
        super::events::blocked(&pool, pid).await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert_eq!(task.await.unwrap(), Err(MetricReadError::Unavailable));
        }
        held.rollback().await.unwrap();
        let setting: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&single)
            .await
            .unwrap();
        assert_eq!(setting, "0");
        assert_eq!(r.weekly(day).await.unwrap().participants, 0);
    }
    single.close().await;
    close_auth_pool(pool).await;
}

async fn many_sources(pool: &sqlx::PgPool, account: Uuid, at: i64, reason: &str) {
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,end_elapsed_ms,reason,recorded_at,seed_expires_at) SELECT md5(n::text)::uuid,$2,243000,$3,to_timestamp($1::double precision),to_timestamp($1::double precision)+interval '7 days' FROM generate_series(1,10001) n")
        .bind(at).bind("a".repeat(64)).bind(reason).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) SELECT id,0,$1,false,'draw',4,0,0,0 FROM online_match_results")
        .bind(account).execute(pool).await.unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn raw_and_history_queries_detect_the_extra_row_instead_of_truncating_counts() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    initialized(&pool, at).await;
    let owner = super::privacy::participant(&pool, at).await;
    many_sources(&pool, owner.account, at + 1, "forfeit").await;
    sqlx::query("INSERT INTO product_events(id,account_id,occurred_at,received_at,online_match_id,opponent_type,end_reason) SELECT md5('event'||id::text)::uuid,$1,$2,$2,id,'human','forfeit' FROM online_match_results")
        .bind(owner.account).bind(at+1).execute(&pool).await.unwrap();
    assert_eq!(
        reader(&pool, at + 2).weekly(day).await,
        Err(MetricReadError::Policy(EventError::Capacity))
    );
    sqlx::query("DELETE FROM product_events WHERE id=(SELECT id FROM product_events LIMIT 1)")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader(&pool, at + 2)
            .weekly(day)
            .await
            .unwrap()
            .participants,
        0
    );
    close_auth_pool(pool).await;
    let pool = auth_pool().await;
    initialized(&pool, at).await;
    let owner = super::privacy::participant(&pool, at).await;
    many_sources(&pool, owner.account, at + 1, "timeout").await;
    let game: Uuid = sqlx::query_scalar("SELECT id FROM online_match_results LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    PgEventStore::new(pool.clone(), Arc::new(Clock(at + 1)))
        .capture(
            id(Uuid::new_v4()),
            EventSource::Online {
                account: id(owner.account),
                match_id: id(game),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        reader(&pool, at + 2).d7(day).await,
        Err(MetricReadError::Policy(EventError::Capacity))
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL role administration in isolated test database"]
async fn collection_initialization_and_snapshot_need_only_documented_serving_dml() {
    let pool = auth_pool().await;
    let day = UtcDay::parse("2026-10-05").unwrap();
    let at = day.start();
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_metrics_test_dml NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$").execute(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(isolated_schema_sql(
        "GRANT USAGE ON SCHEMA",
        &schema,
        "TO liar_metrics_test_dml",
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("GRANT SELECT ON auth_accounts,online_match_results,online_match_players,product_events TO liar_metrics_test_dml").execute(&pool).await.unwrap();
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
                sqlx::query("SET ROLE liar_metrics_test_dml")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    assert_eq!(
        reader(&dml, at).initialize().await,
        Err(MetricReadError::Unavailable)
    );
    sqlx::query("GRANT SELECT,INSERT ON metric_collection TO liar_metrics_test_dml")
        .execute(&pool)
        .await
        .unwrap();
    initialized(&dml, at).await;
    initialized(&dml, at + 1).await;
    assert_eq!(
        reader(&dml, at + 1).weekly(day).await.unwrap().participants,
        0
    );
    assert_eq!(
        reader(&dml, at + 8 * 86400)
            .d7(day)
            .await
            .unwrap()
            .sample_size,
        0
    );
    for sql in [
        "UPDATE metric_collection SET coverage_since=coverage_since+1",
        "DELETE FROM metric_collection",
        "SELECT * FROM _sqlx_migrations",
        "CREATE TABLE forbidden_metric(id integer)",
    ] {
        let err = sqlx::query(sql).execute(&dml).await.unwrap_err();
        assert_eq!(err.as_database_error().unwrap().code().unwrap(), "42501");
    }
    dml.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn real_main_initializes_collection_before_serving_and_preserves_it_on_restart() {
    use super::result_owner::{Server, server_command, wait_ready};
    let pool = auth_pool().await;
    let before = SystemAuthClock.now();
    let (mut command, origin) = server_command(&pool).await;
    let mut first = Server(command.spawn().unwrap());
    wait_ready(&mut first, &origin).await;
    let marker: (i64, i64) =
        sqlx::query_as("SELECT started_at,coverage_since FROM metric_collection")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(marker.0 >= before && marker.0 <= SystemAuthClock.now());
    assert_eq!(marker.0, marker.1);
    drop(first);
    let (mut command, origin) = server_command(&pool).await;
    let mut second = Server(command.spawn().unwrap());
    wait_ready(&mut second, &origin).await;
    let after: (i64, i64) =
        sqlx::query_as("SELECT started_at,coverage_since FROM metric_collection")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, marker);
    drop(second);
    close_auth_pool(pool).await;
}
