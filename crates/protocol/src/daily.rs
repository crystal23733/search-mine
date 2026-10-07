//! Solo public daily projection and bounded replay input contract.
use crate::game::{GamePhase, LocalAck, LocalInput, OwnBoard, PROTOCOL_VERSION, PublicError};
use liar_core::{
    daily::{DailyMetadata, DailyPuzzle},
    game::Seat,
    rules::RulesSnapshot,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const MAX_DAILY_REPLAY_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DailyActionKind {
    Open,
    Flag,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyView {
    pub v: u16,
    pub metadata: DailyMetadata,
    pub rules: RulesSnapshot,
    #[ts(type = "number")]
    pub revision: u64,
    pub phase: GamePhase,
    pub countdown_ms: u32,
    pub remaining_ms: u32,
    pub own: OwnBoard,
    pub completed: bool,
    pub elapsed_ms: u32,
    pub available_actions: Vec<DailyActionKind>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyStep {
    pub ack: LocalAck,
    pub view: DailyView,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyTimedInput {
    pub input: LocalInput,
    pub time_ms: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyReplay {
    pub v: u16,
    pub metadata: DailyMetadata,
    pub inputs: Vec<DailyTimedInput>,
    pub final_time_ms: u32,
}
pub fn from_daily(puzzle: &DailyPuzzle) -> DailyView {
    let view = crate::game::from_projection(&puzzle.projection(), Seat::One);
    DailyView {
        v: view.v,
        metadata: puzzle.metadata().clone(),
        rules: view.rules,
        revision: view.revision,
        phase: view.phase,
        countdown_ms: view.countdown_ms,
        remaining_ms: view.remaining_ms,
        own: view.own,
        completed: puzzle.complete(),
        elapsed_ms: puzzle.elapsed_ms(),
        available_actions: vec![DailyActionKind::Open, DailyActionKind::Flag],
    }
}
pub fn decode_replay(source: &str) -> Result<DailyReplay, PublicError> {
    if source.len() > MAX_DAILY_REPLAY_BYTES {
        return Err(PublicError::Malformed);
    }
    let replay: DailyReplay = serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    let raw: serde_json::Value =
        serde_json::from_str(source).map_err(|_| PublicError::Malformed)?;
    if serde_json::to_value(&replay).map_err(|_| PublicError::Malformed)? != raw {
        return Err(PublicError::Malformed);
    }
    if replay.v != PROTOCOL_VERSION {
        return Err(PublicError::UnsupportedVersion);
    }
    if replay.inputs.len() > usize::from(RulesSnapshot::bundled().rules.max_commands_per_seat) {
        return Err(PublicError::CommandLimit);
    }
    for entry in &replay.inputs {
        crate::game::decode_local(
            &serde_json::to_string(&entry.input).map_err(|_| PublicError::Malformed)?,
        )?;
    }
    Ok(replay)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        DailyMetadata::decl(config),
        DailyActionKind::decl(config),
        DailyView::decl(config),
        DailyStep::decl(config),
        DailyTimedInput::decl(config),
        DailyReplay::decl(config),
    ]
}
