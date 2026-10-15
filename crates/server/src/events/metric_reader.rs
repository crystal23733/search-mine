use super::{D7Cohort, EventError, UtcDay, WeeklyNsm};
use std::{future::Future, pin::Pin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricReadError {
    Policy(EventError),
    Unavailable,
}
impl From<EventError> for MetricReadError {
    fn from(error: EventError) -> Self {
        Self::Policy(error)
    }
}
pub type MetricFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, MetricReadError>> + Send + 'a>>;
pub trait EventMetricReader: Send + Sync {
    fn weekly(&self, week: UtcDay) -> MetricFuture<'_, WeeklyNsm>;
    fn d7(&self, cohort: UtcDay) -> MetricFuture<'_, D7Cohort>;
}
