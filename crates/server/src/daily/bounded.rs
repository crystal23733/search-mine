use super::{PublishedDaily, ReplayError, ReplayEvidence, ReplayVerifier};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct BoundedReplayVerifier {
    port: Arc<dyn ReplayVerifier>,
    workers: Arc<Semaphore>,
}
impl BoundedReplayVerifier {
    pub fn new(port: Arc<dyn ReplayVerifier>) -> Self {
        Self {
            port,
            workers: Arc::new(Semaphore::new(2)),
        }
    }
    pub async fn verify(
        &self,
        definition: Arc<PublishedDaily>,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError> {
        if source.len() > liar_protocol::daily::MAX_DAILY_REPLAY_BYTES {
            return Err(ReplayError::Malformed);
        }
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ReplayError::Busy)?;
        let port = self.port.clone();
        let source = source.to_owned();
        let worker = tokio::task::spawn_blocking(move || {
            // Caller cancellation must not return capacity while this CPU task is live.
            let _permit = permit;
            port.verify(&definition, &source)
        });
        match tokio::time::timeout(Duration::from_secs(2), worker).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ReplayError::Unavailable),
            Err(_) => Err(ReplayError::Timeout),
        }
    }
}
#[cfg(test)]
#[path = "../../tests/unit/daily_bounded.rs"]
mod tests;
