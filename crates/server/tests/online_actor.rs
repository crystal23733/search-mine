use liar_core::{
    board::{Board, CellId},
    game::RuleEngine,
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{AckStatus, PublicAction},
    online::{OnlineInput, OnlinePayload},
};
use liar_server::{auth::AuthClock, online::*};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
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
struct Results(AtomicUsize);
impl ResultRepository for Results {
    fn save(
        &self,
        _result: FinishedMatch,
    ) -> PortFuture<'_, Result<SaveResult, liar_protocol::online::OnlineError>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(SaveResult::Saved)
        })
    }
}
fn state(id: Uuid, accounts: [Option<Uuid>; 2]) -> MatchState {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    MatchState::new(
        id,
        RuleEngine::new(board, rules, 0).unwrap(),
        accounts,
        [46; 8],
    )
    .unwrap()
}
#[tokio::test]
async fn serialized_actor_applies_once_then_commits_one_deadline_result() {
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let results = Arc::new(Results(AtomicUsize::new(0)));
    let authority = AuthorityRegistry::new(2).unwrap();
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 1,
            mailbox: 16,
            outgoing: 16,
            proof_workers: 1,
        },
        clock.clone(),
        clock.clone(),
        authority.clone(),
        results.clone(),
    )
    .unwrap();
    let account = Uuid::new_v4();
    let id = Uuid::new_v4();
    let handle = registry
        .create(state(id, [Some(account), Some(Uuid::new_v4())]))
        .unwrap();
    let lease = authority
        .bind(
            authority.generation().unwrap(),
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
    } = initial.payload
    else {
        panic!("initial snapshot")
    };
    assert_eq!(view.own.gauge, 0);
    clock.0.store(3000, Ordering::SeqCst);
    let command = OnlineInput {
        v: 1,
        match_id: id.to_string(),
        command_id: Uuid::new_v4().to_string(),
        client_seq: 1,
        session_epoch,
        known_revision: 0,
        action: PublicAction::Open { cell: 8 },
    };
    connection.send(command.clone()).unwrap();
    let ack = connection.next().await.unwrap();
    assert!(matches!(
        ack.payload,
        OnlinePayload::Ack {
            status: AckStatus::Applied,
            duplicate: false,
            ..
        }
    ));
    let change = connection.next().await.unwrap();
    let OnlinePayload::Delta { view } = change.payload else {
        panic!("public update")
    };
    assert_eq!(view.own.gauge, 1);
    connection.send(command).unwrap();
    assert!(matches!(
        connection.next().await.unwrap().payload,
        OnlinePayload::Ack {
            duplicate: true,
            ..
        }
    ));
    clock.0.store(243000, Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if matches!(
                connection.next().await.unwrap().payload,
                OnlinePayload::MatchEnd {
                    recording: liar_protocol::online::RecordingStatus::Saved,
                    ..
                }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(results.0.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn refuses_capacity_and_duplicate_account_ownership() {
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let authority = AuthorityRegistry::new(2).unwrap();
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 1,
            mailbox: 4,
            outgoing: 4,
            proof_workers: 1,
        },
        clock.clone(),
        clock,
        authority,
        Arc::new(Results(AtomicUsize::new(0))),
    )
    .unwrap();
    let accounts = [Some(Uuid::new_v4()), Some(Uuid::new_v4())];
    let _handle = registry.create(state(Uuid::new_v4(), accounts)).unwrap();
    assert!(registry.create(state(Uuid::new_v4(), accounts)).is_err());
    assert!(
        registry
            .create(state(Uuid::new_v4(), [Some(Uuid::new_v4()), None]))
            .is_err()
    );
}
