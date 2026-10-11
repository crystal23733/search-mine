use super::*;
use std::sync::atomic::{AtomicI64, Ordering};
struct MovingClock(AtomicI64);
impl AuthClock for MovingClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
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
            if sqlx::query_scalar::<_, bool>("SELECT cardinality(pg_blocking_pids($1))>0")
                .bind(pid)
                .fetch_one(pool)
                .await
                .unwrap()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn account_lock_resamples_clock_and_observes_concurrent_revocation_deletion() {
    let pool = auth_pool().await;
    for change in 0..5 {
        let session = participant(&pool, START + 60).await;
        let clock = Arc::new(MovingClock(AtomicI64::new(START + 60)));
        let (single, pid) = single_pool(&pool).await;
        let reader = PgGameExportReader::new(single.clone(), clock.clone());
        let mut held = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(session.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let task =
            tokio::spawn(async move { reader.page(session, ExportStream::Online, None).await });
        blocked(&pool, pid).await;
        let expected = match change {
            0 => {
                clock.0.store(START + 359, Ordering::SeqCst);
                None
            }
            1 => {
                clock.0.store(START + 360, Ordering::SeqCst);
                Some(ExportError::Reauth)
            }
            2 => {
                clock.0.store(START + 59, Ordering::SeqCst);
                Some(ExportError::Unavailable)
            }
            3 => {
                sqlx::query("DELETE FROM auth_sessions WHERE id=$1")
                    .bind(session.revision)
                    .execute(&mut *held)
                    .await
                    .unwrap();
                Some(ExportError::Unauthorized)
            }
            _ => {
                sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
                    .bind(session.account)
                    .execute(&mut *held)
                    .await
                    .unwrap();
                Some(ExportError::Unauthorized)
            }
        };
        held.commit().await.unwrap();
        let result = task.await.unwrap();
        if let Some(expected) = expected {
            assert_eq!(result, Err(expected));
        } else {
            assert!(result.unwrap().records.is_empty());
        }
        single.close().await;
    }
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn lock_and_pool_deadlines_cancellation_and_sql_failure_do_not_become_empty_pages() {
    let pool = auth_pool().await;
    let session = participant(&pool, START + 60).await;
    let (single, pid) = single_pool(&pool).await;
    let reader = PgGameExportReader::new(single.clone(), Arc::new(Clock(START + 60)));
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(session.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let running = reader.clone();
    let task = tokio::spawn(async move { running.page(session, ExportStream::Online, None).await });
    blocked(&pool, pid).await;
    assert_eq!(task.await.unwrap(), Err(ExportError::Unavailable));
    held.rollback().await.unwrap();
    let held = single.acquire().await.unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Online, None).await,
        Err(ExportError::Unavailable)
    );
    drop(held);
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(session.account)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let running = reader.clone();
    let task = tokio::spawn(async move { running.page(session, ExportStream::Online, None).await });
    blocked(&pool, pid).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    held.rollback().await.unwrap();
    assert!(
        reader
            .page(session, ExportStream::Online, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    sqlx::query("ALTER TABLE online_match_players RENAME TO unavailable_players")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Online, None).await,
        Err(ExportError::Unavailable)
    );
    sqlx::query("ALTER TABLE unavailable_players RENAME TO online_match_players")
        .execute(&pool)
        .await
        .unwrap();
    single.close().await;
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn daily_metadata_pages_cross_fifty_with_stable_ids_and_timezone_independent_expiry() {
    let pool = auth_pool().await;
    let session = participant(&pool, START + 60).await;
    let rules = liar_core::rules::RulesSnapshot::bundled();
    for n in 0..51i32 {
        let date: String = sqlx::query_scalar("SELECT to_char(date '2026-10-07'-$1,'YYYY-MM-DD')")
            .bind(n)
            .fetch_one(&pool)
            .await
            .unwrap();
        let metadata = liar_core::daily::DailyMetadata::derive(&date, 1, &rules).unwrap();
        let start = START - i64::from(n) * 86400;
        sqlx::query("INSERT INTO daily_definitions(id,utc_date,seed_version,rules_snapshot,published_at,verification_expires_at) VALUES($1,$2::text::date,1,$3::text::jsonb,to_timestamp($4::double precision),to_timestamp($5::double precision))").bind(metadata.id()).bind(&date).bind(serde_json::to_string(&rules).unwrap()).bind(start+60).bind(start+604800).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO daily_completions(id,account_id,daily_id,attempt_id,replay_digest,opened_safe,mistakes,replayed_elapsed_ms,completed_at,replay_json,replay_expires_at,record_expires_at) VALUES($1,$2,$3,$4,$5,216,0,3200,to_timestamp($6::double precision),NULL,to_timestamp($7::double precision),to_timestamp($8::double precision))").bind(Uuid::from_u128((n+1) as u128)).bind(session.account).bind(metadata.id()).bind(Uuid::new_v4()).bind([0u8;32].as_slice()).bind(start+60).bind(start+60+604800).bind(start+60+7776000).execute(&pool).await.unwrap();
    }
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(START + 60)));
    let first = reader
        .page(session, ExportStream::Daily, None)
        .await
        .unwrap();
    assert_eq!(first.records.len(), 50);
    assert_eq!(first.next, Some(Uuid::from_u128(50).to_string()));
    let second = reader
        .page(session, ExportStream::Daily, Some(Uuid::from_u128(50)))
        .await
        .unwrap();
    assert_eq!(second.records.len(), 1);
    assert!(second.next.is_none());
    let (tz, _) = single_pool(&pool).await;
    sqlx::query("SET TIME ZONE 'America/New_York'")
        .execute(&tz)
        .await
        .unwrap();
    let tz_reader = PgGameExportReader::new(tz.clone(), Arc::new(Clock(START + 60)));
    assert_eq!(
        tz_reader
            .page(session, ExportStream::Daily, None)
            .await
            .unwrap(),
        first
    );
    tz.close().await;
    close_auth_pool(pool).await;
}
