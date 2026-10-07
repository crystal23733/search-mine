use liar_server::auth::*;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn closed_database_fails_without_granting_authentication_or_leaking_details() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:9/unused")
        .unwrap();
    pool.close().await;
    let store = PgAuthStore::new(pool);
    let browser = SecretToken::generate().unwrap();
    let service = AuthService::new(
        store.clone(),
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
    );
    assert!(matches!(
        service
            .start(
                &browser,
                Provider::Google,
                AuthIntent::Login,
                ReturnPath::Home,
                1000
            )
            .await,
        Err(AuthError::Unavailable)
    ));
    assert!(matches!(
        service
            .consume(&browser, &browser, Provider::Google, 1000)
            .await,
        Err(AuthError::Unavailable)
    ));
    assert!(matches!(
        store
            .login(LoginWrite {
                intent: AuthIntent::Login,
                bound_session: None,
                apple_refresh: None,
                provider: Provider::Google,
                digests: vec![(1, [1; 32])],
                session_hash: browser.hash(),
                previous_session: None,
                now: 1000
            })
            .await,
        Err(AuthError::Unavailable)
    ));
    assert!(matches!(
        service.session(&browser, 1000).await,
        Err(AuthError::Unavailable)
    ));
    assert_eq!(
        store.logout(browser.hash()).await,
        Err(AuthError::Unavailable)
    );
    assert_eq!(
        store
            .nickname(Uuid::new_v4(), Nickname::parse("게임").unwrap())
            .await,
        Err(AuthError::Unavailable)
    );
    assert_eq!(
        store.revoke_account(Uuid::new_v4()).await,
        Err(AuthError::Unavailable)
    );
    assert_eq!(store.cleanup(1000).await, Err(AuthError::Unavailable));
    assert_eq!(store.cleanup(-1).await, Err(AuthError::Invalid));
    assert!(matches!(
        store
            .login(LoginWrite {
                intent: AuthIntent::Login,
                bound_session: None,
                apple_refresh: None,
                provider: Provider::Google,
                digests: vec![],
                session_hash: browser.hash(),
                previous_session: None,
                now: 1000
            })
            .await,
        Err(AuthError::Invalid)
    ));
    assert!(matches!(
        store
            .login(LoginWrite {
                intent: AuthIntent::Login,
                bound_session: None,
                apple_refresh: None,
                provider: Provider::Google,
                digests: vec![(0, [1; 32])],
                session_hash: browser.hash(),
                previous_session: None,
                now: 1000
            })
            .await,
        Err(AuthError::Invalid)
    ));
    assert!(matches!(
        store
            .login(LoginWrite {
                intent: AuthIntent::Login,
                bound_session: None,
                apple_refresh: None,
                provider: Provider::Google,
                digests: vec![(1, [1; 32])],
                session_hash: browser.hash(),
                previous_session: None,
                now: i64::MAX
            })
            .await,
        Err(AuthError::Invalid)
    ));
}
