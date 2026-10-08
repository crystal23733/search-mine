use liar_core::{
    board::{Cell, CellId},
    bot::Difficulty,
    generator::{BoardGenerator, GenerationBudget},
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::PublicEndReason,
    online::{OnlineError, OnlinePayload},
};
use liar_server::{auth::AuthClock, lobby::*, online::*};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

struct Clock(AtomicU64);
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        18000
    }
}
#[derive(Default)]
struct Results(Mutex<Vec<FinishedMatch>>);
impl ResultRepository for Results {
    fn save(&self, result: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(result);
            Ok(SaveResult::Saved)
        })
    }
}
struct FixedSeed(u64);
impl SeedSource for FixedSeed {
    fn seed(&self) -> Result<[u8; 8], OnlineError> {
        Ok(self.0.to_le_bytes())
    }
}
struct Codes;
impl RoomCodeSource for Codes {
    fn code(&self) -> Result<RoomCode, OnlineError> {
        Ok(RoomCode::parse("ABCD2345").unwrap())
    }
}
fn rules() -> RulesSnapshot {
    let mut r = RulesSnapshot::bundled().rules;
    r.width = 3;
    r.height = 3;
    r.mines = 2;
    RulesSnapshot::from_rules(r).unwrap()
}
fn boards() -> Arc<BoardPool> {
    let r = rules();
    let seed = (0..64)
        .find(|&seed| {
            BoardGenerator::generate(r.rules.board_spec(), seed, GenerationBudget::default())
                .is_ok_and(|g| {
                    g.board.cell(CellId(5)) == Some(Cell::Mine)
                        && g.board.cell(CellId(7)) == Some(Cell::Mine)
                })
        })
        .expect("stable certified fixture seed");
    Arc::new(BoardPool::start(r, 2, 2, Arc::new(FixedSeed(seed))).unwrap())
}
struct Control {
    calls: AtomicUsize,
    active: AtomicUsize,
    open: Mutex<bool>,
    wake: Condvar,
    failure: bool,
}
struct Preparer(Arc<Control>);
impl MatchPreparer for Preparer {
    fn prepare(&self, board: PreparedBoard) -> Result<PreparedMatch, OnlineError> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.active.fetch_add(1, Ordering::SeqCst);
        let mut open = self.0.open.lock().unwrap();
        while !*open {
            open = self.0.wake.wait(open).unwrap();
        }
        drop(open);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        if self.0.failure {
            Err(OnlineError::Unavailable)
        } else {
            CoreMatchPreparer.prepare(board)
        }
    }
}
struct Gate(Arc<Control>);
impl Gate {
    fn new(open: bool, failure: bool) -> Self {
        Self(Arc::new(Control {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            open: Mutex::new(open),
            wake: Condvar::new(),
            failure,
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
struct Fixture {
    clock: Arc<Clock>,
    results: Arc<Results>,
    authorities: Arc<AuthorityRegistry>,
    registry: Arc<MatchRegistry>,
    service: Arc<LobbyService>,
}
impl Fixture {
    fn new(gate: &Gate, capacity: usize, matches: usize, workers: usize) -> Self {
        let clock = Arc::new(Clock(AtomicU64::new(0)));
        let results = Arc::new(Results::default());
        let authorities = AuthorityRegistry::new(16).unwrap();
        let registry = MatchRegistry::new(
            MatchLimits {
                matches,
                mailbox: 16,
                outgoing: 32,
                proof_workers: 2,
            },
            clock.clone(),
            clock.clone(),
            authorities.clone(),
            results.clone(),
        )
        .unwrap();
        let service = LobbyService::new(
            LobbyLimits { capacity, workers },
            boards(),
            Arc::new(Preparer(gate.0.clone())),
            Arc::new(Codes),
            registry.clone(),
            BotExecutor::new(2, Arc::new(CoreBotFactory)).unwrap(),
        )
        .unwrap();
        Self {
            clock,
            results,
            authorities,
            registry,
            service,
        }
    }
    async fn connect(&self, account: Uuid) -> MatchConnection {
        let handle = self.registry.for_account(account).unwrap();
        let lease = self
            .authorities
            .bind(
                self.authorities.generation().unwrap(),
                account,
                [1; 32],
                20000,
                18000,
            )
            .unwrap();
        handle.connect(lease).await.unwrap()
    }
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded service progress");
}
async fn matched(service: &LobbyService, account: Uuid) -> LobbyView {
    eventually(|| matches!(service.status(account), Ok(LobbyView::Matched { .. }))).await;
    service.status(account).unwrap()
}
fn pair(f: &Fixture, a: Uuid, b: Uuid) {
    f.service.join(a, Difficulty::Normal).unwrap();
    f.service.join(b, Difficulty::Hard).unwrap();
}
fn room(f: &Fixture, a: Uuid, b: Uuid) -> Uuid {
    let LobbyView::Waiting(LobbyStatus::Room { id, code, .. }) = f.service.create_room(a).unwrap()
    else {
        panic!("room")
    };
    f.service.join_room(b, code).unwrap();
    f.service.ready(a, id, true).unwrap();
    f.service.ready(b, id, true).unwrap();
    id
}

#[tokio::test]
async fn actual_human_pair_gets_same_board_and_countdown_only_at_ready_admission() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    f.clock.0.store(1200, Ordering::SeqCst);
    pair(&f, a, b);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    f.clock.0.store(3500, Ordering::SeqCst);
    gate.release();
    let first = matched(&f.service, a).await;
    let second = matched(&f.service, b).await;
    assert!(matches!(
        first,
        LobbyView::Matched {
            opponent: OpponentKind::Human,
            own_seat: 0,
            ..
        }
    ));
    assert!(matches!(
        second,
        LobbyView::Matched {
            opponent: OpponentKind::Human,
            own_seat: 1,
            ..
        }
    ));
    assert_eq!(
        f.registry.for_account(a).unwrap().id(),
        f.registry.for_account(b).unwrap().id()
    );
    let mut one = f.connect(a).await;
    let mut two = f.connect(b).await;
    let OnlinePayload::Snapshot { view: one, .. } = one.next().await.unwrap().payload else {
        panic!("snapshot")
    };
    let OnlinePayload::Snapshot { view: two, .. } = two.next().await.unwrap().payload else {
        panic!("snapshot")
    };
    assert_eq!(one.own.cells, two.own.cells);
    assert_eq!(one.countdown_ms, 3000);
    assert_eq!(two.countdown_ms, 3000);
    assert!(matches!(
        f.service.join(a, Difficulty::Easy),
        Err(LobbyServiceError::Policy(LobbyError::Busy))
    ));
}

#[tokio::test]
async fn ten_second_fallback_starts_a_real_explicit_core_bot() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::new_v4();
    let queued = f.service.join(a, Difficulty::Hard).unwrap();
    assert_eq!(f.service.join(a, Difficulty::Hard).unwrap(), queued);
    f.clock.0.store(9999, Ordering::SeqCst);
    assert!(matches!(
        f.service.status(a),
        Ok(LobbyView::Waiting(LobbyStatus::Queued { .. }))
    ));
    f.clock.0.store(10000, Ordering::SeqCst);
    assert!(matches!(
        matched(&f.service, a).await,
        LobbyView::Matched {
            opponent: OpponentKind::Bot,
            ..
        }
    ));
    let mut connection = f.connect(a).await;
    connection.next().await.unwrap();
    for (at, opened) in [(13000, 5), (13180, 6), (13360, 7)] {
        f.clock.0.store(at, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let event = connection.next().await.unwrap();
                let view = match event.payload {
                    OnlinePayload::Delta { view } | OnlinePayload::MatchEnd { view, .. } => {
                        Some(view)
                    }
                    _ => None,
                };
                if view.is_some_and(|view| view.opponent.opened_safe == opened) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    assert_eq!(
        f.results.0.lock().unwrap()[0].reason,
        PublicEndReason::Clear
    );
    assert_eq!(f.results.0.lock().unwrap()[0].players[1].account, None);
}

#[tokio::test]
async fn canceled_blocking_preparation_does_not_start_old_match_or_release_worker_early() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    let old = f.service.status(a).unwrap().identity().unwrap();
    f.service.cancel(a, old).unwrap();
    f.service.join(c, Difficulty::Easy).unwrap();
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(gate.0.active.load(Ordering::SeqCst), 1);
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    gate.release();
    matched(&f.service, b).await;
    matched(&f.service, c).await;
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
    assert_eq!(
        f.registry.for_account(b).unwrap().id(),
        f.registry.for_account(c).unwrap().id()
    );
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn room_unready_and_new_generation_discard_old_preparation() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let id = room(&f, a, b);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    let old = f.service.status(a).unwrap().identity().unwrap();
    f.service.ready(a, id, false).unwrap();
    f.service.ready(a, id, true).unwrap();
    let new = f.service.status(a).unwrap().identity().unwrap();
    assert_ne!(old, new);
    gate.release();
    matched(&f.service, a).await;
    matched(&f.service, b).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        f.registry.for_account(a).unwrap().id(),
        f.registry.for_account(b).unwrap().id()
    );
}

#[tokio::test]
async fn prep_and_near_expiry_room_finish_without_starting_a_late_match() {
    for is_room in [false, true] {
        let gate = Gate::new(false, false);
        let f = Fixture::new(&gate, 2, 1, 1);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        if is_room {
            let LobbyView::Waiting(LobbyStatus::Room { id, code, .. }) =
                f.service.create_room(a).unwrap()
            else {
                panic!("room")
            };
            f.service.join_room(b, code).unwrap();
            f.clock.0.store(599999, Ordering::SeqCst);
            f.service.ready(a, id, true).unwrap();
            f.service.ready(b, id, true).unwrap();
        } else {
            pair(&f, a, b);
        }
        eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
        f.clock
            .0
            .store(if is_room { 600000 } else { 5000 }, Ordering::SeqCst);
        assert_eq!(f.service.status(a).unwrap(), LobbyView::Idle);
        gate.release();
        eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(matches!(
            f.registry.for_account(a),
            Err(OnlineError::NotMatched)
        ));
        assert!(matches!(
            f.registry.for_account(b),
            Err(OnlineError::NotMatched)
        ));
    }
}

#[tokio::test]
async fn failure_is_terminal_and_a_new_membership_wins_over_old_error() {
    let gate = Gate::new(true, true);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| f.service.status(a) == Ok(LobbyView::Failed(OnlineError::Unavailable))).await;
    assert_eq!(
        f.service.status(b).unwrap(),
        LobbyView::Failed(OnlineError::Unavailable)
    );
    assert!(matches!(
        f.service.create_room(a),
        Ok(LobbyView::Waiting(LobbyStatus::Room { .. }))
    ));
    f.clock.0.store(30000, Ordering::SeqCst);
    assert_eq!(f.service.status(b).unwrap(), LobbyView::Idle);
    assert!(matches!(
        f.service.status(a),
        Ok(LobbyView::Waiting(LobbyStatus::Room { .. }))
    ));
}

#[tokio::test]
async fn registry_capacity_failure_does_not_occupy_accounts_or_change_active_match() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 4, 1, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();
    pair(&f, a, b);
    let old = matched(&f.service, a).await;
    pair(&f, c, d);
    eventually(|| f.service.status(c) == Ok(LobbyView::Failed(OnlineError::Capacity))).await;
    assert_eq!(f.service.status(a).unwrap(), old);
    assert!(matches!(
        f.registry.for_account(c),
        Err(OnlineError::NotMatched)
    ));
    assert!(matches!(
        f.service.join(c, Difficulty::Easy),
        Ok(LobbyView::Waiting(LobbyStatus::Queued { .. }))
    ));
}

#[tokio::test]
async fn stale_cancel_cannot_remove_a_new_ticket() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::new_v4();
    let old = f
        .service
        .join(a, Difficulty::Easy)
        .unwrap()
        .identity()
        .unwrap();
    f.service.cancel(a, old).unwrap();
    let new = f.service.join(a, Difficulty::Easy).unwrap();
    assert!(matches!(
        f.service.cancel(a, old),
        Err(LobbyServiceError::Policy(LobbyError::Stale))
    ));
    assert_eq!(f.service.status(a).unwrap(), new);
}

