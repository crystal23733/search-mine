use super::RecoveryError;
use serde::Deserialize;
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Erasure {
    pub kind: Kind,
    pub id: Uuid,
    pub deleted_at: i64,
    pub expires_at: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
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
        if !(0..=253402300799).contains(&now)
            || self.version != 1
            || self.recovery_id.is_nil()
            || self.captured_at < 0
            || self.captured_at > now
            || now - self.captured_at > 300
            || self.entries.len() > 500
        {
            return Err(RecoveryError::Invalid);
        }
        let mut keys = std::collections::HashSet::new();
        for entry in &self.entries {
            if entry.id.is_nil()
                || entry.deleted_at < 0
                || entry.deleted_at > self.captured_at
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
}
#[cfg(test)]
#[path = "../../tests/unit/recovery_model.rs"]
mod tests;
