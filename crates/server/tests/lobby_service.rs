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
use liar_server::{
    auth::{AuthClock, SessionInvalidator},
    lobby::*,
    online::*,
};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

struct Clock(AtomicU64, AtomicI64);
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.1.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
struct Results(Mutex<Vec<FinishedMatch>>);
#[derive(Default)]
struct Journal {
    held: AtomicBool,
    hold_first: AtomicBool,
    registered: Mutex<Vec<ActiveMatch>>,
    discarded: Mutex<Vec<Uuid>>,
    wake: tokio::sync::Notify,
    failed: AtomicUsize,
    reject: AtomicBool,
    discard_failure: AtomicBool,
}
impl Journal {
    fn release(&self) {
        self.held.store(false, Ordering::SeqCst);
        self.wake.notify_waiters();
    }
}
impl AdmissionJournal for Journal {
    fn fail_closed(&self) {
        self.failed.fetch_add(1, Ordering::SeqCst);
    }
    fn register(&self, active: ActiveMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async move {
            let first = {
                let mut registered = self.registered.lock().unwrap();
                registered.push(active);
                registered.len() == 1
            };
            loop {
                let changed = self.wake.notified();
                if !self.held.load(Ordering::SeqCst)
                    || (self.hold_first.load(Ordering::SeqCst) && !first)
                {
                    break;
                }
                changed.await;
            }
            if self.reject.load(Ordering::SeqCst) {
                Err(OnlineError::Unavailable)
            } else {
                Ok(SaveResult::Saved)
            }
        })
    }
    fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        Box::pin(async move {
            self.discarded.lock().unwrap().push(id);
            if self.discard_failure.load(Ordering::SeqCst) {
                Err(OnlineError::Unavailable)
            } else {
                Ok(())
            }
        })
    }
}
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
struct LobbyHarness {
    inner: Arc<LobbyService>,
    authentication: LobbyAuthentication,
}
impl LobbyHarness {
    fn lease(&self, account: Uuid) -> Result<ConnectionAuthority, LobbyServiceError> {
        if account.is_nil() {
            return Err(OnlineError::Malformed.into());
        }
        let mut hash = [1; 32];
        hash[..16].copy_from_slice(account.as_bytes());
        let registry = &self.authentication.authorities;
        Ok(registry.bind_shared(
            registry.generation()?,
            account,
            hash,
            20000,
            self.authentication.clock.now(),
        )?)
    }
    fn status(&self, account: Uuid) -> Result<LobbyView, LobbyServiceError> {
        self.inner.status(&self.lease(account)?)
    }
    fn join(&self, account: Uuid, difficulty: Difficulty) -> Result<LobbyView, LobbyServiceError> {
        self.inner.join(&self.lease(account)?, difficulty)
    }
    fn create_room(&self, account: Uuid) -> Result<LobbyView, LobbyServiceError> {
        self.inner.create_room(&self.lease(account)?)
    }
    fn join_room(&self, account: Uuid, code: RoomCode) -> Result<LobbyView, LobbyServiceError> {
        self.inner.join_room(&self.lease(account)?, code)
    }
    fn ready(&self, account: Uuid, id: Uuid, ready: bool) -> Result<LobbyView, LobbyServiceError> {
        self.inner.ready(&self.lease(account)?, id, ready)
    }
    fn cancel(
        &self,
        account: Uuid,
        identity: LobbyIdentity,
    ) -> Result<LobbyView, LobbyServiceError> {
        self.inner.cancel(&self.lease(account)?, identity)
    }
}
struct Fixture {
    clock: Arc<Clock>,
    results: Arc<Results>,
    journal: Arc<Journal>,
    authorities: Arc<AuthorityRegistry>,
    registry: Arc<MatchRegistry>,
    service: Arc<LobbyHarness>,
}
impl Fixture {
    fn new(gate: &Gate, capacity: usize, matches: usize, workers: usize) -> Self {
        let clock = Arc::new(Clock(AtomicU64::new(0), AtomicI64::new(18000)));
        let results = Arc::new(Results::default());
        let journal = Arc::new(Journal::default());
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
        let authentication = LobbyAuthentication {
            authorities: AuthorityRegistry::new(16).unwrap(),
            clock: clock.clone(),
        };
        let inner = LobbyService::new(
            LobbyLimits { capacity, workers },
            boards(),
            Arc::new(Preparer(gate.0.clone())),
            Arc::new(Codes),
            LobbyMatchServices {
                registry: registry.clone(),
                bots: BotExecutor::new(2, Arc::new(CoreBotFactory)).unwrap(),
                journal: journal.clone(),
            },
            authentication.clone(),
        )
        .unwrap();
        let service = Arc::new(LobbyHarness {
            inner,
            authentication,
        });
        Self {
            clock,
            results,
            journal,
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
async fn matched(service: &LobbyHarness, account: Uuid) -> LobbyView {
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
async fn durable_ack_precedes_publication_with_applied_hash_and_a_fresh_countdown() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 6, 2, 1);
    f.journal.held.store(true, Ordering::SeqCst);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let active = f.journal.registered.lock().unwrap()[0].clone();
    assert_eq!(active.rules_hash, rules().hash);
    assert_eq!(active.players, [Some(a), Some(b)]);
    assert!(matches!(
        f.service.status(a),
        Ok(LobbyView::Waiting(LobbyStatus::Preparing { .. }))
    ));
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
    let outsider = Uuid::new_v4();
    assert!(matches!(
        f.service.join(outsider, Difficulty::Easy),
        Ok(LobbyView::Waiting(_))
    ));
    f.clock.0.store(3500, Ordering::SeqCst);
    f.journal.release();
    let LobbyView::Matched { match_id, .. } = matched(&f.service, a).await else {
        panic!("matched")
    };
    assert_eq!(match_id, active.id);
    let mut connected = f.connect(a).await;
    let event = connected.next().await.unwrap();
    let OnlinePayload::Snapshot { view, .. } = event.payload else {
        panic!("snapshot")
    };
    assert_eq!(view.countdown_ms, 3000);
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ordinary_cancel_after_register_waits_for_ack_discards_old_id_and_keeps_owner() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 6, 2, 1);
    f.journal.held.store(true, Ordering::SeqCst);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let id = f.journal.registered.lock().unwrap()[0].id;
    let identity = f.service.status(a).unwrap().identity().unwrap();
    f.service.cancel(a, identity).unwrap();
    assert!(f.journal.discarded.lock().unwrap().is_empty());
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
    f.journal.release();
    eventually(|| f.journal.discarded.lock().unwrap().contains(&id)).await;
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
    f.service.join(a, Difficulty::Hard).unwrap();
    let LobbyView::Matched { match_id, .. } = matched(&f.service, a).await else {
        panic!("matched")
    };
    assert_ne!(match_id, id);
    assert_eq!(f.journal.discarded.lock().unwrap().as_slice(), &[id]);
}

#[tokio::test]
async fn pending_admission_keeps_the_worker_until_cancel_cleanup_and_uses_new_uuid() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 6, 3, 1);
    f.journal.held.store(true, Ordering::SeqCst);
    let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
    pair(&f, a, b);
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let old = f.journal.registered.lock().unwrap()[0].id;
    f.service
        .cancel(a, f.service.status(a).unwrap().identity().unwrap())
        .unwrap();
    pair(&f, c, d);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    f.journal.release();
    eventually(|| f.journal.discarded.lock().unwrap().contains(&old)).await;
    eventually(|| f.journal.registered.lock().unwrap().len() >= 2).await;
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
}

