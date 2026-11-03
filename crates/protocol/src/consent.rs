//! Current-session consent control; client choices never mint permission.
use crate::me::MeConsent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_CONSENT_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ConsentApiError {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Unavailable,
    Conflict,
    RateLimited,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConsentReadInput {
    pub v: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConsentRefreshInput {
    pub v: u16,
    pub expected_revision: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ConsentWithdrawal {
    Analytics,
    Advertising,
    All,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConsentWithdrawInput {
    pub v: u16,
    pub purpose: ConsentWithdrawal,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct ConsentSnapshot {
    pub v: u16,
    pub account_id: String,
    #[ts(type = "number")]
    pub observed_at: i64,
    pub revision: Option<String>,
    pub consent: MeConsent,
}
fn decode<T: serde::de::DeserializeOwned>(source: &[u8]) -> Result<T, ConsentApiError> {
    if source.len() > MAX_CONSENT_BYTES {
        return Err(ConsentApiError::Malformed);
    }
    serde_json::from_slice(source).map_err(|_| ConsentApiError::Malformed)
}
fn version(v: u16) -> Result<(), ConsentApiError> {
    if v != crate::game::PROTOCOL_VERSION {
        return Err(ConsentApiError::UnsupportedVersion);
    }
    Ok(())
}
pub fn decode_consent_read(source: &[u8]) -> Result<ConsentReadInput, ConsentApiError> {
    let value: ConsentReadInput = decode(source)?;
    version(value.v)?;
    Ok(value)
}
pub fn decode_consent_refresh(source: &[u8]) -> Result<ConsentRefreshInput, ConsentApiError> {
    let value: ConsentRefreshInput = decode(source)?;
    version(value.v)?;
    if let Some(raw) = &value.expected_revision {
        let id = uuid::Uuid::parse_str(raw).map_err(|_| ConsentApiError::Malformed)?;
        if id.is_nil() || id.to_string() != *raw {
            return Err(ConsentApiError::Malformed);
        }
    }
    Ok(value)
}
pub fn decode_consent_withdraw(source: &[u8]) -> Result<ConsentWithdrawInput, ConsentApiError> {
    let value: ConsentWithdrawInput = decode(source)?;
    version(value.v)?;
    Ok(value)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        ConsentApiError::decl(config),
        ConsentReadInput::decl(config),
        ConsentRefreshInput::decl(config),
        ConsentWithdrawal::decl(config),
        ConsentWithdrawInput::decl(config),
        ConsentSnapshot::decl(config),
    ]
}
