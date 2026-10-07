use liar_server::auth::*;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct FakeStore {
    malicious: bool,
    transactions: Mutex<Vec<AuthTransaction>>,
    logins: Mutex<Vec<LoginWrite>>,
}
#[tokio::test]
async fn localized_transaction_keeps_only_an_allowed_server_bound_locale() {
    let service = AuthService::new(
        FakeStore::default(),
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
    );
    let browser = SecretToken::generate().unwrap();
    let a = service
        .start_localized(
            &browser,
            Provider::Google,
            AuthIntent::Login,
            ReturnPath::Daily,
            AuthLocale::parse("ko").unwrap(),
            1000,
        )
        .await
        .unwrap();
    let transaction = service
        .consume(&a.state, &browser, Provider::Google, 1001)
        .await
        .unwrap();
    assert_eq!(transaction.locale.as_str(), "ko");
    assert_eq!(transaction.return_path, ReturnPath::Daily);
    for locale in ["en", "ko", "ja", "zh-CN", "es", "pt-BR", "de", "fr"] {
        assert_eq!(AuthLocale::parse(locale).unwrap().as_str(), locale);
    }
    for locale in ["zh", "EN", "../../evil", "en?url=evil", ""] {
        assert!(AuthLocale::parse(locale).is_err());
    }
}
impl AuthStore for FakeStore {
    async fn insert_transaction(&self, value: AuthTransaction) -> Result<(), AuthError> {
        self.transactions.lock().unwrap().push(value);
        Ok(())
    }
    async fn consume_transaction(
        &self,
        state: [u8; 32],
        browser: [u8; 32],
        provider: Provider,
        now: i64,
    ) -> Result<Option<AuthTransaction>, AuthError> {
        let mut values = self.transactions.lock().unwrap();
        if self.malicious {
            return Ok(values.pop());
        }
        let index = values.iter().position(|t| {
            t.state_hash == state
                && t.browser_hash == browser
                && t.provider == provider
                && now >= t.created_at
                && now < t.expires_at
        });
        Ok(index.map(|i| values.remove(i)))
    }
    async fn login(&self, value: LoginWrite) -> Result<LoginRecord, AuthError> {
        self.logins.lock().unwrap().push(value);
        Ok(LoginRecord {
            account: Account {
                id: Uuid::new_v4(),
                nickname: None,
            },
            identity_id: Uuid::new_v4(),
        })
    }
    async fn session(&self, _hash: [u8; 32], _now: i64) -> Result<Option<Session>, AuthError> {
        Ok(None)
    }
    async fn nickname(&self, _account: Uuid, _value: Nickname) -> Result<bool, AuthError> {
        Ok(false)
    }
    async fn logout(&self, _hash: [u8; 32]) -> Result<(), AuthError> {
        Ok(())
    }
    async fn revoke_account(&self, _account: Uuid) -> Result<(), AuthError> {
        Ok(())
    }
    async fn cleanup(&self, _now: i64) -> Result<(), AuthError> {
        Ok(())
    }
}
fn service() -> AuthService<FakeStore, AeadVault> {
    AuthService::new(
        FakeStore::default(),
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
    )
}

#[tokio::test]
async fn account_intents_cannot_start_without_an_authoritative_session() {
    let service = service();
    let browser = SecretToken::generate().unwrap();
    for intent in [
        AuthIntent::Link(Uuid::new_v4()),
        AuthIntent::Reauth(Uuid::new_v4()),
    ] {
        assert!(matches!(
            service
                .start(
                    &browser,
                    Provider::Google,
                    intent,
                    ReturnPath::Settings,
                    1000
                )
                .await,
            Err(AuthError::Invalid)
        ));
    }
    assert!(service.store.transactions.lock().unwrap().is_empty());
}

#[tokio::test]
async fn service_rejects_a_repository_returning_misbound_or_expired_transactions() {
    let service = AuthService::new(
        FakeStore {
            malicious: true,
            ..FakeStore::default()
        },
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
    );
    let browser = SecretToken::generate().unwrap();
    for (provider, now, foreign) in [
        (Provider::Apple, 1001, false),
        (Provider::Google, 999, false),
        (Provider::Google, 1300, false),
        (Provider::Google, 1001, true),
    ] {
        let auth = service
            .start(
                &browser,
                Provider::Google,
                AuthIntent::Login,
                ReturnPath::Home,
                1000,
            )
            .await
            .unwrap();
        let other = SecretToken::generate().unwrap();
        assert!(matches!(
            service
                .consume(
                    &auth.state,
                    if foreign { &other } else { &browser },
                    provider,
                    now
                )
                .await,
            Err(AuthError::Invalid)
        ));
    }
}

