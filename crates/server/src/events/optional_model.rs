use super::*;
use crate::consent::{ConsentSession, CurrentConsent};
use std::future::Future;
use uuid::Uuid;

#[derive(Clone)]
pub struct OptionalCapture {
    pub session: ConsentSession,
    pub expected_revision: Uuid,
    pub draft: ClientDraft,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionalEventError {
    Invalid,
    Unauthorized,
    Consent,
    Expired,
    Conflict,
    Unavailable,
}
pub trait OptionalEventStore: Send + Sync {
    fn capture(
        &self,
        input: OptionalCapture,
    ) -> impl Future<Output = Result<StoredEvent, OptionalEventError>> + Send {
        self.capture_until(
            input,
            std::time::Instant::now() + crate::consent::CONSENT_REQUEST_TIMEOUT,
        )
    }
    fn capture_until(
        &self,
        input: OptionalCapture,
        deadline: std::time::Instant,
    ) -> impl Future<Output = Result<StoredEvent, OptionalEventError>> + Send;
}
pub(super) fn admit_current(
    draft: ClientDraft,
    owner: Uuid,
    expected: Uuid,
    current: Option<CurrentConsent>,
    now: i64,
) -> Result<ProductEvent, EventError> {
    if owner.is_nil() || expected.is_nil() {
        return Err(EventError::Invalid);
    }
    let current = current.ok_or(EventError::Consent)?;
    if current.subject != owner || current.revision != expected {
        return Err(EventError::Consent);
    }
    let state = |purpose| {
        if current.allows(purpose, now) {
            ConsentState::Granted {
                revision: current.revision,
                valid_from: current.checked_at,
                expires_at: current.expires_at,
            }
        } else {
            ConsentState::Denied
        }
    };
    // The authenticated server grant is projected onto an ephemeral tab correlation key.
    let snapshot = ConsentSnapshot {
        subject: Some(draft.session_key),
        analytics: state(crate::consent::OptionalPurpose::Analytics),
        advertising: state(crate::consent::OptionalPurpose::Advertising),
    };
    admit_client(draft, snapshot, now)
}

#[cfg(test)]
#[path = "../../tests/unit/optional_event_model.rs"]
mod tests;
