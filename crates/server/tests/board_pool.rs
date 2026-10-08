use liar_core::{
    board::{Cell, CellId},
    generator::{BoardGenerator, GenerationBudget, certify},
    rules::RulesSnapshot,
    solver::SolverBudget,
};
use liar_protocol::online::OnlineError;
use liar_server::lobby::{BoardPool, BoardSource, OsSeedSource, SeedSource};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(2);

fn small_rules() -> RulesSnapshot {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    RulesSnapshot::from_rules(rules).unwrap()
}
struct CounterSeeds {
    calls: AtomicUsize,
    failures: usize,
    called_at: Mutex<Vec<std::time::Instant>>,
}
impl SeedSource for CounterSeeds {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.called_at
            .lock()
            .unwrap()
            .push(std::time::Instant::now());
        if call < self.failures {
            Err(OnlineError::Unavailable)
        } else {
            Ok((call as u64).to_le_bytes())
        }
    }
}
struct FixedSeed;
impl SeedSource for FixedSeed {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        Ok(42_u64.to_le_bytes())
    }
}
struct BlockingSeeds {
    calls: AtomicUsize,
    active: AtomicUsize,
    completed: AtomicUsize,
    open: Mutex<bool>,
    wake: Condvar,
}
impl SeedSource for BlockingSeeds {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_add(1, Ordering::SeqCst);
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.wake.wait(open).unwrap();
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok((call as u64).to_le_bytes())
    }
}
struct Gate(Arc<BlockingSeeds>);
impl Gate {
    fn new() -> Self {
        Self(Arc::new(BlockingSeeds {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            open: Mutex::new(false),
            wake: Condvar::new(),
        }))
    }
    fn release(&self) {
        *self.0.open.lock().unwrap() = true;
        self.0.wake.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
struct ReverseSeeds {
    calls: AtomicUsize,
    first: Arc<BlockingSeeds>,
    later: Arc<BlockingSeeds>,
}
impl SeedSource for ReverseSeeds {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            self.first.seed()?;
        } else if call >= 2 {
            self.later.seed()?;
        }
        Ok((call as u64).to_le_bytes())
    }
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded worker progress");
}

#[tokio::test]
async fn supplies_private_certified_default_board_with_replay_seed_and_snapshot() {
    let rules = RulesSnapshot::bundled();
    let pool = BoardPool::start(rules.clone(), 1, 1, Arc::new(FixedSeed)).unwrap();
    let prepared = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"));
    let (board, actual_rules, seed) = prepared.into_parts();
    assert_eq!(actual_rules, rules);
    assert_eq!(seed, 42_u64.to_le_bytes());
    assert_eq!(board.spec(), rules.rules.board_spec());
    assert_eq!(board.cell(board.spec().opening), Some(Cell::Number(0)));
    assert_eq!(
        (0..256)
            .filter(|&i| board.cell(CellId(i)) == Some(Cell::Mine))
            .count(),
        40
    );
    assert!(certify(&board, SolverBudget::default()).is_ok());
    let replay = BoardGenerator::generate(
        board.spec(),
        u64::from_le_bytes(seed),
        GenerationBudget::default(),
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    for i in 0..256 {
        assert_eq!(board.cell(CellId(i)), replay.board.cell(CellId(i)));
    }
}

#[tokio::test]
async fn cached_and_generating_total_is_bounded_and_consumption_refills() {
    let seeds = Arc::new(CounterSeeds {
        calls: AtomicUsize::new(0),
        failures: 0,
        called_at: Mutex::new(Vec::new()),
    });
    let pool = BoardPool::start(small_rules(), 2, 2, seeds.clone()).unwrap();
    eventually(|| seeds.calls.load(Ordering::SeqCst) == 2).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(seeds.calls.load(Ordering::SeqCst), 2);
    let (_, _, first) = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"))
        .into_parts();
    eventually(|| seeds.calls.load(Ordering::SeqCst) == 3).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(seeds.calls.load(Ordering::SeqCst), 3);
    let (_, _, second) = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"))
        .into_parts();
    assert_ne!(first, second);
}

#[tokio::test]
async fn blocked_generation_never_exceeds_worker_limit_and_take_has_deadline() {
    let gate = Gate::new();
    let pool = BoardPool::start(small_rules(), 4, 2, gate.0.clone()).unwrap();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 2).await;
    assert!(matches!(
        pool.take(Duration::from_millis(20)).await,
        Err(OnlineError::Unavailable)
    ));
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
    gate.release();
    assert!(pool.take(WAIT).await.is_ok());
}

