use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use liar_server::app;
use liar_server::auth::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Row, migrate::Migrator};
use std::path::Path;
use std::{env, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;

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
