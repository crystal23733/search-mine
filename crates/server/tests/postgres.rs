use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_server::app;
use liar_server::auth::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Row, migrate::Migrator};
use std::path::Path;
use std::sync::Arc;
use std::{env, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;
use zeroize::Zeroizing;

struct HttpClock;
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn apple_credentials_and_authorized_erasure_are_atomic() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [11; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone());
    let token = SecretToken::generate().unwrap();
    let result = store
        .login(LoginWrite {
            intent: AuthIntent::Login,
            bound_session: None,
            provider: Provider::Apple,
            digests: DigestKeys::new(1, vec![(1, [12; 32])])
                .unwrap()
                .digest(Provider::Apple, &Uuid::new_v4().to_string())
                .unwrap(),
            session_hash: token.hash(),
            previous_session: None,
            now: 4000,
            apple_refresh: Some(Zeroizing::new("fixture-refresh-secret".into())),
        })
        .await
        .expect("Apple login must atomically persist its encrypted revoke credential");
    let bytes: Vec<u8> = sqlx::query_scalar(
        "SELECT encrypted_refresh_token FROM auth_credentials WHERE identity_id=$1",
    )
    .bind(result.identity_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!bytes.windows(22).any(|w| w == b"fixture-refresh-secret"));
    assert_eq!(
        vault
            .open(
                result.identity_id,
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                &bytes
            )
            .unwrap()
            .as_slice(),
        b"fixture-refresh-secret"
    );
    let erased = store
        .erase_authorized(SessionAuthority {
            account: result.account.id,
            hash: token.hash(),
            now: 4001,
        })
        .await
        .expect("local erasure must succeed without a reachable provider");
    assert!(!erased.manual_apple_disconnect);
    assert!(store.session(token.hash(), 4001).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM auth_deletion_tombstones WHERE account_id=$1"
        )
        .bind(result.account.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM auth_apple_revoke_queue WHERE identity_id=$1"
        )
        .bind(result.identity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let job = store
        .claim_revoke(4001)
        .await
        .expect("encrypted queue must be claimable after local deletion")
        .unwrap();
    assert_eq!(job.identity, result.identity_id);
    assert_eq!(
        vault
            .open(
                job.identity,
                Provider::Apple,
                CredentialPurpose::AppleRevoke,
                &job.encrypted
            )
            .unwrap()
            .as_slice(),
        b"fixture-refresh-secret"
    );
    assert!(store.claim_revoke(4001).await.unwrap().is_none());
    store.finish_revoke(&job, false, 4002).await.unwrap();
    assert!(store.claim_revoke(4003).await.unwrap().is_none());
    let retry = store.claim_revoke(7602).await.unwrap().unwrap();
    store.finish_revoke(&job, true, 7602).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_apple_revoke_queue WHERE id=$1")
            .bind(retry.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1,
        "stale lease cannot acknowledge a newer claim"
    );
    store.finish_revoke(&retry, true, 7603).await.unwrap();
    pool.close().await;
}
impl AuthClock for HttpClock {
    fn now(&self) -> i64 {
        1100
    }
}
struct HttpProvider {
    subject: String,
}
impl OAuthProvider for HttpProvider {
    fn available(&self, p: Provider) -> bool {
        p == Provider::Google
    }
    fn authorize(&self, _p: Provider, a: &Authorization) -> Result<String, AuthError> {
        Ok(format!(
            "https://fixture.example/?state={}",
            a.state.expose().as_str()
        ))
    }
    async fn exchange(
        &self,
        _t: &AuthTransaction,
        _s: &SecretToken,
        code: &str,
        _v: Option<&[u8]>,
        _now: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        if code != "fixture-code" {
            return Err(AuthError::Invalid);
        }
        Ok(VerifiedIdentity {
            subject: Zeroizing::new(self.subject.clone()),
            apple_refresh: None,
        })
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn http_auth_with_real_storage_binds_locale_consumes_once_and_rotates_revision() {
    let pool = auth_pool().await;
    let router = auth_router(
        AuthService::new(
            PgAuthStore::new(pool.clone()),
            AeadVault::new(1, vec![(1, [7; 32])]).unwrap(),
            DigestKeys::new(1, vec![(1, [8; 32])]).unwrap(),
        ),
        HttpProvider {
            subject: Uuid::new_v4().to_string(),
        },
        BrowserSecurity::new("https://game.example", [9; 32]).unwrap(),
        Arc::new(HttpClock),
    );
    let mut cookies = String::new();
    let mut revision = None;
    let mut account_id = None;
    for round in 0..2 {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/bootstrap")
                    .header("cookie", &cookies)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let browser = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .find(|v| v.to_str().unwrap().starts_with("__Host-liar_browser="))
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let bootstrap: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        let csrf = bootstrap["csrf"].as_str().unwrap();
        if round == 0 {
            cookies = browser.clone();
        }
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/google/start")
                    .header("cookie", &cookies)
                    .header("origin", "https://game.example")
                    .header("x-liar-csrf", csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"locale":"ko","return_path":"daily"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let start: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        let url = url::Url::parse(start["authorize_url"].as_str().unwrap()).unwrap();
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .to_string();
        let state_token = SecretToken::parse(&state).unwrap();
        let saved: String =
            sqlx::query_scalar("SELECT locale FROM auth_transactions WHERE state_hash=$1")
                .bind(state_token.hash().as_slice())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(saved, "ko");
        let callback = format!(
            "/api/v1/auth/google/callback?state={state}&code=fixture-code&iss=https%3A%2F%2Faccounts.google.com"
        );
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&callback)
                    .header("cookie", &cookies)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let session_cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let token = SecretToken::parse(session_cookie.split_once('=').unwrap().1).unwrap();
        let session = PgAuthStore::new(pool.clone())
            .session(token.hash(), 1100)
            .await
            .unwrap()
            .unwrap();
        if let Some(id) = account_id {
            assert_eq!(session.account.id, id);
        } else {
            account_id = Some(session.account.id);
        }
        if let Some(old) = revision {
            assert_ne!(session.id, old);
            let old_token =
                SecretToken::parse(cookies.split("__Host-liar_session=").nth(1).unwrap()).unwrap();
            assert!(
                PgAuthStore::new(pool.clone())
                    .session(old_token.hash(), 1100)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        revision = Some(session.id);
        cookies = format!("{browser}; {session_cookie}");
        let replay = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&callback)
                    .header("cookie", &cookies)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    }
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(account_id.unwrap())
        .execute(&pool)
        .await
        .unwrap();
}

async fn auth_pool() -> sqlx::PgPool {
    let url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
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
    pool
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn simultaneous_identity_creation_and_failed_rotation_preserve_one_account_and_old_session() {
    let pool = auth_pool().await;
    let store = PgAuthStore::with_vault(
        pool.clone(),
        Arc::new(AeadVault::new(1, vec![(1, [13; 32])]).unwrap()),
    );
    let keys = DigestKeys::new(2, vec![(1, [1; 32]), (2, [2; 32])]).unwrap();
    let subject = Uuid::new_v4().to_string();
    let first_token = SecretToken::generate().unwrap();
    let second_token = SecretToken::generate().unwrap();
    let request = |token: &SecretToken| LoginWrite {
        intent: AuthIntent::Login,
        bound_session: None,
        apple_refresh: None,
        provider: Provider::Google,
        digests: keys.digest(Provider::Google, &subject).unwrap(),
        session_hash: token.hash(),
        previous_session: None,
        now: 2000,
    };
    let (first, second) = tokio::join!(
        store.login(request(&first_token)),
        store.login(request(&second_token))
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.account.id, second.account.id);
    assert_eq!(first.identity_id, second.identity_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_identities WHERE account_id=$1")
            .bind(first.account.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let conflict = LoginWrite {
        intent: AuthIntent::Login,
        bound_session: None,
        apple_refresh: None,
        provider: Provider::Google,
        digests: keys.digest(Provider::Google, &subject).unwrap(),
        session_hash: second_token.hash(),
        previous_session: Some(first_token.hash()),
        now: 2001,
    };
    assert!(matches!(
        store.login(conflict).await,
        Err(AuthError::Conflict)
    ));
    assert!(
        store
            .session(first_token.hash(), 2001)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .session(second_token.hash(), 2001)
            .await
            .unwrap()
            .is_some()
    );
    let fresh = SecretToken::generate().unwrap();
    let failed = LoginWrite {
        intent: AuthIntent::Login,
        bound_session: None,
        apple_refresh: Some(Zeroizing::new("fixture-apple-refresh".into())),
        provider: Provider::Apple,
        digests: keys.digest(Provider::Apple, &subject).unwrap(),
        session_hash: second_token.hash(),
        previous_session: None,
        now: 2001,
    };
    assert!(matches!(
        store.login(failed).await,
        Err(AuthError::Conflict)
    ));
    let apple = store
        .login(LoginWrite {
            intent: AuthIntent::Login,
            bound_session: None,
            apple_refresh: Some(Zeroizing::new("fixture-apple-refresh".into())),
            provider: Provider::Apple,
            digests: keys.digest(Provider::Apple, &subject).unwrap(),
            session_hash: fresh.hash(),
            previous_session: None,
            now: 2001,
        })
        .await
        .unwrap();
    assert_ne!(apple.account.id, first.account.id);
    let row=sqlx::query("SELECT EXTRACT(EPOCH FROM created_at)::bigint AS created_at FROM auth_accounts WHERE id=$1").bind(apple.account.id).fetch_one(&pool).await.unwrap();
    assert_eq!(row.get::<i64, _>("created_at"), 2001);
    store.revoke_account(first.account.id).await.unwrap();
    assert!(
        store
            .session(first_token.hash(), 2002)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .session(second_token.hash(), 2002)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.session(fresh.hash(), 2002).await.unwrap().is_some());
    sqlx::query("DELETE FROM auth_accounts WHERE id=ANY($1)")
        .bind(vec![first.account.id, apple.account.id])
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL role administration in isolated test database"]
async fn serving_dml_role_can_authenticate_but_cannot_migrate_or_create_tables() {
    let pool = auth_pool().await;
    sqlx::query("DO $$ BEGIN CREATE ROLE liar_auth_test_dml NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$").execute(&pool).await.unwrap();
    sqlx::query("GRANT USAGE ON SCHEMA public TO liar_auth_test_dml")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("GRANT SELECT,INSERT,UPDATE,DELETE ON auth_accounts,auth_identities,auth_credentials,auth_sessions,auth_transactions TO liar_auth_test_dml").execute(&pool).await.unwrap();
    let url = env::var("DATABASE_URL").unwrap();
    let dml = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE liar_auth_test_dml")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    assert!(
        sqlx::query("CREATE TABLE auth_forbidden_test(id integer)")
            .execute(&dml)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("SELECT * FROM _sqlx_migrations")
            .fetch_all(&dml)
            .await
            .is_err()
    );
    let store = PgAuthStore::new(dml.clone());
    let token = SecretToken::generate().unwrap();
    let result = store
        .login(LoginWrite {
            intent: AuthIntent::Login,
            bound_session: None,
            apple_refresh: None,
            provider: Provider::Naver,
            digests: DigestKeys::new(1, vec![(1, [1; 32])])
                .unwrap()
                .digest(Provider::Naver, &Uuid::new_v4().to_string())
                .unwrap(),
            session_hash: token.hash(),
            previous_session: None,
            now: 3000,
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .session(token.hash(), 3000)
            .await
            .unwrap()
            .unwrap()
            .account
            .id,
        result.account.id
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(result.account.id)
        .execute(&dml)
        .await
        .unwrap();
    dml.close().await;
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn auth_storage_enforces_minimal_data_atomic_consumption_rotation_and_constraints() {
    let url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(&url)
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
    let store = PgAuthStore::new(pool.clone());
    let service = AuthService::new(
        store.clone(),
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
    );
    let browser = SecretToken::generate().unwrap();
    let auth = service
        .start(
            &browser,
            Provider::Google,
            AuthIntent::Login,
            ReturnPath::Daily,
            1000,
        )
        .await
        .unwrap();
    let raw=sqlx::query("SELECT state_hash, browser_hash, nonce_hash, encrypted_verifier FROM auth_transactions WHERE id=$1").bind(auth.id).fetch_one(&pool).await.unwrap();
    assert_eq!(raw.get::<Vec<u8>, _>("state_hash"), auth.state.hash());
    assert_ne!(
        raw.get::<Vec<u8>, _>("state_hash"),
        auth.state.expose().as_bytes()
    );
    assert!(
        service
            .consume(
                &auth.state,
                &SecretToken::generate().unwrap(),
                Provider::Google,
                1001
            )
            .await
            .is_err()
    );
    assert!(
        service
            .consume(&auth.state, &browser, Provider::Apple, 1001)
            .await
            .is_err()
    );
    assert!(
        service
            .consume(&auth.state, &browser, Provider::Google, 999)
            .await
            .is_err()
    );
    let (one, two) = tokio::join!(
        service.consume(&auth.state, &browser, Provider::Google, 1001),
        service.consume(&auth.state, &browser, Provider::Google, 1001)
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let tx = one.or(two).unwrap();
    let subject = Uuid::new_v4().to_string();
    let first = service
        .finish_login(tx, Provider::Google, &subject, None, 1001)
        .await
        .unwrap();
    assert_eq!(first.account.nickname, None);
    let session = service.session(&first.token, 1001).await.unwrap().unwrap();
    assert_eq!(session.expires_at - session.created_at, SESSION_SECONDS);
    assert!(service.session(&first.token, 1000).await.unwrap().is_none());
    assert!(
        service
            .session(&first.token, session.expires_at)
            .await
            .unwrap()
            .is_none()
    );
    let rotated = AuthService::new(
        store.clone(),
        AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
        DigestKeys::new(2, vec![(1, [2; 32]), (2, [3; 32])]).unwrap(),
    );
    let auth = rotated
        .start(
            &browser,
            Provider::Google,
            AuthIntent::Login,
            ReturnPath::Home,
            1010,
        )
        .await
        .unwrap();
    let tx = rotated
        .consume(&auth.state, &browser, Provider::Google, 1010)
        .await
        .unwrap();
    let second = rotated
        .finish_login(tx, Provider::Google, &subject, Some(&first.token), 1010)
        .await
        .unwrap();
    assert_eq!(first.account.id, second.account.id);
    assert_eq!(first.identity_id, second.identity_id);
    assert!(service.session(&first.token, 1010).await.unwrap().is_none());
    assert!(
        store
            .nickname(first.account.id, Nickname::parse("게임 이름").unwrap())
            .await
            .unwrap()
    );
    assert_eq!(
        service
            .session(&second.token, 1011)
            .await
            .unwrap()
            .unwrap()
            .account
            .nickname
            .unwrap()
            .as_str(),
        "게임 이름"
    );
    let row = sqlx::query("SELECT digest_key_version FROM auth_identities WHERE id=$1")
        .bind(first.identity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i32, _>("digest_key_version"), 2);
    assert!(sqlx::query("INSERT INTO auth_credentials(identity_id,encrypted_refresh_token,updated_at) VALUES($1,$2,to_timestamp(1010))").bind(first.identity_id).bind(vec![1u8;40]).execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO auth_sessions(id,account_id,token_hash,created_at,authenticated_at,expires_at) VALUES($1,$2,$3,to_timestamp(1000),to_timestamp(1000),to_timestamp(1001))").bind(Uuid::new_v4()).bind(first.account.id).bind(vec![1u8;31]).execute(&pool).await.is_err());
    let fields=sqlx::query("SELECT column_name FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('auth_accounts','auth_identities','auth_credentials','auth_sessions','auth_transactions')").fetch_all(&pool).await.unwrap();
    for field in fields {
        let name: String = field.get("column_name");
        assert!(
            ![
                "password",
                "email",
                "phone",
                "avatar",
                "subject",
                "access_token",
                "refresh_token",
                "id_token"
            ]
            .contains(&name.as_str()),
            "{name}"
        );
    }
    let expired = service
        .start(
            &browser,
            Provider::Apple,
            AuthIntent::Login,
            ReturnPath::Home,
            1000,
        )
        .await
        .unwrap();
    assert!(
        service
            .consume(&expired.state, &browser, Provider::Apple, 1300)
            .await
            .is_err()
    );
    store.cleanup(1300).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_transactions WHERE id=$1")
            .bind(expired.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    store.logout(second.token.hash()).await.unwrap();
    assert!(
        service
            .session(&second.token, 1011)
            .await
            .unwrap()
            .is_none()
    );
    store.revoke_account(first.account.id).await.unwrap();
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(first.account.id)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires a real PostgreSQL DATABASE_URL; executed in the database CI job"]
async fn readiness_tracks_actual_database_availability() {
    let url = env::var("DATABASE_URL").expect("DATABASE_URL is required for this test");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(1))
        .connect(&url)
        .await
        .expect("test PostgreSQL must be running");
    let server = app(Some(pool.clone()));
    let request = || {
        Request::builder()
            .uri("/health/ready")
            .body(Body::empty())
            .unwrap()
    };
    let response = server.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"status":"ok"})
    );
    pool.close().await;
    let response = server.oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"status":"unavailable"})
    );
}
