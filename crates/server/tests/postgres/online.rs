use super::rights::account_write;
use super::*;
use liar_server::online::AuthorityRegistry;

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
