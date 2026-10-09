//! Versioned lobby intents and self-only state. Credentials and board truth are private.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_LOBBY_BYTES: usize = 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LobbyDifficulty {
    Easy,
    Normal,
    Hard,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LobbyOpponent {
    Human,
    Bot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum LobbyErrorCode {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Unavailable,
    Capacity,
    RateLimited,
    Busy,
    NotFound,
    Full,
    Stale,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LobbyCancellation {
    Queue { queue_id: String },
    Room { room_id: String },
    Preparing { entity: String, generation: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyCommand {
    Status,
    QueueJoin { difficulty: LobbyDifficulty },
    RoomCreate,
    RoomJoin { code: String },
    Ready { room_id: String, ready: bool },
    Cancel { identity: LobbyCancellation },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct LobbyInput {
    pub v: u16,
    pub command: LobbyCommand,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyState {
    Idle,
    Queued {
        queue_id: String,
        #[ts(type = "number")]
        deadline_ms: u64,
        difficulty: LobbyDifficulty,
    },
    Room {
        room_id: String,
        code: String,
        own_seat: u8,
        occupied: [bool; 2],
        ready: [bool; 2],
        #[ts(type = "number")]
        expires_at_ms: u64,
    },
    Preparing {
        identity: LobbyCancellation,
        opponent: LobbyOpponent,
    },
    Matched {
        match_id: String,
        own_seat: u8,
        opponent: LobbyOpponent,
    },
    Failed {
        error: LobbyErrorCode,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct LobbyResponse {
    pub v: u16,
    #[ts(type = "number")]
    pub server_time_ms: u64,
    pub state: LobbyState,
}
pub fn decode_lobby(source: &[u8]) -> Result<LobbyInput, LobbyErrorCode> {
    if source.len() > MAX_LOBBY_BYTES {
        return Err(LobbyErrorCode::Malformed);
    }
    let input: LobbyInput =
        serde_json::from_slice(source).map_err(|_| LobbyErrorCode::Malformed)?;
    let raw: serde_json::Value =
        serde_json::from_slice(source).map_err(|_| LobbyErrorCode::Malformed)?;
    if serde_json::to_value(&input).map_err(|_| LobbyErrorCode::Malformed)? != raw {
        return Err(LobbyErrorCode::Malformed);
    }
    if input.v != crate::game::PROTOCOL_VERSION {
        return Err(LobbyErrorCode::UnsupportedVersion);
    }
    let uuid = |value: &str| {
        uuid::Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
    };
    let valid = match &input.command {
        LobbyCommand::Ready { room_id, .. } => uuid(room_id),
        LobbyCommand::RoomJoin { code } => code.len() == 8,
        LobbyCommand::Cancel { identity } => match identity {
            LobbyCancellation::Queue { queue_id } => uuid(queue_id),
            LobbyCancellation::Room { room_id } => uuid(room_id),
            LobbyCancellation::Preparing { entity, generation } => {
                uuid(entity)
                    && generation
                        .parse::<u64>()
                        .is_ok_and(|n| n > 0 && n.to_string() == *generation)
            }
        },
        _ => true,
    };
    if !valid {
        return Err(LobbyErrorCode::Malformed);
    }
    Ok(input)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        LobbyDifficulty::decl(config),
        LobbyOpponent::decl(config),
        LobbyErrorCode::decl(config),
        LobbyCancellation::decl(config),
        LobbyCommand::decl(config),
        LobbyInput::decl(config),
        LobbyState::decl(config),
        LobbyResponse::decl(config),
    ]
}
