use crate::{auth::AuthClock, events::MAX_EVENT_TIMESTAMP};
use std::{future::Future, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentError {
    Unauthorized,
    Unavailable,
    Invalid,
    Conflict,
}
#[derive(Clone, Copy)]
pub struct ConsentSession {
    pub account: Uuid,
    pub revision: Uuid,
    pub hash: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionalPurpose {
    Analytics,
    Advertising,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedConsent {
    pub subject: Uuid,
    pub analytics: bool,
    pub advertising: bool,
    pub expires_at: i64,
}
pub trait ConsentVerifier: Send + Sync {
    fn verify(
        &self,
        subject: Uuid,
        now: i64,
    ) -> impl Future<Output = Result<VerifiedConsent, ConsentError>> + Send;
}
pub struct DisabledConsentVerifier;
impl ConsentVerifier for DisabledConsentVerifier {
    async fn verify(&self, _: Uuid, _: i64) -> Result<VerifiedConsent, ConsentError> {
        Err(ConsentError::Unavailable)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentConsent {
    pub subject: Uuid,
    pub revision: Uuid,
    pub checked_at: i64,
    pub expires_at: i64,
    pub analytics: bool,
    pub advertising: bool,
}
impl CurrentConsent {
    pub fn allows(&self, purpose: OptionalPurpose, now: i64) -> bool {
        self.checked_at <= now
            && now < self.expires_at
            && match purpose {
                OptionalPurpose::Analytics => self.analytics,
                OptionalPurpose::Advertising => self.advertising,
            }
    }
}
pub(super) fn checked_now(clock: &Arc<dyn AuthClock>, previous: i64) -> Result<i64, ConsentError> {
    let now = clock.now();
    if now < previous || !(0..=MAX_EVENT_TIMESTAMP).contains(&now) {
        return Err(ConsentError::Invalid);
    }
    Ok(now)
}
pub(super) fn refreshed(
    subject: Uuid,
    expected: Option<Uuid>,
    current: Option<CurrentConsent>,
    evidence: VerifiedConsent,
    now: i64,
    session_expiry: i64,
) -> Result<CurrentConsent, ConsentError> {
    check_revision(subject, expected, current, now, session_expiry)?;
    if evidence.subject != subject
        || evidence.expires_at <= now
        || evidence.expires_at > MAX_EVENT_TIMESTAMP
    {
        return Err(ConsentError::Invalid);
    }
    Ok(CurrentConsent {
        subject,
        revision: Uuid::new_v4(),
        checked_at: now,
        expires_at: evidence.expires_at.min(session_expiry),
        analytics: evidence.analytics,
        advertising: evidence.advertising,
    })
}
fn check_revision(
    subject: Uuid,
    expected: Option<Uuid>,
    current: Option<CurrentConsent>,
    now: i64,
    session_expiry: i64,
) -> Result<(), ConsentError> {
    validate(subject, current, now, session_expiry)?;
    if expected.is_some_and(|id| id.is_nil()) {
        return Err(ConsentError::Invalid);
    }
    if current.map(|value| value.revision) != expected {
        return Err(ConsentError::Conflict);
    }
    Ok(())
}
pub(super) fn suspended(
    subject: Uuid,
    expected: Option<Uuid>,
    current: Option<CurrentConsent>,
    now: i64,
    session_expiry: i64,
) -> Result<Option<CurrentConsent>, ConsentError> {
    check_revision(subject, expected, current, now, session_expiry)?;
    current
        .map(|value| {
            let mut denied = withdrawn(
                subject,
                Some(value),
                OptionalPurpose::Analytics,
                now,
                session_expiry,
            )?;
            denied.advertising = false;
            Ok(denied)
        })
        .transpose()
}
pub(super) fn withdrawn(
    subject: Uuid,
    current: Option<CurrentConsent>,
    purpose: OptionalPurpose,
    now: i64,
    session_expiry: i64,
) -> Result<CurrentConsent, ConsentError> {
    validate(subject, current, now, session_expiry)?;
    let live = current.filter(|value| value.expires_at > now);
    Ok(CurrentConsent {
        subject,
        revision: Uuid::new_v4(),
        checked_at: now,
        expires_at: live.map_or(session_expiry, |value| value.expires_at.min(session_expiry)),
        analytics: purpose != OptionalPurpose::Analytics
            && live.is_some_and(|value| value.analytics),
        advertising: purpose != OptionalPurpose::Advertising
            && live.is_some_and(|value| value.advertising),
    })
}
pub(super) fn validate(
    subject: Uuid,
    current: Option<CurrentConsent>,
    now: i64,
    session_expiry: i64,
) -> Result<(), ConsentError> {
    if subject.is_nil()
        || !(0..MAX_EVENT_TIMESTAMP).contains(&now)
        || !(now + 1..=MAX_EVENT_TIMESTAMP).contains(&session_expiry)
        || current.is_some_and(|value| {
            value.subject != subject
                || value.revision.is_nil()
                || value.checked_at < 0
                || value.checked_at > now
                || value.expires_at <= value.checked_at
                || value.expires_at > MAX_EVENT_TIMESTAMP
        })
    {
        return Err(ConsentError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/consent_model.rs"]
mod tests;
