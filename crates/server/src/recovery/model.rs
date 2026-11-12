use super::RecoveryError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const MAX_BYTES: usize = 128 * 1024;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub(super) version: u8,
    pub(super) recovery_id: Uuid,
    pub(super) captured_at: i64,
    pub(super) entries: Vec<Erasure>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Erasure {
    pub kind: Kind,
    pub id: Uuid,
    pub deleted_at: i64,
    pub expires_at: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Account,
    Identity,
}
impl Ledger {
    pub fn parse(bytes: &[u8], now: i64) -> Result<Self, RecoveryError> {
        if bytes.len() > MAX_BYTES {
            return Err(RecoveryError::Invalid);
        }
        let ledger: Self = serde_json::from_slice(bytes).map_err(|_| RecoveryError::Invalid)?;
        ledger.validate(now)?;
        Ok(ledger)
    }
    pub(super) fn validate(&self, now: i64) -> Result<(), RecoveryError> {
        validate_header(self.version, self.recovery_id, self.captured_at, now, 300)?;
        validate_entries(&self.entries, self.captured_at, now)
    }
}
/// A committed source snapshot, distinct from a current recovery incident.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub(super) version: u8,
    pub(super) source_id: Uuid,
    pub(super) captured_at: i64,
    pub(super) entries: Vec<Erasure>,
}
#[derive(Serialize)]
pub struct SnapshotInspection {
    source_id: Uuid,
    captured_at: i64,
    entries: u16,
}
#[derive(Serialize)]
pub struct SnapshotMatch {
    captured_at: i64,
    observed_at: i64,
    entries: u16,
}
impl Snapshot {
    pub fn compare(&self, observed: &Self, now: i64) -> Result<SnapshotMatch, RecoveryError> {
        self.validate(now)?;
        observed.validate(now)?;
        if self.captured_at > observed.captured_at {
            return Err(RecoveryError::Invalid);
        }
        if self.source_id != observed.source_id {
            return Err(RecoveryError::Blocked);
        }
        let current = |snapshot: &Self| {
            snapshot
                .entries
                .iter()
                .filter(|e| e.expires_at > observed.captured_at)
                .map(|e| (e.kind, e.id, e.deleted_at, e.expires_at))
                .collect::<std::collections::HashSet<_>>()
        };
        let actual = current(observed);
        if current(self) != actual {
            return Err(RecoveryError::Blocked);
        }
        Ok(SnapshotMatch {
            captured_at: self.captured_at,
            observed_at: observed.captured_at,
            entries: actual.len() as u16,
        })
    }

    pub fn inspect(&self, now: i64) -> Result<SnapshotInspection, RecoveryError> {
        self.validate(now)?;
        Ok(SnapshotInspection {
            source_id: self.source_id,
            captured_at: self.captured_at,
            entries: self.entries.len() as u16,
        })
    }

    pub fn parse(bytes: &[u8], now: i64) -> Result<Self, RecoveryError> {
        if bytes.len() > MAX_BYTES {
            return Err(RecoveryError::Invalid);
        }
        let snapshot: Self = serde_json::from_slice(bytes).map_err(|_| RecoveryError::Invalid)?;
        snapshot.validate(now)?;
        Ok(snapshot)
    }
    pub fn to_bytes(&self, now: i64) -> Result<Vec<u8>, RecoveryError> {
        self.validate(now)?;
        let bytes = serde_json::to_vec(self).map_err(|_| RecoveryError::Invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(RecoveryError::Invalid);
        }
        Ok(bytes)
    }
    pub(super) fn validate(&self, now: i64) -> Result<(), RecoveryError> {
        validate_header(self.version, self.source_id, self.captured_at, now, 2419200)?;
        validate_entries(&self.entries, self.captured_at, self.captured_at)
    }
}
fn validate_header(
    version: u8,
    id: Uuid,
    captured_at: i64,
    now: i64,
    max_age: i64,
) -> Result<(), RecoveryError> {
    if !(0..=253402300799).contains(&now)
        || version != 1
        || id.is_nil()
        || captured_at < 0
        || captured_at > now
        || now - captured_at > max_age
    {
        return Err(RecoveryError::Invalid);
    }
    Ok(())
}
fn validate_entries(entries: &[Erasure], captured_at: i64, now: i64) -> Result<(), RecoveryError> {
    if entries.len() > 500 {
        return Err(RecoveryError::Invalid);
    }
    let mut keys = std::collections::HashSet::new();
    for entry in entries {
        if entry.id.is_nil()
            || entry.deleted_at < 0
            || entry.deleted_at > captured_at
            || entry.deleted_at.checked_add(2419200) != Some(entry.expires_at)
            || entry.expires_at > 253402300799
            || entry.expires_at <= now
            || !keys.insert((entry.kind, entry.id))
        {
            return Err(RecoveryError::Invalid);
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "../../tests/unit/recovery_model.rs"]
mod tests;
