use super::*;
use std::future::Future;
use zeroize::Zeroizing;

pub enum AppleCredentialStatus {
    Valid {
        subject: Zeroizing<String>,
        replacement: Option<Zeroizing<String>>,
    },
    Revoked,
}
pub trait AppleProvider: OAuthProvider {
    fn notification(
        &self,
        token: &str,
        now: i64,
    ) -> impl Future<Output = Result<AppleNotification, AuthError>> + Send;
    fn revoke_apple(
        &self,
        refresh: &str,
        now: i64,
    ) -> impl Future<Output = Result<(), AuthError>> + Send;
    fn check_apple(
        &self,
        refresh: &str,
        now: i64,
    ) -> impl Future<Output = Result<AppleCredentialStatus, AuthError>> + Send;
}
pub struct RevokeJob {
    pub id: uuid::Uuid,
    pub identity: uuid::Uuid,
    pub lease: uuid::Uuid,
    pub encrypted: Vec<u8>,
}
pub struct CredentialJob {
    pub identity: uuid::Uuid,
    pub encrypted: Vec<u8>,
    pub revision: uuid::Uuid,
}
pub struct AppleNoticeWrite {
    pub jti_hash: [u8; 32],
    pub digests: Option<Vec<(u32, [u8; 32])>>,
    pub occurred_at: i64,
    pub now: i64,
}
pub enum CredentialCheck {
    Valid {
        digests: Vec<(u32, [u8; 32])>,
        replacement: Option<Zeroizing<String>>,
    },
    Revoked,
    Unavailable,
}
pub trait AppleMaintenanceStore: AccountStore {
    fn apply_notification(
        &self,
        event: AppleNoticeWrite,
    ) -> impl Future<Output = Result<Option<uuid::Uuid>, AuthError>> + Send;
    fn claim_revoke(
        &self,
        now: i64,
    ) -> impl Future<Output = Result<Option<RevokeJob>, AuthError>> + Send;
    fn finish_revoke(
        &self,
        job: &RevokeJob,
        success: bool,
        now: i64,
    ) -> impl Future<Output = Result<(), AuthError>> + Send;
    fn claim_credential(
        &self,
        now: i64,
    ) -> impl Future<Output = Result<Option<CredentialJob>, AuthError>> + Send;
    fn finish_credential(
        &self,
        job: &CredentialJob,
        status: CredentialCheck,
        now: i64,
    ) -> impl Future<Output = Result<Option<uuid::Uuid>, AuthError>> + Send;
}
#[derive(serde::Deserialize)]
struct RefreshClaims {
    sub: String,
    aud: super::provider::Audience,
    exp: i64,
    iat: i64,
    nbf: Option<i64>,
    azp: Option<String>,
}
pub fn verify_apple_refresh_identity(
    keys: &[u8],
    token: &str,
    client: &str,
    now: i64,
) -> Result<Zeroizing<String>, AuthError> {
    let c: RefreshClaims = super::provider::decode_signed(
        keys,
        token,
        client,
        &[Provider::Apple.issuer()],
        &["exp", "iss", "aud", "sub"],
    )?;
    if now < 0
        || !c.aud.matches(client)
        || c.azp.is_some_and(|a| a != client)
        || c.exp <= now
        || c.iat < now.saturating_sub(30)
        || c.iat > now.saturating_add(30)
        || c.exp <= c.iat
        || c.nbf.is_some_and(|n| n > now)
        || c.sub.is_empty()
        || c.sub.len() > 512
        || c.sub.chars().any(char::is_control)
    {
        return Err(AuthError::Invalid);
    }
    Ok(Zeroizing::new(c.sub))
}

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
    pub occurred_at: i64,
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
        occurred_at: event.event_time,
    })
}
