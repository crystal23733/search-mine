use liar_core::{
    board::{Board, CellId},
    bot::Difficulty,
    game::{Action, Projection, RuleEngine},
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{AckStatus, GameView, PublicAction, PublicEndReason},
    online::{OnlineError, OnlineEvent, OnlineInput, OnlinePayload},
};
use liar_server::{auth::AuthClock, online::*};
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
fn state(account: Uuid, human_pair: bool) -> MatchState {
    state_at(account, human_pair, Uuid::new_v4(), 0)
}
fn state_at(account: Uuid, human_pair: bool, id: Uuid, at: u64) -> MatchState {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    MatchState::new(
        id,
        RuleEngine::new(board, rules, at).unwrap(),
        [Some(account), human_pair.then(Uuid::new_v4)],
        [46; 8],
        at,
    )
    .unwrap()
}
struct Fixture {
    clock: Arc<Clock>,
    results: Arc<Results>,
    authority: Arc<AuthorityRegistry>,
    registry: Arc<MatchRegistry>,
}
impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(Clock(AtomicU64::new(0)));
        let results = Arc::new(Results::default());
        let authority = AuthorityRegistry::new(8).unwrap();
        let registry = MatchRegistry::new(
            MatchLimits {
                matches: 4,
                mailbox: 16,
                outgoing: 32,
                proof_workers: 2,
            },
            clock.clone(),
            clock.clone(),
            authority.clone(),
            results.clone(),
        )
        .unwrap();
        Self {
            clock,
            results,
            authority,
            registry,
        }
    }
    async fn connect(&self, account: Uuid, handle: &MatchHandle) -> (MatchConnection, u32) {
        let (connection, epoch, _) = self.connect_full(account, handle).await;
        (connection, epoch)
    }
    async fn connect_full(
        &self,
        account: Uuid,
        handle: &MatchHandle,
    ) -> (MatchConnection, u32, GameView) {
        let lease = self
            .authority
            .bind(
                self.authority.generation().unwrap(),
                account,
                [1; 32],
                20000,
                18000,
            )
            .unwrap();
        let mut connection = handle.connect(lease).await.unwrap();
        let initial = connection.next().await.unwrap();
        let OnlinePayload::Snapshot {
            session_epoch,
            view,
            ..
        } = initial.payload
        else {
            panic!("snapshot")
        };
        (connection, session_epoch, view)
    }
}
async fn view_where(
    connection: &mut MatchConnection,
    condition: impl Fn(&GameView) -> bool,
) -> OnlineEvent {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = connection.next().await.expect("live actor output");
            let view = match &event.payload {
                OnlinePayload::Snapshot { view, .. }
                | OnlinePayload::Delta { view }
                | OnlinePayload::MatchEnd { view, .. } => Some(view),
                _ => None,
            };
            if view.is_some_and(&condition) {
                return event;
            }
        }
    })
    .await
    .expect("bounded observable bot progress")
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded worker progress");
}

#[tokio::test]
async fn real_core_bots_progress_and_finish_with_difficulty_intervals() {
    for (difficulty, interval) in [
        (Difficulty::Easy, 900),
        (Difficulty::Normal, 500),
        (Difficulty::Hard, 180),
    ] {
        let f = Fixture::new();
        let account = Uuid::new_v4();
        let executor = BotExecutor::new(2, Arc::new(CoreBotFactory)).unwrap();
        let handle = f
            .registry
            .create_with_bot(state(account, false), difficulty, executor)
            .unwrap();
        let (mut connection, _) = f.connect(account, &handle).await;
        f.clock.0.store(3000, Ordering::SeqCst);
        let event = view_where(&mut connection, |view| view.opponent.opened_safe == 5).await;
        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("seed") && !serialized.contains("truth"));
        f.clock.0.store(3000 + interval - 20, Ordering::SeqCst);
        assert!(
            tokio::time::timeout(Duration::from_millis(70), connection.next())
                .await
                .is_err()
        );
        f.clock.0.store(3000 + interval, Ordering::SeqCst);
        view_where(&mut connection, |view| view.opponent.opened_safe == 6).await;
        f.clock.0.store(3000 + interval * 2, Ordering::SeqCst);
        view_where(&mut connection, |view| {
            view.result
                .as_ref()
                .is_some_and(|result| result.reason == PublicEndReason::Clear)
        })
        .await;
        eventually(|| f.results.0.lock().unwrap().len() == 1).await;
        let results = f.results.0.lock().unwrap();
        assert_eq!(results[0].players[0].account, Some(account));
        assert_eq!(results[0].players[1].account, None);
        assert_eq!(results[0].players[1].opened_safe, 7);
    }
}

