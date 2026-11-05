//! Local planning contract; this is not a public ad authorization endpoint.
use crate::game::{PROTOCOL_VERSION, PublicError};
pub use liar_core::ads::{
    AdBlockReason, AdCompletion, AdCounters, AdFrequency, AdOpportunity, AdPlacement, AdPlan,
    AdResult, AdScreen,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_AD_INPUT_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AdPolicyInput {
    pub v: u16,
    pub opportunity: AdOpportunity,
}

pub fn plan_ad_json(source: &str) -> Result<String, PublicError> {
    if source.len() > MAX_AD_INPUT_BYTES {
        return Err(PublicError::Malformed);
    }
    let input: AdPolicyInput = serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    // Canonical comparison also rejects enum extras and omitted nullable fields.
    let raw: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    if serde_json::to_value(input).map_err(|_| PublicError::Malformed)? != raw {
        return Err(PublicError::Malformed);
    }
    if input.v != PROTOCOL_VERSION {
        return Err(PublicError::UnsupportedVersion);
    }
    serde_json::to_string(&liar_core::ads::plan_ad_request(input.opportunity))
        .map_err(|_| PublicError::Unavailable)
}

pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        AdPlacement::decl(config),
        AdScreen::decl(config),
        AdFrequency::decl(config),
        AdCounters::decl(config),
        AdCompletion::decl(config),
        AdResult::decl(config),
        AdOpportunity::decl(config),
        AdBlockReason::decl(config),
        AdPlan::decl(config),
        AdPolicyInput::decl(config),
    ]
}
