use super::{ConsentError, CurrentConsent};
use liar_protocol::me::{MeChoice, MeConsent, MeConsentStatus};
pub(crate) fn project_current(
    value: Option<CurrentConsent>,
    now: i64,
) -> Result<MeConsent, ConsentError> {
    let Some(value) = value else {
        return Ok(MeConsent {
            cmp_available: false,
            status: MeConsentStatus::Unknown,
            analytics: MeChoice::Unknown,
            advertising: MeChoice::Unknown,
            checked_at: None,
            expires_at: None,
        });
    };
    if value.subject.is_nil()
        || value.revision.is_nil()
        || value.checked_at < 0
        || value.checked_at > now
        || value.expires_at <= value.checked_at
        || value.expires_at > crate::events::MAX_EVENT_TIMESTAMP
    {
        return Err(ConsentError::Unavailable);
    }
    let live = now < value.expires_at;
    let choice = |granted| {
        if !live {
            MeChoice::Unknown
        } else if granted {
            MeChoice::Granted
        } else {
            MeChoice::Denied
        }
    };
    Ok(MeConsent {
        cmp_available: false,
        status: if live {
            MeConsentStatus::Current
        } else {
            MeConsentStatus::Expired
        },
        analytics: choice(value.analytics),
        advertising: choice(value.advertising),
        checked_at: Some(value.checked_at),
        expires_at: Some(value.expires_at),
    })
}
