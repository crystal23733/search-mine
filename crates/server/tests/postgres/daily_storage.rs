use super::*;

async fn single_pool(pool: &sqlx::PgPool) -> (sqlx::PgPool, i32) {
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await
        .unwrap();
    let single = PgPoolOptions::new()
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
    let pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&single)
        .await
        .unwrap();
    (single, pid)
}
async fn blocked(pool: &sqlx::PgPool, pid: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT cardinality(pg_blocking_pids($1))>0")
                .bind(pid)
                .fetch_one(pool)
                .await
                .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("submission must be waiting on the held actual PostgreSQL lock");
}
async fn lock(pool: &sqlx::PgPool, account: Uuid) -> sqlx::Transaction<'_, sqlx::Postgres> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(account)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn lock_timeout_cancellation_pool_wait_and_sql_failure_roll_back_without_a_receipt() {
    let pool = auth_pool().await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let (single, pid) = single_pool(&pool).await;
    let store = PgDailyRepository::new(single.clone(), clock);
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    let attempt = Uuid::new_v4();
    let held = lock(&pool, session.account).await;
    let task_store = store.clone();
    let task = tokio::spawn(async move { task_store.submit(session, attempt, &evidence()).await });
    blocked(&pool, pid).await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        Err(DailyStoreError::Unavailable)
    );
    held.rollback().await.unwrap();
    assert_eq!(count(&pool).await, 0);
    let held = lock(&pool, session.account).await;
    let task_store = store.clone();
    let task = tokio::spawn(async move { task_store.submit(session, attempt, &evidence()).await });
    blocked(&pool, pid).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    held.rollback().await.unwrap();
    // The repository deadline also includes waiting for a connection, before any SQL.
    let occupied = single.acquire().await.unwrap();
    assert!(matches!(
        store.definition("2026-10-07", 1).await,
        Err(DailyStoreError::Unavailable)
    ));
    drop(occupied);
    sqlx::raw_sql("CREATE FUNCTION fail_daily_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected storage failure'; END; $$; CREATE TRIGGER fail_daily BEFORE INSERT ON daily_completions FOR EACH ROW EXECUTE FUNCTION fail_daily_insert();").execute(&pool).await.unwrap();
    assert_eq!(
        store.submit(session, attempt, &evidence()).await,
        Err(DailyStoreError::Unavailable)
    );
    assert_eq!(count(&pool).await, 0);
    sqlx::query("DROP TRIGGER fail_daily ON daily_completions")
        .execute(&pool)
        .await
        .unwrap();
    let first = official(store.submit(session, attempt, &evidence()).await.unwrap());
    // Losing the first response does not require a new operation or a new retention anchor.
    let restarted =
        PgDailyRepository::new(single.clone(), Arc::new(Clock(AtomicI64::new(START + 120))));
    assert_eq!(
        restarted
            .submit(session, attempt, &evidence())
            .await
            .unwrap(),
        DailySubmission::Official(first)
    );
    assert_eq!(count(&pool).await, 1);
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_revocation_and_erasure_win_before_a_blocked_submission() {
    let pool = auth_pool().await;
    let (single, pid) = single_pool(&pool).await;
    let store = PgDailyRepository::new(single.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    for erase in [false, true] {
        let session = participant(&pool, START + 60).await;
        store
            .submit(session, Uuid::new_v4(), &evidence())
            .await
            .unwrap();
        let mut tx = lock(&pool, session.account).await;
        let task_store = store.clone();
        let task = tokio::spawn(async move {
            task_store
                .submit(session, Uuid::new_v4(), &evidence())
                .await
        });
        blocked(&pool, pid).await;
        if erase {
            sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
                .bind(session.account)
                .execute(&mut *tx)
                .await
                .unwrap();
        } else {
            sqlx::query("DELETE FROM auth_sessions WHERE account_id=$1")
                .bind(session.account)
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
        assert_eq!(task.await.unwrap(), Err(DailyStoreError::Unauthorized));
        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM daily_completions WHERE account_id=$1")
                .bind(session.account)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, if erase { 0 } else { 1 });
    }
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn trusted_clock_is_resampled_after_the_account_lock() {
    let pool = auth_pool().await;
    let (single, pid) = single_pool(&pool).await;
    let clock = Arc::new(Clock(AtomicI64::new(START + 60)));
    let store = PgDailyRepository::new(single.clone(), clock.clone());
    store.publish(definition()).await.unwrap();
    let session = participant(&pool, START + 60).await;
    for (before, after, expected) in [
        (START + 604799, START + 604800, DailyStoreError::Expired),
        (START + 120, START + 119, DailyStoreError::Unavailable),
        (
            START + 60 + SESSION_SECONDS - 1,
            START + 60 + SESSION_SECONDS,
            DailyStoreError::Unauthorized,
        ),
    ] {
        clock.0.store(before, Ordering::SeqCst);
        let tx = lock(&pool, session.account).await;
        let task_store = store.clone();
        let task = tokio::spawn(async move {
            task_store
                .submit(session, Uuid::new_v4(), &evidence())
                .await
        });
        blocked(&pool, pid).await;
        clock.0.store(after, Ordering::SeqCst);
        tx.commit().await.unwrap();
        assert_eq!(task.await.unwrap(), Err(expected));
    }
    assert_eq!(count(&pool).await, 0);
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn forward_migration_preserves_existing_auth_and_online_admissions() {
    let pool = auth_pool_through(Some(202610100003)).await;
    let session = participant(&pool, START + 60).await;
    let match_id = Uuid::new_v4();
    sqlx::query("INSERT INTO online_active_matches(id,rules_hash,owner_id,admitted_at) VALUES($1,$2,$3,to_timestamp($4::double precision))").bind(match_id).bind(RulesSnapshot::bundled().hash).bind(Uuid::new_v4()).bind(START+60).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO online_active_players(match_id,seat,account_id,is_bot) VALUES($1,0,$2,false)",
    )
    .bind(match_id)
    .bind(session.account)
    .execute(&pool)
    .await
    .unwrap();
    Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../migrations"
    )))
    .await
    .unwrap()
    .run(&pool)
    .await
    .unwrap();
    assert_eq!(
        PgAuthStore::new(pool.clone())
            .session(session.hash, START + 60)
            .await
            .unwrap()
            .unwrap()
            .account
            .id,
        session.account
    );
    let present:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM online_active_matches m JOIN online_active_players p ON p.match_id=m.id WHERE m.id=$1 AND p.account_id=$2)").bind(match_id).bind(session.account).fetch_one(&pool).await.unwrap();
    assert!(present);
    let store = PgDailyRepository::new(pool.clone(), Arc::new(Clock(AtomicI64::new(START + 60))));
    store.publish(definition()).await.unwrap();
    store
        .submit(session, Uuid::new_v4(), &evidence())
        .await
        .unwrap();
    assert_eq!(count(&pool).await, 1);
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn dst_and_immutable_rows_allow_only_replay_erasure_and_reject_invalid_storage() {
    let pool = auth_pool().await;
    let (single, _) = single_pool(&pool).await;
    sqlx::query("SELECT set_config('TimeZone','America/New_York',false)")
        .execute(&single)
        .await
        .unwrap();
    let now = START + 24 * 86400 + 60; // 2026-10-31, before the November daylight-saving transition.
    let store = PgDailyRepository::new(single.clone(), Arc::new(Clock(AtomicI64::new(now))));
    let published = store
        .publish(PublishedDaily::new("2026-10-31", 1, RulesSnapshot::bundled()).unwrap())
        .await
        .unwrap();
    let verified = swept(&published);
    let session = participant(&pool, now).await;
    let first = official(
        store
            .submit(session, Uuid::new_v4(), &verified)
            .await
            .unwrap(),
    );
    let durations:(i64,i64,i64)=sqlx::query_as("SELECT (EXTRACT(EPOCH FROM replay_expires_at)-EXTRACT(EPOCH FROM completed_at))::bigint,(EXTRACT(EPOCH FROM record_expires_at)-EXTRACT(EPOCH FROM completed_at))::bigint,(SELECT (EXTRACT(EPOCH FROM verification_expires_at)-EXTRACT(EPOCH FROM (utc_date::timestamp AT TIME ZONE 'UTC')))::bigint FROM daily_definitions) FROM daily_completions").fetch_one(&single).await.unwrap();
    assert_eq!(durations, (604800, 7776000, 604800));
    for query in [
        "UPDATE daily_completions SET completed_at=completed_at+interval '1 second', replay_expires_at=replay_expires_at+interval '1 second', record_expires_at=record_expires_at+interval '1 second'",
        "UPDATE daily_completions SET mistakes=mistakes+1",
        "UPDATE daily_completions SET replay_json='{}'::jsonb",
    ] {
        let error = sqlx::query(query).execute(&single).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    sqlx::query("UPDATE daily_completions SET replay_json=NULL")
        .execute(&single)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE daily_completions SET replay_json='{}'::jsonb")
            .execute(&single)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .submit(session, first.attempt, &verified)
            .await
            .unwrap(),
        DailySubmission::Official(first.clone())
    );
    assert_eq!(
        store.own(session, published.metadata()).await.unwrap(),
        Some(first)
    );
    // Invalid values fail the CHECK itself; they are not merely rejected by uniqueness.
    for (digest, opened, raw, offset) in [
        (vec![1u8; 31], 216, "{}".to_owned(), 0.0),
        (vec![1u8; 32], 0, "{}".to_owned(), 0.0),
        (vec![1u8; 32], 216, "[]".to_owned(), 0.0),
        (
            vec![1u8; 32],
            216,
            format!("{{\"padding\":\"{}\"}}", "x".repeat(2097152)),
            0.0,
        ),
        (vec![1u8; 32], 216, "{}".to_owned(), 0.25),
    ] {
        let other = participant(&pool, now).await;
        let error=sqlx::query("INSERT INTO daily_completions(id,account_id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at) SELECT $1,$2,daily_id,$3,$4,$5,mistakes,replayed_elapsed_ms,completed_at+($7*interval '1 second'),$6::text::jsonb,replay_expires_at+($7*interval '1 second'),record_expires_at+($7*interval '1 second') FROM daily_completions LIMIT 1")
            .bind(Uuid::new_v4()).bind(other.account).bind(Uuid::new_v4()).bind(digest).bind(opened).bind(raw).bind(offset).execute(&single).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    single.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn corrupt_stored_snapshot_or_identifier_fails_closed_without_bundled_fallback() {
    let pool = auth_pool().await;
    let store = PgDailyRepository::new(
        pool.clone(),
        Arc::new(Clock(AtomicI64::new(START + 86400 + 60))),
    );
    for (date, id, rules) in [
        ("2026-10-07", definition().metadata().id(), "{}".to_owned()),
        (
            "2026-10-08",
            format!("2026-10-08:v1:{}", "a".repeat(64)),
            serde_json::to_string(&RulesSnapshot::bundled()).unwrap(),
        ),
    ] {
        sqlx::query("INSERT INTO daily_definitions(id,utc_date,seed_version,rules_snapshot,published_at,verification_expires_at) VALUES($1,$2::text::date,1,$3::text::jsonb,$2::text::date::timestamp AT TIME ZONE 'UTC',($2::text::date::timestamp AT TIME ZONE 'UTC')+604800*interval '1 second')").bind(id).bind(date).bind(rules).execute(&pool).await.unwrap();
        assert!(matches!(
            store.definition(date, 1).await,
            Err(DailyStoreError::Unavailable)
        ));
    }
    close_auth_pool(pool).await;
}
