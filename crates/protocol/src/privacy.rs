//! Bounded self-only game export; account/session proofs are never exported.
use crate::{
    auth::AuthExport,
    daily::{DailyReplay, DailyTrust},
    game::PublicEndReason,
    results::PersonalResult,
};
use liar_core::daily::DailyMetadata;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportStream {
    Online,
    Daily,
    Event,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventOpponent {
    Human,
    Bot,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExportEventFact {
    MatchEnd {
        match_id: String,
        opponent: EventOpponent,
        reason: PublicEndReason,
    },
    DailyVerified {
        completion_id: String,
        date: String,
        mistakes: u16,
        trust: DailyTrust,
    },
}
#[derive(Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GameExportInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub stream: ExportStream,
    pub after: Option<String>,
}
#[derive(Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplayExportInput {
    pub v: u16,
    pub account_id: String,
    pub session_revision: String,
    pub completion_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportRecord {
    Event {
        event_id: String,
        #[ts(type = "number")]
        occurred_at: i64,
        #[ts(type = "number")]
        received_at: i64,
        fact: ExportEventFact,
    },
    Online {
        result: PersonalResult,
        #[ts(type = "number")]
        recorded_at: i64,
        #[ts(type = "number")]
        retention_started_at: i64,
    },
    Daily {
        completion_id: String,
        attempt_id: String,
        metadata: DailyMetadata,
        opened_safe: u32,
        mistakes: u16,
        replayed_elapsed_ms: u32,
        verified_elapsed_ms: Option<u32>,
        #[ts(type = "number")]
        completed_at: i64,
        replay_available: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct GameExportPage {
    pub v: u16,
    pub account_id: String,
    pub stream: ExportStream,
    pub records: Vec<ExportRecord>,
    pub next: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct ReplayExport {
    pub v: u16,
    pub account_id: String,
    pub completion_id: String,
    pub replay: Option<DailyReplay>,
}
#[derive(Debug, Serialize, TS)]
pub struct PersonalDataExport {
    pub v: u16,
    pub account: AuthExport,
    pub games: Vec<ExportRecord>,
    pub replays: Vec<ReplayExport>,
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        ExportStream::decl(config),
        EventOpponent::decl(config),
        ExportEventFact::decl(config),
        GameExportInput::decl(config),
        ReplayExportInput::decl(config),
        ExportRecord::decl(config),
        GameExportPage::decl(config),
        ReplayExport::decl(config),
        PersonalDataExport::decl(config),
    ]
}
