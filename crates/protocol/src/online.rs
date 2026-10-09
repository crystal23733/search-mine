//! Online input has no client clock, seat, board or provider credential.
use crate::game::{
    AckStatus, GameView, MAX_INPUT_BYTES, PROTOCOL_VERSION, PublicAction, PublicError,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct OnlineInput {
    pub v: u16,
    pub match_id: String,
    pub command_id: String,
    pub client_seq: u32,
    pub session_epoch: u32,
    pub known_revision: u32,
    pub action: PublicAction,
}

pub fn decode_online(source: &str) -> Result<OnlineInput, PublicError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(PublicError::Malformed);
    }
    let raw: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let action = &raw["action"];
    if matches!(action["type"].as_str(), Some("open" | "flag" | "accuse"))
        && let Some(cell) = action.get("cell")
        && !cell.as_u64().is_some_and(|n| n <= 255)
    {
        return Err(PublicError::InvalidCell);
    }
    let input: OnlineInput = serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let uuid = |value: &str| {
        uuid::Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
    };
    if !uuid(&input.match_id)
        || !uuid(&input.command_id)
        || input.client_seq == 0
        || input.session_epoch == 0
        || serde_json::to_value(&input).map_err(|_| PublicError::Malformed)? != raw
    {
        return Err(PublicError::Malformed);
    }
    if input.v != PROTOCOL_VERSION {
        return Err(PublicError::UnsupportedVersion);
    }
    Ok(input)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OnlineError {
    Malformed,
    Unauthorized,
    Unavailable,
    RateLimited,
    Capacity,
    NotMatched,
    WrongMatch,
    InvalidEpoch,
    UnsupportedVersion,
    InvalidCell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Pending,
    Saved,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OnlinePayload {
    Snapshot {
        session_epoch: u32,
        last_client_seq: u32,
        view: GameView,
    },
    Delta {
        view: GameView,
    },
    Ack {
        command_id: String,
        revision: u32,
        status: AckStatus,
        error: Option<PublicError>,
        duplicate: bool,
    },
    Error {
        code: OnlineError,
    },
    MatchEnd {
        view: GameView,
        recording: RecordingStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct OnlineEvent {
    pub v: u16,
    pub match_id: String,
    pub server_seq: u32,
    #[ts(type = "number")]
    pub server_time_ms: u64,
    pub payload: OnlinePayload,
}

pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        OnlineInput::decl(config),
        OnlineError::decl(config),
        RecordingStatus::decl(config),
        OnlinePayload::decl(config),
        OnlineEvent::decl(config),
    ]
}
