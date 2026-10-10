use super::*;
use crate::lobby::{OpponentKind, ReservationKey};
use crate::online::{PortFuture, SaveResult};
use liar_core::{
    board::{Board, CellId},
    bot::Difficulty,
    game::RuleEngine,
    rules::RulesSnapshot,
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Semaphore;

#[derive(Default)]
struct Journal {
    lost: AtomicUsize,
    discarded: Mutex<Vec<Uuid>>,
}
impl AdmissionJournal for Journal {
    fn fail_closed(&self) {
        self.lost.fetch_add(1, Ordering::SeqCst);
    }
    fn register(&self, _: ActiveMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async { Ok(SaveResult::Saved) })
    }
    fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        Box::pin(async move {
            self.discarded.lock().unwrap().push(id);
            Ok(())
        })
    }
}
fn ready(journal: Arc<Journal>, capacity: Arc<Semaphore>) -> PreparedAdmission {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    let id = Uuid::new_v4();
    PreparedAdmission {
        id,
        prepared: PreparedMatch {
            engine: RuleEngine::prepare(board, rules).unwrap(),
            seed: [1; 8],
        },
        participants: [None, None],
        reservation: MatchReservation {
            key: ReservationKey {
                entity: Uuid::new_v4(),
                generation: 1,
            },
            players: [Some(Uuid::new_v4()), None],
            difficulty: Difficulty::Normal,
            opponent: OpponentKind::Bot,
            expires_at: 5000,
        },
        receipt: Receipt {
            id,
            journal,
            _permit: capacity.try_acquire_owned().unwrap(),
            confirmed: false,
        },
    }
}

#[tokio::test]
async fn already_delivered_receipt_is_discarded_on_job_drop_without_losing_owner() {
    let journal = Arc::new(Journal::default());
    let capacity = Arc::new(Semaphore::new(1));
    let prepared = ready(journal.clone(), capacity.clone());
    let id = prepared.id;
    let (completed, result) = oneshot::channel();
    let (cancel, _) = watch::channel(false);
    assert!(completed.send(Ok(prepared)).is_ok());
    drop(Job { result, cancel });
    tokio::time::timeout(Duration::from_secs(1), async {
        while capacity.available_permits() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(journal.discarded.lock().unwrap().as_slice(), &[id]);
    assert_eq!(journal.lost.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn closed_job_returns_late_receipt_to_sender_for_explicit_cleanup() {
    let journal = Arc::new(Journal::default());
    let capacity = Arc::new(Semaphore::new(1));
    let prepared = ready(journal.clone(), capacity.clone());
    let id = prepared.id;
    let (completed, result) = oneshot::channel();
    let (cancel, _) = watch::channel(false);
    drop(Job { result, cancel });
    let Err(Ok(returned)) = completed.send(Ok(prepared)) else {
        panic!("closed receiver must return receipt")
    };
    returned.receipt.discard().await;
    assert_eq!(journal.discarded.lock().unwrap().as_slice(), &[id]);
    assert_eq!(journal.lost.load(Ordering::SeqCst), 0);
    assert_eq!(capacity.available_permits(), 1);
}

#[test]
fn unwinding_after_possible_actor_creation_closes_owner_without_deleting_journal() {
    let journal = Arc::new(Journal::default());
    let capacity = Arc::new(Semaphore::new(1));
    let prepared = ready(journal.clone(), capacity.clone());
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = prepared;
        panic!("controlled interruption between actor publication and receipt confirmation");
    }));
    assert!(panic.is_err());
    assert_eq!(journal.lost.load(Ordering::SeqCst), 1);
    assert!(journal.discarded.lock().unwrap().is_empty());
    assert_eq!(capacity.available_permits(), 1);
}

#[test]
fn committed_receipt_releases_capacity_without_discarding_the_actors_journal() {
    let journal = Arc::new(Journal::default());
    let capacity = Arc::new(Semaphore::new(1));
    let prepared = ready(journal.clone(), capacity.clone());
    prepared.receipt.commit();
    assert_eq!(journal.lost.load(Ordering::SeqCst), 0);
    assert!(journal.discarded.lock().unwrap().is_empty());
    assert_eq!(capacity.available_permits(), 1);
}
