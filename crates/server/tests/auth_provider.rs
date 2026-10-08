use jsonwebtoken::{Algorithm, EncodingKey, Header};
use liar_server::auth::*;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

fn config(provider: Provider) -> ProviderConfig {
    ProviderConfig {
        provider,
        client_id: "test-client".into(),
        client_secret: Zeroizing::new("fixture-client-secret".into()),
        callback: url::Url::parse(&format!(
            "https://game.example/api/v1/auth/{}/callback",
            provider.as_str()
        ))
        .unwrap(),
        apple: (provider == Provider::Apple).then(|| AppleSigning {
            team_id: "TEAMTEST01".into(),
            key_id: "KEYTEST001".into(),
            private_key: Zeroizing::new(
                include_bytes!("fixtures/auth/apple-test-only.pem").to_vec(),
            ),
        }),
    }
}
fn auth() -> Authorization {
    Authorization {
        id: Uuid::new_v4(),
        state: SecretToken::generate().unwrap(),
        nonce: SecretToken::generate().unwrap(),
        verifier: Some(SecretToken::generate().unwrap()),
    }
}
fn tx(auth: &Authorization, provider: Provider) -> AuthTransaction {
    AuthTransaction {
        bound_session_hash: None,
        id: auth.id,
        state_hash: auth.state.hash(),
        browser_hash: [9; 32],
        nonce_hash: auth.nonce.hash(),
        provider,
        intent: AuthIntent::Login,
        return_path: ReturnPath::Home,
        locale: AuthLocale::parse("en").unwrap(),
        created_at: 1_000,
        expires_at: 1_300,
        encrypted_verifier: None,
    }
}
fn signed(claims: &Value, algorithm: Algorithm, kid: &str) -> String {
    let mut header = Header::new(algorithm);
    header.kid = Some(kid.into());
    jsonwebtoken::encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(include_bytes!("fixtures/auth/rsa-test-only.pem")).unwrap(),
    )
    .unwrap()
}
#[test]
fn provider_requests_are_minimal_and_bind_state_nonce_pkce() {
    let a = auth();
    for provider in [
        Provider::Google,
        Provider::Apple,
        Provider::Kakao,
        Provider::Naver,
    ] {
        let url = url::Url::parse(
            &config(provider)
                .authorize(&a)
                .expect("Configured provider must authorize"),
        )
        .unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["state"], a.state.expose().as_str());
        assert!(!q.contains_key("client_secret"));
        assert!(
            !q.values()
                .any(|v| v.contains("email") || v.contains("profile"))
        );
        if provider.uses_pkce() {
            assert_eq!(q["scope"], "openid");
            assert_eq!(q["code_challenge_method"], "S256");
            assert_eq!(
                q["code_challenge"],
                a.verifier.as_ref().unwrap().pkce_challenge()
            );
        } else {
            assert!(!q.contains_key("scope"));
            assert!(!q.contains_key("code_challenge"));
        }
        if provider != Provider::Naver {
            assert_eq!(q["nonce"], a.nonce.expose().as_str());
        }
        if provider == Provider::Apple {
            assert_eq!(q["response_mode"], "query");
        }
        let mut c = config(provider);
        c.callback = url::Url::parse("http://evil.example/").unwrap();
        assert!(c.authorize(&a).is_err());
    }
}
#[test]
fn oidc_verifies_signed_minimum_claims_and_rejects_each_boundary() {
    let a = auth();
    let jwks = include_bytes!("fixtures/auth/jwks.json");
    for provider in [Provider::Google, Provider::Apple, Provider::Kakao] {
        let c = config(provider);
        let t = tx(&a, provider);
        let claims = json!({"iss":provider.issuer(),"sub":"fixture-subject","aud":"test-client",
            "exp":1200,"iat":1000,"nonce":a.nonce.expose().as_str(),"email":"discard@example.com","name":"discard"});
        assert_eq!(
            verify_id_token(
                &c,
                jwks,
                &signed(&claims, Algorithm::RS256, "fixture-1"),
                &t,
                None,
                1100
            )
            .expect("Valid signed identity must verify")
            .as_str(),
            "fixture-subject"
        );
        for (key, value) in [
            ("iss", json!("https://attacker.example")),
            ("aud", json!("other-client")),
            ("azp", json!("other-client")),
            ("nonce", json!("wrong")),
            ("exp", json!(1100)),
            ("iat", json!(1131)),
            ("iat", json!(969)),
            ("sub", json!("")),
        ] {
            let mut changed = claims.clone();
            changed[key] = value;
            assert!(
                verify_id_token(
                    &c,
                    jwks,
                    &signed(&changed, Algorithm::RS256, "fixture-1"),
                    &t,
                    None,
                    1100
                )
                .is_err(),
                "{key}"
            );
        }
        for algorithm in [Algorithm::RS384, Algorithm::RS512] {
            assert!(
                verify_id_token(
                    &c,
                    jwks,
                    &signed(&claims, algorithm, "fixture-1"),
                    &t,
                    None,
                    1100
                )
                .is_err()
            );
        }
        assert!(
            verify_id_token(
                &c,
                jwks,
                &signed(&claims, Algorithm::RS256, "unknown"),
                &t,
                None,
                1100
            )
            .is_err()
        );
        let mut no_nonce = claims.clone();
        no_nonce.as_object_mut().unwrap().remove("nonce");
        assert!(
            verify_id_token(
                &c,
                jwks,
                &signed(&no_nonce, Algorithm::RS256, "fixture-1"),
                &t,
                None,
                1100
            )
            .is_err()
        );
        if provider == Provider::Google {
            let mut bare = claims.clone();
            bare["iss"] = json!("accounts.google.com");
            assert!(
                verify_id_token(
                    &c,
                    jwks,
                    &signed(&bare, Algorithm::RS256, "fixture-1"),
                    &t,
                    None,
                    1100
                )
                .is_ok()
            );
        }
    }
}
#[test]
fn apple_assertion_uses_short_lived_es256_services_id() {
    let assertion = config(Provider::Apple)
        .assertion(1000)
        .expect("Apple client assertion");
    let header = jsonwebtoken::decode_header(&assertion).unwrap();
    assert_eq!(header.alg, Algorithm::ES256);
    assert_eq!(header.kid.as_deref(), Some("KEYTEST001"));
    let mut validation = jsonwebtoken::Validation::new(Algorithm::ES256);
    validation.validate_exp = false;
    validation.set_audience(&["https://appleid.apple.com"]);
    validation.set_issuer(&["TEAMTEST01"]);
    let value = jsonwebtoken::decode::<Value>(
        &assertion,
        &jsonwebtoken::DecodingKey::from_ec_pem(include_bytes!("fixtures/auth/apple-public.pem"))
            .unwrap(),
        &validation,
    )
    .unwrap();
    assert_eq!(value.claims["sub"], "test-client");
    assert_eq!(value.claims["iat"], 1000);
    assert_eq!(value.claims["exp"], 1300);
}
#[test]
fn latest_http_client_constructs_with_enabled_tls_backends() {
    assert!(
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .is_ok()
    );
}
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        1100
    }
}
#[derive(Clone)]
struct FixtureTransport {
    calls: std::sync::Arc<std::sync::Mutex<Vec<UpstreamCall>>>,
    body: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    jwks: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}
