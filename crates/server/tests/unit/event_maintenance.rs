use super::*;
use crate::events::{RetentionBatch, RetentionFuture};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

type Reply = (Duration, Result<RetentionBatch, EventStoreError>);
struct Store {
    replies: Vec<Reply>,
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    budgets: Mutex<Vec<Duration>>,
}
impl Store {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies,
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            budgets: Mutex::new(Vec::new()),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl EventRetentionStore for Store {
    fn reap(&self, budget: Duration) -> RetentionFuture<'_> {
        Box::pin(async move {
            let index = self.calls.fetch_add(1, Ordering::SeqCst);
            self.budgets.lock().unwrap().push(budget);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            let _active = Active(&self.active);
            let (delay, result) = self.replies[index.min(self.replies.len() - 1)];
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            result
        })
    }
}
fn batch(deleted: u16, has_more: bool) -> Reply {
    (Duration::ZERO, Ok(RetentionBatch { deleted, has_more }))
}
#[tokio::test(start_paused = true)]
async fn cycle_caps_eight_batches_and_stops_when_complete_or_no_progress() {
    for (reply, expected) in [
        (
            batch(500, true),
            RetentionCycle {
                deleted: 4000,
                has_more: true,
                batches: 8,
            },
        ),
        (
            batch(7, false),
            RetentionCycle {
                deleted: 7,
                has_more: false,
                batches: 1,
            },
        ),
        (
            batch(0, true),
            RetentionCycle {
                deleted: 0,
                has_more: true,
                batches: 1,
            },
        ),
    ] {
        let store = Store::new(vec![reply]);
        assert_eq!(
            EventRetentionMaintenance::new(store.clone())
                .run_cycle()
                .await,
            Ok(expected)
        );
        assert_eq!(store.calls(), usize::from(expected.batches));
        assert_eq!(store.max_active.load(Ordering::SeqCst), 1);
        assert!(
            store
                .budgets
                .lock()
                .unwrap()
                .iter()
                .all(|b| !b.is_zero() && *b <= Duration::from_secs(2))
        );
    }
}
#[tokio::test(start_paused = true)]
async fn cycle_shares_the_two_second_budget_and_cancels_pending_work() {
    let store = Store::new(vec![(
        Duration::from_millis(1500),
        Ok(RetentionBatch {
            deleted: 500,
            has_more: true,
        }),
    )]);
    assert_eq!(
        EventRetentionMaintenance::new(store.clone())
            .run_cycle()
            .await,
        Err(EventStoreError::Unavailable)
    );
    assert_eq!(store.calls(), 2);
    assert_eq!(store.active.load(Ordering::SeqCst), 0);
    assert_eq!(
        store.budgets.lock().unwrap().as_slice(),
        &[Duration::from_secs(2), Duration::from_millis(500)]
    );
}
#[tokio::test(start_paused = true)]
async fn partial_failure_and_invalid_port_counts_never_become_zero_success() {
    for (reply, error) in [
        (
            (Duration::ZERO, Err(EventStoreError::Invalid)),
            EventStoreError::Invalid,
        ),
        (batch(501, true), EventStoreError::Unavailable),
    ] {
        let store = Store::new(vec![batch(500, true), reply]);
        assert_eq!(
            EventRetentionMaintenance::new(store.clone())
                .run_cycle()
                .await,
            Err(error)
        );
        assert_eq!(store.calls(), 2);
    }
}
#[tokio::test(start_paused = true)]
async fn worker_runs_immediately_then_every_minute_and_skips_missed_ticks() {
    let store = Store::new(vec![
        (Duration::ZERO, Err(EventStoreError::Invalid)),
        batch(0, false),
    ]);
    let task = tokio::spawn(EventRetentionMaintenance::new(store.clone()).run());
    tokio::task::yield_now().await;
    assert_eq!(store.calls(), 1);
    tokio::time::advance(Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(store.calls(), 1);
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(store.calls(), 2);
    tokio::time::advance(Duration::from_secs(180)).await;
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    assert_eq!(store.calls(), 3);
    assert_eq!(store.max_active.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}
