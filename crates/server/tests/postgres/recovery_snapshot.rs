use super::event_retention::Clock;
use super::*;
use liar_server::recovery::*;
const NOW: i64 = 1_800_000_000;
async fn marker(pool: &sqlx::PgPool, kind: &str, id: Uuid, deleted: i64) {
    let sql = match kind {
        "account" => {
            "INSERT INTO auth_deletion_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))"
        }
        _ => {
            "INSERT INTO auth_identity_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))"
        }
    };
    sqlx::query(sql)
        .bind(id)
        .bind(deleted)
        .bind(deleted + 2419200)
        .execute(pool)
        .await
        .unwrap();
}
async fn source(pool: &sqlx::PgPool) -> Uuid {
    sqlx::query_scalar("SELECT source_id FROM service_recovery_origin")
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn incident(pool: &sqlx::PgPool) -> Uuid {
    quarantine(pool, &Clock(NOW)).await.unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capture_is_minimal_committed_and_keeps_original_times() {
    let pool = auth_pool().await;
    let a = Uuid::new_v4();
    let i = Uuid::new_v4();
    marker(&pool, "account", a, NOW - 10).await;
    marker(&pool, "identity", i, NOW - 20).await;
    marker(&pool, "identity", Uuid::new_v4(), NOW - 2419200).await;
    let mut pending = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO auth_identity_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))").bind(Uuid::new_v4()).bind(NOW).bind(NOW+2419200).execute(&mut *pending).await.unwrap();
    let doc = capture(&pool, &Clock(NOW)).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&doc.to_bytes(NOW).unwrap()).unwrap();
    assert_eq!(value["source_id"], source(&pool).await.to_string());
    assert_eq!(value["captured_at"], NOW);
    let entries = value["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    for (kind, id, deleted) in [("account", a, NOW - 10), ("identity", i, NOW - 20)] {
        assert!(entries.contains(&serde_json::json!({"kind":kind,"id":id,"deleted_at":deleted,"expires_at":deleted+2419200})));
    }
    assert_eq!(value.as_object().unwrap().len(), 4);
    pending.rollback().await.unwrap();
    ensure_serving(&pool).await.unwrap();
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capture_reads_gate_and_both_kinds_from_one_mvcc_snapshot() {
    let pool = auth_pool().await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE service_recovery_control IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let reading = pool.clone();
    let task = tokio::spawn(async move { capture(&reading, &Clock(NOW)).await });
    tokio::time::timeout(Duration::from_secs(1),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation='service_recovery_control'::regclass AND NOT granted)").fetch_one(&pool).await.unwrap();
            if waiting {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    marker(&pool, "account", Uuid::new_v4(), NOW).await;
    marker(&pool, "identity", Uuid::new_v4(), NOW).await;
    lock.commit().await.unwrap();
    let result = task.await.unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&result.to_bytes(NOW).unwrap()).unwrap();
    assert_eq!(value["entries"], serde_json::json!([]));
    let value: serde_json::Value = serde_json::from_slice(
        &capture(&pool, &Clock(NOW))
            .await
            .unwrap()
            .to_bytes(NOW)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["entries"].as_array().unwrap().len(), 2);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capture_refuses_partial_oversized_future_fractional_or_unknown_sources() {
    let pool = auth_pool().await;
    sqlx::query("INSERT INTO auth_identity_tombstones SELECT gen_random_uuid(),to_timestamp($1::double precision),to_timestamp($2::double precision) FROM generate_series(1,501)").bind(NOW-1).bind(NOW-1+2419200).execute(&pool).await.unwrap();
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Invalid)
    ));
    sqlx::query("DELETE FROM auth_identity_tombstones")
        .execute(&pool)
        .await
        .unwrap();
    marker(&pool, "identity", Uuid::new_v4(), NOW + 1).await;
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Invalid)
    ));
    sqlx::query("UPDATE auth_identity_tombstones SET deleted_at=deleted_at-interval '1.5 seconds',expires_at=expires_at-interval '1.5 seconds'").execute(&pool).await.unwrap();
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Invalid)
    ));
    sqlx::query("DELETE FROM auth_identity_tombstones")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM service_recovery_origin")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Blocked)
    ));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn source_binding_survives_quarantine_and_rejects_other_or_missing_origin() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    marker(&pool, "account", owner.account, NOW - 10).await;
    let doc = capture(&pool, &Clock(NOW)).await.unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), Uuid::new_v4(), &doc).await,
        Err(RecoveryError::Blocked)
    );
    let original = source(&pool).await;
    let id = incident(&pool).await;
    assert_eq!(source(&pool).await, original);
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Blocked)
    ));
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), Uuid::new_v4(), &doc).await,
        Err(RecoveryError::Blocked)
    );
    sqlx::query("UPDATE service_recovery_origin SET source_id=$1")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), id, &doc).await,
        Err(RecoveryError::Blocked)
    );
    sqlx::query("DELETE FROM service_recovery_origin")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), id, &doc).await,
        Err(RecoveryError::Blocked)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn snapshot_reapplication_preserves_expiry_skips_expired_and_is_atomic() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    let keep = super::personal::participant(&pool, NOW - 100).await;
    marker(&pool, "account", owner.account, NOW - 10).await;
    marker(&pool, "account", keep.account, NOW - 2419199).await;
    let doc = capture(&pool, &Clock(NOW)).await.unwrap();
    sqlx::query("DELETE FROM auth_deletion_tombstones")
        .execute(&pool)
        .await
        .unwrap();
    let id = incident(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION fail_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture'; END; $$; CREATE TRIGGER fail_snapshot BEFORE INSERT ON metric_collection FOR EACH ROW EXECUTE FUNCTION fail_snapshot();").execute(&pool).await.unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW + 301), id, &doc).await,
        Err(RecoveryError::Unavailable)
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_sessions),(SELECT count(*) FROM auth_deletion_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (2, 2, 0));
    sqlx::query("DROP TRIGGER fail_snapshot ON metric_collection")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW + 301), id, &doc).await,
        Ok(1)
    );
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW + 302), id, &doc).await,
        Ok(1)
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_sessions),(SELECT count(*) FROM auth_deletion_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 0, 1));
    let times:(i64,i64)=sqlx::query_as("SELECT extract(epoch FROM deleted_at)::bigint,extract(epoch FROM expires_at)::bigint FROM auth_deletion_tombstones").fetch_one(&pool).await.unwrap();
    assert_eq!(times, (NOW - 10, NOW - 10 + 2419200));
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn origin_upgrade_only_initializes_a_matching_active_gate() {
    for mode in ["active", "quarantined", "wrong_binding", "missing"] {
        let pool = auth_pool_through(Some(202611090002)).await;
        let id = Uuid::new_v4();
        if mode != "missing" {
            sqlx::query("INSERT INTO service_recovery_control VALUES(true,$1,(SELECT oid::bigint FROM pg_database WHERE datname=current_database()),current_schema()::regnamespace::oid::bigint,$2,$3)").bind(if mode=="quarantined" {"quarantined"}else{"active"}).bind(id).bind(NOW).execute(&pool).await.unwrap();
        }
        if mode == "wrong_binding" {
            sqlx::query("UPDATE service_recovery_control SET schema_oid=schema_oid+1")
                .execute(&pool)
                .await
                .unwrap();
        }
        Migrator::new(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../migrations"
        )))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
        let origin: Option<Uuid> =
            sqlx::query_scalar("SELECT source_id FROM service_recovery_origin")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(origin, if mode == "active" { Some(id) } else { None });
        close_auth_pool(pool).await;
    }
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn owner_cli_transfers_minimal_snapshot_and_never_opens_the_target() {
    use super::recovery::cli;
    let source_pool = auth_pool().await;
    let target = auth_pool().await;
    let now = SystemAuthClock.now();
    let owner = super::personal::participant(&target, now - 100).await;
    marker(&source_pool, "account", owner.account, now - 10).await;
    let output = cli(&source_pool, &["capture"], b"", true).await;
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let snapshot = output.stdout;
    Snapshot::parse(&snapshot, SystemAuthClock.now()).unwrap();
    let quarantined = cli(&target, &["quarantine"], b"", true).await;
    assert!(quarantined.status.success());
    let value: serde_json::Value = serde_json::from_slice(&quarantined.stdout).unwrap();
    let id = value["recovery_id"].as_str().unwrap();
    let rejected = cli(&target, &["apply-snapshot", id], &snapshot, true).await;
    assert!(!rejected.status.success());
    sqlx::query("UPDATE service_recovery_origin SET source_id=$1")
        .bind(source(&source_pool).await)
        .execute(&target)
        .await
        .unwrap();
    let output = cli(&target, &["apply-snapshot", id], &snapshot, true).await;
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"applied":1,"state":"quarantined"})
    );
    for (args, input) in [
        (vec!["apply-snapshot"], b"".as_slice()),
        (vec!["apply-snapshot", "bad"], b"".as_slice()),
        (
            vec!["apply-snapshot", "00000000-0000-0000-0000-000000000000"],
            b"".as_slice(),
        ),
        (vec!["apply-snapshot", id], b"sensitive sentinel".as_slice()),
        (vec!["capture"], b"".as_slice()),
    ] {
        let output = cli(&target, &args, input, true).await;
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!({"status":"failed","code":"recovery_failed"})
        );
    }
    assert_eq!(ensure_serving(&target).await, Err(RecoveryError::Blocked));
    close_auth_pool(target).await;
    close_auth_pool(source_pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capture_remains_read_only_under_a_live_owner_but_snapshot_apply_is_busy() {
    use liar_server::online::PgJournalRuntime;
    let pool = auth_pool().await;
    let runtime = PgJournalRuntime::claim(pool.clone(), Arc::new(Clock(NOW)))
        .await
        .unwrap();
    let doc = capture(&pool, &Clock(NOW)).await.unwrap();
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), Uuid::new_v4(), &doc).await,
        Err(RecoveryError::Busy)
    );
    assert_eq!(
        apply_snapshot(&pool, &Clock(NOW), Uuid::nil(), &doc).await,
        Err(RecoveryError::Invalid)
    );
    ensure_serving(&pool).await.unwrap();
    drop(runtime);
    close_auth_pool(pool).await;
}