#[tokio::test]
async fn dropping_service_during_actual_preparation_never_creates_a_match() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    let weak = Arc::downgrade(&f.service);
    let registry = f.registry.clone();
    drop(f);
    assert!(weak.upgrade().is_none());
    assert_eq!(gate.0.active.load(Ordering::SeqCst), 1);
    gate.release();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(matches!(
        registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
}

#[tokio::test]
async fn bounded_failure_history_evicts_older_entries_even_at_the_same_timestamp() {
    let gate = Gate::new(true, true);
    let f = Fixture::new(&gate, 2, 1, 1);
    let a = Uuid::from_u128(100);
    let b = Uuid::from_u128(200);
    let c = Uuid::from_u128(1);
    let d = Uuid::from_u128(2);
    pair(&f, a, b);
    eventually(|| f.service.status(a) == Ok(LobbyView::Failed(OnlineError::Unavailable))).await;
    pair(&f, c, d);
    eventually(|| gate.0.calls.load(Ordering::SeqCst) == 2).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        f.service.status(c).unwrap(),
        LobbyView::Failed(OnlineError::Unavailable)
    );
    assert_eq!(
        f.service.status(d).unwrap(),
        LobbyView::Failed(OnlineError::Unavailable)
    );
    assert_eq!(f.service.status(a).unwrap(), LobbyView::Idle);
    assert_eq!(f.service.status(b).unwrap(), LobbyView::Idle);
}

