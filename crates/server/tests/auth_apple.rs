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