async fn wait_for_table_reader(pool: &sqlx::PgPool, table: &str) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation=$1::regclass AND NOT granted)",
            )
            .bind(table)
            .fetch_one(pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn snapshot_expiring_while_sql_waits_rolls_back_the_entire_erasure() {
    use std::sync::atomic::{AtomicI64, Ordering};
    struct Moving(AtomicI64);
    impl AuthClock for Moving {
        fn now(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    marker(&pool, "account", owner.account, NOW - 2419199).await;
    let doc = capture(&pool, &Clock(NOW)).await.unwrap();
    sqlx::query("DELETE FROM auth_deletion_tombstones")
        .execute(&pool)
        .await
        .unwrap();
    let id = incident(&pool).await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE metric_collection IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let clock = Arc::new(Moving(AtomicI64::new(NOW)));
    let moving = clock.clone();
    let writing = pool.clone();
    let task = tokio::spawn(async move { apply_snapshot(&writing, &*moving, id, &doc).await });
    wait_for_table_reader(&pool, "metric_collection").await;
    clock.0.store(NOW + 1, Ordering::SeqCst);
    lock.commit().await.unwrap();
    assert_eq!(task.await.unwrap(), Err(RecoveryError::Invalid));
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_sessions),(SELECT count(*) FROM auth_deletion_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1, 0));
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn capture_timeout_or_clock_reversal_never_returns_a_snapshot_or_changes_source() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reversing(AtomicUsize);
    impl AuthClock for Reversing {
        fn now(&self) -> i64 {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                NOW
            } else {
                NOW - 1
            }
        }
    }
    let pool = auth_pool().await;
    assert!(matches!(
        capture(&pool, &Reversing(AtomicUsize::new(0))).await,
        Err(RecoveryError::Invalid)
    ));
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE service_recovery_control IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert!(matches!(
        capture(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Unavailable)
    ));
    lock.rollback().await.unwrap();
    ensure_serving(&pool).await.unwrap();
    capture(&pool, &Clock(NOW)).await.unwrap();
    close_auth_pool(pool).await;
}