#[tokio::test]
async fn registration_failure_never_creates_an_actor_and_cleanup_failure_fails_closed() {
    for reject in [true, false] {
        let gate = Gate::new(true, false);
        let f = Fixture::new(&gate, 4, 2, 1);
        f.journal.reject.store(reject, Ordering::SeqCst);
        f.journal.discard_failure.store(!reject, Ordering::SeqCst);
        f.journal.held.store(!reject, Ordering::SeqCst);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        pair(&f, a, b);
        eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
        if reject {
            eventually(|| f.service.status(a) == Ok(LobbyView::Failed(OnlineError::Unavailable)))
                .await;
            assert!(f.journal.discarded.lock().unwrap().is_empty());
            assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
        } else {
            f.service
                .cancel(a, f.service.status(a).unwrap().identity().unwrap())
                .unwrap();
            f.journal.release();
            eventually(|| f.journal.failed.load(Ordering::SeqCst) == 1).await;
        }
        assert!(matches!(
            f.registry.for_account(a),
            Err(OnlineError::NotMatched)
        ));
    }
}

#[tokio::test]
async fn service_drop_during_registration_observes_ack_and_discards_without_owner_loss() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    f.journal.held.store(true, Ordering::SeqCst);
    pair(&f, Uuid::new_v4(), Uuid::new_v4());
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let Fixture {
        service,
        journal,
        registry,
        ..
    } = f;
    let weak = Arc::downgrade(&service.inner);
    drop(service);
    eventually(|| weak.upgrade().is_none()).await;
    journal.release();
    eventually(|| journal.discarded.lock().unwrap().len() == 1).await;
    assert_eq!(journal.failed.load(Ordering::SeqCst), 0);
    let player = journal.registered.lock().unwrap()[0].players[0].unwrap();
    assert!(matches!(
        registry.for_account(player),
        Err(OnlineError::NotMatched)
    ));
}

