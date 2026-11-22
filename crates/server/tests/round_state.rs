use liar_core::{
    board::{Board, CellId},
    game::multiplayer::PreparedRace,
    random::SeededRng,
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{AckStatus, CellState, PublicAction},
    rooms::ROOM_PROTOCOL_VERSION,
    rounds::{RoundInput, RoundPayload},
};
use liar_server::{
    auth::Nickname,
    online::{RoundParticipant, RoundState},
    rooms::RoundKey,
};
use uuid::Uuid;

#[test]
fn every_socket_seat_gets_only_its_board_and_the_core_freezes_the_four_player_standings() {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    let prepared = PreparedRace::prepare(board, rules, 4).unwrap();
    let engine = prepared.start(0, SeededRng::new(5)).unwrap();
    let key = RoundKey {
        room_id: Uuid::new_v4(),
        round_id: Uuid::new_v4(),
        generation: 1,
    };
    let members: Vec<_> = (0..4)
        .map(|index| RoundParticipant {
            account: Uuid::new_v4(),
            nickname: Nickname::parse(&format!("Member {index}")).unwrap(),
        })
        .collect();
    let ids: Vec<_> = members
        .iter()
        .map(|member| member.account.to_string())
        .collect();
    let mut state = RoundState::new(key, engine, [46; 8], members).unwrap();
    let epoch = state.attach(2, 0).unwrap();
    let private = state.view(2).unwrap();
    assert_eq!(private.opponents.len(), 3);
    assert_eq!(private.own.cells[8].state, CellState::Closed);
    let command = RoundInput {
        v: ROOM_PROTOCOL_VERSION,
        round_id: key.round_id.to_string(),
        command_id: Uuid::new_v4().to_string(),
        client_seq: 1,
        session_epoch: epoch,
        known_revision: private.revision,
        action: PublicAction::Open { cell: 8 },
    };
    assert!(matches!(
        state.apply(2, command.clone(), 3000),
        RoundPayload::Ack {
            status: AckStatus::Applied,
            duplicate: false,
            ..
        }
    ));
    assert!(matches!(
        state.apply(2, command, 3000),
        RoundPayload::Ack {
            status: AckStatus::Applied,
            duplicate: true,
            ..
        }
    ));
    let observer = state.view(0).unwrap();
    assert_eq!(observer.own.cells[8].state, CellState::Closed);
    assert_eq!(state.view(2).unwrap().own.cells[8].state, CellState::Safe);
    let json = serde_json::to_string(&observer).unwrap();
    assert!(ids.iter().all(|account| !json.contains(account)));
    assert_eq!(json.matches("\"cells\"").count(), 1);
    for secret in ["seed", "board_truth", "source_seat", "invite_code"] {
        assert!(!json.contains(secret));
    }
    state.advance(243000).unwrap();
    let saved = state.finished().unwrap();
    assert_eq!(
        saved
            .players
            .iter()
            .map(|player| player.rank)
            .collect::<Vec<_>>(),
        vec![Some(2), Some(2), Some(1), Some(2)]
    );
    assert_eq!(saved.ended_ms, 240000);
}
