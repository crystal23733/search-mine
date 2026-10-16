use super::*;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Clock;
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        UtcDay::parse("2026-10-21").unwrap().start() + 3600
    }
}
#[derive(Default)]
struct Store {
    calls: Mutex<Vec<(MetricKind, UtcDay, Duration)>>,
    delay: Duration,
    active: AtomicUsize,
    maximum: AtomicUsize,
    response: Option<AggregateReport>,
}
struct Lease<'a>(&'a Store);
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
impl AggregateStore for Store {
    fn refresh(
        &self,
        kind: MetricKind,
        period: UtcDay,
        allowance: Duration,
    ) -> MetricFuture<'_, AggregateReport> {
        self.calls.lock().unwrap().push((kind, period, allowance));
        Box::pin(async move {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            let _lease = Lease(self);
            tokio::time::sleep(self.delay).await;
            self.response.clone().ok_or(EventError::Incomplete.into())
        })
    }
    fn read(
        &self,
        _kind: MetricKind,
        _period: UtcDay,
    ) -> MetricFuture<'_, Option<AggregateReport>> {
        Box::pin(async { Err(MetricReadError::Unavailable) })
    }
}
#[tokio::test(start_paused = true)]
async fn an_incomplete_current_week_does_not_block_the_rotating_report() {
    let store = Arc::new(Store::default());
    let mut job = AggregationMaintenance::new(store.clone(), Arc::new(Clock));
    let result = job.run_cycle().await;
    let calls = store.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "Both independent report requests must be attempted"
    );
    assert_eq!(calls[0].0, MetricKind::WeeklyNsm);
    assert_eq!(calls[0].1, UtcDay::parse("2026-10-19").unwrap());
    assert_eq!(calls[1].1, UtcDay::parse("2026-10-12").unwrap());
    assert!(calls.iter().all(|c| c.2 <= Duration::from_secs(2)));
    let result = result.unwrap();
    assert_eq!(result.current, Err(EventError::Incomplete.into()));
    assert_eq!(result.rotating, Err(EventError::Incomplete.into()));
}

#[tokio::test(start_paused = true)]
async fn cycles_rotate_once_per_minute_skip_missed_ticks_and_never_overlap() {
    let store = Arc::new(Store::default());
    let job = AggregationMaintenance::new(store.clone(), Arc::new(Clock));
    let task = tokio::spawn(job.run());
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    assert_eq!(store.calls.lock().unwrap().len(), 2);
    tokio::time::advance(Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(store.calls.lock().unwrap().len(), 2);
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    assert_eq!(store.calls.lock().unwrap().len(), 4);
    tokio::time::advance(Duration::from_secs(180)).await;
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    assert_eq!(store.calls.lock().unwrap().len(), 6);
    assert_eq!(store.maximum.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let mut job = AggregationMaintenance::new(store.clone(), Arc::new(Clock));
    for _ in 0..24 {
        job.run_cycle().await.unwrap();
    }
    let calls = store.calls.lock().unwrap();
    assert_eq!(calls[7].1, calls[53].1); // The twenty-fourth cycle wraps to the previous week.
}
#[tokio::test(start_paused = true)]
async fn four_second_cycle_and_two_second_report_budgets_cancel_actual_work() {
    let store = Arc::new(Store {
        delay: Duration::from_secs(10),
        ..Store::default()
    });
    let mut job = AggregationMaintenance::new(store.clone(), Arc::new(Clock));
    let started = Instant::now();
    match job.run_cycle().await {
        Ok(cycle) => {
            assert_eq!(cycle.current, Err(MetricReadError::Unavailable));
            assert_eq!(cycle.rotating, Err(MetricReadError::Unavailable));
        }
        Err(error) => assert_eq!(error, MetricReadError::Unavailable),
    }
    assert_eq!(started.elapsed(), Duration::from_secs(4));
    assert_eq!(store.active.load(Ordering::SeqCst), 0);
    assert_eq!(store.maximum.load(Ordering::SeqCst), 1);
    let calls = store.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|c| !c.2.is_zero() && c.2 <= Duration::from_secs(2))
    );
}
#[tokio::test(start_paused = true)]
async fn a_report_for_another_period_is_rejected_and_invalid_clock_starts_no_work() {
    let week = UtcDay::parse("2026-10-12").unwrap();
    let at = Clock.now();
    let response = AggregateReport::new(
        MetricSnapshot::WeeklyNsm(WeeklyNsm {
            week,
            observed_through: week.start() + 7 * 86400,
            complete: true,
            participants: 0,
            repeat_accounts: 0,
            human_participations: 0,
            bot_participations: 0,
        }),
        at,
    )
    .unwrap();
    let store = Arc::new(Store {
        response: Some(response),
        ..Store::default()
    });
    let mut job = AggregationMaintenance::new(store.clone(), Arc::new(Clock));
    let cycle = job.run_cycle().await.unwrap();
    assert_eq!(cycle.current, Err(EventError::Conflict.into()));
    assert!(cycle.rotating.is_ok());
    struct BadClock;
    impl AuthClock for BadClock {
        fn now(&self) -> i64 {
            -1
        }
    }
    let store = Arc::new(Store::default());
    let mut job = AggregationMaintenance::new(store.clone(), Arc::new(BadClock));
    assert!(job.run_cycle().await.is_err());
    assert!(store.calls.lock().unwrap().is_empty());
}