#[derive(Clone, Copy)]
enum SourceFault {
    Unavailable,
    Panic,
    Pending,
}
struct BrokenBoards {
    fault: SourceFault,
    calls: AtomicUsize,
    dropped: AtomicUsize,
}
struct DropCount<'a>(&'a AtomicUsize);
impl Drop for DropCount<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl BoardSource for BrokenBoards {
    fn take(&self, _: Duration) -> PortFuture<'_, Result<PreparedBoard, OnlineError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _drop = DropCount(&self.dropped);
            match self.fault {
                SourceFault::Unavailable => Err(OnlineError::Unavailable),
                SourceFault::Panic => panic!("controlled board source panic"),
                SourceFault::Pending => std::future::pending().await,
            }
        })
    }
}
impl BrokenBoards {
    fn new(fault: SourceFault) -> Arc<Self> {
        Arc::new(Self {
            fault,
            calls: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        })
    }
}
fn replace_ports(f: &mut Fixture, boards: Arc<dyn BoardSource>, codes: Arc<dyn RoomCodeSource>) {
    f.service = LobbyService::new(
        LobbyLimits {
            capacity: 2,
            workers: 1,
        },
        boards,
        Arc::new(CoreMatchPreparer),
        codes,
        f.registry.clone(),
        BotExecutor::new(2, Arc::new(CoreBotFactory)).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn board_source_error_or_panic_is_terminal_and_never_assigns_accounts() {
    for fault in [SourceFault::Unavailable, SourceFault::Panic] {
        let gate = Gate::new(true, false);
        let mut f = Fixture::new(&gate, 2, 1, 1);
        let source = BrokenBoards::new(fault);
        replace_ports(&mut f, source.clone(), Arc::new(Codes));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        pair(&f, a, b);
        eventually(|| f.service.status(a) == Ok(LobbyView::Failed(OnlineError::Unavailable))).await;
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(source.dropped.load(Ordering::SeqCst), 1);
        assert!(matches!(
            f.registry.for_account(a),
            Err(OnlineError::NotMatched)
        ));
    }
}

#[tokio::test]
async fn a_source_ignoring_its_deadline_is_timed_out_without_holding_lobby_lock() {
    let gate = Gate::new(true, false);
    let mut f = Fixture::new(&gate, 2, 1, 1);
    let source = BrokenBoards::new(SourceFault::Pending);
    replace_ports(&mut f, source.clone(), Arc::new(Codes));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| source.calls.load(Ordering::SeqCst) == 1).await;
    assert!(matches!(
        f.service.status(a),
        Ok(LobbyView::Waiting(LobbyStatus::Preparing { .. }))
    ));
    tokio::time::timeout(Duration::from_secs(3), async {
        while f.service.status(a) != Ok(LobbyView::Failed(OnlineError::Unavailable)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("application owns the source deadline");
    assert_eq!(source.dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
}

#[tokio::test]
async fn canceling_waiting_take_releases_slot_for_a_new_generation() {
    let gate = Gate::new(true, false);
    let mut f = Fixture::new(&gate, 4, 2, 1);
    let source = BrokenBoards::new(SourceFault::Pending);
    replace_ports(&mut f, source.clone(), Arc::new(Codes));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| source.calls.load(Ordering::SeqCst) == 1).await;
    let identity = f.service.status(a).unwrap().identity().unwrap();
    f.service.cancel(a, identity).unwrap();
    eventually(|| source.dropped.load(Ordering::SeqCst) == 1).await;
    let c = Uuid::new_v4();
    f.service.join(c, Difficulty::Hard).unwrap();
    eventually(|| source.calls.load(Ordering::SeqCst) == 2).await;
    assert!(matches!(
        f.service.status(b),
        Ok(LobbyView::Waiting(LobbyStatus::Preparing { .. }))
    ));
    assert_eq!(f.service.status(a).unwrap(), LobbyView::Idle);
}

struct RepeatedCodes(AtomicUsize);
impl RoomCodeSource for RepeatedCodes {
    fn code(&self) -> Result<RoomCode, OnlineError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Codes.code()
    }
}
#[tokio::test]
async fn room_code_collision_has_bounded_retries_and_create_is_idempotent() {
    let gate = Gate::new(true, false);
    let mut f = Fixture::new(&gate, 2, 1, 1);
    let codes = Arc::new(RepeatedCodes(AtomicUsize::new(0)));
    replace_ports(&mut f, boards(), codes.clone());
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let first = f.service.create_room(a).unwrap();
    assert_eq!(f.service.create_room(a).unwrap(), first);
    assert_eq!(codes.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        f.service.create_room(b),
        Err(LobbyServiceError::Policy(LobbyError::Collision))
    ));
    assert_eq!(codes.0.load(Ordering::SeqCst), 9);
    assert_eq!(f.service.status(a).unwrap(), first);
    assert_eq!(f.service.status(b).unwrap(), LobbyView::Idle);
    assert!(RoomCode::parse(OsRoomCodeSource.code().unwrap().as_str()).is_ok());
}

#[tokio::test]
async fn invalid_limits_identity_and_clock_fail_closed() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    for (capacity, workers) in [(0, 1), (4097, 1), (1, 0), (65, 65), (1, 2)] {
        assert!(matches!(
            LobbyService::new(
                LobbyLimits { capacity, workers },
                BrokenBoards::new(SourceFault::Unavailable),
                Arc::new(CoreMatchPreparer),
                Arc::new(Codes),
                f.registry.clone(),
                BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap()
            ),
            Err(LobbyServiceError::Online(OnlineError::Capacity))
        ));
    }
    assert_eq!(
        f.service.status(Uuid::nil()),
        Err(LobbyServiceError::Online(OnlineError::Malformed))
    );
    assert_eq!(
        f.service.join(Uuid::nil(), Difficulty::Hard),
        Err(LobbyServiceError::Online(OnlineError::Malformed))
    );
    let account = Uuid::new_v4();
    f.clock.0.store(100, Ordering::SeqCst);
    assert_eq!(f.service.status(account).unwrap(), LobbyView::Idle);
    f.clock.0.store(99, Ordering::SeqCst);
    assert_eq!(
        f.service.status(account),
        Err(LobbyServiceError::Policy(LobbyError::InvalidTime))
    );
}
