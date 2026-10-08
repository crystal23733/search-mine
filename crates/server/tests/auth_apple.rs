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
