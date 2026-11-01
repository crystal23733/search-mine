use crate::{auth::Session, consent::CurrentConsent, online::PortFuture};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use liar_protocol::{
    game::{Outcome, PROTOCOL_VERSION},
    me::*,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ME_RETENTION_SECONDS: i64 = 7776000;
pub const ME_PAGE_SIZE: usize = 20;
pub const MAX_ME_RECORDS: usize = 10000;
pub const MAX_ME_TIMESTAMP: i64 = 253402300799;
#[derive(Clone, Copy)]
pub struct MeSession {
    pub account: Uuid,
    pub revision: Uuid,
    pub hash: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeKey {
    pub primary: i64,
    pub secondary: i64,
    pub id: Uuid,
}
#[derive(Clone, Debug)]
pub enum MeQuery {
    Overview,
    History {
        kind: MeKind,
        opponent: MeOpponentFilter,
        after: Option<MeKey>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    v: u16,
    kind: MeKind,
    opponent: MeOpponentFilter,
    primary: i64,
    secondary: i64,
    id: String,
}
impl MeQuery {
    pub fn history(input: MeHistoryInput) -> Result<Self, MeError> {
        let after = input
            .cursor
            .map(|value| {
                if value.len() > 512 || value.is_empty() {
                    return Err(MeError::Malformed);
                }
                let bytes = URL_SAFE_NO_PAD
                    .decode(&value)
                    .map_err(|_| MeError::Malformed)?;
                if URL_SAFE_NO_PAD.encode(&bytes) != value {
                    return Err(MeError::Malformed);
                }
                let c: Cursor = serde_json::from_slice(&bytes).map_err(|_| MeError::Malformed)?;
                let id = Uuid::parse_str(&c.id).map_err(|_| MeError::Malformed)?;
                if c.v != 1
                    || c.kind != input.kind
                    || c.opponent != input.opponent
                    || id.is_nil()
                    || id.to_string() != c.id
                    || c.primary < 0
                    || c.secondary < c.primary
                    || c.secondary > MAX_ME_TIMESTAMP * 1000000
                    || (c.kind == MeKind::Daily && c.primary != c.secondary)
                {
                    return Err(MeError::Malformed);
                }
                Ok(MeKey {
                    primary: c.primary,
                    secondary: c.secondary,
                    id,
                })
            })
            .transpose()?;
        if input.v != PROTOCOL_VERSION
            || (input.kind == MeKind::Daily && input.opponent != MeOpponentFilter::All)
        {
            return Err(MeError::Malformed);
        }
        Ok(Self::History {
            kind: input.kind,
            opponent: input.opponent,
            after,
        })
    }
}
fn cursor(kind: MeKind, opponent: MeOpponentFilter, key: MeKey) -> Result<String, MeError> {
    Ok(URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&Cursor {
            v: 1,
            kind,
            opponent,
            primary: key.primary,
            secondary: key.secondary,
            id: key.id.to_string(),
        })
        .map_err(|_| MeError::Unavailable)?,
    ))
}
#[derive(Clone)]
pub struct MeOnlineRow {
    pub key: MeKey,
    pub entry: MeOnlineRecord,
}
#[derive(Clone)]
pub struct MeDailyRow {
    pub key: MeKey,
    pub entry: MeDailyRecord,
}
pub struct MeData {
    pub account: MeAccount,
    pub online: Vec<MeOnlineRow>,
    pub daily: Vec<MeDailyRow>,
    pub consent: Option<CurrentConsent>,
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum MePayload {
    Overview(MeOverview),
    History(MeHistory),
}
pub struct MeRead {
    pub account: Uuid,
    pub revision: Uuid,
    pub observed_at: i64,
    pub valid_until: i64,
    pub payload: MePayload,
}
pub trait MeReader: Send + Sync {
    fn read(&self, session: MeSession, query: MeQuery) -> PortFuture<'_, Result<MeRead, MeError>>;
}
pub(super) fn valid_session(value: &Session, now: i64) -> bool {
    !value.account.id.is_nil()
        && !value.id.is_nil()
        && (0..=MAX_ME_TIMESTAMP).contains(&now)
        && value.created_at >= 0
        && value.created_at <= value.authenticated_at
        && value.authenticated_at <= now
        && now < value.expires_at
        && value.expires_at <= MAX_ME_TIMESTAMP
}
pub(super) fn same_session(first: &Session, last: &Session, now: i64) -> bool {
    valid_session(last, now)
        && first.id == last.id
        && first.account.id == last.account.id
        && first.created_at == last.created_at
        && first.authenticated_at == last.authenticated_at
        && first.expires_at == last.expires_at
}
pub(super) fn summary(rows: &[MeOnlineRow]) -> Result<MeMatchSummary, MeError> {
    if rows.len() > MAX_ME_RECORDS {
        return Err(MeError::Unavailable);
    }
    let mut value = MeMatchSummary {
        wins: 0,
        losses: 0,
        draws: 0,
        aborted: 0,
        cancelled: 0,
        played: 0,
        win_rate: None,
    };
    for row in rows {
        let result = &row.entry.result.result;
        if result.completed
            != matches!(result.outcome, Outcome::Win | Outcome::Loss | Outcome::Draw)
        {
            return Err(MeError::Unavailable);
        }
        match result.outcome {
            Outcome::Win => value.wins += 1,
            Outcome::Loss => value.losses += 1,
            Outcome::Draw => value.draws += 1,
            Outcome::Abort => value.aborted += 1,
            Outcome::Cancelled => value.cancelled += 1,
        }
    }
    value.played = value.wins + value.losses + value.draws;
    value.win_rate = (value.played > 0).then(|| f64::from(value.wins) / f64::from(value.played));
    Ok(value)
}
pub(super) fn consent_view(value: Option<CurrentConsent>, now: i64) -> Result<MeConsent, MeError> {
    let Some(value) = value else {
        return Ok(MeConsent {
            cmp_available: false,
            status: MeConsentStatus::Unknown,
            analytics: MeChoice::Unknown,
            advertising: MeChoice::Unknown,
            checked_at: None,
            expires_at: None,
        });
    };
    if value.subject.is_nil()
        || value.revision.is_nil()
        || value.checked_at < 0
        || value.checked_at > now
        || value.expires_at <= value.checked_at
        || value.expires_at > MAX_ME_TIMESTAMP
    {
        return Err(MeError::Unavailable);
    }
    let live = now < value.expires_at;
    let choice = |granted| {
        if !live {
            MeChoice::Unknown
        } else if granted {
            MeChoice::Granted
        } else {
            MeChoice::Denied
        }
    };
    Ok(MeConsent {
        cmp_available: false,
        status: if live {
            MeConsentStatus::Current
        } else {
            MeConsentStatus::Expired
        },
        analytics: choice(value.analytics),
        advertising: choice(value.advertising),
        checked_at: Some(value.checked_at),
        expires_at: Some(value.expires_at),
    })
}
pub(super) fn build(
    mut data: MeData,
    query: MeQuery,
    now: i64,
) -> Result<(MePayload, i64), MeError> {
    if !(0..=MAX_ME_TIMESTAMP).contains(&now)
        || data.online.len() > MAX_ME_RECORDS
        || data.daily.len() > MAX_ME_RECORDS
        || Uuid::parse_str(&data.account.id)
            .map_or(true, |id| id.is_nil() || id.to_string() != data.account.id)
        || data.account.created_at < 0
        || data.account.created_at > data.account.last_seen_at
        || data.account.last_seen_at > now
    {
        return Err(MeError::Unavailable);
    }
    let lower = (now - ME_RETENTION_SECONDS).max(0);
    let mut seen = std::collections::HashSet::new();
    for row in &data.online {
        if !valid_key(row.key)
            || row.entry.result.match_id != row.key.id.to_string()
            || row.entry.retention_started_at != row.key.primary / 1000000
            || row.entry.recorded_at != row.key.secondary / 1000000
            || !seen.insert(row.key.id)
        {
            return Err(MeError::Unavailable);
        }
    }
    seen.clear();
    for row in &data.daily {
        if !valid_key(row.key)
            || row.key.primary != row.key.secondary
            || row.entry.completion_id != row.key.id.to_string()
            || row.entry.completed_at != row.key.primary / 1000000
            || crate::events::UtcDay::parse(&row.entry.date).is_err()
            || !seen.insert(row.key.id)
        {
            return Err(MeError::Unavailable);
        }
    }
    data.online
        .retain(|row| row.key.primary > lower * 1000000 && row.key.secondary <= now * 1000000);
    data.daily
        .retain(|row| row.key.primary > lower * 1000000 && row.key.secondary <= now * 1000000);
    data.online.sort_by_key(|row| std::cmp::Reverse(row.key));
    data.daily.sort_by_key(|row| std::cmp::Reverse(row.key));
    match query {
        MeQuery::Overview => {
            let mut until = data
                .online
                .iter()
                .map(|r| expiry(r.key))
                .chain(data.daily.iter().map(|r| expiry(r.key)))
                .min()
                .unwrap_or(MAX_ME_TIMESTAMP + 1);
            let consent = consent_view(data.consent, now)?;
            if let Some(value) = data.consent.filter(|v| v.expires_at > now) {
                until = until.min(value.expires_at);
            }
            Ok((
                MePayload::Overview(MeOverview {
                    v: 1,
                    account: data.account,
                    observed_at: now,
                    window_started_at: lower,
                    online: summary(&data.online)?,
                    recent_online: data.online.into_iter().take(3).map(|r| r.entry).collect(),
                    daily_total: data.daily.len() as u32,
                    recent_daily: data.daily.into_iter().take(3).map(|r| r.entry).collect(),
                    consent,
                }),
                until,
            ))
        }
        MeQuery::History {
            kind,
            opponent,
            after,
        } => {
            if (kind == MeKind::Daily && opponent != MeOpponentFilter::All)
                || after.is_some_and(|key| {
                    !valid_key(key)
                        || key.secondary > now * 1000000
                        || (kind == MeKind::Daily && key.primary != key.secondary)
                })
            {
                return Err(MeError::Malformed);
            }
            data.online.retain(|row| match opponent {
                MeOpponentFilter::All => true,
                MeOpponentFilter::Human => row.entry.opponent == MeOpponent::Human,
                MeOpponentFilter::Bot => row.entry.opponent == MeOpponent::Bot,
            });
            let (total, online, mut items): (
                usize,
                Option<MeMatchSummary>,
                Vec<(MeKey, MeRecord)>,
            ) = match kind {
                MeKind::Online => (
                    data.online.len(),
                    Some(summary(&data.online)?),
                    data.online
                        .into_iter()
                        .map(|r| (r.key, MeRecord::Online(r.entry)))
                        .collect(),
                ),
                MeKind::Daily => (
                    data.daily.len(),
                    None,
                    data.daily
                        .into_iter()
                        .map(|r| (r.key, MeRecord::Daily(r.entry)))
                        .collect(),
                ),
            };
            let until = items
                .iter()
                .map(|(key, _)| expiry(*key))
                .min()
                .unwrap_or(MAX_ME_TIMESTAMP + 1);
            items.retain(|(key, _)| after.is_none_or(|after| *key < after));
            let next = if items.len() > ME_PAGE_SIZE {
                Some(cursor(kind, opponent, items[ME_PAGE_SIZE - 1].0)?)
            } else {
                None
            };
            Ok((
                MePayload::History(MeHistory {
                    v: 1,
                    account_id: data.account.id,
                    kind,
                    opponent,
                    observed_at: now,
                    window_started_at: lower,
                    total: total as u32,
                    online,
                    items: items
                        .into_iter()
                        .take(ME_PAGE_SIZE)
                        .map(|(_, r)| r)
                        .collect(),
                    next,
                }),
                until,
            ))
        }
    }
}
fn valid_key(key: MeKey) -> bool {
    !key.id.is_nil()
        && key.primary >= 0
        && key.secondary >= key.primary
        && key.secondary <= MAX_ME_TIMESTAMP * 1000000
}
fn expiry(key: MeKey) -> i64 {
    (key.primary + ME_RETENTION_SECONDS * 1000000 + 999999) / 1000000
}

#[cfg(test)]
#[path = "../../tests/unit/me_model.rs"]
mod tests;