#[tokio::test]
async fn expiry_and_revocation_while_registering_discard_the_original_receipt() {
    for expiry in [false, true] {
        let gate = Gate::new(true, false);
        let f = Fixture::new(&gate, 4, 2, 1);
        f.journal.held.store(true, Ordering::SeqCst);
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        pair(&f, a, b);
        eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
        let old = f.journal.registered.lock().unwrap()[0].id;
        if expiry {
            f.clock.0.store(5000, Ordering::SeqCst);
        } else {
            drop(SessionInvalidator::account(
                &*f.service.authentication.authorities,
                a,
            ));
        }
        // Tick observes the revoked/expired reservation before its ACK arrives.
        eventually(|| {
            !matches!(
                f.service.status(b),
                Ok(LobbyView::Waiting(LobbyStatus::Preparing { .. }))
            )
        })
        .await;
        f.journal.release();
        eventually(|| f.journal.discarded.lock().unwrap().contains(&old)).await;
        assert!(matches!(
            f.registry.for_account(a),
            Err(OnlineError::NotMatched)
        ));
        assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn replaced_lease_cannot_promote_an_old_durable_preparation() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    f.journal.held.store(true, Ordering::SeqCst);
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    pair(&f, a, b);
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let old = f.journal.registered.lock().unwrap()[0].id;
    let authorities = &f.service.authentication.authorities;
    let replacement = authorities
        .bind_shared(authorities.generation().unwrap(), a, [99; 32], 20000, 18000)
        .unwrap();
    f.service
        .inner
        .join(&replacement, Difficulty::Normal)
        .unwrap();
    f.journal.release();
    eventually(|| f.journal.discarded.lock().unwrap().contains(&old)).await;
    assert!(!matches!(f.registry.for_account(a),Ok(handle) if handle.id()==old));
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn overlapping_durable_jobs_complete_out_of_order_without_crossing_match_identity() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 8, 3, 2);
    f.journal.held.store(true, Ordering::SeqCst);
    f.journal.hold_first.store(true, Ordering::SeqCst);
    let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
    pair(&f, a, b);
    eventually(|| f.journal.registered.lock().unwrap().len() == 1).await;
    let first = f.journal.registered.lock().unwrap()[0].clone();
    pair(&f, c, d);
    let LobbyView::Matched {
        match_id: second, ..
    } = matched(&f.service, c).await
    else {
        panic!("second matched")
    };
    assert_eq!(f.journal.registered.lock().unwrap()[1].id, second);
    assert_ne!(first.id, second);
    assert!(matches!(
        f.registry.for_account(a),
        Err(OnlineError::NotMatched)
    ));
    f.service
        .cancel(a, f.service.status(a).unwrap().identity().unwrap())
        .unwrap();
    f.journal.release();
    eventually(|| f.journal.discarded.lock().unwrap().contains(&first.id)).await;
    assert_eq!(f.registry.for_account(c).unwrap().id(), second);
    assert_eq!(f.registry.for_account(d).unwrap().id(), second);
    assert_eq!(f.journal.discarded.lock().unwrap().as_slice(), &[first.id]);
    assert_eq!(f.journal.failed.load(Ordering::SeqCst), 0);
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
    let inner = LobbyService::new(
        LobbyLimits {
            capacity: 2,
            workers: 1,
        },
        boards,
        Arc::new(CoreMatchPreparer),
        codes,
        LobbyMatchServices {
            registry: f.registry.clone(),
            bots: BotExecutor::new(2, Arc::new(CoreBotFactory)).unwrap(),
            journal: f.journal.clone(),
        },
        f.service.authentication.clone(),
    )
    .unwrap();
    f.service = Arc::new(LobbyHarness {
        inner,
        authentication: f.service.authentication.clone(),
    });
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
                LobbyMatchServices {
                    registry: f.registry.clone(),
                    bots: BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap(),
                    journal: f.journal.clone()
                },
                f.service.authentication.clone(),
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

#[tokio::test]
async fn every_lobby_entry_rejects_foreign_revoked_and_exactly_expired_authority() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    let account = Uuid::new_v4();
    let own = f.service.lease(account).unwrap();
    let foreign_registry = AuthorityRegistry::new(1).unwrap();
    let foreign = foreign_registry
        .bind_shared(0, account, [3; 32], 20000, 18000)
        .unwrap();
    let _barrier = f.service.authentication.authorities.account(account);
    for lease in [&foreign, &own] {
        let service = &f.service.inner;
        let expected = Err(LobbyServiceError::Online(OnlineError::Unauthorized));
        assert_eq!(service.status(lease), expected);
        assert_eq!(service.join(lease, Difficulty::Hard), expected);
        assert_eq!(service.create_room(lease), expected);
        assert_eq!(service.join_room(lease, Codes.code().unwrap()), expected);
        assert_eq!(service.ready(lease, Uuid::new_v4(), true), expected);
        assert_eq!(
            service.cancel(lease, LobbyIdentity::Queue(Uuid::new_v4())),
            expected
        );
    }
    drop(_barrier);
    let fresh = f.service.lease(account).unwrap();
    f.clock.1.store(20000, Ordering::SeqCst);
    assert_eq!(
        f.service.inner.join(&fresh, Difficulty::Easy),
        Err(OnlineError::Unauthorized.into())
    );
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn revocation_during_actual_cpu_restores_peer_and_old_completion_cannot_admit() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    let a = f.service.lease(Uuid::new_v4()).unwrap();
    let b = f.service.lease(Uuid::new_v4()).unwrap();
    f.service.inner.join(&a, Difficulty::Normal).unwrap();
    f.service.inner.join(&b, Difficulty::Hard).unwrap();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    drop(f.service.authentication.authorities.account(a.account()));
    eventually(|| {
        matches!(
            f.service.inner.status(&b),
            Ok(LobbyView::Waiting(LobbyStatus::Queued {
                deadline: 10000,
                ..
            }))
        )
    })
    .await;
    assert_eq!(
        f.service.inner.status(&a),
        Err(OnlineError::Unauthorized.into())
    );
    f.clock.0.store(9999, Ordering::SeqCst);
    let c = f.service.lease(Uuid::new_v4()).unwrap();
    f.service.inner.join(&c, Difficulty::Easy).unwrap();
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 1);
    gate.release();
    eventually(|| f.registry.for_account(b.account()).is_ok()).await;
    assert_eq!(
        f.registry.for_account(b.account()).unwrap().id(),
        f.registry.for_account(c.account()).unwrap().id()
    );
    assert!(matches!(
        f.registry.for_account(a.account()),
        Err(OnlineError::NotMatched)
    ));
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn expired_queue_is_removed_before_ten_second_bot_reservation() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let registry = &f.service.authentication.authorities;
    let lease = registry
        .bind_shared(0, Uuid::new_v4(), [8; 32], 18001, 18000)
        .unwrap();
    f.service.inner.join(&lease, Difficulty::Hard).unwrap();
    f.clock.1.store(18001, Ordering::SeqCst);
    f.clock.0.store(10000, Ordering::SeqCst);
    let observer = f.service.lease(Uuid::new_v4()).unwrap();
    assert_eq!(f.service.inner.status(&observer).unwrap(), LobbyView::Idle);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.service.inner.status(&lease),
        Err(OnlineError::Unauthorized.into())
    );
    assert!(matches!(
        f.registry.for_account(lease.account()),
        Err(OnlineError::NotMatched)
    ));
}

