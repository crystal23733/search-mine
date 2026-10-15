use super::{
    EventError, EventId, EventSubject, OpponentKind, ProductEvent, ServerFact, UtcDay,
    model::EventContent, valid_timestamp,
};
use chrono::{Datelike, Days, Weekday};
use liar_protocol::game::PublicEndReason;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_METRIC_INPUTS: usize = 10000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirstNormalCompletion {
    pub account: EventId,
    pub day: UtcDay,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WeeklyNsm {
    pub week: UtcDay,
    pub observed_through: i64,
    pub complete: bool,
    pub participants: u32,
    pub repeat_accounts: u32,
    pub human_participations: u32,
    pub bot_participations: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct D7Cohort {
    pub cohort: UtcDay,
    pub return_day: UtcDay,
    pub sample_size: u32,
    pub retained: Option<u32>,
    pub mature: bool,
}
impl D7Cohort {
    pub fn rate(&self) -> Option<f64> {
        self.retained
            .filter(|_| self.sample_size != 0)
            .map(|n| f64::from(n) / f64::from(self.sample_size))
    }
}
struct Normal {
    account: EventId,
    occurred_at: i64,
    opponent: OpponentKind,
}
fn normal_events(
    events: &[ProductEvent],
    now: i64,
    complete: bool,
) -> Result<Vec<Normal>, EventError> {
    valid_timestamp(now)?;
    if !complete {
        return Err(EventError::Incomplete);
    }
    if events.len() > MAX_METRIC_INPUTS {
        return Err(EventError::Capacity);
    }
    let mut ids = BTreeMap::new();
    let mut facts = BTreeMap::new();
    let mut matches = BTreeMap::new();
    let mut result = Vec::new();
    for event in events {
        if event.occurred_at > now {
            return Err(EventError::Invalid);
        }
        if let Some(prior) = ids.insert(event.event_id, event) {
            if prior != event {
                return Err(EventError::Conflict);
            }
            continue;
        }
        let (
            EventSubject::Account(account),
            EventContent::Server(ServerFact::MatchEnd {
                match_id,
                opponent,
                reason,
            }),
        ) = (event.subject, &event.content)
        else {
            continue;
        };
        let fact = (event.occurred_at, *opponent, *reason);
        if let Some(previous) = facts.insert((account, *match_id), fact) {
            if previous != fact {
                return Err(EventError::Conflict);
            }
            continue;
        }
        let previous = matches.entry(*match_id).or_insert((fact, 0u8));
        if previous.0 != fact || previous.1 >= if *opponent == OpponentKind::Bot { 1 } else { 2 } {
            return Err(EventError::Conflict);
        }
        previous.1 += 1;
        if matches!(reason, PublicEndReason::Clear | PublicEndReason::Timeout) {
            result.push(Normal {
                account,
                occurred_at: event.occurred_at,
                opponent: *opponent,
            });
        }
    }
    Ok(result)
}
pub fn weekly_nsm(
    events: &[ProductEvent],
    week: UtcDay,
    now: i64,
    snapshot_complete: bool,
) -> Result<WeeklyNsm, EventError> {
    if week.0.weekday() != Weekday::Mon || week.start() > now {
        return Err(EventError::Invalid);
    }
    let end = UtcDay(
        week.0
            .checked_add_days(Days::new(7))
            .ok_or(EventError::Invalid)?,
    )
    .start();
    valid_timestamp(end)?;
    let normal = normal_events(events, now, snapshot_complete)?;
    let mut accounts = BTreeMap::<EventId, u32>::new();
    let mut human = 0;
    let mut bot = 0;
    for event in normal
        .into_iter()
        .filter(|event| event.occurred_at >= week.start() && event.occurred_at < end)
    {
        *accounts.entry(event.account).or_default() += 1;
        match event.opponent {
            OpponentKind::Human => human += 1,
            OpponentKind::Bot => bot += 1,
        }
    }
    Ok(WeeklyNsm {
        week,
        observed_through: now.min(end),
        complete: now >= end,
        participants: accounts.len() as u32,
        repeat_accounts: accounts.values().filter(|n| **n >= 2).count() as u32,
        human_participations: human,
        bot_participations: bot,
    })
}
pub fn d7_cohort(
    first: &[FirstNormalCompletion],
    events: &[ProductEvent],
    cohort: UtcDay,
    now: i64,
    snapshot_complete: bool,
) -> Result<D7Cohort, EventError> {
    if first.len() > MAX_METRIC_INPUTS {
        return Err(EventError::Capacity);
    }
    let return_day = UtcDay(
        cohort
            .0
            .checked_add_days(Days::new(7))
            .ok_or(EventError::Invalid)?,
    );
    let end = UtcDay(return_day.0.succ_opt().ok_or(EventError::Invalid)?).start();
    valid_timestamp(end)?;
    let normal = normal_events(events, now, snapshot_complete)?;
    if cohort.start() > now {
        return Err(EventError::Invalid);
    }
    let mut anchors = BTreeMap::new();
    for fact in first {
        if fact.day.start() > now {
            return Err(EventError::Invalid);
        }
        if let Some(prior) = anchors.insert(fact.account, fact.day)
            && prior != fact.day
        {
            return Err(EventError::Conflict);
        }
    }
    // A retained event is never evidence of the lifetime first completion date.
    let members: BTreeSet<_> = anchors
        .iter()
        .filter_map(|(account, day)| (*day == cohort).then_some(*account))
        .collect();
    let mut returned = BTreeSet::new();
    for event in normal {
        if let Some(first_day) = anchors.get(&event.account) {
            if event.occurred_at < first_day.start() {
                return Err(EventError::Conflict);
            }
        } else {
            return Err(EventError::Incomplete);
        }
        if members.contains(&event.account)
            && event.occurred_at >= return_day.start()
            && event.occurred_at < end
        {
            returned.insert(event.account);
        }
    }
    Ok(D7Cohort {
        cohort,
        return_day,
        sample_size: members.len() as u32,
        retained: (now >= end).then_some(returned.len() as u32),
        mature: now >= end,
    })
}

#[cfg(test)]
#[path = "../../tests/unit/event_metrics.rs"]
mod tests;
