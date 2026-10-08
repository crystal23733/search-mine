use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use liar_server::auth::*;
use std::sync::Arc;
use tower::ServiceExt;
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        1000
    }
}
struct Providers;
impl OAuthProvider for Providers {
    fn available(&self, _: Provider) -> bool {
        false
    }
    fn authorize(&self, _: Provider, _: &Authorization) -> Result<String, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn exchange(
        &self,
        _: &AuthTransaction,
        _: &SecretToken,
        _: &str,
        _: Option<&[u8]>,
        _: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        Err(AuthError::Unavailable)
    }
}
impl AppleProvider for Providers {
    async fn notification(&self, _: &str, _: i64) -> Result<AppleNotification, AuthError> {
        Err(AuthError::Invalid)
    }
    async fn revoke_apple(&self, _: &str, _: i64) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn check_apple(&self, _: &str, _: i64) -> Result<AppleCredentialStatus, AuthError> {
        Err(AuthError::Unavailable)
    }
}
async fn router() -> axum::Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:9/unused")
        .unwrap();
    pool.close().await;
    account_auth_router(
        AuthService::new(
            PgAuthStore::new(pool),
            AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
            DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
        ),
        Providers,
        BrowserSecurity::new("https://game.example", [3; 32]).unwrap(),
        Arc::new(Clock),
    )
}
#[tokio::test]
async fn rights_routes_require_authentication_origin_and_csrf_before_storage() {
    let r = router().await;
    for (method, path, expected) in [
        ("GET", "/api/v1/me/identities", StatusCode::UNAUTHORIZED),
        ("POST", "/api/v1/me/export", StatusCode::BAD_REQUEST),
        ("DELETE", "/api/v1/me", StatusCode::BAD_REQUEST),
        (
            "DELETE",
            "/api/v1/me/identities/apple",
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/me/identities/google/link",
            StatusCode::BAD_REQUEST,
        ),
        (
            "POST",
            "/api/v1/auth/google/reauth",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let result = r
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("origin", "https://attacker.example")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.status(), expected, "{method} {path}");
        assert_eq!(result.headers()["cache-control"], "no-store");
        assert!(result.headers().get("set-cookie").is_none());
    }
}