struct Control {
    calls: AtomicUsize,
    active: AtomicUsize,
    seen: Mutex<Vec<Projection>>,
    open: Mutex<bool>,
    wake: Condvar,
    failure: bool,
    panic: bool,
}
struct ControlledFactory(Arc<Control>);
struct ControlledAgent {
    control: Arc<Control>,
    used: bool,
}
impl BotFactory for ControlledFactory {
    fn create(&self, _: Difficulty) -> Result<Box<dyn BotAgent>, OnlineError> {
        Ok(Box::new(ControlledAgent {
            control: self.0.clone(),
            used: false,
        }))
    }
}
impl BotAgent for ControlledAgent {
    fn choose(&mut self, projection: &Projection, _: u64) -> Result<Option<Action>, OnlineError> {
        if self.used {
            return Ok(None);
        }
        self.used = true;
        self.control.calls.fetch_add(1, Ordering::SeqCst);
        self.control.active.fetch_add(1, Ordering::SeqCst);
        self.control.seen.lock().unwrap().push(projection.clone());
        let mut open = self.control.open.lock().unwrap();
        while !*open {
            open = self.control.wake.wait(open).unwrap();
        }
        drop(open);
        self.control.active.fetch_sub(1, Ordering::SeqCst);
        assert!(!self.control.panic, "controlled planner panic");
        if self.control.failure {
            Err(OnlineError::Unavailable)
        } else {
            Ok(Some(Action::Open(CellId(8))))
        }
    }
}
struct Gate(Arc<Control>);
impl Gate {
    fn new(open: bool, failure: bool) -> Self {
        Self::with_fault(open, failure, false)
    }
    fn with_fault(open: bool, failure: bool, panic: bool) -> Self {
        Self(Arc::new(Control {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
            open: Mutex::new(open),
            wake: Condvar::new(),
            failure,
            panic,
        }))
    }
    fn release(&self) {
        *self.0.open.lock().unwrap() = true;
        self.0.wake.notify_all();
    }
    fn executor(&self, workers: usize) -> Arc<BotExecutor> {
        BotExecutor::new(workers, Arc::new(ControlledFactory(self.0.clone()))).unwrap()
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
async fn human_open(
    f: &Fixture,
    handle: &MatchHandle,
    connection: &mut MatchConnection,
    epoch: u32,
) {
    connection
        .send(OnlineInput {
            v: 1,
            match_id: handle.id().to_string(),
            command_id: Uuid::new_v4().to_string(),
            client_seq: 1,
            session_epoch: epoch,
            known_revision: 0,
            action: PublicAction::Open { cell: 8 },
        })
        .unwrap();
    let event = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            let event = connection.next().await.unwrap();
            if matches!(event.payload, OnlinePayload::Ack { .. }) {
                break event;
            }
        }
    })
    .await
    .expect("human input is independent of blocking bot");
    assert!(matches!(
        event.payload,
        OnlinePayload::Ack {
            status: AckStatus::Applied,
            ..
        }
    ));
    assert_eq!(f.clock.now_ms(), 3000);
}

#[tokio::test]
async fn human_input_proceeds_while_bot_is_blocked_and_stale_intent_is_discarded() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(false, false);
    let handle = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Normal, gate.executor(1))
        .unwrap();
    let (mut connection, epoch) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    human_open(&f, &handle, &mut connection, epoch).await;
    gate.release();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (mut replacement, _, snapshot) = f.connect_full(account, &handle).await;
    assert_eq!(snapshot.opponent.opened_safe, 4);
    f.clock.0.store(3020, Ordering::SeqCst);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(80),
            view_where(&mut replacement, |view| view.opponent.opened_safe > 4)
        )
        .await
        .is_err()
    );
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    assert_eq!(gate.0.seen.lock().unwrap()[0].own.cells.opened_safe(), 4);
}

#[tokio::test]
async fn global_executor_and_each_bot_keep_actual_blocking_work_bounded() {
    let f = Fixture::new();
    let gate = Gate::new(false, false);
    let executor = gate.executor(1);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let first = f
        .registry
        .create_with_bot(state(a, false), Difficulty::Easy, executor.clone())
        .unwrap();
    let second = f
        .registry
        .create_with_bot(state(b, false), Difficulty::Hard, executor)
        .unwrap();
    let (mut one, _) = f.connect(a, &first).await;
    let (mut two, _) = f.connect(b, &second).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    f.clock.0.store(3040, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    gate.release();
    view_where(&mut one, |view| view.opponent.opened_safe == 5).await;
    view_where(&mut two, |view| view.opponent.opened_safe == 5).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn planner_failure_ends_as_server_failure_and_records_once() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(true, true);
    let handle = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Hard, gate.executor(1))
        .unwrap();
    let (mut connection, _) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    view_where(&mut connection, |view| {
        view.result
            .as_ref()
            .is_some_and(|result| result.reason == PublicEndReason::ServerFailure)
    })
    .await;
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
}

#[tokio::test]
async fn completion_after_match_end_cannot_mutate_or_record_another_result() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(false, false);
    let handle = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Hard, gate.executor(1))
        .unwrap();
    let (mut connection, _) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    f.clock.0.store(243000, Ordering::SeqCst);
    view_where(&mut connection, |view| view.result.is_some()).await;
    gate.release();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let results = f.results.0.lock().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].players[1].opened_safe, 4);
    assert_eq!(results[0].reason, PublicEndReason::Timeout);
}

