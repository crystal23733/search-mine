use super::*;

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn login_waiting_on_account_lock_cannot_reuse_a_concurrently_unlinked_identity() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let subject = Uuid::new_v4().to_string();
    let old_token = SecretToken::generate().unwrap();
    let account = store
        .login(account_write(Provider::Google, &subject, &old_token, 14000))
        .await
        .unwrap();
    let naver_token = SecretToken::generate().unwrap();
    let mut link = account_write(
        Provider::Naver,
        &Uuid::new_v4().to_string(),
        &naver_token,
        14001,
    );
    link.intent = AuthIntent::Link(account.account.id);
    link.bound_session = Some(old_token.hash());
    link.previous_session = Some(old_token.hash());
    store.login(link).await.unwrap();

    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let login_pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |conn, _| {
            let schema = schema.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(schema)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let login_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&login_pool)
        .await
        .unwrap();
    let mut unlink = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_accounts WHERE id=$1 FOR UPDATE")
        .bind(account.account.id)
        .fetch_one(&mut *unlink)
        .await
        .unwrap();
    let new_token = SecretToken::generate().unwrap();
    let write = account_write(Provider::Google, &subject, &new_token, 14002);
    let login_store = PgAuthStore::new(login_pool.clone());
    let login = tokio::spawn(async move { login_store.login(write).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT cardinality(pg_blocking_pids($1)) > 0")
                .bind(login_pid)
                .fetch_one(&pool)
                .await
                .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("login must have read the identity and be waiting for the held account lock");
    // This is the atomic provider-removal effect while owning the same account lock as unlink.
    sqlx::query("DELETE FROM auth_identities WHERE id=$1")
        .bind(account.identity_id)
        .execute(&mut *unlink)
        .await
        .unwrap();
    sqlx::query("DELETE FROM auth_sessions WHERE account_id=$1")
        .bind(account.account.id)
        .execute(&mut *unlink)
        .await
        .unwrap();
    unlink.commit().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), login)
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .session(new_token.hash(), 14002)
            .await
            .unwrap()
            .is_none(),
        "a concurrently removed identity must not issue a session to its old account"
    );
    assert!(matches!(result, Err(AuthError::Unauthenticated)));
    login_pool.close().await;
    close_auth_pool(pool).await;
}

#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn apple_notification_http_requires_signed_whitelisted_payload_and_deduplicates() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone());
    let subject = Uuid::new_v4().to_string();
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(Provider::Apple, &subject, &token, 12000))
        .await
        .unwrap();
    let router = account_auth_router(
        AuthService::new(
            store.clone(),
            vault,
            DigestKeys::new(1, vec![(1, [16; 32])]).unwrap(),
        ),
        RightsProvider {
            subject: subject.clone(),
        },
        BrowserSecurity::new("https://game.example", [17; 32]).unwrap(),
        Arc::new(RightsClock),
    );
    let signed = |audience: &str, kind: &str| {
        let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        h.kid = Some("fixture-1".into());
        jsonwebtoken::encode(
            &h,
            &serde_json::json!({
                "iss": "https://appleid.apple.com",
                "aud": audience,
                "iat": 12000,
                "jti": Uuid::new_v4().to_string(),
                "events": {
                    "type": kind,
                    "sub": subject,
                    "event_time": 12001,
                    "email": "discard@example.com"
                }
            }),
            &jsonwebtoken::EncodingKey::from_rsa_pem(include_bytes!(
                "../fixtures/auth/rsa-test-only.pem"
            ))
            .unwrap(),
        )
        .unwrap()
    };
    for body in [
        serde_json::json!({"payload":signed("wrong.audience","account-deleted")}).to_string(),
        serde_json::json!({"payload":signed("fixture.notification","account-deleted"),"account_id":record.account.id.to_string()}).to_string(),
        "x".repeat(32769),
    ] {
        let response = router.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/apple/notifications")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(store.session(token.hash(), 12001).await.unwrap().is_some());
    }
    let email =
        serde_json::json!({"payload":signed("fixture.notification","email-disabled")}).to_string();
    let response = rights_request(
        &router,
        "POST",
        "/api/v1/auth/apple/notifications",
        "",
        None,
        &email,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(store.session(token.hash(), 12001).await.unwrap().is_some());
    let body =
        serde_json::json!({"payload":signed("fixture.notification","account-deleted")}).to_string();
    for _ in 0..2 {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/apple/notifications")
                    .header("content-type", "application/json")
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.headers().get("set-cookie").is_none());
    }
    assert!(store.session(token.hash(), 12001).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_apple_notification_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn apple_queue_claims_recover_leases_and_expire_after_twenty_four_hours() {
    let pool = auth_pool().await;
    let store = PgAuthStore::with_vault(
        pool.clone(),
        Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap()),
    );
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Apple,
            &Uuid::new_v4().to_string(),
            &token,
            13000,
        ))
        .await
        .unwrap();
    store
        .erase_authorized(SessionAuthority {
            account: record.account.id,
            hash: token.hash(),
            now: 13001,
        })
        .await
        .unwrap();
    let (a, b) = tokio::join!(store.claim_revoke(13001), store.claim_revoke(13001));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(usize::from(a.is_some()) + usize::from(b.is_some()), 1);
    let old = a.or(b).unwrap();
    let recovered = store.claim_revoke(13061).await.unwrap().unwrap();
    assert_ne!(recovered.lease, old.lease);
    store.finish_revoke(&old, true, 13062).await.unwrap();
    assert!(store.claim_revoke(13062).await.unwrap().is_none());
    assert!(store.claim_revoke(13001 + 86400).await.unwrap().is_none());
    store.cleanup(13001 + 86400).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_apple_revoke_queue")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
