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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyDefinitionInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub date: String,
    pub seed_version: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailySubmitInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub attempt_id: String,
    pub replay: DailyReplay,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyOwnInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub metadata: DailyMetadata,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyStartInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub attempt_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyHeartbeatInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub ticket_id: String,
    pub sequence: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DailyTimedSubmitInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub attempt_id: String,
    pub ticket_id: Option<String>,
    pub replay: DailyReplay,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyStartResponse {
    pub v: u16,
    pub account_id: String,
    pub attempt_id: String,
    pub ticket_id: String,
    pub metadata: DailyMetadata,
    pub rules: RulesSnapshot,
    #[ts(type = "number")]
    pub ttl_ms: u64,
    pub heartbeat_interval_ms: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyHeartbeatAck {
    pub sequence: u32,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyHeartbeatResponse {
    pub v: u16,
    pub account_id: String,
    pub ticket_id: String,
    pub ack: Option<DailyHeartbeatAck>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyDefinition {
    pub v: u16,
    pub metadata: DailyMetadata,
    pub rules: RulesSnapshot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DailyTrust {
    VerifiedCompletion,
    VerifiedTime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DailyDisposition {
    Official,
    Practice,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyCompletion {
    pub completion_id: String,
    pub attempt_id: String,
    pub metadata: DailyMetadata,
    pub opened_safe: u32,
    pub mistakes: u16,
    pub replayed_elapsed_ms: u32,
    pub verified_elapsed_ms: Option<u32>,
    #[ts(type = "number")]
    pub completed_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailySubmitResponse {
    pub v: u16,
    pub account_id: String,
    pub attempt_id: String,
    pub disposition: DailyDisposition,
    pub trust: DailyTrust,
    pub receipt: DailyCompletion,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct DailyOwnResponse {
    pub v: u16,
    pub account_id: String,
    pub trust: DailyTrust,
    pub receipt: Option<DailyCompletion>,
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
        DailyDefinitionInput::decl(config),
        DailySubmitInput::decl(config),
        DailyOwnInput::decl(config),
        DailyStartInput::decl(config),
        DailyHeartbeatInput::decl(config),
        DailyTimedSubmitInput::decl(config),
        DailyStartResponse::decl(config),
        DailyHeartbeatAck::decl(config),
        DailyHeartbeatResponse::decl(config),
        DailyDefinition::decl(config),
        DailyTrust::decl(config),
        DailyDisposition::decl(config),
        DailyCompletion::decl(config),
        DailySubmitResponse::decl(config),
        DailyOwnResponse::decl(config),
    ]
}
