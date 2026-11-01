use liar_protocol::me::MeError;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
#[derive(Clone)]
pub struct MeWorkers(Arc<Semaphore>);
impl Default for MeWorkers {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(2)))
    }
}
impl MeWorkers {
    pub async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, MeError> + Send + 'static,
    ) -> Result<T, MeError> {
        let permit = self
            .0
            .clone()
            .try_acquire_owned()
            .map_err(|_| MeError::Unavailable)?;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        });
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .map_err(|_| MeError::Unavailable)?
            .map_err(|_| MeError::Unavailable)?
    }
}

#[cfg(test)]
#[path = "../../tests/unit/me_workers.rs"]
mod tests;
