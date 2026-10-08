//! Loopback-only browser fixture. Production never selects these provider proofs or clock controls.
use axum::{
    Json, Router,
    routing::{get, post},
};
use liar_server::auth::*;
use sqlx::postgres::PgPoolOptions;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use zeroize::Zeroizing;

const ORIGIN: &str = "https://localhost:8443";
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Providers;
impl OAuthProvider for Providers {
    fn available(&self, _: Provider) -> bool {
        true
    }
    fn authorize(&self, provider: Provider, auth: &Authorization) -> Result<String, AuthError> {
        let endpoint = match provider {
            Provider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Provider::Apple => "https://appleid.apple.com/auth/authorize",
            Provider::Kakao => "https://kauth.kakao.com/oauth/authorize",
            Provider::Naver => "https://nid.naver.com/oauth2.0/authorize",
        };
        let mut url = url::Url::parse(endpoint).map_err(|_| AuthError::Invalid)?;
        url.query_pairs_mut()
            .append_pair("state", &auth.state.expose())
            .append_pair("nonce", &auth.nonce.expose())
            .append_pair(
                "redirect_uri",
                &format!("{ORIGIN}/api/v1/auth/{}/callback", provider.as_str()),
            );
        Ok(url.into())
    }
    async fn exchange(
        &self,
        transaction: &AuthTransaction,
        _: &SecretToken,
        code: &str,
        _: Option<&[u8]>,
        _: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        let subject = code.strip_prefix("fixture-").ok_or(AuthError::Invalid)?;
        uuid::Uuid::parse_str(subject).map_err(|_| AuthError::Invalid)?;
        Ok(VerifiedIdentity {
            subject: Zeroizing::new(subject.to_owned()),
            apple_refresh: (transaction.provider == Provider::Apple)
                .then(|| Zeroizing::new(format!("fixture-refresh-{subject}"))),
        })
    }
}
impl AppleProvider for Providers {
    async fn notification(&self, _: &str, _: i64) -> Result<AppleNotification, AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn revoke_apple(&self, _: &str, _: i64) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn check_apple(&self, _: &str, _: i64) -> Result<AppleCredentialStatus, AuthError> {
        Err(AuthError::Unavailable)
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Advance {
    seconds: u16,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    // Migrations run before this process. The serving fixture uses the production adapters.
    let vault = Arc::new(AeadVault::new(1, vec![(1, [46; 32])]).expect("test vault"));
    let service = AuthService::new(
        PgAuthStore::with_vault(pool, vault.clone()),
        vault,
        DigestKeys::new(1, vec![(1, [47; 32])]).expect("test digests"),
    );
    let clock = Arc::new(Clock(AtomicI64::new(1_800_000_000)));
    let advance_clock = clock.clone();
    let router = account_auth_router(
        service,
        Providers,
        BrowserSecurity::new(ORIGIN, [48; 32]).expect("loopback origin"),
        clock,
    )
    .merge(
        Router::new()
            .route("/__fixture/ready", get(|| async { "test-only" }))
            .route(
                "/__fixture/advance",
                post(move |Json(body): Json<Advance>| {
                    let clock = advance_clock.clone();
                    async move {
                        clock
                            .0
                            .fetch_add(i64::from(body.seconds.min(3600)), Ordering::SeqCst);
                        "advanced"
                    }
                }),
            ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    axum::serve(listener, router).await?;
    Ok(())
}
