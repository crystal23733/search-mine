use super::*;
use crate::daily::{NativeReplayVerifier, PublishedDaily};
use liar_core::rules::RulesSnapshot;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Notify;

fn definition() -> Arc<PublishedDaily> {
    Arc::new(PublishedDaily::new("2026-10-07", 1, RulesSnapshot::bundled()).unwrap())
}
fn golden() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native-wasm.json"
    )))
    .unwrap()["daily"]["replay"]
        .to_string()
}
fn evidence() -> ReplayEvidence {
    NativeReplayVerifier
        .verify(&definition(), &golden())
        .unwrap()
}
#[tokio::test]
async fn bounded_native_worker_returns_the_actual_completion_evidence() {
    assert_eq!(
        BoundedReplayVerifier::new(Arc::new(NativeReplayVerifier))
            .verify(definition(), &golden())
            .await,
        Ok(evidence())
    );
}

struct Gate {
    released: Mutex<bool>,
    changed: Condvar,
    calls: AtomicUsize,
    entered: Notify,
    output: ReplayEvidence,
}
impl Gate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            changed: Condvar::new(),
            calls: AtomicUsize::new(0),
            entered: Notify::new(),
            output: evidence(),
        })
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
    async fn wait_for_two_workers(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while self.calls.load(Ordering::SeqCst) < 2 {
                self.entered.notified().await;
            }
        })
        .await
        .expect("both blocking workers must enter");
    }
}
impl ReplayVerifier for Gate {
    fn verify(&self, _: &PublishedDaily, _: &str) -> Result<ReplayEvidence, ReplayError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        let _guard = self
            .changed
            .wait_while(self.released.lock().unwrap(), |released| !*released)
            .unwrap();
        Ok(self.output.clone())
    }
}
struct ReleaseOnDrop(Arc<Gate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
fn start(
    verifier: &BoundedReplayVerifier,
) -> tokio::task::JoinHandle<Result<ReplayEvidence, ReplayError>> {
    let verifier = verifier.clone();
    tokio::spawn(async move { verifier.verify(definition(), &golden()).await })
}
async fn succeeds_after_workers_exit(verifier: &BoundedReplayVerifier) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            match verifier.verify(definition(), &golden()).await {
                Err(ReplayError::Busy) => tokio::task::yield_now().await,
                value => {
                    assert!(value.is_ok());
                    break;
                }
            }
        }
    })
    .await
    .expect("worker exit must release capacity");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_timeout_does_not_release_live_worker_permits_or_publish_late_success() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(gate.clone());
    let verifier = BoundedReplayVerifier::new(gate.clone());
    let a = start(&verifier);
    let b = start(&verifier);
    gate.wait_for_two_workers().await;
    assert_eq!(
        verifier.verify(definition(), &golden()).await,
        Err(ReplayError::Busy)
    );
    let (a, b) = tokio::join!(a, b);
    assert_eq!(a.unwrap(), Err(ReplayError::Timeout));
    assert_eq!(b.unwrap(), Err(ReplayError::Timeout));
    assert_eq!(
        verifier.verify(definition(), &golden()).await,
        Err(ReplayError::Busy)
    );
    assert_eq!(gate.calls.load(Ordering::SeqCst), 2);
    gate.release();
    succeeds_after_workers_exit(&verifier).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_callers_keeps_capacity_until_actual_blocking_exit() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(gate.clone());
    let verifier = BoundedReplayVerifier::new(gate.clone());
    let a = start(&verifier);
    let b = start(&verifier);
    gate.wait_for_two_workers().await;
    a.abort();
    b.abort();
    assert!(a.await.unwrap_err().is_cancelled());
    assert!(b.await.unwrap_err().is_cancelled());
    assert_eq!(
        verifier.verify(definition(), &golden()).await,
        Err(ReplayError::Busy)
    );
    gate.release();
    succeeds_after_workers_exit(&verifier).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_utf8_is_rejected_before_busy_admission_or_invoking_the_port() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(gate.clone());
    let verifier = BoundedReplayVerifier::new(gate.clone());
    let a = start(&verifier);
    let b = start(&verifier);
    gate.wait_for_two_workers().await;
    let large = "é".repeat(liar_protocol::daily::MAX_DAILY_REPLAY_BYTES / 2 + 1);
    assert_eq!(
        verifier.verify(definition(), &large).await,
        Err(ReplayError::Malformed)
    );
    assert_eq!(gate.calls.load(Ordering::SeqCst), 2);
    gate.release();
    assert!(a.await.unwrap().is_ok());
    assert!(b.await.unwrap().is_ok());
}
struct PanicOnce {
    calls: AtomicUsize,
    output: ReplayEvidence,
}
impl ReplayVerifier for PanicOnce {
    fn verify(&self, _: &PublishedDaily, _: &str) -> Result<ReplayEvidence, ReplayError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("injected verifier panic");
        }
        Ok(self.output.clone())
    }
}
#[tokio::test]
async fn panic_is_unavailable_and_releases_worker_capacity() {
    let verifier = BoundedReplayVerifier::new(Arc::new(PanicOnce {
        calls: AtomicUsize::new(0),
        output: evidence(),
    }));
    assert_eq!(
        verifier.verify(definition(), &golden()).await,
        Err(ReplayError::Unavailable)
    );
    assert!(verifier.verify(definition(), &golden()).await.is_ok());
}
struct Reject;
impl ReplayVerifier for Reject {
    fn verify(&self, _: &PublishedDaily, _: &str) -> Result<ReplayEvidence, ReplayError> {
        Err(ReplayError::MetadataMismatch)
    }
}
#[tokio::test]
async fn worker_validation_errors_are_preserved() {
    let verifier = BoundedReplayVerifier::new(Arc::new(Reject));
    assert_eq!(
        verifier.verify(definition(), &golden()).await,
        Err(ReplayError::MetadataMismatch)
    );
}

struct Reordered {
    slow: Arc<Gate>,
}
impl ReplayVerifier for Reordered {
    fn verify(
        &self,
        definition: &PublishedDaily,
        source: &str,
    ) -> Result<ReplayEvidence, ReplayError> {
        if definition.metadata().date == "2026-10-07" {
            self.slow.verify(definition, source)?;
            Err(ReplayError::Incomplete)
        } else {
            Err(ReplayError::MetadataMismatch)
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overlapping_definitions_keep_their_own_results_when_completion_is_reversed() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(gate.clone());
    let verifier = BoundedReplayVerifier::new(Arc::new(Reordered { slow: gate.clone() }));
    let slow = start(&verifier);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while gate.calls.load(Ordering::SeqCst) == 0 {
            gate.entered.notified().await;
        }
    })
    .await
    .unwrap();
    let other = Arc::new(PublishedDaily::new("2026-10-08", 1, RulesSnapshot::bundled()).unwrap());
    assert_eq!(
        verifier.verify(other, "{}").await,
        Err(ReplayError::MetadataMismatch)
    );
    assert!(!slow.is_finished());
    gate.release();
    assert_eq!(slow.await.unwrap(), Err(ReplayError::Incomplete));
}