#[tokio::test]
async fn dropping_pool_does_not_claim_to_cancel_actual_blocking_workers() {
    let gate = Gate::new();
    let pool = BoardPool::start(small_rules(), 4, 2, gate.0.clone()).unwrap();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 2).await;
    drop(pool);
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(gate.0.active.load(Ordering::SeqCst), 2);
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
    gate.release();
    eventually(|| gate.0.completed.load(Ordering::SeqCst) == 2).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn entropy_failure_releases_capacity_and_retries_without_busy_loop() {
    let seeds = Arc::new(CounterSeeds {
        calls: AtomicUsize::new(0),
        failures: 1,
        called_at: Mutex::new(Vec::new()),
    });
    let pool = BoardPool::start(small_rules(), 1, 1, seeds.clone()).unwrap();
    assert!(pool.take(WAIT).await.is_ok());
    assert!(seeds.calls.load(Ordering::SeqCst) >= 2);
    let called_at = seeds.called_at.lock().unwrap();
    assert!(called_at[1].duration_since(called_at[0]) >= Duration::from_millis(100));
}

#[tokio::test]
async fn invalid_snapshot_limits_and_take_budget_fail_closed() {
    let rules = small_rules();
    for (capacity, workers) in [(0, 1), (257, 1), (1, 0), (65, 65), (1, 2)] {
        assert!(matches!(
            BoardPool::start(rules.clone(), capacity, workers, Arc::new(FixedSeed)),
            Err(OnlineError::Capacity)
        ));
    }
    let mut tampered = rules.clone();
    tampered.hash.push('x');
    assert!(matches!(
        BoardPool::start(tampered, 1, 1, Arc::new(FixedSeed)),
        Err(OnlineError::Malformed)
    ));
    let pool = BoardPool::start(rules, 1, 1, Arc::new(FixedSeed)).unwrap();
    for wait in [Duration::ZERO, Duration::from_millis(2001)] {
        assert!(matches!(pool.take(wait).await, Err(OnlineError::Malformed)));
    }
}

#[test]
fn needs_running_runtime_and_os_entropy_is_available() {
    assert!(matches!(
        BoardPool::start(small_rules(), 1, 1, Arc::new(FixedSeed)),
        Err(OnlineError::Unavailable)
    ));
    assert!(OsSeedSource.seed().is_ok());
}

#[tokio::test]
async fn canceling_a_waiter_releases_receiver_and_does_not_consume_future_board() {
    let gate = Gate::new();
    let pool = Arc::new(BoardPool::start(small_rules(), 1, 1, gate.0.clone()).unwrap());
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    let waiting_pool = pool.clone();
    let waiter = tokio::spawn(async move { waiting_pool.take(WAIT).await });
    tokio::task::yield_now().await;
    assert!(matches!(
        pool.take(Duration::from_millis(20)).await,
        Err(OnlineError::Unavailable)
    ));
    waiter.abort();
    assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));
    gate.release();
    let (_, _, seed) = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"))
        .into_parts();
    assert_eq!(seed, 0_u64.to_le_bytes());
}

#[tokio::test]
async fn drop_before_worker_poll_prevents_any_entropy_or_generation() {
    let seeds = Arc::new(CounterSeeds {
        calls: AtomicUsize::new(0),
        failures: 0,
        called_at: Mutex::new(Vec::new()),
    });
    drop(BoardPool::start(small_rules(), 2, 2, seeds.clone()).unwrap());
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(seeds.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn persistent_entropy_failure_returns_unavailable_and_keeps_retries_bounded() {
    let seeds = Arc::new(CounterSeeds {
        calls: AtomicUsize::new(0),
        failures: usize::MAX,
        called_at: Mutex::new(Vec::new()),
    });
    let pool = BoardPool::start(small_rules(), 1, 1, seeds.clone()).unwrap();
    assert!(matches!(
        pool.take(Duration::from_millis(20)).await,
        Err(OnlineError::Unavailable)
    ));
    assert!(seeds.calls.load(Ordering::SeqCst) >= 1);
    drop(pool);
    tokio::time::sleep(Duration::from_millis(120)).await;
    let called_at = seeds.called_at.lock().unwrap();
    assert!(
        called_at
            .windows(2)
            .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_millis(100))
    );
}

#[tokio::test]
async fn reversed_generation_completion_keeps_each_board_bound_to_its_own_seed() {
    let first = Gate::new();
    let later = Gate::new();
    let seeds = Arc::new(ReverseSeeds {
        calls: AtomicUsize::new(0),
        first: first.0.clone(),
        later: later.0.clone(),
    });
    let pool = BoardPool::start(small_rules(), 2, 2, seeds.clone()).unwrap();
    eventually(|| seeds.calls.load(Ordering::SeqCst) == 2).await;
    let one = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"));
    first.release();
    let zero = pool
        .take(WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"));
    for (prepared, expected) in [(one, 1_u64), (zero, 0_u64)] {
        let (board, rules, seed) = prepared.into_parts();
        assert_eq!(seed, expected.to_le_bytes());
        let replay = BoardGenerator::generate(
            rules.rules.board_spec(),
            expected,
            GenerationBudget::default(),
        )
        .unwrap_or_else(|error| panic!("{error:?}"));
        for i in 0..9 {
            assert_eq!(board.cell(CellId(i)), replay.board.cell(CellId(i)));
        }
    }
}
