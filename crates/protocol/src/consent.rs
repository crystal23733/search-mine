//! Current-session consent control; client flags never bypass validated purpose authority.
use crate::me::MeConsent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_CONSENT_BYTES: usize = 4096;
pub const MAX_TCF_CHARS: usize = 3072;
pub const GOOGLE_CMP_ID: u16 = 300;
pub const GOOGLE_VENDOR_ID: u16 = 755;
pub const GOOGLE_AD_PURPOSES: [u16; 3] = [1, 3, 4];
pub const GOOGLE_ANALYTICS_PURPOSES: [u16; 3] = [1, 8, 10];
pub const GOOGLE_MEASUREMENT_PURPOSES: [u16; 2] = [1, 7];
pub const TCF_POLICY_VERSIONS: [u8; 2] = [4, 5];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ConsentEvidenceProvider {
    GoogleTcfV2,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ConsentEvidence {
    pub provider: ConsentEvidenceProvider,
    pub tc_string: String,
}
impl std::fmt::Debug for ConsentEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsentEvidence")
            .field("provider", &self.provider)
            .field("tc_string", &"[redacted]")
            .finish()
    }
}
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ConsentEvidence>,
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
    if let Some(evidence) = &value.evidence {
        let raw = &evidence.tc_string;
        if raw.is_empty()
            || raw.len() > MAX_TCF_CHARS
            || raw.split('.').count() > 4
            || raw.split('.').any(str::is_empty)
            || !raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
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
        format!("const GOOGLE_CMP_ID = {GOOGLE_CMP_ID} as const;"),
        format!("const GOOGLE_VENDOR_ID = {GOOGLE_VENDOR_ID} as const;"),
        format!("const GOOGLE_AD_PURPOSES = {GOOGLE_AD_PURPOSES:?} as const;"),
        format!("const GOOGLE_ANALYTICS_PURPOSES = {GOOGLE_ANALYTICS_PURPOSES:?} as const;"),
        format!("const GOOGLE_MEASUREMENT_PURPOSES = {GOOGLE_MEASUREMENT_PURPOSES:?} as const;"),
        format!("const TCF_POLICY_VERSIONS = {TCF_POLICY_VERSIONS:?} as const;"),
        format!("const MAX_TCF_CHARS = {MAX_TCF_CHARS} as const;"),
        ConsentEvidenceProvider::decl(config),
        ConsentEvidence::decl(config),
        ConsentApiError::decl(config),
        ConsentReadInput::decl(config),
        ConsentRefreshInput::decl(config),
        ConsentWithdrawal::decl(config),
        ConsentWithdrawInput::decl(config),
        ConsentSnapshot::decl(config),
    ]
}
