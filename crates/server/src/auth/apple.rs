use super::*;
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppleChange {
    Revoke,
    Delete,
    Ignore,
}
pub struct AppleNotification {
    pub jti_hash: [u8; 32],
    pub change: AppleChange,
    pub subject: Zeroizing<String>,
}
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Events {
    Object(Event),
    Encoded(String),
}
#[derive(serde::Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    sub: String,
    event_time: i64,
}
#[derive(serde::Deserialize)]
struct NotificationClaims {
    aud: super::provider::Audience,
    iat: i64,
    jti: String,
    exp: Option<i64>,
    nbf: Option<i64>,
    events: Events,
}
pub fn verify_apple_notification(
    keys: &[u8],
    token: &str,
    audience: &str,
    now: i64,
) -> Result<AppleNotification, AuthError> {
    use sha2::{Digest, Sha256};
    if !super::provider::bounded_text(audience, 512) || now < 0 {
        return Err(AuthError::Invalid);
    }
    let c: NotificationClaims = super::provider::decode_signed(
        keys,
        token,
        audience,
        &[Provider::Apple.issuer()],
        &["iss", "aud"],
    )?;
    let fresh = |time: i64| {
        time >= 0 && time >= now.saturating_sub(86400) && time <= now.saturating_add(30)
    };
    if !c.aud.matches(audience)
        || !fresh(c.iat)
        || !super::provider::bounded_text(&c.jti, 256)
        || c.exp.is_some_and(|e| e <= now || e <= c.iat)
        || c.nbf.is_some_and(|n| n > now)
    {
        return Err(AuthError::Invalid);
    }
    let event = match c.events {
        Events::Object(e) => e,
        Events::Encoded(s) if s.len() <= 4096 => {
            serde_json::from_str(&s).map_err(|_| AuthError::Invalid)?
        }
        _ => return Err(AuthError::Invalid),
    };
    if !fresh(event.event_time)
        || event.sub.is_empty()
        || event.sub.len() > 512
        || event.sub.chars().any(char::is_control)
    {
        return Err(AuthError::Invalid);
    }
    let change = match event.kind.as_str() {
        "consent-revoked" => AppleChange::Revoke,
        "account-deleted" | "account-delete" => AppleChange::Delete,
        "email-enabled" | "email-disabled" => AppleChange::Ignore,
        _ => return Err(AuthError::Invalid),
    };
    Ok(AppleNotification {
        jti_hash: Sha256::digest(c.jti.as_bytes()).into(),
        change,
        subject: Zeroizing::new(event.sub),
    })
}
