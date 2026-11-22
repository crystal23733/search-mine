use liar_core::{
    board::{Board, CellId},
    game::multiplayer::PreparedRace,
    random::SeededRng,
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{GamePhase, PublicAction},
    online::{OnlineError, RecordingStatus},
    rounds::{RoundInput, RoundPayload},
};
use liar_server::{
    auth::{AuthClock, Nickname},
    lobby::LobbyAuthentication,
    online::*,
    rooms::RoundKey,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
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
struct Stored {
    starts: Mutex<Vec<Uuid>>,
    saved: Mutex<Vec<FinishedRace>>,
    discarded: Mutex<Vec<Uuid>>,
}
impl ResultRepository for Stored {
    fn save(&self, _: FinishedMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async { Ok(SaveResult::Saved) })
    }
}
impl AdmissionJournal for Stored {
    fn fail_closed(&self) {}
    fn register(&self, _: ActiveMatch) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        Box::pin(async { Ok(SaveResult::Saved) })
    }
    fn discard(&self, id: Uuid) -> PortFuture<'_, Result<(), OnlineError>> {
        self.discarded.lock().unwrap().push(id);
        Box::pin(async { Ok(()) })
    }
    fn start_race(&self, id: Uuid) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.starts.lock().unwrap().push(id);
        Box::pin(async { Ok(SaveResult::Saved) })
    }
    fn save_race(&self, race: FinishedRace) -> PortFuture<'_, Result<SaveResult, OnlineError>> {
        self.saved.lock().unwrap().push(race);
        Box::pin(async { Ok(SaveResult::Saved) })
    }
}

struct Room;
impl RoundLifecycle for Room {
    fn contains(&self, _: RoundKey, _: Uuid) -> bool {
        true
    }
    fn begin(&self, _: RoundKey, _: u64, _: &[ConnectionAuthority]) -> bool {
        true
    }
    fn complete(&self, _: RoundKey, _: u64, _: bool) {}
}

#[tokio::test]
async fn race_actor_starts_after_all_four_initial_sockets_and_saves_the_first_clear_once() {
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let writer = Arc::new(Stored::default());
    let sockets = AuthorityRegistry::new(8).unwrap();
    let lobby = AuthorityRegistry::new(8).unwrap();
    let registry = MatchRegistry::new(
        MatchLimits {
            matches: 1,
            mailbox: 16,
            outgoing: 16,
            proof_workers: 1,
        },
        clock.clone(),
        clock.clone(),
        sockets.clone(),
        writer.clone(),
    )
    .unwrap();
    let accounts = [(); 4].map(|()| Uuid::new_v4());
    let leases = accounts
        .iter()
        .enumerate()
        .map(|(seat, &account)| {
            lobby
                .bind_shared(0, account, [(seat + 1) as u8; 32], 20000, 18000)
                .unwrap()
        })
        .collect();
    let key = RoundKey {
        room_id: Uuid::new_v4(),
        round_id: Uuid::new_v4(),
        generation: 1,
    };
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    let game = PreparedRace::prepare(board, rules, 4)
        .unwrap()
        .start(0, SeededRng::new(11))
        .unwrap();
    let players = accounts
        .iter()
        .enumerate()
        .map(|(seat, &account)| RoundParticipant {
            account,
            nickname: Nickname::parse(&format!("Player {seat}")).unwrap(),
        })
        .collect();
    let state = RoundState::new(key, game, [2; 8], players).unwrap();
    let slots = ConnectionSlots::new(4).unwrap();
    let reservation = registry
        .reserve_race(key.round_id, &accounts, &slots)
        .unwrap();
    let round = registry
        .admit_round(
            reservation,
            state,
            RoundAdmission {
                authentication: LobbyAuthentication {
                    authorities: lobby,
                    clock: clock.clone(),
                },
                leases,
                lifecycle: Arc::new(Room),
                journal: writer.clone(),
            },
        )
        .unwrap();
    let mut connections = Vec::new();
    for (seat, &account) in accounts.iter().enumerate() {
        let authority = sockets
            .bind(0, account, [(seat + 1) as u8; 32], 20000, 18000)
            .unwrap();
        let mut connection = round.connect(authority).await.unwrap();
        let event = connection.next().await.unwrap();
        assert!(
            matches!(event.payload, RoundPayload::Awaiting { own_seat, .. } if usize::from(own_seat) == seat)
        );
        connections.push(connection);
    }
    assert!(writer.starts.lock().unwrap().is_empty());
    clock.0.store(3000, Ordering::SeqCst);
    let winner = &mut connections[3];
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(event) = winner.next().await {
            if matches!(event.payload, RoundPayload::Delta { view } if view.phase == GamePhase::Playing) { return; }
        }
        panic!("Round closed before all four could start");
    }).await.unwrap();
    assert_eq!(*writer.starts.lock().unwrap(), vec![key.round_id]);
    registry.begin_drain().unwrap();
    assert!(!registry.drained().unwrap());
    let open = |socket: &RoundConnection, seq, cell| {
        socket
            .send(RoundInput {
                v: 2,
                round_id: key.round_id.to_string(),
                command_id: Uuid::new_v4().to_string(),
                client_seq: seq,
                session_epoch: socket.epoch(),
                known_revision: 0,
                action: PublicAction::Open { cell },
            })
            .unwrap();
    };
    for seat in [0, 3] {
        for (seq, cell) in [(1, 8), (2, 2)] {
            open(&connections[seat], seq, cell);
        }
    }
    // Both final moves have the same server ingress time but enter in reverse seat order.
    open(&connections[3], 3, 6);
    open(&connections[0], 3, 6);
    let observer = &mut connections[3];
    let end = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let RoundPayload::End {
                view,
                recording: RecordingStatus::Saved,
            } = observer.next().await.unwrap().payload
            {
                return view.result.unwrap();
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(end.winner, Some(0));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !registry.drained().unwrap() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let saved = writer.saved.lock().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].players[0].account, accounts[0]);
    assert_eq!(saved[0].players[0].rank, Some(1));
}