#[tokio::test]
async fn start_stores_only_bound_digests_and_encrypted_pkce_then_consumes_once() {
    let service = service();
    let browser = SecretToken::generate().unwrap();
    for provider in [
        Provider::Google,
        Provider::Apple,
        Provider::Kakao,
        Provider::Naver,
    ] {
        let auth = service
            .start(
                &browser,
                provider,
                AuthIntent::Login,
                ReturnPath::Daily,
                1000,
            )
            .await
            .unwrap();
        assert!(
            service
                .consume(
                    &auth.state,
                    &SecretToken::generate().unwrap(),
                    provider,
                    1001
                )
                .await
                .is_err()
        );
        let other = if provider == Provider::Apple {
            Provider::Google
        } else {
            Provider::Apple
        };
        assert!(
            service
                .consume(&auth.state, &browser, other, 1001)
                .await
                .is_err()
        );
        assert!(
            service
                .consume(&auth.state, &browser, provider, 999)
                .await
                .is_err()
        );
        let tx = service
            .consume(&auth.state, &browser, provider, 1299)
            .await
            .unwrap();
        assert_eq!(tx.nonce_hash, auth.nonce.hash());
        assert_eq!(tx.intent, AuthIntent::Login);
        assert_eq!(tx.return_path, ReturnPath::Daily);
        assert_eq!(tx.expires_at - tx.created_at, 300);
        assert_eq!(tx.encrypted_verifier.is_some(), provider.uses_pkce());
        if let Some(verifier) = auth.verifier {
            let encrypted = tx.encrypted_verifier.as_ref().unwrap();
            assert!(
                !encrypted
                    .windows(43)
                    .any(|w| w == verifier.expose().as_bytes())
            );
            assert_eq!(
                service.verifier(&tx).unwrap().unwrap().as_slice(),
                verifier.expose().as_bytes()
            );
        } else {
            assert!(service.verifier(&tx).unwrap().is_none())
        }
        assert!(
            service
                .consume(&auth.state, &browser, provider, 1001)
                .await
                .is_err()
        );
    }
    let auth = service
        .start(
            &browser,
            Provider::Google,
            AuthIntent::Login,
            ReturnPath::Settings,
            1000,
        )
        .await
        .unwrap();
    assert!(
        service
            .consume(&auth.state, &browser, Provider::Google, 1300)
            .await
            .is_err()
    );
    for now in [-1, i64::MAX] {
        assert!(
            service
                .start(
                    &browser,
                    Provider::Google,
                    AuthIntent::Login,
                    ReturnPath::Home,
                    now
                )
                .await
                .is_err()
        )
    }
}

#[tokio::test]
async fn verified_login_issues_new_session_and_rejects_mixup_link_or_expired_transaction() {
    let service = service();
    let browser = SecretToken::generate().unwrap();
    let old = SecretToken::generate().unwrap();
    let auth = service
        .start(
            &browser,
            Provider::Google,
            AuthIntent::Login,
            ReturnPath::Home,
            1000,
        )
        .await
        .unwrap();
    let tx = service
        .consume(&auth.state, &browser, Provider::Google, 1001)
        .await
        .unwrap();
    let session = service
        .finish_login(tx, Provider::Google, "verified-sub", Some(&old), 1001)
        .await
        .unwrap();
    assert_eq!(session.account.nickname, None);
    assert_ne!(session.token.hash(), old.hash());
    {
        let writes = service.store.logins.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].previous_session, Some(old.hash()));
        assert_eq!(writes[0].session_hash, session.token.hash());
        assert_eq!(writes[0].digests.len(), 1);
    }
    for (intent, provider, now, subject) in [
        (
            AuthIntent::Link(Uuid::new_v4()),
            Provider::Google,
            1001,
            "sub",
        ),
        (
            AuthIntent::Reauth(Uuid::new_v4()),
            Provider::Google,
            1001,
            "sub",
        ),
        (AuthIntent::Login, Provider::Apple, 1001, "sub"),
        (AuthIntent::Login, Provider::Google, 1300, "sub"),
        (AuthIntent::Login, Provider::Google, 1001, ""),
    ] {
        let auth = service
            .start(
                &browser,
                Provider::Google,
                AuthIntent::Login,
                ReturnPath::Home,
                1000,
            )
            .await
            .unwrap();
        let mut tx = service
            .consume(&auth.state, &browser, Provider::Google, 1001)
            .await
            .unwrap();
        tx.intent = intent;
        assert!(
            service
                .finish_login(tx, provider, subject, None, now)
                .await
                .is_err()
        );
    }
    assert_eq!(service.store.logins.lock().unwrap().len(), 1);
}
