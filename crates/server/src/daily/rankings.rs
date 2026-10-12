use super::{DailyFuture, DailyReceipt, DailyStoreError, PublishedDaily};
use crate::auth::Nickname;
use liar_protocol::daily::*;
use std::collections::HashSet;

pub const DAILY_RANKING_LIMIT: usize = 50;

pub struct RankedDailyCompletion {
    pub nickname: String,
    pub receipt: DailyReceipt,
}
pub struct DailyRankingSnapshot {
    pub definition: PublishedDaily,
    pub time: Vec<RankedDailyCompletion>,
    pub completion: Vec<RankedDailyCompletion>,
}
pub trait DailyRankingReader: Send + Sync {
    fn read<'a>(
        &'a self,
        date: &'a str,
        version: u16,
    ) -> DailyFuture<'a, Result<Option<DailyRankingSnapshot>, DailyStoreError>>;
}

fn validate(
    rows: &[RankedDailyCompletion],
    definition: &PublishedDaily,
    timed: bool,
    now: i64,
) -> Result<(), DailyStoreError> {
    if rows.len() > DAILY_RANKING_LIMIT + 1 {
        return Err(DailyStoreError::Unavailable);
    }
    let mut ids = HashSet::new();
    let mut previous = None;
    for row in rows {
        let saved = &row.receipt;
        saved.validate(definition, now)?;
        if Nickname::parse(&row.nickname)
            .ok()
            .is_none_or(|name| name.as_str() != row.nickname)
            || saved.expired(now)
            || saved.replayed_elapsed_ms >= definition.rules().rules.duration_ms
            || !ids.insert(saved.id)
        {
            return Err(DailyStoreError::Unavailable);
        }
        let elapsed = if timed {
            saved
                .verified_elapsed_ms
                .ok_or(DailyStoreError::Unavailable)?
        } else {
            0
        };
        let order = (elapsed, saved.mistakes, saved.completed_at, saved.id);
        if previous.is_some_and(|last| last >= order) {
            return Err(DailyStoreError::Unavailable);
        }
        previous = Some(order);
    }
    Ok(())
}
impl DailyRankingSnapshot {
    pub fn project(
        self,
        date: &str,
        version: u16,
        now: i64,
    ) -> Result<DailyRankingsResponse, DailyStoreError> {
        let checked = PublishedDaily::new(date, version, self.definition.rules().clone())
            .map_err(|_| DailyStoreError::Unavailable)?;
        if now <= 0 || checked.metadata() != self.definition.metadata() {
            return Err(DailyStoreError::Unavailable);
        }
        validate(&self.time, &self.definition, true, now)?;
        validate(&self.completion, &self.definition, false, now)?;
        let time_more = self.time.len() > DAILY_RANKING_LIMIT;
        let completion_more = self.completion.len() > DAILY_RANKING_LIMIT;
        Ok(DailyRankingsResponse {
            v: 1,
            metadata: Some(self.definition.metadata().clone()),
            time: DailyTimeRankingPage {
                has_more: time_more,
                entries: self
                    .time
                    .into_iter()
                    .take(DAILY_RANKING_LIMIT)
                    .enumerate()
                    .map(|(index, row)| DailyTimeRankingEntry {
                        position: (index + 1) as u16,
                        nickname: row.nickname,
                        mistakes: row.receipt.mistakes,
                        elapsed_ms: row
                            .receipt
                            .verified_elapsed_ms
                            .expect("Validated timed row"),
                    })
                    .collect(),
            },
            completion: DailyCompletionRankingPage {
                has_more: completion_more,
                entries: self
                    .completion
                    .into_iter()
                    .take(DAILY_RANKING_LIMIT)
                    .enumerate()
                    .map(|(index, row)| DailyCompletionRankingEntry {
                        position: (index + 1) as u16,
                        nickname: row.nickname,
                        mistakes: row.receipt.mistakes,
                    })
                    .collect(),
            },
        })
    }
}
pub(super) fn empty_rankings() -> DailyRankingsResponse {
    DailyRankingsResponse {
        v: 1,
        metadata: None,
        time: DailyTimeRankingPage {
            entries: Vec::new(),
            has_more: false,
        },
        completion: DailyCompletionRankingPage {
            entries: Vec::new(),
            has_more: false,
        },
    }
}

#[cfg(test)]
#[path = "../../tests/unit/daily_rankings.rs"]
mod tests;
