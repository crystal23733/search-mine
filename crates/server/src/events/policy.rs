use super::{ClientDraft, EventError, EventId, ProductEvent, Purpose};
use chrono::{DateTime, Datelike, Months, NaiveDate};
use serde::Serialize;
use uuid::Uuid;

pub const RAW_EVENT_SECONDS: i64 = 30 * 86400;
pub const MAX_EVENT_TIMESTAMP: i64 = 253402300799;
pub(super) fn valid_timestamp(value: i64) -> Result<DateTime<chrono::Utc>, EventError> {
    if !(0..=MAX_EVENT_TIMESTAMP).contains(&value) {
        return Err(EventError::Invalid);
    }
    DateTime::from_timestamp(value, 0).ok_or(EventError::Invalid)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtcDay(pub(super) NaiveDate);
impl UtcDay {
    pub fn parse(value: &str) -> Result<Self, EventError> {
        let day = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| EventError::Invalid)?;
        if !(1970..=9999).contains(&day.year()) || day.format("%Y-%m-%d").to_string() != value {
            return Err(EventError::Invalid);
        }
        Ok(Self(day))
    }
    pub fn from_timestamp(value: i64) -> Result<Self, EventError> {
        Ok(Self(valid_timestamp(value)?.date_naive()))
    }
    pub fn start(self) -> i64 {
        self.0
            .and_hms_opt(0, 0, 0)
            .expect("UTC midnight")
            .and_utc()
            .timestamp()
    }
}
impl Serialize for UtcDay {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.format("%Y-%m-%d").to_string())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentState {
    Unknown,
    Denied,
    Failed,
    Granted {
        revision: Uuid,
        valid_from: i64,
        expires_at: i64,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsentSnapshot {
    pub subject: Option<EventId>,
    pub analytics: ConsentState,
    pub advertising: ConsentState,
}
impl Default for ConsentSnapshot {
    fn default() -> Self {
        Self {
            subject: None,
            analytics: ConsentState::Unknown,
            advertising: ConsentState::Unknown,
        }
    }
}
pub fn admit_client(
    draft: ClientDraft,
    snapshot: ConsentSnapshot,
    now: i64,
) -> Result<ProductEvent, EventError> {
    raw_event_alive(draft.occurred_at, now)?;
    if snapshot.subject != Some(draft.session_key) {
        return Err(EventError::Consent);
    }
    let state = match draft.event.purpose() {
        Purpose::Analytics => snapshot.analytics,
        Purpose::Advertising => snapshot.advertising,
        Purpose::Essential => return Err(EventError::Consent),
    };
    if let ConsentState::Granted {
        revision,
        valid_from,
        expires_at,
    } = state
    {
        valid_timestamp(valid_from)?;
        valid_timestamp(expires_at)?;
        if !revision.is_nil() && valid_from <= draft.occurred_at && now < expires_at {
            return Ok(ProductEvent::optional(draft));
        }
    }
    Err(EventError::Consent)
}
pub fn raw_event_alive(occurred_at: i64, now: i64) -> Result<(), EventError> {
    valid_timestamp(occurred_at)?;
    valid_timestamp(now)?;
    if now < occurred_at {
        return Err(EventError::Invalid);
    }
    if now - occurred_at >= RAW_EVENT_SECONDS {
        return Err(EventError::Expired);
    }
    Ok(())
}
pub fn aggregate_expires_at(day: UtcDay) -> Result<i64, EventError> {
    let expiry = day
        .0
        .checked_add_months(Months::new(13))
        .ok_or(EventError::Invalid)?;
    if expiry.year() > 9999 {
        return Err(EventError::Invalid);
    }
    Ok(UtcDay(expiry).start())
}
pub fn aggregate_alive(day: UtcDay, now: i64) -> Result<(), EventError> {
    valid_timestamp(now)?;
    if now < day.start() {
        return Err(EventError::Invalid);
    }
    if now >= aggregate_expires_at(day)? {
        return Err(EventError::Expired);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/event_policy.rs"]
mod tests;
