use super::event_retention::Clock;
use super::*;
use liar_server::recovery::*;
const NOW: i64 = 1_800_000_000;
async fn blocked(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO service_recovery_control VALUES(true,'quarantined',(SELECT oid::bigint FROM pg_database WHERE datname=current_database()),current_schema()::regnamespace::oid::bigint,$1,$2) ON CONFLICT(singleton) DO UPDATE SET state='quarantined',recovery_id=$1,updated_at=$2")
 .bind(id).bind(NOW).execute(pool).await.unwrap();
    id
}
fn ledger(id: Uuid, account: Uuid) -> Ledger {
    Ledger::parse(serde_json::json!({"version":1,"recovery_id":id,"captured_at":NOW,"entries":[{"kind":"account","id":account,"deleted_at":NOW-10,"expires_at":NOW-10+2419200}]}).to_string().as_bytes(),NOW).unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn quarantined_gate_is_never_serving_and_real_main_exits_before_http_bind() {
    let pool = auth_pool().await;
    blocked(&pool).await;
    let (mut cmd, origin) = super::result_owner::server_command(&pool).await;
    let mut child = super::result_owner::Server(cmd.spawn().unwrap());
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if let Some(exit) = child.0.try_wait().unwrap() {
                assert!(!exit.success());
                break;
            }
            assert!(
                super::result_owner::client()
                    .get(format!("{origin}/health/live"))
                    .send()
                    .await
                    .is_err(),
                "A quarantined DB must never expose an HTTP listener"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn apply_erases_the_restored_account_and_sessions_but_keeps_quarantine() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    let id = blocked(&pool).await;
    assert_eq!(
        apply(&pool, &Clock(NOW), &ledger(id, owner.account)).await,
        Ok(1)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn missing_wrong_binding_and_existing_schema_never_auto_activate() {
    let pool = auth_pool().await;
    ensure_serving(&pool).await.unwrap();
    sqlx::query("DELETE FROM service_recovery_control")
        .execute(&pool)
        .await
        .unwrap();
    FreshSchema::inspect(&pool)
        .await
        .unwrap()
        .initialize(&pool)
        .await
        .unwrap();
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    let id = quarantine(&pool, &Clock(NOW)).await.unwrap();
    assert_eq!(quarantine(&pool, &Clock(NOW + 1)).await.unwrap(), id);
    sqlx::query("UPDATE service_recovery_control SET state='active',schema_oid=schema_oid+1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    let new = quarantine(&pool, &Clock(NOW + 2)).await.unwrap();
    assert_ne!(id, new);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn live_journal_owner_prevents_quarantine_and_erasure() {
    use liar_server::online::PgJournalRuntime;
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    let runtime = PgJournalRuntime::claim(pool.clone(), Arc::new(Clock(NOW)))
        .await
        .unwrap();
    assert_eq!(
        quarantine(&pool, &Clock(NOW)).await,
        Err(RecoveryError::Busy)
    );
    assert_eq!(
        apply(&pool, &Clock(NOW), &ledger(Uuid::new_v4(), owner.account)).await,
        Err(RecoveryError::Busy)
    );
    ensure_serving(&pool).await.unwrap();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_accounts)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(exists);
    drop(runtime);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn apply_invalidates_other_restored_sessions_consents_tokens_and_metric_coverage() {
    let pool = auth_pool().await;
    let erased = super::personal::participant(&pool, NOW - 100).await;
    let survivor = super::personal::participant(&pool, NOW - 100).await;
    let identity: Uuid = sqlx::query_scalar("SELECT id FROM auth_identities WHERE account_id=$1")
        .bind(survivor.account)
        .fetch_one(&pool)
        .await
        .unwrap();
    let consent = Uuid::new_v4();
    sqlx::query("INSERT INTO session_consents VALUES($1,$2,$3,$4,true,true)")
        .bind(survivor.revision)
        .bind(consent)
        .bind(NOW - 100)
        .bind(NOW + 1000)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO optional_product_events(id,owner_session_id,tab_key,consent_revision,purpose,occurred_at,received_at,locale,action) VALUES($1,$2,$3,$4,'analytics',$5,$5,'en','{\"type\":\"landing_view\",\"route\":\"home\"}')")
 .bind(Uuid::new_v4()).bind(survivor.revision).bind(Uuid::new_v4()).bind(consent).bind(NOW-100).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO auth_apple_revoke_queue(id,identity_id,encrypted_refresh_token,created_at,expires_at,next_attempt_at) VALUES($1,$2,$3,to_timestamp($4::double precision),to_timestamp($4::double precision)+interval '24 hours',to_timestamp($4::double precision))")
 .bind(Uuid::new_v4()).bind(identity).bind(vec![8u8;34]).bind(NOW-100).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO metric_collection VALUES(true,1000,1000)")
        .execute(&pool)
        .await
        .unwrap();
    let id = blocked(&pool).await;
    let doc = ledger(id, erased.account);
    assert_eq!(apply(&pool, &Clock(NOW), &doc).await, Ok(1));
    assert_eq!(apply(&pool, &Clock(NOW + 1), &doc).await, Ok(1));
    let counts:(i64,i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_sessions),(SELECT count(*) FROM session_consents),(SELECT count(*) FROM optional_product_events),(SELECT count(*) FROM auth_apple_revoke_queue),(SELECT count(*) FROM auth_deletion_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 0, 0, 0, 0, 1));
    let bounds: (i64, i64) =
        sqlx::query_as("SELECT started_at,coverage_since FROM metric_collection")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bounds, (1000, NOW + 1));
    let at: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM deleted_at)::bigint FROM auth_deletion_tombstones",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(at, NOW - 10);
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn wrong_incident_sql_failure_and_reversing_clock_do_not_partially_erase() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reversing(AtomicUsize);
    impl AuthClock for Reversing {
        fn now(&self) -> i64 {
            NOW - i64::from(self.0.fetch_add(1, Ordering::SeqCst) >= 6)
        }
    }
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    let id = blocked(&pool).await;
    assert_eq!(
        apply(&pool, &Clock(NOW), &ledger(Uuid::new_v4(), owner.account)).await,
        Err(RecoveryError::Blocked)
    );
    assert_eq!(
        apply(
            &pool,
            &Reversing(AtomicUsize::new(0)),
            &ledger(id, owner.account)
        )
        .await,
        Err(RecoveryError::Invalid)
    );
    sqlx::raw_sql("CREATE FUNCTION fail_recovery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture'; END; $$; CREATE TRIGGER fail_recovery BEFORE INSERT ON metric_collection FOR EACH ROW EXECUTE FUNCTION fail_recovery();").execute(&pool).await.unwrap();
    assert_eq!(
        apply(&pool, &Clock(NOW), &ledger(id, owner.account)).await,
        Err(RecoveryError::Unavailable)
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_sessions),(SELECT count(*) FROM auth_deletion_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 1, 0));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn unlink_tombstone_is_minimal_and_expires_without_extending_on_repetition() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let token = SecretToken::generate().unwrap();
    let first = store
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            NOW - 100,
        ))
        .await
        .unwrap();
    let next = SecretToken::generate().unwrap();
    let mut link = super::rights::account_write(
        Provider::Naver,
        &Uuid::new_v4().to_string(),
        &next,
        NOW - 99,
    );
    link.intent = AuthIntent::Link(first.account.id);
    link.bound_session = Some(token.hash());
    link.previous_session = Some(token.hash());
    store.login(link).await.unwrap();
    let authority = SessionAuthority {
        account: first.account.id,
        hash: next.hash(),
        now: NOW,
    };
    store.unlink(authority, Provider::Google).await.unwrap();
    let marker:(Uuid,i64,i64)=sqlx::query_as("SELECT identity_id,EXTRACT(EPOCH FROM deleted_at)::bigint,EXTRACT(EPOCH FROM expires_at)::bigint FROM auth_identity_tombstones").fetch_one(&pool).await.unwrap();
    assert_eq!(marker, (first.identity_id, NOW, NOW + 2419200));
    store.cleanup(NOW + 2419199).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_identity_tombstones")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    store.cleanup(NOW + 2419200).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_identity_tombstones")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn restored_identity_erasure_is_idempotent_and_never_releases_quarantine() {
    let pool = auth_pool().await;
    let owner = super::personal::participant(&pool, NOW - 100).await;
    let identity: Uuid = sqlx::query_scalar("SELECT id FROM auth_identities WHERE account_id=$1")
        .bind(owner.account)
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = blocked(&pool).await;
    let doc=Ledger::parse(serde_json::json!({"version":1,"recovery_id":id,"captured_at":NOW,"entries":[{"kind":"identity","id":identity,"deleted_at":NOW-10,"expires_at":NOW-10+2419200}]}).to_string().as_bytes(),NOW).unwrap();
    assert_eq!(apply(&pool, &Clock(NOW), &doc).await, Ok(1));
    assert_eq!(apply(&pool, &Clock(NOW + 1), &doc).await, Ok(1));
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM auth_accounts),(SELECT count(*) FROM auth_identities),(SELECT count(*) FROM auth_identity_tombstones)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 0, 1));
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn pre_gate_backup_upgrade_stays_blocked_instead_of_becoming_a_fresh_install() {
    let pool = auth_pool_through(Some(202611090001)).await;
    super::personal::participant(&pool, NOW - 100).await;
    let proof = FreshSchema::inspect(&pool).await.unwrap();
    Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap()
    .run(&pool)
    .await
    .unwrap();
    proof.initialize(&pool).await.unwrap();
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn serving_can_read_but_never_change_the_runtime_gate() {
    let pool = auth_pool().await;
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_recovery217_test_read NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$").execute(&pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(isolated_schema_sql(
        "GRANT USAGE ON SCHEMA",
        &schema,
        "TO liar_recovery217_test_read",
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("GRANT SELECT ON service_recovery_control TO liar_recovery217_test_read")
        .execute(&pool)
        .await
        .unwrap();
    let serving = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SET ROLE liar_recovery217_test_read")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    ensure_serving(&serving).await.unwrap();
    assert!(
        sqlx::query("UPDATE service_recovery_control SET state='active'")
            .execute(&serving)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM service_recovery_control")
            .execute(&serving)
            .await
            .is_err()
    );
    serving.close().await;
    close_auth_pool(pool).await;
}
pub(super) async fn cli(
    pool: &sqlx::PgPool,
    args: &[&str],
    input: &[u8],
    database: bool,
) -> std::process::Output {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut url = url::Url::parse(&env::var("DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_recovery"));
    command
        .env_clear()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if database {
        command.env("MIGRATION_DATABASE_URL", url.as_str());
    }
    for key in [
        "SystemRoot",
        "WINDIR",
        "PATH",
        "TEMP",
        "TMP",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = env::var_os(key) {
            command.env(key, v);
        }
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn owner_cli_only_quarantines_and_sanitizes_with_fixed_failure_output() {
    let pool = auth_pool().await;
    let now = SystemAuthClock.now();
    let owner = super::personal::participant(&pool, now - 1).await;
    let output = cli(&pool, &["quarantine"], b"", true).await;
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = result["recovery_id"].as_str().unwrap();
    let input=serde_json::json!({"version":1,"recovery_id":id,"captured_at":now,"entries":[{"kind":"account","id":owner.account,"deleted_at":now-1,"expires_at":now-1+2419200}]}).to_string();
    let output = cli(&pool, &["apply"], input.as_bytes(), true).await;
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"applied":1,"state":"quarantined"})
    );
    for (args, input, database) in [
        (vec!["release"], b"".as_slice(), true),
        (vec![], b"".as_slice(), true),
        (vec!["apply"], b"sensitive sentinel".as_slice(), true),
        (vec!["quarantine"], b"".as_slice(), false),
    ] {
        let output = cli(&pool, &args, input, database).await;
        assert!(!output.status.success());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!({"status":"failed","code":"recovery_failed"})
        );
        assert!(output.stderr.is_empty());
    }
    assert_eq!(ensure_serving(&pool).await, Err(RecoveryError::Blocked));
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn factory_proof_cannot_activate_another_schema_or_a_populated_target() {
    let pool = auth_pool().await;
    let schema = format!("auth_test_{}", Uuid::new_v4().simple());
    sqlx::query(isolated_schema_sql("CREATE SCHEMA", &schema, ""))
        .execute(&pool)
        .await
        .unwrap();
    let raw = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let wrong = FreshSchema::inspect(&raw).await.unwrap();
    let populated = FreshSchema::inspect(&raw).await.unwrap();
    assert_eq!(wrong.initialize(&pool).await, Err(RecoveryError::Blocked));
    ensure_serving(&pool).await.unwrap();
    Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap()
    .run(&raw)
    .await
    .unwrap();
    super::personal::participant(&raw, NOW - 100).await;
    assert_eq!(
        populated.initialize(&raw).await,
        Err(RecoveryError::Blocked)
    );
    assert_eq!(ensure_serving(&raw).await, Err(RecoveryError::Blocked));
    close_auth_pool(raw).await;
    close_auth_pool(pool).await;
}
