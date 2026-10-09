use liar_core::{
    board::{Board, CellId},
    bot::Difficulty,
    game::RuleEngine,
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{AckStatus, PublicAction, PublicEndReason},
    online::{OnlineError, OnlineInput, OnlinePayload, RecordingStatus},
};
use liar_server::{
    auth::{AuthClock, SessionInvalidator},
    online::*,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

struct Clock {
    game: AtomicU64,
    auth: AtomicI64,
}
impl MatchClock for Clock {
    fn now_ms(&self) -> u64 {
        self.game.load(Ordering::SeqCst)
    }
}
impl AuthClock for Clock {
    fn now(&self) -> i64 {
        self.auth.load(Ordering::SeqCst)
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
fn hash(account: Uuid) -> [u8; 32] {
    let mut value = [1; 32];
    value[..16].copy_from_slice(account.as_bytes());
    value
}
fn state(players: [Option<Uuid>; 2]) -> MatchState {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    MatchState::new(
        Uuid::new_v4(),
        RuleEngine::new(board, rules, 0).unwrap(),
        players,
        [46; 8],
        0,
    )
    .unwrap()
}
struct Fixture {
    clock: Arc<Clock>,
    sockets: Arc<AuthorityRegistry>,
    lobby: Arc<AuthorityRegistry>,
    results: Arc<Results>,
    registry: Arc<MatchRegistry>,
}
impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(Clock {
            game: AtomicU64::new(0),
            auth: AtomicI64::new(18000),
        });
        let sockets = AuthorityRegistry::new(4).unwrap();
        let lobby = AuthorityRegistry::new(4).unwrap();
        let results = Arc::new(Results::default());
        let registry = MatchRegistry::new(
            MatchLimits {
                matches: 2,
                mailbox: 16,
                outgoing: 32,
                proof_workers: 1,
            },
            clock.clone(),
            clock.clone(),
            sockets.clone(),
            results.clone(),
        )
        .unwrap();
        Self {
            clock,
            sockets,
            lobby,
            results,
            registry,
        }
    }
    fn lease(&self, expires: i64) -> ConnectionAuthority {
        let account = Uuid::new_v4();
        self.lobby
            .bind_shared(
                self.lobby.generation().unwrap(),
                account,
                hash(account),
                expires,
                18000,
            )
            .unwrap()
    }
    fn admission(&self, participants: [Option<ConnectionAuthority>; 2]) -> MatchAdmission {
        MatchAdmission {
            authorities: self.lobby.clone(),
            clock: self.clock.clone(),
            participants,
        }
    }
    fn create(&self, participants: [Option<ConnectionAuthority>; 2], bot: bool) -> MatchHandle {
        let players = participants
            .each_ref()
            .map(|p| p.as_ref().map(ConnectionAuthority::account));
        self.registry
            .create_admitted(
                state(players),
                self.admission(participants),
                bot.then(|| {
                    (
                        Difficulty::Normal,
                        BotExecutor::new(1, Arc::new(CoreBotFactory)).unwrap(),
                    )
                }),
            )
            .unwrap()
    }
    async fn connect(
        &self,
        handle: &MatchHandle,
        account: Uuid,
    ) -> Result<MatchConnection, OnlineError> {
        let lease = self
            .sockets
            .bind(
                self.sockets.generation().unwrap(),
                account,
                hash(account),
                20000,
                self.clock.now(),
            )
            .unwrap();
        handle.connect(lease).await
    }
}
async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded initial admission progress");
}
async fn saved_cancel(connection: &mut MatchConnection) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let OnlinePayload::MatchEnd {
                view,
                recording: RecordingStatus::Saved,
            } = connection.next().await.unwrap().payload
            {
                let result = view.result.unwrap();
                assert_eq!(result.reason, PublicEndReason::Cancelled);
                assert!(!result.completed);
                break;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn a_late_tick_cancels_missing_peer_at_logical_start_and_releases_accounts_once() {
    let f = Fixture::new();
    let a = f.lease(20000);
    let b = f.lease(20000);
    let accounts = [a.account(), b.account()];
    let handle = f.create([Some(a), Some(b)], false);
    let mut connection = f.connect(&handle, accounts[0]).await.unwrap();
    assert!(matches!(
        connection.next().await.unwrap().payload,
        OnlinePayload::Snapshot { .. }
    ));
    f.clock.game.store(40000, Ordering::SeqCst);
    saved_cancel(&mut connection).await;
    {
        let results = f.results.0.lock().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            (results[0].reason, results[0].ended_ms),
            (PublicEndReason::Cancelled, 3000)
        );
    }
    for account in accounts {
        assert!(matches!(
            f.registry.for_account(account),
            Err(OnlineError::NotMatched)
        ));
    }
    f.clock.game.store(40001, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(f.results.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn no_socket_ever_attached_still_owns_initial_leases_and_cancels_without_a_false_match() {
    let f = Fixture::new();
    let a = f.lease(20000);
    let b = f.lease(20000);
    let revoked = [a.revoked(), b.revoked()];
    let _handle = f.create([Some(a), Some(b)], false);
    assert!(
        revoked.iter().all(|r| !*r.borrow()),
        "actor must own admission until first attach"
    );
    f.clock.game.store(3000, Ordering::SeqCst);
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    assert_eq!(
        f.results.0.lock().unwrap()[0].reason,
        PublicEndReason::Cancelled
    );
    assert!(revoked.iter().all(|r| *r.borrow()));
}

#[tokio::test]
async fn pending_session_or_account_revocation_and_exact_expiry_cancel_before_any_socket() {
    for kind in 0..3 {
        let f = Fixture::new();
        let a = f.lease(18001);
        let b = f.lease(20000);
        let account = a.account();
        let handle = f.create([Some(a), Some(b)], false);
        f.clock.game.store(1500, Ordering::SeqCst);
        match kind {
            0 => drop(f.lobby.session(hash(account))),
            1 => drop(f.lobby.account(account)),
            _ => f.clock.auth.store(18001, Ordering::SeqCst),
        }
        eventually(|| f.results.0.lock().unwrap().len() == 1).await;
        {
            let results = f.results.0.lock().unwrap();
            assert_eq!(
                (results[0].reason, results[0].ended_ms),
                (PublicEndReason::Cancelled, 1500)
            );
        }
        assert!(matches!(
            f.connect(&handle, account).await,
            Err(OnlineError::InvalidEpoch)
        ));
        assert!(matches!(
            f.registry.for_account(account),
            Err(OnlineError::NotMatched)
        ));
    }
}

#[tokio::test]
async fn both_humans_attaching_before_boundary_release_initial_lease_and_use_game_authority() {
    let f = Fixture::new();
    let a = f.lease(20000);
    let b = f.lease(20000);
    let accounts = [a.account(), b.account()];
    let revoked = [a.revoked(), b.revoked()];
    let handle = f.create([Some(a), Some(b)], false);
    f.clock.game.store(2999, Ordering::SeqCst);
    let mut one = f.connect(&handle, accounts[0]).await.unwrap();
    let mut two = f.connect(&handle, accounts[1]).await.unwrap();
    for connection in [&mut one, &mut two] {
        let OnlinePayload::Snapshot { view, .. } = connection.next().await.unwrap().payload else {
            panic!("snapshot");
        };
        assert_eq!(view.countdown_ms, 1);
    }
    eventually(|| revoked.iter().all(|r| *r.borrow())).await;
    f.clock.game.store(3000, Ordering::SeqCst);
    one.send(OnlineInput {
        v: 1,
        match_id: handle.id().to_string(),
        command_id: Uuid::new_v4().to_string(),
        client_seq: 1,
        session_epoch: one.epoch(),
        known_revision: 0,
        action: PublicAction::Open { cell: 8 },
    })
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let OnlinePayload::Ack { status, .. } = one.next().await.unwrap().payload {
                assert_eq!(status, AckStatus::Applied);
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(f.results.0.lock().unwrap().is_empty());
    assert!(
        f.sockets
            .with_authority(one.authority(), 18000, || ())
            .is_ok()
    );
    assert!(
        f.sockets
            .with_authority(two.authority(), 18000, || ())
            .is_ok()
    );
}

#[tokio::test]
async fn exact_boundary_attach_cannot_turn_a_missing_initial_seat_into_playing() {
    let f = Fixture::new();
    let a = f.lease(20000);
    let b = f.lease(20000);
    let account = a.account();
    let handle = f.create([Some(a), Some(b)], false);
    f.clock.game.store(3000, Ordering::SeqCst);
    assert!(matches!(
        f.connect(&handle, account).await,
        Err(OnlineError::InvalidEpoch)
    ));
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    assert_eq!(
        f.results.0.lock().unwrap()[0].reason,
        PublicEndReason::Cancelled
    );
}

#[tokio::test]
async fn a_bot_does_not_need_a_session_but_a_missing_human_still_cancels_before_bot_input() {
    let f = Fixture::new();
    let a = f.lease(20000);
    let _handle = f.create([None, Some(a)], true);
    f.clock.game.store(3000, Ordering::SeqCst);
    eventually(|| f.results.0.lock().unwrap().len() == 1).await;
    let results = f.results.0.lock().unwrap();
    assert_eq!(
        (results[0].reason, results[0].ended_ms),
        (PublicEndReason::Cancelled, 3000)
    );
    assert_eq!(results[0].players[0].account, None);
    assert_eq!(results[0].players[0].opened_safe, 4);
}

#[tokio::test]
async fn admission_rejects_foreign_namespace_seat_mismatch_and_revoked_sessions_before_registry_effect()
 {
    let f = Fixture::new();
    let a = f.lease(20000);
    let b = f.lease(20000);
    let players = [Some(a.account()), Some(b.account())];
    let wrong = f.admission([Some(b.clone()), Some(a.clone())]);
    assert!(matches!(
        f.registry.create_admitted(state(players), wrong, None),
        Err(OnlineError::Malformed)
    ));
    let game = f
        .sockets
        .bind(0, a.account(), hash(a.account()), 20000, 18000)
        .unwrap();
    let wrong = MatchAdmission {
        authorities: f.sockets.clone(),
        clock: f.clock.clone(),
        participants: [Some(game), Some(b.clone())],
    };
    assert!(matches!(
        f.registry.create_admitted(state(players), wrong, None),
        Err(OnlineError::Malformed)
    ));
    drop(f.lobby.account(a.account()));
    assert!(matches!(
        f.registry
            .create_admitted(state(players), f.admission([Some(a), Some(b)]), None),
        Err(OnlineError::Unauthorized)
    ));
    for account in players.into_iter().flatten() {
        assert!(matches!(
            f.registry.for_account(account),
            Err(OnlineError::NotMatched)
        ));
    }
}
