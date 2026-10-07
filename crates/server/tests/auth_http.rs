use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use liar_server::auth::*;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Default)]
struct Memory {
    transactions: Mutex<Vec<AuthTransaction>>,
    sessions: Mutex<Vec<([u8; 32], Session)>>,
}
#[derive(Clone, Default)]
struct Store(Arc<Memory>);
impl AuthStore for Store {
    async fn insert_transaction(&self, t: AuthTransaction) -> Result<(), AuthError> {
        self.0.transactions.lock().unwrap().push(t);
        Ok(())
    }
    async fn consume_transaction(
        &self,
        state: [u8; 32],
        browser: [u8; 32],
        provider: Provider,
        now: i64,
    ) -> Result<Option<AuthTransaction>, AuthError> {
        let mut values = self.0.transactions.lock().unwrap();
        let i = values.iter().position(|t| {
            t.state_hash == state
                && t.browser_hash == browser
                && t.provider == provider
                && t.created_at <= now
                && now < t.expires_at
        });
        Ok(i.map(|i| values.remove(i)))
    }
    async fn login(&self, write: LoginWrite) -> Result<LoginRecord, AuthError> {
        let mut sessions = self.0.sessions.lock().unwrap();
        if let Some(old) = write.previous_session {
            sessions.retain(|(h, _)| *h != old)
        }
        let account = Account {
            id: Uuid::new_v4(),
            nickname: None,
        };
        let identity_id = Uuid::new_v4();
        sessions.push((
            write.session_hash,
            Session {
                id: Uuid::new_v4(),
                account: account.clone(),
                created_at: write.now,
                authenticated_at: write.now,
                expires_at: write.now + SESSION_SECONDS,
            },
        ));
        Ok(LoginRecord {
            account,
            identity_id,
        })
    }
    async fn session(&self, hash: [u8; 32], now: i64) -> Result<Option<Session>, AuthError> {
        Ok(self
            .0
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|(h, s)| *h == hash && now < s.expires_at)
            .map(|(_, s)| s.clone()))
    }
    async fn nickname(&self, account: Uuid, nickname: Nickname) -> Result<bool, AuthError> {
        let mut sessions = self.0.sessions.lock().unwrap();
        let mut changed = false;
        for (_, s) in sessions.iter_mut().filter(|(_, s)| s.account.id == account) {
            s.account.nickname = Some(nickname.clone());
            changed = true
        }
        Ok(changed)
    }
    async fn logout(&self, hash: [u8; 32]) -> Result<(), AuthError> {
        self.0.sessions.lock().unwrap().retain(|(h, _)| *h != hash);
        Ok(())
    }
    async fn revoke_account(&self, id: Uuid) -> Result<(), AuthError> {
        self.0
            .sessions
            .lock()
            .unwrap()
            .retain(|(_, s)| s.account.id != id);
        Ok(())
    }
    async fn cleanup(&self, _now: i64) -> Result<(), AuthError> {
        Ok(())
    }
}
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        1100
    }
}
struct Providers;
impl OAuthProvider for Providers {
    fn available(&self, p: Provider) -> bool {
        p == Provider::Google
    }
    fn authorize(&self, _p: Provider, a: &Authorization) -> Result<String, AuthError> {
        Ok(format!(
            "https://provider.example/authorize?state={}",
            a.state.expose().as_str()
        ))
    }
    async fn exchange(
        &self,
        _t: &AuthTransaction,
        _state: &SecretToken,
        code: &str,
        _v: Option<&[u8]>,
        _now: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        if code != "valid-code" {
            return Err(AuthError::Invalid);
        }
        Ok(VerifiedIdentity {
            subject: Zeroizing::new("fixture-subject".into()),
            apple_refresh: None,
        })
    }
}
fn router(store: Store) -> Router {
    auth_router(
        AuthService::new(
            store,
            AeadVault::new(1, vec![(1, [1; 32])]).unwrap(),
            DigestKeys::new(1, vec![(1, [2; 32])]).unwrap(),
        ),
        Providers,
        BrowserSecurity::new("https://game.example", [3; 32]).unwrap(),
        Arc::new(Clock),
    )
}
async fn bootstrap(router: &Router, cookie: Option<&str>) -> (String, serde_json::Value) {
    let mut request = Request::builder().uri("/api/v1/auth/bootstrap");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie)
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let cookie = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with("__Host-liar_browser="))
        .unwrap();
    for attribute in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=300",
    ] {
        assert!(cookie.contains(attribute));
    }
    assert!(!cookie.contains("Domain="));
    let cookie = cookie.split(';').next().unwrap().to_string();
    let json =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    (cookie, json)
}
#[tokio::test]
async fn anonymous_bootstrap_creates_no_account_and_exposes_only_status_and_memory_csrf() {
    let store = Store::default();
    let (cookie, value) = bootstrap(&router(store.clone()), None).await;
    assert!(cookie.starts_with("__Host-liar_browser="));
    assert!(value["account"].is_null());
    assert!(value["session_revision"].is_null());
    assert!(value["csrf"].as_str().is_some());
    assert_eq!(value["providers"].as_array().unwrap().len(), 4);
    assert!(store.0.sessions.lock().unwrap().is_empty());
    assert!(store.0.transactions.lock().unwrap().is_empty());
}
fn mutation(method: &str, path: &str, cookie: &str, csrf: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, cookie)
        .header(header::ORIGIN, "https://game.example")
        .header("x-liar-csrf", csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn start(router: &Router, cookie: &str, csrf: &str) -> String {
    let response = router
        .clone()
        .oneshot(mutation(
            "POST",
            "/api/v1/auth/google/start",
            cookie,
            csrf,
            r#"{"locale":"ko","return_path":"daily"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    url::Url::parse(value["authorize_url"].as_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .to_string()
}
#[tokio::test]
async fn csrf_origin_cookie_and_bounded_strict_body_protect_start() {
    let store = Store::default();
    let router = router(store.clone());
    let (cookie, value) = bootstrap(&router, None).await;
    let csrf = value["csrf"].as_str().unwrap();
    for change in ["csrf", "origin", "cookie", "origin_missing"] {
        let mut request = mutation(
            "POST",
            "/api/v1/auth/google/start",
            &cookie,
            csrf,
            r#"{"locale":"ko","return_path":"daily"}"#,
        );
        match change {
            "csrf" => {
                request
                    .headers_mut()
                    .insert("x-liar-csrf", header::HeaderValue::from_static("bad"));
            }
            "origin" => {
                request.headers_mut().insert(
                    header::ORIGIN,
                    header::HeaderValue::from_static("https://evil.example"),
                );
            }
            "cookie" => {
                request.headers_mut().remove(header::COOKIE);
            }
            _ => {
                request.headers_mut().remove(header::ORIGIN);
            }
        }
        assert!(
            !router
                .clone()
                .oneshot(request)
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    for body in [
        r#"{"locale":"../../evil","return_path":"daily"}"#,
        r#"{"locale":"ko","return_path":"https://evil.example"}"#,
        r#"{"locale":"ko","return_path":"daily","password":"must-not-be-collected"}"#,
    ] {
        assert!(
            !router
                .clone()
                .oneshot(mutation(
                    "POST",
                    "/api/v1/auth/google/start",
                    &cookie,
                    csrf,
                    body
                ))
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    assert!(store.0.transactions.lock().unwrap().is_empty());
    let _ = start(&router, &cookie, csrf).await;
    assert_eq!(store.0.transactions.lock().unwrap().len(), 1);
    let response = router
        .clone()
        .oneshot(mutation(
            "POST",
            "/api/v1/auth/apple/start",
            &cookie,
            csrf,
            r#"{"locale":"ko","return_path":"daily"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
#[tokio::test]
async fn callback_consumes_cancel_and_success_once_and_logout_invalidates_session() {
    let store = Store::default();
    let router = router(store.clone());
    let (browser, value) = bootstrap(&router, None).await;
    let csrf = value["csrf"].as_str().unwrap();
    let cancelled = start(&router, &browser, csrf).await;
    let cancel = format!(
        "/api/v1/auth/google/callback?state={cancelled}&error=access_denied&error_description=private-value"
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&cancel)
                .header(header::COOKIE, &browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/ko/login?error=auth_failed"
    );
    assert!(store.0.transactions.lock().unwrap().is_empty());
    assert_eq!(
        router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&cancel)
                    .header(header::COOKIE, &browser)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let state = start(&router, &browser, csrf).await;
    let path = format!("/api/v1/auth/google/callback?state={state}&code=valid-code");
    let wrong = SecretToken::generate().unwrap();
    let wrong_browser = format!("__Host-liar_browser={}", wrong.expose().as_str());
    assert_eq!(
        router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header(header::COOKIE, wrong_browser)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header(header::COOKIE, &browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/ko/onboarding?return_path=daily"
    );
    let session = response.headers()[header::SET_COOKIE].to_str().unwrap();
    for attr in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=2592000",
    ] {
        assert!(session.contains(attr));
    }
    let cookies = format!("{browser}; {}", session.split(';').next().unwrap());
    let (_, logged) = bootstrap(&router, Some(&cookies)).await;
    assert!(logged["account"]["id"].as_str().is_some());
    assert!(logged["account"]["nickname"].is_null());
    assert!(logged["session_revision"].as_str().is_some());
    assert_eq!(logged["account"].as_object().unwrap().len(), 2);
    let csrf = logged["csrf"].as_str().unwrap();
    let response = router
        .clone()
        .oneshot(mutation(
            "PATCH",
            "/api/v1/me",
            &cookies,
            csrf,
            r#"{"nickname":"Player 7"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (_, changed) = bootstrap(&router, Some(&cookies)).await;
    assert_eq!(changed["account"]["nickname"], "Player 7");
    let response = router
        .clone()
        .oneshot(mutation(
            "POST",
            "/api/v1/auth/logout",
            &cookies,
            csrf,
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert!(store.0.sessions.lock().unwrap().is_empty());
}
#[test]
fn csrf_expiry_and_session_binding_reject_reuse_after_rotation() {
    let security = BrowserSecurity::new("https://game.example", [3; 32]).unwrap();
    let browser = SecretToken::generate().unwrap();
    let session = SecretToken::generate().unwrap();
    let csrf = security.csrf(&browser, Some(&session), 1000).unwrap();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::ORIGIN,
        header::HeaderValue::from_static("https://game.example"),
    );
    headers.insert("x-liar-csrf", csrf.parse().unwrap());
    let cookies = BrowserCookies {
        browser: Some(browser),
        session: Some(session),
    };
    assert!(security.require(&headers, &cookies, 1299).is_ok());
    assert!(security.require(&headers, &cookies, 1300).is_err());
    assert!(security.require(&headers, &cookies, 999).is_err());
    let rotated = BrowserCookies {
        browser: cookies.browser,
        session: Some(SecretToken::generate().unwrap()),
    };
    assert!(security.require(&headers, &rotated, 1100).is_err());
    for origin in [
        "https://game.example/path",
        "http://game.example",
        "https://127.0.0.1",
        "https://name:password@game.example",
        "https://game.example?query=x",
    ] {
        assert!(BrowserSecurity::new(origin, [3; 32]).is_err());
    }
}
#[tokio::test]
async fn disabled_auth_preserves_local_mode_and_never_sets_credentials() {
    let router = liar_server::app(None);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert!(value["csrf"].is_null());
    assert!(
        value["providers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["available"] == false)
    );
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/google/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
#[tokio::test]
async fn overlapping_transactions_resolve_their_own_locale_even_when_callbacks_reverse() {
    let store = Store::default();
    let router = router(store.clone());
    let (browser, bootstrap) = bootstrap(&router, None).await;
    let csrf = bootstrap["csrf"].as_str().unwrap();
    let korean = start(&router, &browser, csrf).await;
    let response = router
        .clone()
        .oneshot(mutation(
            "POST",
            "/api/v1/auth/google/start",
            &browser,
            csrf,
            r#"{"locale":"fr","return_path":"settings"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    let french = url::Url::parse(body["authorize_url"].as_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .to_string();
    for (state, path) in [
        (french, "/fr/onboarding?return_path=settings"),
        (korean, "/ko/onboarding?return_path=daily"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/auth/google/callback?state={state}&code=valid-code"
                    ))
                    .header(header::COOKIE, &browser)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], path);
    }
    assert!(store.0.transactions.lock().unwrap().is_empty());
}
#[tokio::test]
async fn nickname_response_uses_normalized_authority_and_failed_exchange_consumes_state() {
    let store = Store::default();
    let router = router(store.clone());
    let (browser, value) = bootstrap(&router, None).await;
    let csrf = value["csrf"].as_str().unwrap();
    let state = start(&router, &browser, csrf).await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/auth/google/callback?state={state}&code=invalid-code"
                ))
                .header(header::COOKIE, &browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(store.0.sessions.lock().unwrap().is_empty());
    assert!(store.0.transactions.lock().unwrap().is_empty());
    let state = start(&router, &browser, csrf).await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/auth/google/callback?state={state}&code=valid-code"
                ))
                .header(header::COOKIE, &browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let session = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let cookies = format!("{browser}; {session}");
    let (_, logged) = bootstrap(&router, Some(&cookies)).await;
    let response = router
        .clone()
        .oneshot(mutation(
            "PATCH",
            "/api/v1/me",
            &cookies,
            logged["csrf"].as_str().unwrap(),
            "{\"nickname\":\"e\\u0301clair\"}",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(value["nickname"], "éclair");
}
#[tokio::test]
async fn duplicate_cookies_oversized_bodies_and_start_quota_fail_without_extra_transactions() {
    let store = Store::default();
    let router = router(store.clone());
    let (browser, value) = bootstrap(&router, None).await;
    let csrf = value["csrf"].as_str().unwrap();
    let request = mutation(
        "POST",
        "/api/v1/auth/google/start",
        &format!("{browser}; {browser}"),
        csrf,
        r#"{"locale":"ko","return_path":"daily"}"#,
    );
    assert_eq!(
        router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    let body = format!(
        "{{\"locale\":\"{}\",\"return_path\":\"daily\"}}",
        "a".repeat(4096)
    );
    assert!(
        !router
            .clone()
            .oneshot(mutation(
                "POST",
                "/api/v1/auth/google/start",
                &browser,
                csrf,
                &body
            ))
            .await
            .unwrap()
            .status()
            .is_success()
    );
    assert!(store.0.transactions.lock().unwrap().is_empty());
    for _ in 0..5 {
        let _ = start(&router, &browser, csrf).await;
    }
    let response = router
        .oneshot(mutation(
            "POST",
            "/api/v1/auth/google/start",
            &browser,
            csrf,
            r#"{"locale":"ko","return_path":"daily"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(store.0.transactions.lock().unwrap().len(), 5);
}