#[tokio::test]
async fn expired_ready_host_promotes_guest_and_replacement_cannot_use_old_lease() {
    let gate = Gate::new(false, false);
    let f = Fixture::new(&gate, 4, 2, 1);
    let registry = &f.service.authentication.authorities;
    let host = registry
        .bind_shared(0, Uuid::new_v4(), [8; 32], 18001, 18000)
        .unwrap();
    let guest = f.service.lease(Uuid::new_v4()).unwrap();
    let LobbyView::Waiting(LobbyStatus::Room { id, code, .. }) =
        f.service.inner.create_room(&host).unwrap()
    else {
        panic!("room");
    };
    f.service.inner.join_room(&guest, code).unwrap();
    f.service.inner.ready(&host, id, true).unwrap();
    f.service.inner.ready(&guest, id, true).unwrap();
    eventually(|| gate.0.active.load(Ordering::SeqCst) == 1).await;
    f.clock.1.store(18001, Ordering::SeqCst);
    eventually(|| {
        matches!(
            f.service.inner.status(&guest),
            Ok(LobbyView::Waiting(LobbyStatus::Room {
                own_seat: 0,
                occupied: [true, false],
                ready: [false, false],
                ..
            }))
        )
    })
    .await;
    let new = registry
        .bind_shared(
            registry.generation().unwrap(),
            host.account(),
            [9; 32],
            20000,
            18001,
        )
        .unwrap();
    f.service.inner.join_room(&new, code).unwrap();
    assert_eq!(
        f.service.inner.cancel(&host, LobbyIdentity::Room(id)),
        Err(OnlineError::Unauthorized.into())
    );
    f.service.inner.ready(&guest, id, true).unwrap();
    f.service.inner.ready(&new, id, true).unwrap();
    drop(host);
    gate.release();
    eventually(|| f.registry.for_account(guest.account()).is_ok()).await;
    assert_eq!(
        f.registry.for_account(new.account()).unwrap().id(),
        f.registry.for_account(guest.account()).unwrap().id()
    );
    assert_eq!(gate.0.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn membership_owns_shared_lease_after_requests_drop_and_releases_it_on_cancel() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    let account = Uuid::new_v4();
    let lease = f.service.lease(account).unwrap();
    let token = lease.token();
    let revoked = lease.revoked();
    let queued = f.service.inner.join(&lease, Difficulty::Normal).unwrap();
    drop(lease);
    assert!(
        !*revoked.borrow(),
        "membership must retain the authority between HTTP requests"
    );
    for _ in 0..3 {
        let poll = f.service.lease(account).unwrap();
        assert_eq!(poll.token(), token);
        assert_eq!(
            f.service.inner.join(&poll, Difficulty::Normal).unwrap(),
            queued
        );
    }
    let socket = f
        .authorities
        .bind(0, account, [1; 32], 20000, 18000)
        .unwrap();
    socket.close();
    assert!(
        !*revoked.borrow(),
        "socket closure must not cancel the lobby namespace"
    );
    let cancel = f.service.lease(account).unwrap();
    assert_eq!(
        f.service
            .inner
            .cancel(&cancel, queued.identity().unwrap())
            .unwrap(),
        LobbyView::Idle
    );
    drop(cancel);
    assert!(
        *revoked.borrow(),
        "inactive membership must release its final lease owner"
    );
}

#[tokio::test]
async fn composition_rejects_reusing_socket_authority_for_http_lobby_requests() {
    let gate = Gate::new(true, false);
    let f = Fixture::new(&gate, 2, 1, 1);
    assert!(matches!(
        LobbyService::new(
            LobbyLimits {
                capacity: 2,
                workers: 1
            },
            boards(),
            Arc::new(CoreMatchPreparer),
            Arc::new(Codes),
            LobbyMatchServices {
                registry: f.registry.clone(),
                bots: BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap(),
                journal: f.journal.clone()
            },
            LobbyAuthentication {
                authorities: f.authorities.clone(),
                clock: f.clock.clone()
            },
        ),
        Err(LobbyServiceError::Online(OnlineError::Malformed))
    ));
}

#[tokio::test]
async fn actual_lobby_hands_initial_leases_to_human_and_bot_matches_until_start_cancellation() {
    for bot in [false, true] {
        let gate = Gate::new(true, false);
        let f = Fixture::new(&gate, 2, 1, 1);
        let a = f.service.lease(Uuid::new_v4()).unwrap();
        let account = a.account();
        let watcher = a.revoked();
        f.service.inner.join(&a, Difficulty::Normal).unwrap();
        let b = if bot {
            f.clock.0.store(10000, Ordering::SeqCst);
            None
        } else {
            let b = f.service.lease(Uuid::new_v4()).unwrap();
            f.service.inner.join(&b, Difficulty::Hard).unwrap();
            Some(b)
        };
        eventually(|| f.registry.for_account(account).is_ok()).await;
        let id = f.registry.for_account(account).unwrap().id();
        drop(a);
        drop(b);
        assert!(
            !*watcher.borrow(),
            "the actor must own the initial lobby lease"
        );
        f.clock.0.store(40000, Ordering::SeqCst);
        eventually(|| !f.results.0.lock().unwrap().is_empty()).await;
        let results = f.results.0.lock().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert_eq!(results[0].reason, PublicEndReason::Cancelled);
        assert_eq!(results[0].ended_ms, 3000);
        assert_eq!(results[0].players[1].account.is_none(), bot);
        assert!(*watcher.borrow());
        assert!(matches!(
            f.registry.for_account(account),
            Err(OnlineError::NotMatched)
        ));
    }
}
