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
        let port = self.port.clone();
        let source = source.to_owned();
        self.run(move || {
            let evidence = port.verify(&definition, &source)?;
            let canonical = liar_protocol::daily::decode_replay(&source)
                .ok()
                .and_then(|replay| serde_json::to_string(&replay).ok())
                .ok_or(ReplayError::Unavailable)?;
            if evidence.metadata() != definition.metadata()
                || evidence.canonical_replay() != canonical
            {
                return Err(ReplayError::Unavailable);
            }
            Ok(evidence)
        })
        .await
    }
    pub(crate) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, ReplayError> + Send + 'static,
    ) -> Result<T, ReplayError> {
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| ReplayError::Busy)?;
        let worker = tokio::task::spawn_blocking(move || {
            // Caller cancellation must not return capacity while this CPU task is live.
            let _permit = permit;
            work()
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
