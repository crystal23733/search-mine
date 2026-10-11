use super::*;
use liar_protocol::privacy::*;
use liar_server::privacy::*;

struct Clock(i64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0
    }
}
async fn participant(pool: &sqlx::PgPool, now: i64) -> ExportSession {
    let token = SecretToken::generate().unwrap();
    let store = PgAuthStore::new(pool.clone());
    let login = store
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            now,
        ))
        .await
        .unwrap();
    let session = store.session(token.hash(), now).await.unwrap().unwrap();
    ExportSession {
        account: login.account.id,
        revision: session.id,
        hash: token.hash(),
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn nickname_is_not_required_for_empty_self_export_and_freshness_is_strict() {
    let pool = auth_pool().await;
    let session = participant(&pool, 14000).await;
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(14299)));
    for stream in [ExportStream::Online, ExportStream::Daily] {
        let page = reader.page(session, stream, None).await.unwrap();
        assert!(page.records.is_empty());
        assert!(page.next.is_none());
        assert_eq!(page.account_id, session.account.to_string());
        assert_eq!(page.stream, stream);
    }
    assert_eq!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(14300)))
            .page(session, ExportStream::Online, None)
            .await,
        Err(ExportError::Reauth)
    );
    close_auth_pool(pool).await;
}

async fn seed_online(pool: &sqlx::PgPool, account: Uuid, id: Uuid, at: i64) {
    sqlx::query("INSERT INTO online_match_results(id,rules_hash,secret_seed,end_elapsed_ms,reason,recorded_at,seed_expires_at) VALUES($1,$2,$3,243000,'timeout',to_timestamp($4::double precision),to_timestamp($4::double precision)+interval '7 days')").bind(id).bind(liar_core::rules::RulesSnapshot::bundled().hash).bind([46u8;8].as_slice()).bind(at).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO online_match_players(match_id,seat,account_id,is_bot,outcome,opened_safe,mistakes,accusations,correct_accusations) VALUES($1,0,$2,false,'draw',4,0,0,0),($1,1,NULL,true,'draw',99,8,5,3)").bind(id).bind(account).execute(pool).await.unwrap();
}
async fn fresh(pool: &sqlx::PgPool, session: ExportSession, at: i64) {
    sqlx::query("UPDATE auth_sessions SET created_at=to_timestamp($2::double precision),authenticated_at=to_timestamp($2::double precision),expires_at=to_timestamp($2::double precision)+interval '30 days' WHERE id=$1").bind(session.revision).bind(at).execute(pool).await.unwrap();
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn own_online_pages_advance_over_fifty_records_without_truth_or_foreign_rows() {
    let pool = auth_pool().await;
    let session = participant(&pool, 14000).await;
    let other = participant(&pool, 14000).await;
    for n in 1..=51 {
        seed_online(&pool, session.account, Uuid::from_u128(n), 14000).await;
    }
    seed_online(&pool, other.account, Uuid::from_u128(52), 14000).await;
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(14000)));
    let first = reader
        .page(session, ExportStream::Online, None)
        .await
        .unwrap();
    assert_eq!(first.records.len(), 50);
    assert_eq!(first.next, Some(Uuid::from_u128(50).to_string()));
    let second = reader
        .page(
            session,
            ExportStream::Online,
            first.next.as_deref().map(|s| Uuid::parse_str(s).unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(second.records.len(), 1);
    assert!(second.next.is_none());
    assert!(
        reader
            .page(session, ExportStream::Online, Some(Uuid::from_u128(52)))
            .await
            .unwrap()
            .records
            .is_empty()
    );
    let json = serde_json::to_string(&first).unwrap();
    for secret in [
        "seed",
        "board",
        "nickname",
        "account_id\":\"secret",
        "correct_accusations\":3",
        "opened_safe\":99",
    ] {
        assert!(!json.contains(secret));
    }
    let bad = ExportSession {
        account: other.account,
        ..session
    };
    assert_eq!(
        reader.page(bad, ExportStream::Online, None).await,
        Err(ExportError::Unauthorized)
    );
    assert_eq!(
        reader
            .page(
                ExportSession {
                    revision: Uuid::new_v4(),
                    ..session
                },
                ExportStream::Online,
                None
            )
            .await,
        Err(ExportError::Unauthorized)
    );
    assert_eq!(
        reader
            .page(session, ExportStream::Online, Some(Uuid::nil()))
            .await,
        Err(ExportError::Malformed)
    );
    fresh(&pool, session, 14000 + 7775999).await;
    assert_eq!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(14000 + 7775999)))
            .page(session, ExportStream::Online, None)
            .await
            .unwrap()
            .records
            .len(),
        50
    );
    fresh(&pool, session, 14000 + 7776000).await;
    assert!(
        PgGameExportReader::new(pool.clone(), Arc::new(Clock(14000 + 7776000)))
            .page(session, ExportStream::Online, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn duplicate_human_seats_and_corrupt_online_projection_fail_closed() {
    let pool = auth_pool().await;
    let session = participant(&pool, 14000).await;
    let id = Uuid::new_v4();
    seed_online(&pool, session.account, id, 14000).await;
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(14000)));
    sqlx::query(
        "UPDATE online_match_players SET account_id=$1,is_bot=false WHERE match_id=$2 AND seat=1",
    )
    .bind(session.account)
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Online, None).await,
        Err(ExportError::Unavailable)
    );
    sqlx::query("DELETE FROM online_match_players WHERE match_id=$1 AND seat=1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let protected = sqlx::query(
        "UPDATE online_match_players SET correct_accusations=4,accusations=0 WHERE match_id=$1",
    )
    .bind(id)
    .execute(&pool)
    .await;
    assert!(protected.is_err());
    sqlx::query("UPDATE online_match_results SET reason='server_failure' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Online, None).await,
        Err(ExportError::Unavailable)
    );
    close_auth_pool(pool).await;
}
const START: i64 = 1791331200;
#[path = "privacy_lifetime.rs"]
mod lifetime;
async fn completed(pool: &sqlx::PgPool) -> (ExportSession, Uuid) {
    use liar_server::daily::*;
    let session = participant(pool, START + 60).await;
    let auth = PgAuthStore::new(pool.clone());
    auth.nickname(session.account, Nickname::parse("Daily Player").unwrap())
        .await
        .unwrap();
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
    let daily = PgDailyRepository::new(pool.clone(), Arc::new(Clock(START + 60)));
    daily.publish(definition).await.unwrap();
    let DailySubmission::Official(receipt) = daily
        .submit(
            DailySession {
                account: session.account,
                hash: session.hash,
            },
            Uuid::new_v4(),
            &evidence,
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    sqlx::query("UPDATE auth_accounts SET nickname=NULL WHERE id=$1")
        .bind(session.account)
        .execute(pool)
        .await
        .unwrap();
    (session, receipt.id)
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn daily_raw_is_self_only_and_exact_seven_ninety_days_use_elapsed_seconds() {
    let pool = auth_pool().await;
    let (session, completion) = completed(&pool).await;
    let other = participant(&pool, START + 60).await;
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(START + 60)));
    let page = reader
        .page(session, ExportStream::Daily, None)
        .await
        .unwrap();
    assert_eq!(page.records.len(), 1);
    let ExportRecord::Daily {
        replay_available,
        completion_id,
        ..
    } = &page.records[0]
    else {
        panic!()
    };
    assert!(*replay_available);
    assert_eq!(completion_id, &completion.to_string());
    assert!(
        reader
            .replay(session, completion)
            .await
            .unwrap()
            .replay
            .is_some()
    );
    assert!(
        reader
            .replay(other, completion)
            .await
            .unwrap()
            .replay
            .is_none()
    );
    assert!(
        reader
            .page(other, ExportStream::Daily, None)
            .await
            .unwrap()
            .records
            .is_empty()
    );
    for (offset, exists, raw) in [
        (604799, true, true),
        (604800, true, false),
        (7775999, true, false),
        (7776000, false, false),
    ] {
        let now = START + 60 + offset;
        fresh(&pool, session, now).await;
        let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(now)));
        assert_eq!(
            !reader
                .page(session, ExportStream::Daily, None)
                .await
                .unwrap()
                .records
                .is_empty(),
            exists
        );
        assert_eq!(
            reader
                .replay(session, completion)
                .await
                .unwrap()
                .replay
                .is_some(),
            raw
        );
    }
    let raw_exists: bool =
        sqlx::query_scalar("SELECT replay_json IS NOT NULL FROM daily_completions WHERE id=$1")
            .bind(completion)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        raw_exists,
        "expiry filtering must work before physical cleanup"
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn nullable_replay_integrity_and_erasure_are_not_empty_success_on_failure() {
    let pool = auth_pool().await;
    let (session, completion) = completed(&pool).await;
    let reader = PgGameExportReader::new(pool.clone(), Arc::new(Clock(START + 60)));
    sqlx::query("ALTER TABLE daily_completions DISABLE TRIGGER USER")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE daily_completions SET replay_digest=$1 WHERE id=$2")
        .bind([1u8; 32].as_slice())
        .bind(completion)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.replay(session, completion).await,
        Err(ExportError::Unavailable)
    );
    sqlx::query("ALTER TABLE daily_completions ENABLE TRIGGER USER")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE daily_completions SET replay_json=NULL WHERE id=$1")
        .bind(completion)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        reader
            .replay(session, completion)
            .await
            .unwrap()
            .replay
            .is_none()
    );
    assert!(matches!(
        reader
            .page(session, ExportStream::Daily, None)
            .await
            .unwrap()
            .records[0],
        ExportRecord::Daily {
            replay_available: false,
            ..
        }
    ));
    sqlx::query("DELETE FROM auth_sessions WHERE id=$1")
        .bind(session.revision)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Daily, None).await,
        Err(ExportError::Unauthorized)
    );
    assert_eq!(
        reader.replay(session, completion).await,
        Err(ExportError::Unauthorized)
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(session.account)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reader.page(session, ExportStream::Daily, None).await,
        Err(ExportError::Unauthorized)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM daily_completions WHERE id=$1")
            .bind(completion)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
