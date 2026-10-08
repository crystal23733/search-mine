use jsonwebtoken::{Algorithm, EncodingKey, Header};
use liar_server::auth::*;
use serde_json::{Value, json};
fn signed(claims: &Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("fixture-1".into());
    jsonwebtoken::encode(
        &header,
        claims,
        &EncodingKey::from_rsa_pem(include_bytes!("fixtures/auth/rsa-test-only.pem")).unwrap(),
    )
    .unwrap()
}
fn claims() -> Value {
    json!({"iss":"https://appleid.apple.com","aud":"explicit.notification.audience","iat":1000,"jti":"fixture-notification-1","events":{"type":"consent-revoked","sub":"fixture-apple-sub","event_time":1000}})
}
#[test]
fn signed_notifications_accept_current_and_legacy_events_without_requiring_absent_exp() {
    let keys = include_bytes!("fixtures/auth/jwks.json");
    let mut c = claims();
    let n = verify_apple_notification(keys, &signed(&c), "explicit.notification.audience", 1001)
        .expect("official signed notification has no exp");
    assert!(n.change == AppleChange::Revoke);
    assert_eq!(n.subject.as_str(), "fixture-apple-sub");
    c["events"] = json!(
        json!({"type":"account-delete","sub":"fixture-apple-sub","event_time":1000}).to_string()
    );
    assert!(
        verify_apple_notification(keys, &signed(&c), "explicit.notification.audience", 1001)
            .unwrap()
            .change
            == AppleChange::Delete
    );
}
#[test]
fn notification_signature_audience_times_and_types_cannot_change_authority() {
    let keys = include_bytes!("fixtures/auth/jwks.json");
    let c = claims();
    for (field, value) in [
        ("iss", json!("https://attacker.example")),
        ("aud", json!(["explicit.notification.audience", "other"])),
        ("iat", json!(1032)),
        ("iat", json!(0)),
        ("jti", json!("")),
        ("exp", json!(1001)),
        ("nbf", json!(1002)),
        (
            "events",
            json!({"type":"unknown","sub":"fixture-sub","event_time":1000}),
        ),
        (
            "events",
            json!({"type":"consent-revoked","sub":"bad\nsub","event_time":1000}),
        ),
        (
            "events",
            json!({"type":"account-deleted","sub":"fixture-sub","event_time":1032}),
        ),
    ] {
        let mut bad = c.clone();
        bad[field] = value;
        let now = if field == "iat" && bad[field] == json!(0) {
            100000
        } else {
            1001
        };
        assert!(
            verify_apple_notification(keys, &signed(&bad), "explicit.notification.audience", now)
                .is_err(),
            "{field}"
        );
    }
    let mut bad = signed(&c).into_bytes();
    let index = bad.len() - 20;
    bad[index] = if bad[index] == b'A' { b'B' } else { b'A' };
    assert!(
        verify_apple_notification(
            keys,
            &String::from_utf8(bad).unwrap(),
            "explicit.notification.audience",
            1001
        )
        .is_err()
    );
    assert!(verify_apple_notification(keys, &signed(&c), "different-config", 1001).is_err());
    assert!(verify_apple_notification(keys, &signed(&c), "", 1001).is_err());
    for kind in ["email-enabled", "email-disabled", "account-deleted"] {
        let mut event = c.clone();
        event["events"]["type"] = json!(kind);
        event["events"]["email"] = json!("discard@example.com");
        let checked = verify_apple_notification(
            keys,
            &signed(&event),
            "explicit.notification.audience",
            1001,
        )
        .unwrap();
        assert!(
            checked.change
                == if kind.starts_with("email") {
                    AppleChange::Ignore
                } else {
                    AppleChange::Delete
                }
        );
    }
}
#[test]
fn refreshed_identity_uses_its_own_signed_context_without_login_nonce() {
    let keys = include_bytes!("fixtures/auth/jwks.json");
    let c = json!({"iss":"https://appleid.apple.com","aud":"web.services.id","sub":"fixture-sub","iat":1000,"exp":1300});
    assert_eq!(
        verify_apple_refresh_identity(keys, &signed(&c), "web.services.id", 1001)
            .unwrap()
            .as_str(),
        "fixture-sub"
    );
    for (field, value) in [
        ("aud", json!(["web.services.id", "other"])),
        ("azp", json!("other")),
        ("iat", json!(969)),
        ("iat", json!(1032)),
        ("exp", json!(1001)),
        ("nbf", json!(1002)),
        ("sub", json!("")),
        ("iss", json!("accounts.google.com")),
    ] {
        let mut bad = c.clone();
        bad[field] = value;
        assert!(
            verify_apple_refresh_identity(keys, &signed(&bad), "web.services.id", 1001).is_err(),
            "{field}"
        );
    }
}
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        1001
    }
}
#[derive(Clone)]
struct Transport {
    calls: std::sync::Arc<std::sync::Mutex<Vec<UpstreamRequest>>>,
    mode: std::sync::Arc<std::sync::atomic::AtomicU8>,
}
impl OAuthTransport for Transport {
    async fn request(&self, r: UpstreamRequest) -> Result<zeroize::Zeroizing<Vec<u8>>, AuthError> {
        let endpoint = r.endpoint;
        self.calls.lock().unwrap().push(r);
        if endpoint.ends_with("/keys") {
            return Ok(zeroize::Zeroizing::new(
                include_bytes!("fixtures/auth/jwks.json").to_vec(),
            ));
        }
        if endpoint.ends_with("/revoke") {
            return Ok(zeroize::Zeroizing::new(vec![]));
        }
        match self.mode.load(std::sync::atomic::Ordering::SeqCst) {
            1 => return Err(AuthError::CredentialRevoked),
            2 => return Err(AuthError::Unavailable),
            3 => {
                return Ok(zeroize::Zeroizing::new(
                    br#"{"error":"invalid_client","error_description":"discard private details"}"#
                        .to_vec(),
                ));
            }
            _ => {}
        }
        let token = signed(
            &json!({"iss":"https://appleid.apple.com","aud":"web.services.id","sub":"fixture-credential-sub","iat":1000,"exp":1300}),
        );
        Ok(zeroize::Zeroizing::new(serde_json::to_vec(&json!({"id_token":token,"refresh_token":"fixture-replacement-refresh","access_token":"discard-access"})).unwrap()))
    }
}
#[tokio::test]
async fn apple_maintenance_registry_uses_fixed_endpoints_assertions_and_explicit_audience() {
    use std::sync::{Arc, atomic::Ordering};
    use zeroize::Zeroizing;
    let transport = Transport {
        calls: Default::default(),
        mode: Default::default(),
    };
    let config = || ProviderConfig {
        provider: Provider::Apple,
        client_id: "web.services.id".into(),
        client_secret: Zeroizing::new(String::new()),
        callback: url::Url::parse("https://game.example/api/v1/auth/apple/callback").unwrap(),
        apple: Some(AppleSigning {
            team_id: "TEAMTEST01".into(),
            key_id: "KEYTEST001".into(),
            private_key: Zeroizing::new(
                include_bytes!("fixtures/auth/apple-test-only.pem").to_vec(),
            ),
        }),
    };
    let disabled =
        ProviderRegistry::new(transport.clone(), Arc::new(Clock), vec![config()]).unwrap();
    assert!(matches!(
        disabled.notification(&signed(&claims()), 1001).await,
        Err(AuthError::Unavailable)
    ));
    assert!(transport.calls.lock().unwrap().is_empty());
    assert!(
        ProviderRegistry::new(transport.clone(), Arc::new(Clock), vec![])
            .unwrap()
            .with_notification_audience(Some("explicit.notification.audience".into()))
            .is_err()
    );
    let registry = ProviderRegistry::new(transport.clone(), Arc::new(Clock), vec![config()])
        .unwrap()
        .with_notification_audience(Some("explicit.notification.audience".into()))
        .unwrap();
    assert!(
        registry
            .notification(&signed(&claims()), 1001)
            .await
            .unwrap()
            .change
            == AppleChange::Revoke
    );
    match registry.check_apple("fixture-refresh", 1001).await.unwrap() {
        AppleCredentialStatus::Valid {
            subject,
            replacement,
        } => {
            assert_eq!(subject.as_str(), "fixture-credential-sub");
            assert_eq!(
                replacement.as_deref().map(|s| s.as_str()),
                Some("fixture-replacement-refresh")
            );
        }
        AppleCredentialStatus::Revoked => panic!("valid proof cannot revoke an account"),
    }
    registry
        .revoke_apple("fixture-refresh", 1001)
        .await
        .unwrap();
    {
        let calls = transport.calls.lock().unwrap();
        assert_eq!(
            calls
                .iter()
                .filter(|r| r.endpoint.ends_with("/keys"))
                .count(),
            1
        );
        let token = calls
            .iter()
            .find(|r| r.endpoint.ends_with("/token"))
            .unwrap();
        assert!(
            token
                .form
                .iter()
                .any(|(k, v)| *k == "grant_type" && v.as_str() == "refresh_token")
        );
        assert!(
            token
                .form
                .iter()
                .any(|(k, v)| *k == "client_secret" && v.split('.').count() == 3)
        );
        let revoke = calls
            .iter()
            .find(|r| r.endpoint.ends_with("/revoke"))
            .unwrap();
        assert_eq!(revoke.endpoint, "https://appleid.apple.com/auth/revoke");
        assert!(
            revoke
                .form
                .iter()
                .any(|(k, v)| *k == "token_type_hint" && v.as_str() == "refresh_token")
        );
        assert!(
            calls
                .iter()
                .all(|r| !r.endpoint.contains('?') && r.bearer.is_none())
        );
    }
    transport.mode.store(1, Ordering::SeqCst);
    assert!(matches!(
        registry.check_apple("fixture-refresh", 1001).await.unwrap(),
        AppleCredentialStatus::Revoked
    ));
    for mode in [2, 3] {
        transport.mode.store(mode, Ordering::SeqCst);
        assert!(matches!(
            registry.check_apple("fixture-refresh", 1001).await,
            Err(AuthError::Unavailable)
        ));
    }
    assert!(registry.revoke_apple("bad\nrefresh", 1001).await.is_err());
    assert!(registry.check_apple(&"x".repeat(4097), 1001).await.is_err());
}
