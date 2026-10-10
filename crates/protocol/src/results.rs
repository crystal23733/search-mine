//! Self-only persisted result projection. No live board or credentials.
use crate::game::GameResult;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_RESULT_BYTES: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ResultError {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Unavailable,
    RateLimited,
    NotFound,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PersonalResultInput {
    pub v: u16,
    pub match_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct ResultStats {
    pub opened_safe: u16,
    pub mistakes: u16,
    pub accusation_attempts: u16,
    pub correct_accusations: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct PersonalResult {
    pub match_id: String,
    pub rules_hash: String,
    #[ts(type = "number | null")]
    pub end_elapsed_ms: Option<u64>,
    pub result: GameResult,
    pub own: Option<ResultStats>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct PersonalResultResponse {
    pub v: u16,
    pub result: PersonalResult,
}

pub fn decode_result(source: &[u8]) -> Result<PersonalResultInput, ResultError> {
    if source.len() > MAX_RESULT_BYTES {
        return Err(ResultError::Malformed);
    }
    let input: PersonalResultInput =
        serde_json::from_slice(source).map_err(|_| ResultError::Malformed)?;
    if input.v != crate::game::PROTOCOL_VERSION {
        return Err(ResultError::UnsupportedVersion);
    }
    if !uuid::Uuid::parse_str(&input.match_id)
        .is_ok_and(|id| !id.is_nil() && id.to_string() == input.match_id)
    {
        return Err(ResultError::Malformed);
    }
    Ok(input)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        ResultError::decl(config),
        PersonalResultInput::decl(config),
        ResultStats::decl(config),
        PersonalResult::decl(config),
        PersonalResultResponse::decl(config),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_versioned_closed_canonical_non_nil_match_requests_pass() {
        let id = "12345678-1234-4234-9234-123456789abc";
        let valid = format!(r#"{{"v":1,"match_id":"{id}"}}"#);
        assert_eq!(decode_result(valid.as_bytes()).unwrap().match_id, id);
        assert_eq!(
            decode_result(valid.replace("\"v\":1", "\"v\":2").as_bytes()),
            Err(ResultError::UnsupportedVersion)
        );
        for source in [
            "{}".to_string(),
            valid.replace(id, &id.to_uppercase()),
            valid.replace(id, "00000000-0000-0000-0000-000000000000"),
            valid.replace(id, &id.replace('-', "")),
            valid.replace("\"v\":1", "\"v\":1,\"v\":1"),
            valid.replace("\"v\":1", "\"v\":1,\"account_id\":\"x\""),
            format!("{valid}{}", " ".repeat(256)),
            format!("{valid}{{}}"),
            valid.replace("\"v\":1", "\"v\":1.0"),
        ] {
            assert_eq!(
                decode_result(source.as_bytes()),
                Err(ResultError::Malformed),
                "{source}"
            );
        }
    }
}
