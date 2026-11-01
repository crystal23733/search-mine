//! Self-only account views; no credentials, raw replays or event streams.
use crate::{daily::DailyTrust, results::PersonalResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_ME_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeError {
    Malformed,
    UnsupportedVersion,
    Unauthorized,
    Unavailable,
    RateLimited,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeKind {
    Online,
    Daily,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeOpponentFilter {
    All,
    Human,
    Bot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeOpponent {
    Human,
    Bot,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MeOverviewInput {
    pub v: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MeHistoryInput {
    pub v: u16,
    pub kind: MeKind,
    pub opponent: MeOpponentFilter,
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MeIdentity {
    pub provider: String,
    #[ts(type = "number")]
    pub linked_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MeAccount {
    pub id: String,
    pub nickname: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub last_seen_at: i64,
    pub identities: Vec<MeIdentity>,
}
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct MeMatchSummary {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub aborted: u32,
    pub cancelled: u32,
    pub played: u32,
    pub win_rate: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeChoice {
    Unknown,
    Denied,
    Granted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MeConsentStatus {
    Unknown,
    Current,
    Expired,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MeConsent {
    pub cmp_available: bool,
    pub status: MeConsentStatus,
    pub analytics: MeChoice,
    pub advertising: MeChoice,
    #[ts(type = "number | null")]
    pub checked_at: Option<i64>,
    #[ts(type = "number | null")]
    pub expires_at: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MeOnlineRecord {
    pub result: PersonalResult,
    pub opponent: MeOpponent,
    #[ts(type = "number")]
    pub retention_started_at: i64,
    #[ts(type = "number")]
    pub recorded_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
pub struct MeDailyRecord {
    pub completion_id: String,
    pub date: String,
    pub opened_safe: u32,
    pub mistakes: u16,
    pub replayed_elapsed_ms: u32,
    pub verified_elapsed_ms: Option<u32>,
    pub trust: DailyTrust,
    #[ts(type = "number")]
    pub completed_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", content = "entry", rename_all = "snake_case")]
pub enum MeRecord {
    Online(MeOnlineRecord),
    Daily(MeDailyRecord),
}
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct MeOverview {
    pub v: u16,
    pub account: MeAccount,
    #[ts(type = "number")]
    pub observed_at: i64,
    #[ts(type = "number")]
    pub window_started_at: i64,
    pub online: MeMatchSummary,
    pub recent_online: Vec<MeOnlineRecord>,
    pub daily_total: u32,
    pub recent_daily: Vec<MeDailyRecord>,
    pub consent: MeConsent,
}
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
pub struct MeHistory {
    pub v: u16,
    pub account_id: String,
    pub kind: MeKind,
    pub opponent: MeOpponentFilter,
    #[ts(type = "number")]
    pub observed_at: i64,
    #[ts(type = "number")]
    pub window_started_at: i64,
    pub total: u32,
    pub online: Option<MeMatchSummary>,
    pub items: Vec<MeRecord>,
    pub next: Option<String>,
}
pub fn decode_me_overview(source: &[u8]) -> Result<MeOverviewInput, MeError> {
    if source.len() > MAX_ME_BYTES {
        return Err(MeError::Malformed);
    }
    let value: MeOverviewInput = serde_json::from_slice(source).map_err(|_| MeError::Malformed)?;
    if value.v != crate::game::PROTOCOL_VERSION {
        return Err(MeError::UnsupportedVersion);
    }
    Ok(value)
}
pub fn decode_me_history(source: &[u8]) -> Result<MeHistoryInput, MeError> {
    if source.len() > MAX_ME_BYTES {
        return Err(MeError::Malformed);
    }
    let value: MeHistoryInput = serde_json::from_slice(source).map_err(|_| MeError::Malformed)?;
    if value.v != crate::game::PROTOCOL_VERSION {
        return Err(MeError::UnsupportedVersion);
    }
    if (value.kind == MeKind::Daily && value.opponent != MeOpponentFilter::All)
        || value
            .cursor
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 512)
    {
        return Err(MeError::Malformed);
    }
    Ok(value)
}
pub fn declarations(config: &ts_rs::Config) -> Vec<String> {
    vec![
        MeError::decl(config),
        MeKind::decl(config),
        MeOpponentFilter::decl(config),
        MeOpponent::decl(config),
        MeOverviewInput::decl(config),
        MeHistoryInput::decl(config),
        MeIdentity::decl(config),
        MeAccount::decl(config),
        MeMatchSummary::decl(config),
        MeChoice::decl(config),
        MeConsentStatus::decl(config),
        MeConsent::decl(config),
        MeOnlineRecord::decl(config),
        MeDailyRecord::decl(config),
        MeRecord::decl(config),
        MeOverview::decl(config),
        MeHistory::decl(config),
    ]
}