#[tokio::test]
async fn bot_cannot_claim_human_seat_and_executor_limits_fail_closed() {
    let f = Fixture::new();
    let executor = BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap();
    assert!(matches!(
        f.registry
            .create_with_bot(state(Uuid::new_v4(), true), Difficulty::Normal, executor),
        Err(OnlineError::Malformed)
    ));
    for workers in [0, 65] {
        assert!(matches!(
            BotExecutor::new(workers, Arc::new(CoreBotFactory)),
            Err(OnlineError::Capacity)
        ));
    }
}

#[tokio::test]
async fn completion_over_100ms_is_discarded_without_spawning_another_live_job() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(false, false);
    let handle = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Normal, gate.executor(2))
        .unwrap();
    let (_connection, _) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    f.clock.0.store(3101, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    assert_eq!(gate.0.active.load(Ordering::SeqCst), 1);
    gate.release();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (replacement, _, snapshot) = f.connect_full(account, &handle).await;
    assert_eq!(snapshot.opponent.opened_safe, 4);
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    drop(replacement);
}

#[tokio::test]
async fn an_opponents_private_flag_does_not_invalidate_the_bot_public_intent() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(false, false);
    let handle = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Normal, gate.executor(1))
        .unwrap();
    let (mut connection, epoch) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    connection
        .send(OnlineInput {
            v: 1,
            match_id: handle.id().to_string(),
            command_id: Uuid::new_v4().to_string(),
            client_seq: 1,
            session_epoch: epoch,
            known_revision: 0,
            action: PublicAction::Flag { cell: 8 },
        })
        .unwrap();
    tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            if matches!(
                connection.next().await.unwrap().payload,
                OnlinePayload::Ack {
                    status: AckStatus::Applied,
                    ..
                }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    gate.release();
    view_where(&mut connection, |view| view.opponent.opened_safe == 5).await;
}

struct FailingFactory;
impl BotFactory for FailingFactory {
    fn create(&self, _: Difficulty) -> Result<Box<dyn BotAgent>, OnlineError> {
        Err(OnlineError::Unavailable)
    }
}
#[tokio::test]
async fn factory_failure_and_worker_panic_end_fail_closed() {
    let panic_gate = Gate::with_fault(true, false, true);
    for executor in [
        BotExecutor::new(1, Arc::new(FailingFactory)).unwrap(),
        panic_gate.executor(1),
    ] {
        let f = Fixture::new();
        let account = Uuid::new_v4();
        let handle = f
            .registry
            .create_with_bot(state(account, false), Difficulty::Hard, executor)
            .unwrap();
        let (mut connection, _) = f.connect(account, &handle).await;
        f.clock.0.store(3000, Ordering::SeqCst);
        view_where(&mut connection, |view| {
            view.result
                .as_ref()
                .is_some_and(|result| result.reason == PublicEndReason::ServerFailure)
        })
        .await;
        eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    }
}

#[tokio::test]
async fn explicit_bot_in_first_seat_progresses_without_account_or_auth_lease() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(true, false);
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    let state = MatchState::new(
        Uuid::new_v4(),
        RuleEngine::new(board, rules, 0).unwrap(),
        [None, Some(account)],
        [47; 8],
        0,
    )
    .unwrap();
    let handle = f
        .registry
        .create_with_bot(state, Difficulty::Easy, gate.executor(1))
        .unwrap();
    let (mut connection, _) = f.connect(account, &handle).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    view_where(&mut connection, |view| view.opponent.opened_safe == 5).await;
    assert_eq!(gate.0.seen.lock().unwrap()[0].own.cells.opened_safe(), 4);
}

#[tokio::test]
async fn old_completion_cannot_target_a_new_actor_reusing_the_match_uuid() {
    let f = Fixture::new();
    let account = Uuid::new_v4();
    let gate = Gate::new(false, false);
    let executor = gate.executor(2);
    let old = f
        .registry
        .create_with_bot(state(account, false), Difficulty::Hard, executor.clone())
        .unwrap();
    let id = old.id();
    let (mut old_connection, _) = f.connect(account, &old).await;
    f.clock.0.store(3000, Ordering::SeqCst);
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    f.clock.0.store(243000, Ordering::SeqCst);
    view_where(&mut old_connection, |view| view.result.is_some()).await;
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    f.clock.0.store(273000, Ordering::SeqCst);
    eventually(|| matches!(f.registry.by_id(id), Err(OnlineError::NotMatched))).await;
    let new = f
        .registry
        .create_with_bot(
            state_at(account, false, id, 273000),
            Difficulty::Normal,
            executor,
        )
        .unwrap();
    let (_new_connection, _, before) = f.connect_full(account, &new).await;
    assert_eq!(before.opponent.opened_safe, 4);
    gate.release();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 0).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (_replacement, _, after) = f.connect_full(account, &new).await;
    assert_eq!(after.opponent.opened_safe, 4);
    assert!(after.result.is_none());
    assert_eq!(after.countdown_ms, 3000);
    assert_eq!(f.registry.for_account(account).unwrap().id(), id);
    assert_eq!(f.results.0.lock().unwrap().len(), 1);
}
