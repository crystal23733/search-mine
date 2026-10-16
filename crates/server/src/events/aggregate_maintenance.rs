use super::*;
use crate::auth::AuthClock;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
pub struct AggregationCycle {
    pub current: Result<AggregateReport, MetricReadError>,
    pub rotating: Result<AggregateReport, MetricReadError>,
}
pub struct AggregationMaintenance {
    store: Arc<dyn AggregateStore>,
    clock: Arc<dyn AuthClock>,
    cursor: u8,
}
impl AggregationMaintenance {
    pub fn new(store: Arc<dyn AggregateStore>, clock: Arc<dyn AuthClock>) -> Self {
        Self {
            store,
            clock,
            cursor: 0,
        }
    }
    pub async fn run_cycle(&mut self) -> Result<AggregationCycle, MetricReadError> {
        let schedule = aggregate_schedule(self.clock.now(), self.cursor)?;
        self.cursor = (self.cursor + 1) % 23;
        let deadline = Instant::now() + Duration::from_secs(4);
        let work = async {
            let current = self.refresh(schedule[0], deadline).await;
            let rotating = self.refresh(schedule[1], deadline).await;
            Ok(AggregationCycle { current, rotating })
        };
        tokio::time::timeout_at(deadline, work)
            .await
            .unwrap_or(Err(MetricReadError::Unavailable))
    }
    async fn refresh(
        &self,
        target: (MetricKind, UtcDay),
        deadline: Instant,
    ) -> Result<AggregateReport, MetricReadError> {
        let allowance = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(2));
        if allowance.is_zero() {
            return Err(MetricReadError::Unavailable);
        }
        let report =
            tokio::time::timeout(allowance, self.store.refresh(target.0, target.1, allowance))
                .await
                .unwrap_or(Err(MetricReadError::Unavailable))?;
        report.validate()?;
        if report.period != target.1 || report.snapshot.kind() != target.0 {
            return Err(EventError::Conflict.into());
        }
        Ok(report)
    }
    pub async fn run(mut self) {
        let mut ticks = tokio::time::interval(Duration::from_secs(60));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            match self.run_cycle().await {
                Ok(cycle) => {
                    report("current", &cycle.current);
                    report("rotating", &cycle.rotating);
                }
                Err(error) => report("cycle", &Err(error)),
            }
        }
    }
}
fn report(slot: &'static str, result: &Result<AggregateReport, MetricReadError>) {
    let code = match result {
        Ok(_) => "complete",
        Err(MetricReadError::Unavailable) => "unavailable",
        Err(MetricReadError::Policy(EventError::Incomplete)) => "incomplete",
        Err(MetricReadError::Policy(EventError::Capacity)) => "capacity",
        Err(MetricReadError::Policy(EventError::Expired)) => "expired",
        Err(MetricReadError::Policy(EventError::Conflict)) => "conflict",
        Err(MetricReadError::Policy(_)) => "invalid",
    };
    eprintln!(
        "{}",
        serde_json::json!({"component":"product_metrics","slot":slot,"code":code})
    );
}
#[cfg(test)]
#[path = "../../tests/unit/aggregate_maintenance.rs"]
mod tests;
