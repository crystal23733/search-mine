use super::*;
use liar_server::{
    consent::*,
    online::{AuthorityRegistry, PgSessionReader},
};
use std::sync::atomic::{AtomicI64, Ordering};
struct Clock(AtomicI64);
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct Grant;
impl ConsentVerifier for Grant {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        Ok(VerifiedConsent {
            subject,
            analytics: true,
            advertising: true,
            expires_at: 1900,
        })
    }
}
struct Paused {
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
impl ConsentVerifier for Paused {
    async fn verify(&self, subject: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        self.entered.notify_one();
        self.resume.notified().await;
        Grant.verify(subject, 1500).await
    }
}
struct Fixture {
    pool: sqlx::PgPool,
    clock: Arc<Clock>,
    sessions: PgAuthStore,
    owner: ConsentSession,
    browser: SecretToken,
    token: SecretToken,
    security: Arc<BrowserSecurity>,
    authorities: Arc<AuthorityRegistry>,
}
impl Fixture {
    async fn new() -> Self {
        let pool = auth_pool().await;
        let authorities = AuthorityRegistry::new(64).unwrap();
        let sessions = PgAuthStore::new(pool.clone()).with_invalidations(authorities.clone());
        let token = SecretToken::generate().unwrap();
        let record = sessions
            .login(super::rights::account_write(
                Provider::Google,
                &Uuid::new_v4().to_string(),
                &token,
                1000,
            ))
            .await
            .unwrap();
        let row = sessions.session(token.hash(), 1500).await.unwrap().unwrap();
        Self {
            pool,
            clock: Arc::new(Clock(AtomicI64::new(1500))),
            sessions,
            owner: ConsentSession {
                account: record.account.id,
                revision: row.id,
                hash: token.hash(),
            },
            browser: SecretToken::generate().unwrap(),
            token,
            security: Arc::new(BrowserSecurity::new("https://game.example", [73; 32]).unwrap()),
            authorities,
        }
    }
    fn store(&self) -> PgConsentStore {
        PgConsentStore::new(self.pool.clone(), self.clock.clone())
    }
    fn router<V: ConsentVerifier + 'static>(&self, verifier: Arc<V>) -> axum::Router {
        consent_router(
            Arc::new(self.store()),
            verifier,
            Arc::new(PgSessionReader::new(
                self.sessions.clone(),
                self.clock.clone(),
            )),
            self.security.clone(),
            ConsentAuthentication {
                authorities: self.authorities.clone(),
                clock: self.clock.clone(),
            },
            4,
        )
        .unwrap()
    }
    fn request(&self, path: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("origin", "https://game.example")
            .header("content-type", "application/json")
            .header(
                "x-liar-csrf",
                self.security
                    .csrf(&self.browser, Some(&self.token), self.clock.now())
                    .unwrap(),
            )
            .header(
                "cookie",
                format!(
                    "__Host-liar_browser={}; __Host-liar_session={}",
                    self.browser.expose().as_str(),
                    self.token.expose().as_str()
                ),
            )
            .body(Body::from(body.to_owned()))
            .unwrap()
    }
}
async fn json(response: axum::response::Response) -> serde_json::Value {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap()
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn disabled_http_refresh_commits_blocked_revision_and_current_view_reuses_the_same_store() {
    let f = Fixture::new().await;
    let router = f.router(Arc::new(DisabledConsentVerifier));
    let view = json(
        router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(view["revision"], serde_json::Value::Null);
    let verified = json(
        f.router(Arc::new(Grant))
            .oneshot(f.request(
                "/api/v1/me/consent/refresh",
                r#"{"v":1,"expected_revision":null}"#,
            ))
            .await
            .unwrap(),
    )
    .await;
    let first = f.store().read(f.owner).await.unwrap().unwrap();
    assert!(first.analytics && first.advertising);
    assert_eq!(verified["revision"], first.revision.to_string());
    assert!(!verified.to_string().contains(&f.owner.revision.to_string()));
    let request = f.request(
        "/api/v1/me/consent/refresh",
        &format!(r#"{{"v":1,"expected_revision":"{}"}}"#, first.revision),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let blocked = f.store().read(f.owner).await.unwrap().unwrap();
    assert!(!blocked.analytics && !blocked.advertising);
    assert_ne!(blocked.revision, first.revision);
    let view = json(
        router
            .clone()
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(view["revision"], blocked.revision.to_string());
    assert_eq!(view["consent"]["analytics"], "denied");
    let response = router
        .clone()
        .oneshot(f.request(
            "/api/v1/me/consent/refresh",
            &format!(r#"{{"v":1,"expected_revision":"{}"}}"#, first.revision),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let mut forged = f.owner;
    forged.revision = Uuid::new_v4();
    assert_eq!(
        f.store().withdraw_all(forged).await,
        Err(ConsentError::Unauthorized)
    );
    f.sessions.logout(f.owner.hash).await.unwrap();
    assert_eq!(
        router
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_consents")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(f.pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn whole_http_withdrawal_has_one_revision_and_defeats_a_late_initial_grant() {
    let f = Fixture::new().await;
    let paused = Arc::new(Paused {
        entered: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let router = f.router(paused.clone());
    let request = f.request(
        "/api/v1/me/consent/refresh",
        r#"{"v":1,"expected_revision":null}"#,
    );
    let other = router.clone();
    let grant = tokio::spawn(async move { other.oneshot(request).await.unwrap() });
    tokio::time::timeout(std::time::Duration::from_secs(2), paused.entered.notified())
        .await
        .unwrap();
    let value = json(
        router
            .oneshot(f.request("/api/v1/me/consent/withdraw", r#"{"v":1,"purpose":"all"}"#))
            .await
            .unwrap(),
    )
    .await;
    let stored = f.store().read(f.owner).await.unwrap().unwrap();
    assert!(!stored.analytics && !stored.advertising);
    assert_eq!(value["revision"], stored.revision.to_string());
    paused.resume.notify_one();
    assert_eq!(grant.await.unwrap().status(), StatusCode::CONFLICT);
    assert_eq!(f.store().read(f.owner).await.unwrap(), Some(stored));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_consents")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    close_auth_pool(f.pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn cancelled_http_reverification_keeps_the_precommitted_denial_and_purpose_withdrawal_is_separate()
 {
    let f = Fixture::new().await;
    let first = f.store().refresh(f.owner, None, &Grant).await.unwrap();
    let router = f.router(Arc::new(DisabledConsentVerifier));
    let view = json(
        router
            .oneshot(f.request(
                "/api/v1/me/consent/withdraw",
                r#"{"v":1,"purpose":"analytics"}"#,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(view["consent"]["analytics"], "denied");
    assert_eq!(view["consent"]["advertising"], "granted");
    let current = f.store().read(f.owner).await.unwrap().unwrap();
    assert_eq!(current.expires_at, first.expires_at);
    let paused = Arc::new(Paused {
        entered: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let router = f.router(paused.clone());
    let request = f.request(
        "/api/v1/me/consent/refresh",
        &format!(r#"{{"v":1,"expected_revision":"{}"}}"#, current.revision),
    );
    let task = tokio::spawn(async move { router.oneshot(request).await.unwrap() });
    tokio::time::timeout(std::time::Duration::from_secs(2), paused.entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let denied = f.store().read(f.owner).await.unwrap().unwrap();
    assert!(!denied.analytics && !denied.advertising);
    assert_ne!(denied.revision, current.revision);
    f.clock.0.store(1900, Ordering::SeqCst);
    let router = f.router(Arc::new(DisabledConsentVerifier));
    let view = json(
        router
            .oneshot(f.request("/api/v1/me/consent", r#"{"v":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(view["consent"]["status"], "expired");
    assert_eq!(view["consent"]["advertising"], "unknown");
    close_auth_pool(f.pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn actual_logout_invalidates_the_http_waiter_and_removes_its_current_row() {
    let f = Fixture::new().await;
    let first = f.store().refresh(f.owner, None, &Grant).await.unwrap();
    let paused = Arc::new(Paused {
        entered: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let router = f.router(paused.clone());
    let request = f.request(
        "/api/v1/me/consent/refresh",
        &format!(r#"{{"v":1,"expected_revision":"{}"}}"#, first.revision),
    );
    let task = tokio::spawn(async move { router.oneshot(request).await.unwrap() });
    tokio::time::timeout(std::time::Duration::from_secs(2), paused.entered.notified())
        .await
        .unwrap();
    f.sessions.logout(f.owner.hash).await.unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.store().read(f.owner).await,
        Err(ConsentError::Unauthorized)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM session_consents")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(f.pool).await;
}
