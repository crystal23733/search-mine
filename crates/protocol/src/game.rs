//! Allowlisted wire projection. This module receives no secret Board or match engine.
use liar_core::{
    board::ObservedCell,
    game::{ActionStatus, EndReason, Projection, Rejection, Seat},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_INPUT_BYTES: usize = 8192;
pub const MAX_OUTPUT_BYTES: usize = 131072;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PublicAction {
    Open { cell: u16 },
    Flag { cell: u16 },
    Accuse { cell: u16 },
    Attack,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct LocalInput {
    pub v: u16,
    pub command_id: u32,
    pub client_seq: u32,
    pub action: PublicAction,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PublicError {
    Malformed,
    UnsupportedVersion,
    InvalidCell,
    NotStarted,
    Stunned,
    Unavailable,
    Finished,
    Flagged,
    AlreadyOpen,
    Stale,
    CommandLimit,
    Conflict,
    InvalidTime,
    Disconnected,
    InvalidEpoch,
    InvalidSequence,
}
pub fn decode_local(source: &str) -> Result<LocalInput, PublicError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(PublicError::Malformed);
    }
    // Typed parsing rejects duplicate fields; comparison also rejects ignored enum extras.
    let input: LocalInput = serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let raw: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    if input.command_id == 0
        || input.client_seq == 0
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
pub enum CellState {
    Closed,
    Safe,
    Mine,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct PublicCell {
    pub cell: u16,
    pub state: CellState,
    pub number: Option<u8>,
    pub flagged: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct PublicNumericUpdate {
    pub cell: u16,
    pub state: CellState,
    pub number: Option<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct OwnBoard {
    pub cells: Vec<PublicCell>,
    pub gauge: u16,
    pub stun_ms: u32,
    pub history: Vec<Vec<PublicNumericUpdate>>,
    pub stats: MatchStats,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MatchStats {
    pub mistakes: u16,
    pub accusation_attempts: u16,
    pub correct_accusations: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct OpponentProgress {
    pub opened_safe: u16,
    pub stun_ms: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
    Countdown,
    Playing,
    Finished,
    Aborted,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PublicEndReason {
    Clear,
    Timeout,
    Forfeit,
    Abandoned,
    ServerFailure,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Win,
    Loss,
    Draw,
    Abort,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct GameResult {
    pub reason: PublicEndReason,
    pub outcome: Outcome,
    pub completed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct GameView {
    pub v: u16,
    pub rules: liar_core::rules::RulesSnapshot,
    #[ts(type = "number")]
    pub revision: u64,
    pub phase: GamePhase,
    pub countdown_ms: u32,
    pub remaining_ms: u32,
    pub own: OwnBoard,
    pub opponent: OpponentProgress,
    pub result: Option<GameResult>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    Applied,
    Rejected,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct LocalAck {
    pub command_id: u32,
    #[ts(type = "number")]
    pub revision: u64,
    pub status: AckStatus,
    pub error: Option<PublicError>,
    pub duplicate: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct LocalStep {
    pub ack: LocalAck,
    pub view: GameView,
}
pub fn cell_value(cell: ObservedCell) -> (CellState, Option<u8>) {
    match cell {
        ObservedCell::Unknown => (CellState::Closed, None),
        ObservedCell::Number(number) => (CellState::Safe, Some(number)),
        ObservedCell::Mine => (CellState::Mine, None),
    }
}
pub fn from_projection(projection: &Projection, viewer: Seat) -> GameView {
    let result = projection.end.map(|end| {
        let reason = match end.reason {
            EndReason::Clear => PublicEndReason::Clear,
            EndReason::Timeout => PublicEndReason::Timeout,
            EndReason::Forfeit => PublicEndReason::Forfeit,
            EndReason::Abandoned => PublicEndReason::Abandoned,
            EndReason::ServerFailure => PublicEndReason::ServerFailure,
            EndReason::Cancelled => PublicEndReason::Cancelled,
        };
        let outcome = match end.reason {
            EndReason::ServerFailure => Outcome::Abort,
            EndReason::Cancelled => Outcome::Cancelled,
            _ => match end.winner {
                Some(winner) if winner == viewer => Outcome::Win,
                Some(_) => Outcome::Loss,
                None => Outcome::Draw,
            },
        };
        GameResult {
            reason,
            outcome,
            completed: !matches!(end.reason, EndReason::ServerFailure | EndReason::Cancelled),
        }
    });
    let phase = match result.as_ref().map(|r| r.outcome) {
        Some(Outcome::Abort) => GamePhase::Aborted,
        Some(Outcome::Cancelled) => GamePhase::Cancelled,
        Some(_) => GamePhase::Finished,
        None if projection.countdown_ms > 0 => GamePhase::Countdown,
        None => GamePhase::Playing,
    };
    GameView {
        v: PROTOCOL_VERSION,
        rules: projection.rules.clone(),
        revision: projection.revision,
        phase,
        countdown_ms: projection.countdown_ms,
        remaining_ms: projection.remaining_ms,
        own: OwnBoard {
            cells: projection
                .own
                .cells
                .cells()
                .iter()
                .enumerate()
                .map(|(i, &cell)| {
                    let (state, number) = cell_value(cell);
                    PublicCell {
                        cell: i as u16,
                        state,
                        number,
                        flagged: projection.own.flags[i],
                    }
                })
                .collect(),
            gauge: projection.own.gauge,
            stun_ms: projection.own.stun_ms,
            history: projection
                .own
                .history
                .iter()
                .map(|batch| {
                    batch
                        .iter()
                        .map(|update| {
                            let (state, number) = cell_value(update.observed);
                            PublicNumericUpdate {
                                cell: update.cell.0,
                                state,
                                number,
                            }
                        })
                        .collect()
                })
                .collect(),
            stats: MatchStats {
                mistakes: projection.own.stats.mistakes,
                accusation_attempts: projection.own.stats.accusation_attempts,
                correct_accusations: projection.own.stats.correct_accusations,
            },
        },
        opponent: OpponentProgress {
            opened_safe: projection.opponent.opened_safe,
            stun_ms: projection.opponent.stun_ms,
        },
        result,
    }
}
impl From<Rejection> for PublicError {
    fn from(error: Rejection) -> Self {
        match error {
            Rejection::InvalidBoard | Rejection::InvalidRules | Rejection::AttackUnavailable => {
                Self::Unavailable
            }
            Rejection::InvalidTime => Self::InvalidTime,
            Rejection::NotStarted => Self::NotStarted,
            Rejection::Finished => Self::Finished,
            Rejection::Disconnected => Self::Disconnected,
            Rejection::InvalidEpoch => Self::InvalidEpoch,
            Rejection::InvalidSequence => Self::InvalidSequence,
            Rejection::CommandConflict => Self::Conflict,
            Rejection::CommandLimit => Self::CommandLimit,
            Rejection::InvalidCell => Self::InvalidCell,
            Rejection::AlreadyOpen => Self::AlreadyOpen,
            Rejection::Flagged => Self::Flagged,
            Rejection::Stunned => Self::Stunned,
            Rejection::AnalysisStale => Self::Stale,
        }
    }
}
pub fn from_ack(ack: liar_core::game::Ack, id: u32) -> LocalAck {
    let (status, error) = match ack.status {
        ActionStatus::Applied => (AckStatus::Applied, None),
        ActionStatus::Rejected(error) => (AckStatus::Rejected, Some(error.into())),
    };
    LocalAck {
        command_id: id,
        revision: ack.revision,
        status,
        error,
        duplicate: ack.duplicate,
    }
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        PublicAction::decl(config),
        LocalInput::decl(config),
        PublicError::decl(config),
        CellState::decl(config),
        PublicCell::decl(config),
        PublicNumericUpdate::decl(config),
        OwnBoard::decl(config),
        MatchStats::decl(config),
        OpponentProgress::decl(config),
        GamePhase::decl(config),
        PublicEndReason::decl(config),
        Outcome::decl(config),
        GameResult::decl(config),
        GameView::decl(config),
        AckStatus::decl(config),
        LocalAck::decl(config),
        LocalStep::decl(config),
    ]
}
