use super::ExportError;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
#[derive(Clone)]
pub struct ExportWorkers(Arc<Semaphore>);
impl Default for ExportWorkers {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(2)))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/privacy_workers.rs"]
mod tests;
impl ExportWorkers {
    pub async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, ExportError> + Send + 'static,
    ) -> Result<T, ExportError> {
        let permit = self
            .0
            .clone()
            .try_acquire_owned()
            .map_err(|_| ExportError::Unavailable)?;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        });
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .map_err(|_| ExportError::Unavailable)?
            .map_err(|_| ExportError::Unavailable)?
    }
}
