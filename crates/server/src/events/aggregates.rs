use super::{
    D7Cohort, EventError, MAX_METRIC_INPUTS, MetricFuture, UtcDay, WeeklyNsm, aggregate_alive,
    aggregate_expires_at, valid_timestamp,
};
use chrono::{Datelike, Days, Weekday};
use serde::Serialize;
use std::time::Duration;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricKind {
    WeeklyNsm,
    D7,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "metric", content = "data", rename_all = "snake_case")]
pub enum MetricSnapshot {
    WeeklyNsm(WeeklyNsm),
    D7(D7Cohort),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AggregateReport {
    pub period: UtcDay,
    pub computed_at: i64,
    pub observed_through: i64,
    pub expires_at: i64,
    pub snapshot: MetricSnapshot,
}
pub trait AggregateStore: Send + Sync {
    fn refresh(
        &self,
        kind: MetricKind,
        period: UtcDay,
        allowance: Duration,
    ) -> MetricFuture<'_, AggregateReport>;
    fn read(&self, kind: MetricKind, period: UtcDay) -> MetricFuture<'_, Option<AggregateReport>>;
}
impl MetricKind {
    pub fn parse(value: &str) -> Result<Self, EventError> {
        match value {
            "weekly_nsm" => Ok(Self::WeeklyNsm),
            "d7" => Ok(Self::D7),
            _ => Err(EventError::Invalid),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WeeklyNsm => "weekly_nsm",
            Self::D7 => "d7",
        }
    }
}
pub fn aggregate_schedule(now: i64, cursor: u8) -> Result<[(MetricKind, UtcDay); 2], EventError> {
    if cursor > 22 {
        return Err(EventError::Invalid);
    }
    let today = UtcDay::from_timestamp(now)?;
    let week = UtcDay(
        today
            .0
            .checked_sub_days(Days::new(u64::from(
                today.0.weekday().num_days_from_monday(),
            )))
            .ok_or(EventError::Invalid)?,
    );
    let rotating = if cursor == 0 {
        (
            MetricKind::WeeklyNsm,
            UtcDay(
                week.0
                    .checked_sub_days(Days::new(7))
                    .ok_or(EventError::Invalid)?,
            ),
        )
    } else {
        (
            MetricKind::D7,
            UtcDay(
                today
                    .0
                    .checked_sub_days(Days::new(u64::from(cursor) + 7))
                    .ok_or(EventError::Invalid)?,
            ),
        )
    };
    valid_timestamp(week.start())?;
    valid_timestamp(rotating.1.start())?;
    Ok([(MetricKind::WeeklyNsm, week), rotating])
}
impl MetricSnapshot {
    pub fn kind(&self) -> MetricKind {
        match self {
            Self::WeeklyNsm(_) => MetricKind::WeeklyNsm,
            Self::D7(_) => MetricKind::D7,
        }
    }
    pub fn period(&self) -> UtcDay {
        match self {
            Self::WeeklyNsm(r) => r.week,
            Self::D7(r) => r.cohort,
        }
    }
    pub fn complete(&self) -> bool {
        match self {
            Self::WeeklyNsm(r) => r.complete,
            Self::D7(r) => r.mature,
        }
    }
    fn end(&self) -> Result<i64, EventError> {
        let days = if self.kind() == MetricKind::WeeklyNsm {
            7
        } else {
            8
        };
        let end = UtcDay(
            self.period()
                .0
                .checked_add_days(Days::new(days))
                .ok_or(EventError::Invalid)?,
        )
        .start();
        valid_timestamp(end)?;
        Ok(end)
    }
}
impl AggregateReport {
    pub fn new(snapshot: MetricSnapshot, computed_at: i64) -> Result<Self, EventError> {
        let report = Self {
            period: snapshot.period(),
            computed_at,
            observed_through: computed_at.min(snapshot.end()?),
            expires_at: aggregate_expires_at(snapshot.period())?,
            snapshot,
        };
        report.validate()?;
        Ok(report)
    }
    pub fn validate(&self) -> Result<(), EventError> {
        aggregate_alive(self.period, self.computed_at)?;
        let end = self.snapshot.end()?;
        if self.period != self.snapshot.period()
            || self.expires_at != aggregate_expires_at(self.period)?
            || self.observed_through != self.computed_at.min(end)
            || self.snapshot.complete() != (self.computed_at >= end)
        {
            return Err(EventError::Conflict);
        }
        let cap = MAX_METRIC_INPUTS as u32;
        match &self.snapshot {
            MetricSnapshot::WeeklyNsm(r) => {
                let total = r
                    .human_participations
                    .checked_add(r.bot_participations)
                    .ok_or(EventError::Invalid)?;
                if r.week.0.weekday() != Weekday::Mon
                    || r.observed_through != self.observed_through
                    || r.participants > cap
                    || total > cap
                    || r.repeat_accounts > r.participants
                    || total < r.participants + r.repeat_accounts
                    || (r.repeat_accounts == 0 && total != r.participants)
                    || (r.participants == 0 && total != 0)
                {
                    return Err(EventError::Invalid);
                }
            }
            MetricSnapshot::D7(r) => {
                let return_day = UtcDay(
                    r.cohort
                        .0
                        .checked_add_days(Days::new(7))
                        .ok_or(EventError::Invalid)?,
                );
                if r.return_day != return_day
                    || r.sample_size > cap
                    || r.retained.is_some() != r.mature
                    || r.retained.is_some_and(|n| n > r.sample_size)
                {
                    return Err(EventError::Invalid);
                }
            }
        }
        Ok(())
    }
    pub fn stale(&self, now: i64) -> Result<bool, EventError> {
        self.validate()?;
        aggregate_alive(self.period, now)?;
        if now < self.computed_at {
            return Err(EventError::Invalid);
        }
        Ok(now - self.computed_at >= 1800)
    }
    pub fn can_replace(&self, next: &Self) -> Result<bool, EventError> {
        self.validate()?;
        next.validate()?;
        if self.period != next.period
            || self.snapshot.kind() != next.snapshot.kind()
            || next.computed_at < self.computed_at
            || next.observed_through < self.observed_through
            || (self.snapshot.complete() && !next.snapshot.complete())
        {
            return Err(EventError::Conflict);
        }
        if self.computed_at == next.computed_at {
            return if self == next {
                Ok(false)
            } else {
                Err(EventError::Conflict)
            };
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/aggregate_model.rs"]
mod tests;