type UpstreamCall = (&'static str, Vec<(String, String)>, Option<String>);
fn fixture_transport(body: Value) -> FixtureTransport {
    FixtureTransport {
        calls: Default::default(),
        body: std::sync::Arc::new(std::sync::Mutex::new(serde_json::to_vec(&body).unwrap())),
        jwks: std::sync::Arc::new(std::sync::Mutex::new(
            include_bytes!("fixtures/auth/jwks.json").to_vec(),
        )),
    }
}
impl OAuthTransport for FixtureTransport {
    async fn request(&self, request: UpstreamRequest) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        self.calls.lock().unwrap().push((
            request.endpoint,
            request
                .form
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            request.bearer.map(|v| v.to_string()),
        ));
        Ok(Zeroizing::new(
            if request.endpoint.ends_with("certs")
                || request.endpoint.ends_with("keys")
                || request.endpoint.ends_with("jwks.json")
            {
                self.jwks.lock().unwrap().clone()
            } else if request.endpoint.ends_with("/me") {
                br#"{"resultcode":"00","response":{"id":"naver-subject","email":"discard@example.com","name":"discard"}}"#.to_vec()
            } else {
                self.body.lock().unwrap().clone()
            },
        ))
    }
}
#[tokio::test]
async fn exchange_uses_fixed_post_endpoints_pkce_and_naver_id_only() {
    let a = auth();
    for provider in [
        Provider::Google,
        Provider::Apple,
        Provider::Kakao,
        Provider::Naver,
    ] {
        let claims = json!({"iss":provider.issuer(),"sub":"fixture-subject","aud":"test-client","exp":1200,"iat":1000,"nonce":a.nonce.expose().as_str()});
        let transport = fixture_transport(
            json!({"access_token":"fixture-access","token_type":"Bearer","id_token":signed(&claims,Algorithm::RS256,"fixture-1"),"refresh_token":"fixture-refresh"}),
        );
        let registry = ProviderRegistry::new(
            transport.clone(),
            std::sync::Arc::new(Clock),
            vec![config(provider)],
        )
        .unwrap();
        let verifier = a.verifier.as_ref().unwrap().expose();
        let identity = registry
            .exchange(
                &tx(&a, provider),
                &a.state,
                "fixture-code",
                provider.uses_pkce().then_some(verifier.as_bytes()),
                1100,
            )
            .await
            .expect("Exchange must verify identity");
        assert_eq!(
            identity.subject.as_str(),
            if provider == Provider::Naver {
                "naver-subject"
            } else {
                "fixture-subject"
            }
        );
        assert_eq!(
            identity.apple_refresh.as_deref().map(|s| s.as_str()),
            (provider == Provider::Apple).then_some("fixture-refresh")
        );
        let calls = transport.calls.lock().unwrap();
        let form: std::collections::HashMap<_, _> = calls[0].1.iter().cloned().collect();
        assert!(calls[0].0.starts_with("https://"));
        assert!(!calls[0].0.contains('?'));
        assert_eq!(form["code"], "fixture-code");
        assert_eq!(form["client_id"], "test-client");
        assert_eq!(form.contains_key("code_verifier"), provider.uses_pkce());
        if provider == Provider::Naver {
            assert_eq!(form["state"], a.state.expose().as_str());
            assert_eq!(calls[1].2.as_deref(), Some("fixture-access"));
        }
    }
}
#[test]
fn signature_audience_extensions_and_jwks_ambiguity_never_establish_identity() {
    let a = auth();
    let c = config(Provider::Google);
    let t = tx(&a, Provider::Google);
    let jwks = include_bytes!("fixtures/auth/jwks.json");
    let claims = json!({"iss":c.provider.issuer(),"sub":"fixture-subject","aud":"test-client","exp":1200,"iat":1000,"nonce":a.nonce.expose().as_str()});
    for (field, value) in [
        ("aud", json!(["test-client", "untrusted"])),
        ("aud", json!([])),
        ("nbf", json!(1101)),
        ("sub", json!("bad\nsubject")),
        ("at_hash", json!("incorrect")),
    ] {
        let mut altered = claims.clone();
        altered[field] = value;
        assert!(
            verify_id_token(
                &c,
                jwks,
                &signed(&altered, Algorithm::RS256, "fixture-1"),
                &t,
                Some("access"),
                1100
            )
            .is_err()
        );
    }
    let mut good = claims.clone();
    good["aud"] = json!(["test-client"]);
    good["azp"] = json!("test-client");
    good["nbf"] = json!(1099);
    assert!(
        verify_id_token(
            &c,
            jwks,
            &signed(&good, Algorithm::RS256, "fixture-1"),
            &t,
            None,
            1100
        )
        .is_ok()
    );
    let signed_token = signed(&claims, Algorithm::RS256, "fixture-1");
    let mut tampered = signed_token.into_bytes();
    let last = tampered.len() - 5;
    tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
    assert!(
        verify_id_token(
            &c,
            jwks,
            std::str::from_utf8(&tampered).unwrap(),
            &t,
            None,
            1100
        )
        .is_err()
    );
    let hs = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"fixture-secret"),
    )
    .unwrap();
    assert!(verify_id_token(&c, jwks, &hs, &t, None, 1100).is_err());
    let mut duplicate: Value = serde_json::from_slice(jwks).unwrap();
    let key = duplicate["keys"][0].clone();
    duplicate["keys"].as_array_mut().unwrap().push(key);
    assert!(
        verify_id_token(
            &c,
            &serde_json::to_vec(&duplicate).unwrap(),
            &signed(&claims, Algorithm::RS256, "fixture-1"),
            &t,
            None,
            1100
        )
        .is_err()
    );
    for key in ["crit", "jku", "x5u"] {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("fixture-1".into());
        match key {
            "crit" => header.crit = Some(vec!["unsupported".into()]),
            "jku" => header.jku = Some("https://attacker.example".into()),
            _ => header.x5u = Some("https://attacker.example".into()),
        }
        let token = jsonwebtoken::encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(include_bytes!("fixtures/auth/rsa-test-only.pem")).unwrap(),
        )
        .unwrap();
        assert!(verify_id_token(&c, jwks, &token, &t, None, 1100).is_err());
    }
}
#[tokio::test]
async fn unknown_kid_rotation_refreshes_once_and_coalesces_parallel_lookups() {
    let a = auth();
    let t = tx(&a, Provider::Google);
    let claims = json!({"iss":Provider::Google.issuer(),"sub":"fixture-subject","aud":"test-client","exp":1200,"iat":1000,"nonce":a.nonce.expose().as_str()});
    let transport =
        fixture_transport(json!({"id_token":signed(&claims,Algorithm::RS256,"fixture-1")}));
    let registry = ProviderRegistry::new(
        transport.clone(),
        std::sync::Arc::new(Clock),
        vec![config(Provider::Google)],
    )
    .unwrap();
    let verifier = a.verifier.as_ref().unwrap().expose();
    registry
        .exchange(
            &t,
            &a.state,
            "fixture-code",
            Some(verifier.as_bytes()),
            1100,
        )
        .await
        .unwrap();
    let mut keys: Value =
        serde_json::from_slice(include_bytes!("fixtures/auth/jwks.json")).unwrap();
    keys["keys"][0]["kid"] = json!("fixture-2");
    *transport.jwks.lock().unwrap() = serde_json::to_vec(&keys).unwrap();
    *transport.body.lock().unwrap() =
        serde_json::to_vec(&json!({"id_token":signed(&claims,Algorithm::RS256,"fixture-2")}))
            .unwrap();
    let (one, two) = tokio::join!(
        registry.exchange(
            &t,
            &a.state,
            "fixture-code",
            Some(verifier.as_bytes()),
            1100
        ),
        registry.exchange(
            &t,
            &a.state,
            "fixture-code",
            Some(verifier.as_bytes()),
            1100
        )
    );
    assert!(one.is_ok() && two.is_ok());
    assert_eq!(
        transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(url, _, _)| url.ends_with("certs"))
            .count(),
        2
    );
    *transport.body.lock().unwrap() =
        serde_json::to_vec(&json!({"id_token":signed(&claims,Algorithm::RS256,"attacker-kid")}))
            .unwrap();
    assert!(
        registry
            .exchange(
                &t,
                &a.state,
                "fixture-code",
                Some(verifier.as_bytes()),
                1100
            )
            .await
            .is_err()
    );
    assert_eq!(
        transport
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(url, _, _)| url.ends_with("certs"))
            .count(),
        2
    );
}
struct StalledTransport;
impl OAuthTransport for StalledTransport {
    async fn request(&self, _request: UpstreamRequest) -> Result<Zeroizing<Vec<u8>>, AuthError> {
        std::future::pending().await
    }
}
#[tokio::test(start_paused = true)]
async fn stalled_upstream_port_cannot_hold_a_callback_indefinitely() {
    let a = auth();
    let t = tx(&a, Provider::Google);
    let verifier = a.verifier.as_ref().unwrap().expose();
    let registry = ProviderRegistry::new(
        StalledTransport,
        std::sync::Arc::new(Clock),
        vec![config(Provider::Google)],
    )
    .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(11),
        registry.exchange(
            &t,
            &a.state,
            "fixture-code",
            Some(verifier.as_bytes()),
            1100,
        ),
    )
    .await
    .expect("Registry must enforce upstream deadline");
    assert!(matches!(result, Err(AuthError::Unavailable)));
}