struct RightsClock;
impl AuthClock for RightsClock {
    fn now(&self) -> i64 {
        12001
    }
}
struct RightsProvider {
    subject: String,
}
impl OAuthProvider for RightsProvider {
    fn available(&self, _: Provider) -> bool {
        true
    }
    fn authorize(&self, _: Provider, a: &Authorization) -> Result<String, AuthError> {
        Ok(format!(
            "https://fixture.example/?state={}",
            a.state.expose().as_str()
        ))
    }
    async fn exchange(
        &self,
        tx: &AuthTransaction,
        _: &SecretToken,
        code: &str,
        _: Option<&[u8]>,
        _: i64,
    ) -> Result<VerifiedIdentity, AuthError> {
        if code != "fixture-code" {
            return Err(AuthError::Invalid);
        }
        Ok(VerifiedIdentity {
            subject: Zeroizing::new(self.subject.clone()),
            apple_refresh: (tx.provider == Provider::Apple)
                .then(|| Zeroizing::new("fixture-rights-refresh".into())),
        })
    }
}
impl AppleProvider for RightsProvider {
    async fn notification(&self, token: &str, now: i64) -> Result<AppleNotification, AuthError> {
        verify_apple_notification(
            include_bytes!("../fixtures/auth/jwks.json"),
            token,
            "fixture.notification",
            now,
        )
    }
    async fn revoke_apple(&self, _: &str, _: i64) -> Result<(), AuthError> {
        Err(AuthError::Unavailable)
    }
    async fn check_apple(&self, _: &str, _: i64) -> Result<AppleCredentialStatus, AuthError> {
        Err(AuthError::Unavailable)
    }
}
async fn rights_request(
    router: &axum::Router,
    method: &str,
    path: &str,
    cookies: &str,
    csrf: Option<&str>,
    body: &str,
) -> axum::response::Response {
    let mut r = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookies)
        .header("content-type", "application/json")
        .header("origin", "https://game.example");
    if let Some(csrf) = csrf {
        r = r.header("x-liar-csrf", csrf)
    }
    router
        .clone()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn rights_bootstrap(router: &axum::Router, cookies: &mut String) -> String {
    let result = rights_request(router, "GET", "/api/v1/auth/bootstrap", cookies, None, "").await;
    assert_eq!(result.status(), StatusCode::OK);
    let browser = result
        .headers()
        .get_all("set-cookie")
        .iter()
        .find(|v| v.to_str().unwrap().starts_with(BROWSER_COOKIE))
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let session = cookies
        .split(';')
        .map(str::trim)
        .find(|v| v.starts_with(SESSION_COOKIE))
        .map(str::to_string);
    *cookies = if let Some(session) = session {
        format!("{browser}; {session}")
    } else {
        browser
    };
    let bytes = to_bytes(result.into_body(), 8192).await.unwrap();
    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["csrf"]
        .as_str()
        .unwrap()
        .into()
}
fn rights_session_cookie(response: &axum::response::Response, cookies: &mut String) {
    let session = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .find(|v| v.to_str().unwrap().starts_with(SESSION_COOKIE))
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let browser = cookies
        .split(';')
        .map(str::trim)
        .find(|c| c.starts_with(BROWSER_COOKIE))
        .unwrap();
    *cookies = format!("{browser}; {session}");
}
async fn rights_authorize(
    router: &axum::Router,
    path: &str,
    provider: &str,
    cookies: &mut String,
) -> axum::response::Response {
    let csrf = rights_bootstrap(router, cookies).await;
    let start = rights_request(
        router,
        "POST",
        path,
        cookies,
        Some(&csrf),
        r#"{"locale":"ko","return_path":"settings"}"#,
    )
    .await;
    assert_eq!(start.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(start.into_body(), 8192).await.unwrap()).unwrap();
    let url = url::Url::parse(body["authorize_url"].as_str().unwrap()).unwrap();
    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let issuer = if provider == "google" {
        "&iss=https%3A%2F%2Faccounts.google.com"
    } else {
        ""
    };
    rights_request(
        router,
        "GET",
        &format!("/api/v1/auth/{provider}/callback?state={state}&code=fixture-code{issuer}"),
        cookies,
        None,
        "",
    )
    .await
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn account_http_links_reauthenticates_exports_whitelist_and_deletes_with_real_storage() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone());
    let subject = Uuid::new_v4().to_string();
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(Provider::Google, &subject, &token, 12000))
        .await
        .unwrap();
    let router = account_auth_router(
        AuthService::new(
            store.clone(),
            vault,
            DigestKeys::new(1, vec![(1, [16; 32])]).unwrap(),
        ),
        RightsProvider {
            subject: subject.clone(),
        },
        BrowserSecurity::new("https://game.example", [17; 32]).unwrap(),
        Arc::new(RightsClock),
    );
    let mut cookies = format!("{SESSION_COOKIE}={}", token.expose().as_str());
    let csrf = rights_bootstrap(&router, &mut cookies).await;
    let export = rights_request(
        &router,
        "POST",
        "/api/v1/me/export",
        &cookies,
        Some(&csrf),
        "{}",
    )
    .await;
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(export.headers()["cache-control"], "no-store");
    let export: serde_json::Value =
        serde_json::from_slice(&to_bytes(export.into_body(), 8192).await.unwrap()).unwrap();
    let keys: Vec<_> = export
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["account", "created_at", "identities", "last_seen_at"]
    );
    assert_eq!(export["account"]["id"], record.account.id.to_string());
    assert_eq!(export["identities"][0]["provider"], "google");
    assert_eq!(export["identities"][0].as_object().unwrap().len(), 2);
    assert!(!export.to_string().contains(&subject));
    let forbidden = rights_request(
        &router,
        "POST",
        "/api/v1/me/export",
        &cookies,
        Some(&csrf),
        r#"{"account_id":"another"}"#,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::BAD_REQUEST);
    let last = rights_request(
        &router,
        "DELETE",
        "/api/v1/me/identities/google",
        &cookies,
        Some(&csrf),
        "{}",
    )
    .await;
    assert_eq!(last.status(), StatusCode::CONFLICT);
    let linked = rights_authorize(
        &router,
        "/api/v1/me/identities/naver/link",
        "naver",
        &mut cookies,
    )
    .await;
    assert_eq!(linked.status(), StatusCode::SEE_OTHER);
    rights_session_cookie(&linked, &mut cookies);
    assert!(store.session(token.hash(), 12001).await.unwrap().is_none());
    let ids = rights_request(&router, "GET", "/api/v1/me/identities", &cookies, None, "").await;
    assert_eq!(ids.status(), StatusCode::OK);
    let ids: serde_json::Value =
        serde_json::from_slice(&to_bytes(ids.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(ids.as_array().unwrap().len(), 2);
    let reauth = rights_authorize(
        &router,
        "/api/v1/auth/google/reauth",
        "google",
        &mut cookies,
    )
    .await;
    assert_eq!(reauth.status(), StatusCode::SEE_OTHER);
    rights_session_cookie(&reauth, &mut cookies);
    let csrf = rights_bootstrap(&router, &mut cookies).await;
    let unlinked = rights_request(
        &router,
        "DELETE",
        "/api/v1/me/identities/naver",
        &cookies,
        Some(&csrf),
        "{}",
    )
    .await;
    assert_eq!(unlinked.status(), StatusCode::OK);
    assert!(
        unlinked.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let me = rights_request(&router, "GET", "/api/v1/me", &cookies, None, "").await;
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
    cookies.clear();
    let login =
        rights_authorize(&router, "/api/v1/auth/google/start", "google", &mut cookies).await;
    assert_eq!(login.status(), StatusCode::SEE_OTHER);
    rights_session_cookie(&login, &mut cookies);
    let csrf = rights_bootstrap(&router, &mut cookies).await;
    let erased = rights_request(&router, "DELETE", "/api/v1/me", &cookies, Some(&csrf), "{}").await;
    assert_eq!(erased.status(), StatusCode::OK);
    assert_eq!(
        rights_request(&router, "GET", "/api/v1/me", &cookies, None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_accounts WHERE id=$1")
            .bind(record.account.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn bound_links_reauthentication_and_subject_conflicts_cannot_switch_accounts() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone());
    let service = AuthService::new(
        store.clone(),
        vault,
        DigestKeys::new(1, vec![(1, [16; 32])]).unwrap(),
    );
    let subject = Uuid::new_v4().to_string();
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(Provider::Google, &subject, &token, 8000))
        .await
        .unwrap();
    let authority = |hash, now| SessionAuthority {
        account: record.account.id,
        hash,
        now,
    };
    assert!(matches!(
        store
            .unlink(authority(token.hash(), 8001), Provider::Google)
            .await,
        Err(AuthError::Conflict)
    ));
    assert!(matches!(
        store.export(authority(token.hash(), 8300)).await,
        Err(AuthError::ReauthenticationRequired)
    ));
    let foreign_subject = Uuid::new_v4().to_string();
    let foreign_token = SecretToken::generate().unwrap();
    let foreign = store
        .login(account_write(
            Provider::Naver,
            &foreign_subject,
            &foreign_token,
            8000,
        ))
        .await
        .unwrap();
    let new_token = SecretToken::generate().unwrap();
    let mut conflict = account_write(Provider::Naver, &foreign_subject, &new_token, 8001);
    conflict.intent = AuthIntent::Link(record.account.id);
    conflict.bound_session = Some(token.hash());
    conflict.previous_session = Some(token.hash());
    assert!(matches!(
        store.login(conflict).await,
        Err(AuthError::Conflict)
    ));
    assert!(store.session(token.hash(), 8001).await.unwrap().is_some());
    let browser = SecretToken::generate().unwrap();
    let request = |provider, intent| AccountAuthorization {
        provider,
        intent,
        return_path: ReturnPath::Settings,
        locale: AuthLocale::parse("ko").unwrap(),
    };
    let auth = service
        .start_account(
            &browser,
            &token,
            request(Provider::Naver, AuthIntent::Link(record.account.id)),
            8001,
        )
        .await
        .unwrap();
    let tx = service
        .consume(&auth.state, &browser, Provider::Naver, 8002)
        .await
        .unwrap();
    let bound: Vec<u8> =
        sqlx::query_scalar("SELECT token_hash FROM auth_sessions WHERE account_id=$1")
            .bind(record.account.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(tx.bound_session_hash.unwrap().as_slice(), bound);
    let linked = service
        .finish_verified(
            tx,
            VerifiedIdentity {
                subject: Zeroizing::new(subject.clone()),
                apple_refresh: None,
            },
            Some(&token),
            8002,
        )
        .await
        .unwrap();
    assert_eq!(linked.account.id, record.account.id);
    assert!(store.session(token.hash(), 8002).await.unwrap().is_none());
    assert_eq!(
        store
            .identities(authority(linked.token.hash(), 8003))
            .await
            .unwrap()
            .len(),
        2
    );
    let stale_link = service
        .start_account(
            &browser,
            &linked.token,
            request(Provider::Kakao, AuthIntent::Link(record.account.id)),
            8301,
        )
        .await
        .unwrap();
    let tx = service
        .consume(&stale_link.state, &browser, Provider::Kakao, 8302)
        .await
        .unwrap();
    assert!(matches!(
        service
            .finish_verified(
                tx,
                VerifiedIdentity {
                    subject: Zeroizing::new(subject.clone()),
                    apple_refresh: None
                },
                Some(&linked.token),
                8302
            )
            .await,
        Err(AuthError::ReauthenticationRequired)
    ));
    let reauth = service
        .start_account(
            &browser,
            &linked.token,
            request(Provider::Google, AuthIntent::Reauth(record.account.id)),
            8302,
        )
        .await
        .unwrap();
    let tx = service
        .consume(&reauth.state, &browser, Provider::Google, 8302)
        .await
        .unwrap();
    let reauthenticated = service
        .finish_verified(
            tx,
            VerifiedIdentity {
                subject: Zeroizing::new(subject.clone()),
                apple_refresh: None,
            },
            Some(&linked.token),
            8302,
        )
        .await
        .unwrap();
    assert!(
        store
            .export(authority(reauthenticated.token.hash(), 8303))
            .await
            .is_ok()
    );
    let mut revoked = account_write(Provider::Kakao, &subject, &new_token, 8303);
    revoked.intent = AuthIntent::Link(record.account.id);
    revoked.bound_session = Some(linked.token.hash());
    revoked.previous_session = Some(linked.token.hash());
    assert!(matches!(
        store.login(revoked).await,
        Err(AuthError::Unauthenticated)
    ));
    let mut wrong_subject = account_write(Provider::Google, &foreign_subject, &new_token, 8303);
    wrong_subject.intent = AuthIntent::Reauth(record.account.id);
    wrong_subject.bound_session = Some(reauthenticated.token.hash());
    wrong_subject.previous_session = Some(reauthenticated.token.hash());
    assert!(matches!(
        store.login(wrong_subject).await,
        Err(AuthError::Invalid)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_accounts WHERE id IN ($1,$2)")
            .bind(record.account.id)
            .bind(foreign.account.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    close_auth_pool(pool).await;
}
struct RejectVault;
impl CredentialVault for RejectVault {
    fn seal(
        &self,
        _: Uuid,
        _: Provider,
        _: CredentialPurpose,
        _: &[u8],
    ) -> Result<Vec<u8>, AuthError> {
        Err(AuthError::Unavailable)
    }
    fn open(
        &self,
        _: Uuid,
        _: Provider,
        _: CredentialPurpose,
        _: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        Err(AuthError::Unavailable)
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn credential_failure_rolls_back_and_missing_credentials_do_not_prevent_erasure() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault);
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &Uuid::new_v4().to_string(),
            &token,
            9000,
        ))
        .await
        .unwrap();
    let failing = PgAuthStore::with_vault(pool.clone(), Arc::new(RejectVault));
    let new_token = SecretToken::generate().unwrap();
    let mut write = account_write(
        Provider::Apple,
        &Uuid::new_v4().to_string(),
        &new_token,
        9001,
    );
    write.intent = AuthIntent::Link(record.account.id);
    write.bound_session = Some(token.hash());
    write.previous_session = Some(token.hash());
    assert!(matches!(
        failing.login(write).await,
        Err(AuthError::Unavailable)
    ));
    assert!(store.session(token.hash(), 9001).await.unwrap().is_some());
    assert!(
        store
            .session(new_token.hash(), 9001)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .identities(SessionAuthority {
                account: record.account.id,
                hash: token.hash(),
                now: 9001
            })
            .await
            .unwrap()
            .len(),
        1
    );
    let apple_token = SecretToken::generate().unwrap();
    let apple = store
        .login(account_write(
            Provider::Apple,
            &Uuid::new_v4().to_string(),
            &apple_token,
            9001,
        ))
        .await
        .unwrap();
    sqlx::query("DELETE FROM auth_credentials WHERE identity_id=$1")
        .bind(apple.identity_id)
        .execute(&pool)
        .await
        .unwrap();
    let erased = store
        .erase_authorized(SessionAuthority {
            account: apple.account.id,
            hash: apple_token.hash(),
            now: 9002,
        })
        .await
        .unwrap();
    assert!(erased.manual_apple_disconnect);
    assert!(
        store
            .session(apple_token.hash(), 9002)
            .await
            .unwrap()
            .is_none()
    );
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn concurrent_unlinks_and_deletions_preserve_last_provider_and_erase_owned_records() {
    let pool = auth_pool().await;
    let store = PgAuthStore::new(pool.clone());
    let subject = Uuid::new_v4().to_string();
    let first_token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Google,
            &subject,
            &first_token,
            10000,
        ))
        .await
        .unwrap();
    let second_token = SecretToken::generate().unwrap();
    let mut link = account_write(Provider::Naver, &subject, &second_token, 10001);
    link.intent = AuthIntent::Link(record.account.id);
    link.bound_session = Some(first_token.hash());
    link.previous_session = Some(first_token.hash());
    store.login(link).await.unwrap();
    let third_token = SecretToken::generate().unwrap();
    store
        .login(account_write(
            Provider::Google,
            &subject,
            &third_token,
            10001,
        ))
        .await
        .unwrap();
    let auth = |hash| SessionAuthority {
        account: record.account.id,
        hash,
        now: 10002,
    };
    let (a, b) = tokio::join!(
        store.unlink(auth(second_token.hash()), Provider::Google),
        store.unlink(auth(third_token.hash()), Provider::Naver)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let remaining: String =
        sqlx::query_scalar("SELECT provider FROM auth_identities WHERE account_id=$1")
            .bind(record.account.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let provider = Provider::parse(&remaining).unwrap();
    let token = SecretToken::generate().unwrap();
    store
        .login(account_write(provider, &subject, &token, 10003))
        .await
        .unwrap();
    sqlx::query("CREATE TABLE auth_fixture_owned_data(account_id uuid REFERENCES auth_accounts(id) ON DELETE CASCADE, data text)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO auth_fixture_owned_data VALUES($1,'fixture-owned-record')")
        .bind(record.account.id)
        .execute(&pool)
        .await
        .unwrap();
    let auth = SessionAuthority {
        account: record.account.id,
        hash: token.hash(),
        now: 10004,
    };
    let (a, b) = tokio::join!(store.erase_authorized(auth), store.erase_authorized(auth));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_fixture_owned_data")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(store.session(token.hash(), 10004).await.unwrap().is_none());
    store.cleanup(10004 + 28 * 86400).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM auth_deletion_tombstones")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
struct MaintenanceClock;
impl AuthClock for MaintenanceClock {
    fn now(&self) -> i64 {
        7001
    }
}
struct MaintenanceProvider {
    failure: std::sync::atomic::AtomicBool,
}
impl OAuthProvider for MaintenanceProvider {
    fn available(&self, p: Provider) -> bool {
        p == Provider::Apple
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
impl AppleProvider for MaintenanceProvider {
    async fn notification(&self, _: &str, _: i64) -> Result<AppleNotification, AuthError> {
        Err(AuthError::Invalid)
    }
    async fn revoke_apple(&self, refresh: &str, _: i64) -> Result<(), AuthError> {
        assert_eq!(refresh, "fixture-maintenance-refresh");
        if self.failure.load(std::sync::atomic::Ordering::SeqCst) {
            Err(AuthError::Unavailable)
        } else {
            Ok(())
        }
    }
    async fn check_apple(&self, _: &str, _: i64) -> Result<AppleCredentialStatus, AuthError> {
        Err(AuthError::Unavailable)
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn maintenance_worker_runs_real_queue_without_blocking_local_deletion() {
    let pool = auth_pool().await;
    let vault = Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap());
    let store = PgAuthStore::with_vault(pool.clone(), vault.clone());
    let token = SecretToken::generate().unwrap();
    let record = store
        .login(account_write(
            Provider::Apple,
            &Uuid::new_v4().to_string(),
            &token,
            7000,
        ))
        .await
        .unwrap();
    store
        .erase_authorized(SessionAuthority {
            account: record.account.id,
            hash: token.hash(),
            now: 7001,
        })
        .await
        .unwrap();
    let providers = Arc::new(MaintenanceProvider {
        failure: std::sync::atomic::AtomicBool::new(true),
    });
    let worker = AppleMaintenance::new(
        store.clone(),
        vault,
        DigestKeys::new(1, vec![(1, [16; 32])]).unwrap(),
        providers.clone(),
        Arc::new(MaintenanceClock),
    );
    worker
        .run_once()
        .await
        .expect("worker must back off a provider outage after local deletion");
    assert!(store.session(token.hash(), 7001).await.unwrap().is_none());
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM auth_apple_revoke_queue WHERE identity_id=$1")
            .bind(record.identity_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    providers
        .failure
        .store(false, std::sync::atomic::Ordering::SeqCst);
    sqlx::query("UPDATE auth_apple_revoke_queue SET next_attempt_at=to_timestamp(7001) WHERE identity_id=$1").bind(record.identity_id).execute(&pool).await.unwrap();
    worker.run_once().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM auth_apple_revoke_queue WHERE identity_id=$1"
        )
        .bind(record.identity_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    close_auth_pool(pool).await;
}
pub(super) fn account_write(
    provider: Provider,
    subject: &str,
    token: &SecretToken,
    now: i64,
) -> LoginWrite {
    LoginWrite {
        provider,
        digests: DigestKeys::new(1, vec![(1, [16; 32])])
            .unwrap()
            .digest(provider, subject)
            .unwrap(),
        intent: AuthIntent::Login,
        bound_session: None,
        session_hash: token.hash(),
        previous_session: None,
        now,
        apple_refresh: (provider == Provider::Apple)
            .then(|| Zeroizing::new("fixture-maintenance-refresh".into())),
    }
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn apple_notifications_are_atomic_deduplicated_and_keep_other_login_methods() {
    let pool = auth_pool().await;
    let store = PgAuthStore::with_vault(
        pool.clone(),
        Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap()),
    );
    let token = SecretToken::generate().unwrap();
    let subject = Uuid::new_v4().to_string();
    let record = store
        .login(account_write(Provider::Apple, &subject, &token, 5000))
        .await
        .unwrap();
    let digests = DigestKeys::new(1, vec![(1, [16; 32])])
        .unwrap()
        .digest(Provider::Apple, &subject)
        .unwrap();
    let receipt = SecretToken::generate().unwrap().hash();
    assert_eq!(
        store
            .apply_notification(AppleNoticeWrite {
                jti_hash: receipt,
                digests: Some(digests.clone()),
                occurred_at: 5001,
                now: 5001
            })
            .await
            .expect("verified notification must erase the last Apple identity atomically"),
        Some(record.account.id)
    );
    assert!(store.session(token.hash(), 5001).await.unwrap().is_none());
    assert!(
        store
            .apply_notification(AppleNoticeWrite {
                jti_hash: receipt,
                digests: Some(digests.clone()),
                occurred_at: 5002,
                now: 5002
            })
            .await
            .unwrap()
            .is_none()
    );
    // A replay cannot erase a subsequently registered account with the same provider subject.
    let second = store
        .login(account_write(Provider::Apple, &subject, &token, 5003))
        .await
        .unwrap();
    assert_ne!(second.account.id, record.account.id);
    assert!(
        store
            .apply_notification(AppleNoticeWrite {
                jti_hash: receipt,
                digests: Some(digests.clone()),
                occurred_at: 5004,
                now: 5004
            })
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.session(token.hash(), 5004).await.unwrap().is_some());
    assert!(
        store
            .apply_notification(AppleNoticeWrite {
                jti_hash: SecretToken::generate().unwrap().hash(),
                digests: Some(digests.clone()),
                occurred_at: 5001,
                now: 5004
            })
            .await
            .unwrap()
            .is_none(),
        "distinct late events cannot erase a newer credential"
    );
    assert!(store.session(token.hash(), 5004).await.unwrap().is_some());
    let linked_token = SecretToken::generate().unwrap();
    let mut link = account_write(
        Provider::Google,
        &Uuid::new_v4().to_string(),
        &linked_token,
        5005,
    );
    link.intent = AuthIntent::Link(second.account.id);
    link.bound_session = Some(token.hash());
    link.previous_session = Some(token.hash());
    store.login(link).await.unwrap();
    let new_receipt = SecretToken::generate().unwrap().hash();
    assert_eq!(
        store
            .apply_notification(AppleNoticeWrite {
                jti_hash: new_receipt,
                digests: Some(digests),
                occurred_at: 5006,
                now: 5006
            })
            .await
            .unwrap(),
        Some(second.account.id)
    );
    assert!(
        store
            .session(linked_token.hash(), 5006)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM auth_identities WHERE account_id=$1 AND provider='google'"
        )
        .bind(second.account.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    sqlx::query("DELETE FROM auth_accounts WHERE id=$1")
        .bind(second.account.id)
        .execute(&pool)
        .await
        .unwrap();
    close_auth_pool(pool).await;
}
#[tokio::test]
#[ignore = "requires real PostgreSQL18; executed in database CI"]
async fn apple_daily_checks_do_not_delete_accounts_on_outages_or_stale_credentials() {
    let pool = auth_pool().await;
    let store = PgAuthStore::with_vault(
        pool.clone(),
        Arc::new(AeadVault::new(1, vec![(1, [15; 32])]).unwrap()),
    );
    let token = SecretToken::generate().unwrap();
    let subject = Uuid::new_v4().to_string();
    let record = store
        .login(account_write(Provider::Apple, &subject, &token, 6000))
        .await
        .unwrap();
    assert!(
        store
            .claim_credential(6001)
            .await
            .expect("daily schedule must be queryable")
            .is_none()
    );
    let job = store.claim_credential(92400).await.unwrap().unwrap();
    assert_eq!(job.identity, record.identity_id);
    assert!(store.claim_credential(92400).await.unwrap().is_none());
    assert!(
        store
            .finish_credential(&job, CredentialCheck::Unavailable, 92401)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.session(token.hash(), 92401).await.unwrap().is_some());
    // A login replacing the ciphertext protects the new credential from an old in-flight invalid_grant.
    assert!(matches!(
        store
            .login(account_write(Provider::Apple, &subject, &token, 92402))
            .await,
        Err(AuthError::Conflict)
    ));
    let replacement_token = SecretToken::generate().unwrap();
    let mut replacement = account_write(Provider::Apple, &subject, &replacement_token, 92402);
    replacement.previous_session = Some(token.hash());
    store.login(replacement).await.unwrap();
    assert!(
        store
            .finish_credential(&job, CredentialCheck::Revoked, 92403)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .session(replacement_token.hash(), 92403)
            .await
            .unwrap()
            .is_some()
    );
    let new_job = store.claim_credential(178802).await.unwrap().unwrap();
    let digests = DigestKeys::new(1, vec![(1, [16; 32])])
        .unwrap()
        .digest(Provider::Apple, &subject)
        .unwrap();
    assert!(
        store
            .finish_credential(
                &new_job,
                CredentialCheck::Valid {
                    digests,
                    replacement: Some(Zeroizing::new("fixture-rotated-refresh".into()))
                },
                178803
            )
            .await
            .unwrap()
            .is_none()
    );
    let confirmed = store.claim_credential(265202).await.unwrap().unwrap();
    assert_eq!(
        store
            .finish_credential(&confirmed, CredentialCheck::Revoked, 265203)
            .await
            .unwrap(),
        Some(record.account.id)
    );
    assert!(
        store
            .session(replacement_token.hash(), 265203)
            .await
            .unwrap()
            .is_none()
    );
    close_auth_pool(pool).await;
}
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
    close_auth_pool(pool).await;
}
