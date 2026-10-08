use super::rights::account_write;
use super::*;
use liar_server::online::{
    AuthorityRegistry, FinishedMatch, PgResultRepository, PlayerResult, ResultRepository,
    SaveResult,
};

struct ResultClock;
impl AuthClock for ResultClock {
    fn now(&self) -> i64 {
        19000
    }
}
fn finished(accounts: [Option<Uuid>; 2]) -> FinishedMatch {
    use liar_protocol::game::{Outcome, PublicEndReason};
    FinishedMatch {
        id: Uuid::new_v4(),
        rules_hash: liar_core::rules::RulesSnapshot::bundled().hash.clone(),
        seed: [46; 8],
        ended_ms: 243000,
        reason: PublicEndReason::Timeout,
        players: accounts.map(|account| PlayerResult {
            account,
            outcome: Outcome::Draw,
            opened_safe: 4,
            mistakes: 0,
            accusations: 0,
            correct_accusations: 0,
        }),
    }
}
async fn participant(store: &PgAuthStore) -> Uuid {
    let token = SecretToken::generate().unwrap();
    store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            18000,
        ))
        .await
        .unwrap()
        .account
        .id
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn runtime_session_reader_and_logout_share_connection_invalidation() {
    use liar_server::online::{PgSessionReader, SessionReader};
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(2).unwrap();
    let runtime = RuntimeAuthConfig {
        security: BrowserSecurity::new("https://game.example", [5; 32]).unwrap(),
        vault: AeadVault::new(1, vec![(1, [6; 32])]).unwrap(),
        digests: DigestKeys::new(1, vec![(1, [7; 32])]).unwrap(),
        providers: vec![],
        notification_audience: None,
    }
    .initialize_with_invalidations(pool.clone(), registry.clone())
    .unwrap();
    let now = SystemAuthClock.now();
    let token = SecretToken::generate().unwrap();
    let account = runtime
        .store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            now,
        ))
        .await
        .unwrap()
        .account
        .id;
    let reader = PgSessionReader::new(runtime.store.clone(), Arc::new(SystemAuthClock));
    let session = reader
        .read(token.hash())
        .await
        .unwrap()
        .expect("Stored service session must be available");
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            account,
            token.hash(),
            session.expires_at,
            now,
        )
        .unwrap();
    runtime.store.logout(token.hash()).await.unwrap();
    assert!(reader.read(token.hash()).await.unwrap().is_none());
    assert!(registry.with_authority(&lease, now, || ()).is_err());
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn final_result_saves_one_parent_and_both_players_atomically_under_concurrent_retry() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let accounts = [
        Some(participant(&store).await),
        Some(participant(&store).await),
    ];
    let result = finished(accounts);
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    let (a, b) = tokio::join!(
        repository.save(result.clone()),
        repository.save(result.clone())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(matches!(
        (a, b),
        (SaveResult::Saved, SaveResult::Duplicate) | (SaveResult::Duplicate, SaveResult::Saved)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_results WHERE id=$1")
            .bind(result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    let mut conflict = result.clone();
    conflict.ended_ms += 1;
    assert!(repository.save(conflict).await.is_err());
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn deleting_a_participant_and_retrying_cannot_recreate_their_record_or_nickname() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let token = SecretToken::generate().unwrap();
    let account = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            18000,
        ))
        .await
        .unwrap()
        .account
        .id;
    let other = participant(&store).await;
    let result = finished([Some(account), Some(other)]);
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    assert_eq!(
        repository.save(result.clone()).await.unwrap(),
        SaveResult::Saved
    );
    store
        .erase_authorized(SessionAuthority {
            account,
            hash: token.hash(),
            now: 18001,
        })
        .await
        .unwrap();
    assert_eq!(
        repository.save(result.clone()).await.unwrap(),
        SaveResult::Duplicate
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM online_match_players WHERE match_id=$1 AND account_id=$2"
        )
        .bind(result.id)
        .bind(account)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let fresh_result = finished([Some(account), Some(other)]);
    assert_eq!(
        repository.save(fresh_result.clone()).await.unwrap(),
        SaveResult::Saved
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(fresh_result.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let columns: Vec<String> = sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN ('online_match_players','online_match_results')").fetch_all(&pool).await.unwrap();
    for forbidden in [
        "nickname",
        "token_hash",
        "subject_digest",
        "email",
        "password",
    ] {
        assert!(!columns.iter().any(|column| column == forbidden));
    }
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn second_participant_sql_failure_rolls_back_the_parent_and_first_participant() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let accounts = [
        Some(participant(&store).await),
        Some(participant(&store).await),
    ];
    let repository = PgResultRepository::new(pool.clone(), Arc::new(ResultClock));
    assert_eq!(
        repository.save(finished(accounts)).await.unwrap(),
        SaveResult::Saved
    );
    sqlx::query("ALTER TABLE online_match_players ADD CONSTRAINT fixture_fail_second CHECK (seat=0) NOT VALID").execute(&pool).await.unwrap();
    let failed = finished(accounts);
    assert!(repository.save(failed.clone()).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_results WHERE id=$1")
            .bind(failed.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM online_match_players WHERE match_id=$1")
            .bind(failed.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn logout_revokes_memory_authority_before_its_blocked_sql_write_can_commit() {
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(2).unwrap();
    let store = PgAuthStore::new(pool.clone()).with_invalidations(registry.clone());
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            16000,
        ))
        .await
        .unwrap();
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            record.account.id,
            token.hash(),
            20000,
            16000,
        )
        .unwrap();
    let mut revoked = lease.revoked();
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_sessions WHERE token_hash=$1 FOR UPDATE")
        .bind(token.hash().as_slice())
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let hash = token.hash();
    let logout = tokio::spawn(async move { store.logout(hash).await });
    tokio::time::timeout(Duration::from_secs(3), revoked.changed())
        .await
        .expect("memory authority must be revoked before the blocked SQL commit")
        .unwrap();
    assert!(*revoked.borrow());
    assert!(
        registry
            .with_authority(&lease, 16000, || "must not apply")
            .is_err()
    );
    let during = registry.generation().unwrap();
    assert!(
        registry
            .bind(during, record.account.id, hash, 20000, 16000)
            .is_err()
    );
    blocker.rollback().await.unwrap();
    logout.await.unwrap().unwrap();
    assert!(
        PgAuthStore::new(pool.clone())
            .session(hash, 16000)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        registry
            .bind(during, record.account.id, hash, 20000, 16000)
            .is_err()
    );
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn account_erasure_revokes_an_existing_connection_authority() {
    let pool = auth_pool().await;
    let registry = AuthorityRegistry::new(1).unwrap();
    let store = PgAuthStore::new(pool.clone()).with_invalidations(registry.clone());
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            17000,
        ))
        .await
        .unwrap();
    let lease = registry
        .bind(
            registry.generation().unwrap(),
            record.account.id,
            token.hash(),
            20000,
            17000,
        )
        .unwrap();
    store
        .erase_authorized(SessionAuthority {
            account: record.account.id,
            hash: token.hash(),
            now: 17001,
        })
        .await
        .unwrap();
    assert!(
        registry
            .with_authority(&lease, 17001, || "must not apply")
            .is_err()
    );
    assert!(*lease.revoked().borrow());
    close_auth_pool(pool).await;
}
