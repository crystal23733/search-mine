//! Version 2 commands and per-seat observations for room rounds.
use crate::{
    game::{
        AckStatus, GamePhase, MAX_INPUT_BYTES, OpponentProgress, OwnBoard, PublicAction,
        PublicEndReason, PublicError, own_board,
    },
    online::{OnlineError, RecordingStatus},
    rooms::ROOM_PROTOCOL_VERSION,
};
use liar_core::{game::multiplayer::RaceProjection, rules::RulesSnapshot};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RoundInput {
    pub v: u16,
    pub round_id: String,
    pub command_id: String,
    pub client_seq: u32,
    pub session_epoch: u32,
    #[ts(type = "number")]
    pub known_revision: u64,
    pub action: PublicAction,
}

pub fn decode_round(source: &str) -> Result<RoundInput, PublicError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(PublicError::Malformed);
    }
    let raw: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let action = &raw["action"];
    if matches!(action["type"].as_str(), Some("open" | "flag" | "accuse"))
        && let Some(cell) = action.get("cell")
        && !cell.as_u64().is_some_and(|value| value <= 255)
    {
        return Err(PublicError::InvalidCell);
    }
    let input: RoundInput = serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let id = |value: &str| {
        Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
    };
    if serde_json::to_value(&input).map_err(|_| PublicError::Malformed)? != raw
        || !id(&input.round_id)
        || !id(&input.command_id)
        || input.client_seq == 0
        || input.session_epoch == 0
        || input.known_revision > 9007199254740991
    {
        return Err(PublicError::Malformed);
    }
    if input.v != ROOM_PROTOCOL_VERSION {
        return Err(PublicError::UnsupportedVersion);
    }
    Ok(input)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoundOpponent {
    pub seat: u8,
    pub nickname: Option<String>,
    pub progress: OpponentProgress,
    pub forfeited: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoundStanding {
    pub seat: u8,
    pub nickname: Option<String>,
    pub rank: Option<u8>,
    pub opened_safe: u16,
    pub forfeited: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoundResult {
    pub reason: PublicEndReason,
    pub winner: Option<u8>,
    pub standings: Vec<RoundStanding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoundView {
    pub rules: RulesSnapshot,
    pub own: OwnBoard,
    pub forfeited: bool,
    pub opponents: Vec<RoundOpponent>,
    #[ts(type = "number")]
    pub revision: u64,
    pub phase: GamePhase,
    pub countdown_ms: u32,
    pub remaining_ms: u32,
    pub result: Option<RoundResult>,
}

pub fn from_race_projection(projection: &RaceProjection, names: &[Option<String>]) -> RoundView {
    let nickname = |seat| names.get(seat).cloned().flatten();
    let result = projection.result.as_ref().map(|result| RoundResult {
        reason: result.reason.into(),
        winner: result.winner.map(|seat| seat as u8),
        standings: result
            .standings
            .iter()
            .map(|standing| RoundStanding {
                seat: standing.seat as u8,
                nickname: nickname(standing.seat),
                rank: standing.rank,
                opened_safe: standing.opened_safe,
                forfeited: standing.forfeited,
            })
            .collect(),
    });
    let phase = match result.as_ref().map(|result| result.reason) {
        Some(PublicEndReason::ServerFailure | PublicEndReason::Abandoned) => GamePhase::Aborted,
        Some(PublicEndReason::Cancelled) => GamePhase::Cancelled,
        Some(_) => GamePhase::Finished,
        None if projection.countdown_ms != 0 => GamePhase::Countdown,
        None => GamePhase::Playing,
    };
    RoundView {
        rules: projection.rules.clone(),
        own: own_board(&projection.own),
        forfeited: projection.forfeited,
        opponents: projection
            .opponents
            .iter()
            .map(|opponent| RoundOpponent {
                seat: opponent.seat as u8,
                nickname: nickname(opponent.seat),
                progress: OpponentProgress {
                    opened_safe: opponent.progress.opened_safe,
                    stun_ms: opponent.progress.stun_ms,
                    reconnect_ms: opponent.progress.reconnect_ms,
                },
                forfeited: opponent.forfeited,
            })
            .collect(),
        revision: projection.revision,
        phase,
        countdown_ms: projection.countdown_ms,
        remaining_ms: projection.remaining_ms,
        result,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoundPayload {
    Awaiting {
        own_seat: u8,
        connected: u8,
        participant_count: u8,
    },
    Snapshot {
        own_seat: u8,
        session_epoch: u32,
        last_client_seq: u32,
        view: RoundView,
    },
    Delta {
        view: RoundView,
    },
    Ack {
        command_id: String,
        #[ts(type = "number")]
        revision: u64,
        status: AckStatus,
        error: Option<PublicError>,
        duplicate: bool,
    },
    End {
        view: RoundView,
        recording: RecordingStatus,
    },
    Error {
        code: OnlineError,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
pub struct RoundEvent {
    pub v: u16,
    pub room_id: String,
    pub round_id: String,
    pub server_seq: u32,
    #[ts(type = "number")]
    pub server_time_ms: u64,
    pub payload: RoundPayload,
}

pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        RoundInput::decl(config),
        RoundOpponent::decl(config),
        RoundStanding::decl(config),
        RoundResult::decl(config),
        RoundView::decl(config),
        RoundPayload::decl(config),
        RoundEvent::decl(config),
    ]
}
