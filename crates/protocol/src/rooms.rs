//! Version 2 room commands and bounded public projections.
use crate::{online::RecordingStatus, rounds::RoundResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub const ROOM_PROTOCOL_VERSION: u16 = 2;
pub const MAX_ROOM_INPUT_BYTES: usize = 2048;
pub const MAX_ROOM_OUTPUT_BYTES: usize = 32 * 1024;
pub const ROOM_PAGE_SIZE: usize = 20;
pub const MAX_ROOM_TITLE_BYTES: usize = 256;
pub const ROOM_CODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoomVisibility {
    Public,
    Invite,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoomPhase {
    Waiting,
    Preparing,
    Countdown,
    Playing,
    Results,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RoomListCursor {
    pub accepting: bool,
    pub room_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RoomOptions {
    pub title: String,
    pub visibility: RoomVisibility,
    pub capacity: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoomEntrance {
    Public { room_id: String },
    Invite { code: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoomIntent {
    List {
        cursor: Option<RoomListCursor>,
    },
    Status,
    Create {
        request_id: String,
        options: RoomOptions,
    },
    Join {
        request_id: String,
        entrance: RoomEntrance,
    },
    Leave {
        request_id: String,
        room_id: String,
        revision: String,
    },
    Ready {
        request_id: String,
        room_id: String,
        revision: String,
        ready: bool,
    },
    Configure {
        request_id: String,
        room_id: String,
        revision: String,
        options: RoomOptions,
    },
    Start {
        request_id: String,
        room_id: String,
        revision: String,
    },
    Kick {
        request_id: String,
        room_id: String,
        revision: String,
        seat: u8,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RoomRequest {
    pub v: u16,
    pub intent: RoomIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoomWireError {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Forbidden,
    NotFound,
    Busy,
    Full,
    Capacity,
    NotReady,
    Stale,
    Conflict,
    RateLimited,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomCard {
    pub room_id: String,
    pub title: String,
    pub capacity: u8,
    pub members: u8,
    pub phase: RoomPhase,
    pub accepting: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomList {
    pub rooms: Vec<RoomCard>,
    pub next: Option<RoomListCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomMember {
    pub seat: u8,
    pub nickname: String,
    pub ready: bool,
    pub in_round: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomRound {
    pub round_id: String,
    pub seat: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomResultSummary {
    pub result: RoundResult,
    pub recording: RecordingStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomView {
    pub room_id: String,
    pub revision: String,
    pub code: String,
    pub options: RoomOptions,
    pub phase: RoomPhase,
    pub own_seat: u8,
    pub host_seat: u8,
    pub members: Vec<RoomMember>,
    pub round: Option<RoomRound>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub start_error: Option<RoomWireError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result_summary: Option<RoomResultSummary>,
    #[ts(type = "number | null")]
    pub expires_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomPayload {
    List { page: RoomList },
    Current { room: Option<RoomView> },
    Error { error: RoomWireError },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoomResponse {
    pub v: u16,
    #[ts(type = "number")]
    pub server_time_ms: u64,
    pub payload: RoomPayload,
}

pub fn decode_room_request(body: &[u8]) -> Result<RoomRequest, RoomWireError> {
    if body.len() > MAX_ROOM_INPUT_BYTES {
        return Err(RoomWireError::Malformed);
    }
    let input: RoomRequest = serde_json::from_slice(body).map_err(|_| RoomWireError::Malformed)?;
    if input.v != ROOM_PROTOCOL_VERSION {
        return Err(RoomWireError::UnsupportedVersion);
    }
    let valid = match &input.intent {
        RoomIntent::List { cursor } => cursor
            .as_ref()
            .is_none_or(|cursor| valid_id(&cursor.room_id)),
        RoomIntent::Status => true,
        RoomIntent::Create {
            request_id,
            options,
        } => valid_id(request_id) && valid_options(options),
        RoomIntent::Join {
            request_id,
            entrance,
        } => {
            valid_id(request_id)
                && match entrance {
                    RoomEntrance::Public { room_id } => valid_id(room_id),
                    RoomEntrance::Invite { code } => valid_room_code(code),
                }
        }
        RoomIntent::Leave {
            request_id,
            room_id,
            revision,
        }
        | RoomIntent::Ready {
            request_id,
            room_id,
            revision,
            ..
        }
        | RoomIntent::Start {
            request_id,
            room_id,
            revision,
        } => valid_mutation(request_id, room_id, revision),
        RoomIntent::Configure {
            request_id,
            room_id,
            revision,
            options,
        } => valid_mutation(request_id, room_id, revision) && valid_options(options),
        RoomIntent::Kick {
            request_id,
            room_id,
            revision,
            seat,
        } => {
            valid_mutation(request_id, room_id, revision)
                && usize::from(*seat) < liar_core::game::multiplayer::MAX_RACE_PLAYERS
        }
    };
    valid.then_some(input).ok_or(RoomWireError::Malformed)
}

fn valid_mutation(request_id: &str, room_id: &str, revision: &str) -> bool {
    valid_id(request_id)
        && valid_id(room_id)
        && revision
            .parse::<u64>()
            .is_ok_and(|value| value > 0 && value.to_string() == revision)
}

pub fn valid_room_code(value: &str) -> bool {
    value.len() == 8 && value.bytes().all(|byte| ROOM_CODE_ALPHABET.contains(&byte))
}

fn valid_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

fn valid_options(options: &RoomOptions) -> bool {
    use liar_core::game::multiplayer::{MAX_RACE_PLAYERS, MIN_RACE_PLAYERS};
    (MIN_RACE_PLAYERS..=MAX_RACE_PLAYERS).contains(&usize::from(options.capacity))
        && (2..=MAX_ROOM_TITLE_BYTES).contains(&options.title.len())
}

pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        RoomVisibility::decl(config),
        RoomPhase::decl(config),
        RoomListCursor::decl(config),
        RoomOptions::decl(config),
        RoomEntrance::decl(config),
        RoomIntent::decl(config),
        RoomRequest::decl(config),
        RoomWireError::decl(config),
        RoomCard::decl(config),
        RoomList::decl(config),
        RoomMember::decl(config),
        RoomRound::decl(config),
        RoomResultSummary::decl(config),
        RoomView::decl(config),
        RoomPayload::decl(config),
        RoomResponse::decl(config),
    ]
}
