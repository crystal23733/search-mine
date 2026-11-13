use super::event_retention::Clock;
use super::*;
use liar_server::{online::PgJournalRuntime, recovery::*};
const NOW: i64 = 1_800_000_000;
async fn pair() -> (sqlx::PgPool, sqlx::PgPool, Uuid) {
    let source = auth_pool().await;
    let target = auth_pool().await;
    let origin: Uuid = sqlx::query_scalar("SELECT source_id FROM service_recovery_origin")
        .fetch_one(&source)
        .await
        .unwrap();
    sqlx::query("UPDATE service_recovery_origin SET source_id=$1")
        .bind(origin)
        .execute(&target)
        .await
        .unwrap();
    let incident = quarantine(&target, &Clock(NOW)).await.unwrap();
    (source, target, incident)
}
async fn no_owner(pool: &sqlx::PgPool) {
    tokio::time::timeout(Duration::from_secs(1),async {
        loop {
            let gone:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=1280530258::oid AND objid='online_match_results'::regclass::oid AND objsubid=1 AND granted)").fetch_one(pool).await.unwrap();
            if gone {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
}
async fn marker(pool: &sqlx::PgPool, id: Uuid, at: i64) {
    sqlx::query("INSERT INTO auth_deletion_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))")
        .bind(id).bind(at-10).bind(at-10+2419200).execute(pool).await.unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn resume_fences_source_erases_target_and_only_activates_target() {
    let now = SystemAuthClock.now();
    let (source, target, incident) = pair().await;
    let owner = super::personal::participant(&target, now - 100).await;
    marker(&source, owner.account, now).await;
    let snapshot = capture(&source, &Clock(now)).await.unwrap();
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(now)), incident, &snapshot).await,
        Ok(1)
    );
    assert_eq!(ensure_serving(&source).await, Err(RecoveryError::Blocked));
    assert_eq!(
        quarantine(&source, &Clock(now)).await,
        Err(RecoveryError::Blocked)
    );
    ensure_serving(&target).await.unwrap();
    assert!(sqlx::query("INSERT INTO auth_deletion_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))").bind(Uuid::new_v4()).bind(now).bind(now+2419200).execute(&source).await.is_err());
    for query in [
        "UPDATE service_recovery_control SET state='active'",
        "DELETE FROM service_recovery_control",
        "TRUNCATE service_recovery_control",
    ] {
        assert!(sqlx::query(query).execute(&source).await.is_err());
    }
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_accounts")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(accounts, 0);
    let (mut command, origin) = super::result_owner::server_command(&target).await;
    let mut server = super::result_owner::Server(command.spawn().unwrap());
    super::result_owner::wait_ready(&mut server, &origin).await;
    drop(server);
    no_owner(&target).await;
    // An already active destination cannot be silently re-applied after a lost response.
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(now)), incident, &snapshot).await,
        Err(RecoveryError::Blocked)
    );
    close_auth_pool(source).await;
    close_auth_pool(target).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn resume_rejects_live_writer_and_missing_latest_rights_before_fencing() {
    let (source, target, incident) = pair().await;
    let snapshot = capture(&source, &Clock(NOW)).await.unwrap();
    let runtime = PgJournalRuntime::claim(source.clone(), Arc::new(Clock(NOW)))
        .await
        .unwrap();
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(NOW)), incident, &snapshot).await,
        Err(RecoveryError::Busy)
    );
    drop(runtime);
    no_owner(&source).await;
    // A committed right missing from the checkpoint must also block, independently of the lock.
    marker(&source, Uuid::new_v4(), NOW).await;
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(NOW)), incident, &snapshot).await,
        Err(RecoveryError::Blocked)
    );
    ensure_serving(&source).await.unwrap();
    assert_eq!(ensure_serving(&target).await, Err(RecoveryError::Blocked));
    close_auth_pool(source).await;
    close_auth_pool(target).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn sealed_source_retries_after_target_failure_without_reopening_source() {
    let (source, target, incident) = pair().await;
    let snapshot = capture(&source, &Clock(NOW)).await.unwrap();
    // A post-fence target failure is conservative; retry uses the same sealed source.
    let mut lock = target.begin().await.unwrap();
    sqlx::query("LOCK TABLE auth_sessions IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(NOW)), incident, &snapshot).await,
        Err(RecoveryError::Unavailable)
    );
    assert_eq!(ensure_serving(&source).await, Err(RecoveryError::Blocked));
    assert_eq!(ensure_serving(&target).await, Err(RecoveryError::Blocked));
    lock.rollback().await.unwrap();
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(NOW)), incident, &snapshot).await,
        Ok(0)
    );
    ensure_serving(&target).await.unwrap();
    close_auth_pool(source).await;
    close_auth_pool(target).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn resume_reaps_restored_secrets_and_records_before_activation() {
    let (source, target, incident) = pair().await;
    let survivor = super::personal::participant(&target, NOW - 100).await;
    let expired = Uuid::new_v4();
    super::personal::seed_online(&target, survivor.account, expired, NOW - 7776001).await;
    let recent = Uuid::new_v4();
    super::personal::seed_online(&target, survivor.account, recent, NOW - 604801).await;
    let snapshot = capture(&source, &Clock(NOW)).await.unwrap();
    assert_eq!(
        resume_from_source(&source, &target, Arc::new(Clock(NOW)), incident, &snapshot).await,
        Ok(0)
    );
    let retained: Vec<(Uuid, bool)> =
        sqlx::query_as("SELECT id,secret_seed IS NULL FROM online_match_results")
            .fetch_all(&target)
            .await
            .unwrap();
    assert_eq!(retained, vec![(recent, true)]);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_sessions")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    ensure_serving(&target).await.unwrap();
    close_auth_pool(source).await;
    close_auth_pool(target).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL; executed in database CI"]
async fn in_flight_right_must_commit_before_the_final_cutover_comparison() {
    let (source, target, incident) = pair().await;
    let snapshot = capture(&source, &Clock(NOW)).await.unwrap();
    let mut writing = source.begin().await.unwrap();
    sqlx::query("INSERT INTO auth_identity_tombstones VALUES($1,to_timestamp($2::double precision),to_timestamp($3::double precision))")
        .bind(Uuid::new_v4()).bind(NOW).bind(NOW+2419200).execute(&mut *writing).await.unwrap();
    let (reading, destination) = (source.clone(), target.clone());
    let pending = tokio::spawn(async move {
        resume_from_source(
            &reading,
            &destination,
            Arc::new(Clock(NOW)),
            incident,
            &snapshot,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(1),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation='auth_identity_tombstones'::regclass AND NOT granted)").fetch_one(&source).await.unwrap();
            if waiting {break;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    writing.commit().await.unwrap();
    assert_eq!(pending.await.unwrap(), Err(RecoveryError::Blocked));
    ensure_serving(&source).await.unwrap();
    assert_eq!(ensure_serving(&target).await, Err(RecoveryError::Blocked));
    close_auth_pool(source).await;
    close_auth_pool(target).await;
}
