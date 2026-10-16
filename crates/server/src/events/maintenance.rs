use super::{EventRetentionStore, EventStoreError, RETENTION_BATCH_SIZE};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionCycle {
    pub deleted: u32,
    pub has_more: bool,
    pub batches: u8,
}
pub struct EventRetentionMaintenance {
    store: Arc<dyn EventRetentionStore>,
    component: &'static str,
}
impl EventRetentionMaintenance {
    pub fn new(store: Arc<dyn EventRetentionStore>) -> Self {
        Self {
            store,
            component: "event_retention",
        }
    }
    pub fn anonymous(store: Arc<dyn EventRetentionStore>) -> Self {
        Self {
            store,
            component: "metric_retention",
        }
    }
    pub async fn run_cycle(&self) -> Result<RetentionCycle, EventStoreError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        tokio::time::timeout_at(deadline, async {
            let mut result = RetentionCycle {
                deleted: 0,
                has_more: false,
                batches: 0,
            };
            for _ in 0..8 {
                let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
                if budget.is_zero() {
                    return Err(EventStoreError::Unavailable);
                }
                let batch = self.store.reap(budget).await?;
                if batch.deleted > RETENTION_BATCH_SIZE {
                    return Err(EventStoreError::Unavailable);
                }
                result.deleted += u32::from(batch.deleted);
                result.batches += 1;
                result.has_more = batch.has_more;
                if !batch.has_more || batch.deleted == 0 {
                    break;
                }
            }
            Ok(result)
        })
        .await
        .unwrap_or(Err(EventStoreError::Unavailable))
    }
    pub async fn run(self) {
        let mut ticks = tokio::time::interval(Duration::from_secs(60));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            match self.run_cycle().await {
                Ok(cycle) => eprintln!(
                    "{}",
                    serde_json::json!({"component":self.component,"code":"complete","deleted":cycle.deleted,"has_more":cycle.has_more,"batches":cycle.batches})
                ),
                Err(error) => eprintln!(
                    "{}",
                    serde_json::json!({"component":self.component,"code":if error==EventStoreError::Invalid {"invalid_clock"}else{"unavailable"}})
                ),
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/event_maintenance.rs"]
mod tests;
