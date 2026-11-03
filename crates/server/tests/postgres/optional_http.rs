use super::*;
use liar_server::{
    consent::*,
    events::*,
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
            advertising: false,
            expires_at: 1900,
        })
    }
}
struct Fixture {
    pool: sqlx::PgPool,
    clock: Arc<Clock>,
    sessions: PgAuthStore,
    owner: ConsentSession,
    token: SecretToken,
    browser: SecretToken,
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
            token,
            browser: SecretToken::generate().unwrap(),
            security: Arc::new(BrowserSecurity::new("https://game.example", [73; 32]).unwrap()),
            authorities,
        }
    }
    fn consent(&self) -> PgConsentStore {
        PgConsentStore::new(self.pool.clone(), self.clock.clone())
    }
    fn router(&self, store_pool: sqlx::PgPool, session_pool: Option<sqlx::PgPool>) -> axum::Router {
        let sessions = session_pool
            .map(PgAuthStore::new)
            .unwrap_or_else(|| self.sessions.clone());
        optional_event_router(
            Arc::new(PgOptionalEventStore::new(store_pool, self.clock.clone())),
            Arc::new(PgSessionReader::new(sessions, self.clock.clone())),
            self.security.clone(),
            OptionalEventAuthentication {
                authorities: self.authorities.clone(),
                clock: self.clock.clone(),
            },
            4,
        )
        .unwrap()
    }
    fn body(&self, revision: Uuid) -> serde_json::Value {
        serde_json::json!({"v":1,"expected_revision":revision.to_string(),"draft":{"event_version":1,"event_id":Uuid::new_v4().to_string(),"session_key":Uuid::new_v4().to_string(),"occurred_at":1500,"locale":"ko","event":{"type":"landing_view","route":"home"}}})
    }
    fn request(&self, value: &serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/v1/events/optional")
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
            .body(Body::from(value.to_string()))
            .unwrap()
    }
}
async fn receipt(r: axum::response::Response) -> serde_json::Value {
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["cache-control"], "no-store");
    serde_json::from_slice(&to_bytes(r.into_body(), 2048).await.unwrap()).unwrap()
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn real_http_uses_current_purpose_and_immutable_receipts_and_logout_cascade() {
    let f = Fixture::new().await;
    let router = f.router(f.pool.clone(), None);
    let unknown = f.body(Uuid::new_v4());
    assert_eq!(
        router
            .clone()
            .oneshot(f.request(&unknown))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let granted = f.consent().refresh(f.owner, None, &Grant).await.unwrap();
    let value = f.body(granted.revision);
    let (a, b) = tokio::join!(
        router.clone().oneshot(f.request(&value)),
        router.clone().oneshot(f.request(&value))
    );
    let a = receipt(a.unwrap()).await;
    let b = receipt(b.unwrap()).await;
    assert_eq!(a, b);
    assert_eq!(
        a,
        serde_json::json!({"v":1,"event_id":value["draft"]["event_id"],"received_at":1500})
    );
    f.clock.0.store(1501, Ordering::SeqCst);
    assert_eq!(
        receipt(router.clone().oneshot(f.request(&value)).await.unwrap()).await,
        a
    );
    let mut changed = value.clone();
    changed["draft"]["event"]["route"] = serde_json::json!("rules");
    assert_eq!(
        router
            .clone()
            .oneshot(f.request(&changed))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let mut ads = f.body(granted.revision);
    ads["draft"]["event"] = serde_json::json!({"type":"ad_request","placement":"menu"});
    assert_eq!(
        router
            .clone()
            .oneshot(f.request(&ads))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    f.consent().withdraw_all(f.owner).await.unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(f.request(&value))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM optional_product_events")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM product_events")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    f.sessions.logout(f.token.hash()).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM optional_product_events")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        router.oneshot(f.request(&value)).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    close_auth_pool(f.pool).await;
}
async fn deadline_case(session_phase: bool, revoke: bool) {
    let f = Fixture::new().await;
    let current = f.consent().refresh(f.owner, None, &Grant).await.unwrap();
    let (single, pid) = super::events::single_pool(&f.pool).await;
    let mut held = f.pool.begin().await.unwrap();
    if session_phase {
        sqlx::query("LOCK TABLE auth_sessions IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *held)
            .await
            .unwrap();
    } else {
        sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
            .bind(f.owner.account)
            .fetch_one(&mut *held)
            .await
            .unwrap();
    }
    let router = f.router(
        if session_phase {
            f.pool.clone()
        } else {
            single.clone()
        },
        if session_phase {
            Some(single.clone())
        } else {
            None
        },
    );
    let value = f.body(current.revision);
    let mut request = f.request(&value);
    let bytes = value.to_string();
    *request.body_mut() = Body::from_stream(futures_util::stream::once(async move {
        tokio::time::sleep(Duration::from_millis(750)).await;
        Ok::<_, std::io::Error>(axum::body::Bytes::from(bytes))
    }));
    let task = tokio::spawn(async move { router.oneshot(request).await.unwrap() });
    super::events::blocked(&f.pool, pid).await;
    let barrier = if revoke {
        Some(f.authorities.session(f.token.hash()))
    } else {
        None
    };
    let response = task.await.unwrap();
    let gone = super::consent_deadline::stopped(
        &f.pool,
        pid,
        Duration::from_millis(if revoke { 1500 } else { 500 }),
    )
    .await;
    held.rollback().await.unwrap();
    drop(barrier);
    single.close().await;
    assert_eq!(
        response.status(),
        if revoke {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM optional_product_events")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(f.pool).await;
    assert!(
        gone,
        "The original backend must stop before releasing its blocker"
    );
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn slow_body_does_not_restart_the_storage_deadline() {
    deadline_case(false, false).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn slow_body_does_not_restart_the_session_query_deadline() {
    deadline_case(true, false).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn revocation_cancels_a_waiter_and_its_actual_database_work_is_bounded() {
    deadline_case(false, true).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn foreign_session_and_stale_revision_cannot_mint_a_permission_and_scopes_do_not_leak() {
    let mut f = Fixture::new().await;
    let current = f.consent().refresh(f.owner, None, &Grant).await.unwrap();
    let value = f.body(current.revision);
    let store = PgOptionalEventStore::new(f.pool.clone(), f.clock.clone());
    let input = OptionalCapture {
        session: f.owner,
        expected_revision: current.revision,
        draft: decode_client(&serde_json::to_vec(&value["draft"]).unwrap()).unwrap(),
    };
    for deadline in [
        std::time::Instant::now() - Duration::from_secs(1),
        std::time::Instant::now() + Duration::from_secs(3),
    ] {
        assert_eq!(
            store.capture_until(input.clone(), deadline).await,
            Err(OptionalEventError::Unavailable)
        );
    }
    assert!(store.capture(input).await.is_ok());
    let denied = f.consent().withdraw_all(f.owner).await.unwrap();
    let renewed = f
        .consent()
        .refresh(f.owner, Some(denied.revision), &Grant)
        .await
        .unwrap();
    assert_eq!(
        f.router(f.pool.clone(), None)
            .oneshot(f.request(&value))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let foreign_value = f.body(renewed.revision);
    f.token = SecretToken::generate().unwrap();
    f.sessions
        .login(super::rights::account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &f.token,
            1000,
        ))
        .await
        .unwrap();
    assert_eq!(
        f.router(f.pool.clone(), None)
            .oneshot(f.request(&foreign_value))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let row = f
        .sessions
        .session(f.token.hash(), 1500)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(row.account.id, f.owner.account);
    close_auth_pool(f.pool).await;
}
