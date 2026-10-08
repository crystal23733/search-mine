use liar_core::{
    board::{Board, CellId},
    game::{RuleEngine, Seat},
    rules::RulesSnapshot,
};
use liar_protocol::{
    game::{AckStatus, GamePhase, PublicAction, PublicError},
    online::{OnlineInput, OnlinePayload},
};
use liar_server::online::MatchState;
use uuid::Uuid;

fn state() -> MatchState {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    MatchState::new(
        Uuid::new_v4(),
        RuleEngine::new(board, rules, 0).unwrap(),
        [Some(Uuid::from_u128(1)), Some(Uuid::from_u128(2))],
        [46; 8],
    )
    .unwrap()
}
fn input(state: &MatchState, sequence: u32, epoch: u32, action: PublicAction) -> OnlineInput {
    OnlineInput {
        v: 1,
        match_id: state.id().to_string(),
        command_id: Uuid::from_u128(u128::from(sequence)).to_string(),
        client_seq: sequence,
        session_epoch: epoch,
        known_revision: 0,
        action,
    }
}
#[test]
fn own_open_is_idempotent_and_opponent_receives_only_aggregate_progress() {
    let mut state = state();
    let command = input(&state, 1, 1, PublicAction::Open { cell: 8 });
    let ack = state.apply(Seat::One, command.clone(), 3000);
    assert!(matches!(
        ack,
        OnlinePayload::Ack {
            status: AckStatus::Applied,
            duplicate: false,
            ..
        }
    ));
    let changes = state.take_changes();
    assert_eq!(changes.len(), 2);
    assert_eq!(state.view(Seat::One).own.gauge, 1);
    let opponent = state.view(Seat::Two);
    assert_eq!(opponent.own.gauge, 0);
    assert_eq!(opponent.own.cells[8].number, None);
    assert_eq!(opponent.opponent.opened_safe, 5);
    assert!(matches!(
        state.apply(Seat::One, command, 3000),
        OnlinePayload::Ack {
            duplicate: true,
            ..
        }
    ));
    assert!(state.take_changes().is_empty());
    assert_eq!(state.view(Seat::One).own.gauge, 1);
}
#[test]
fn private_flag_does_not_change_opponent_stream_revision_or_send_empty_delta() {
    let mut state = state();
    state.advance(3000);
    state.take_changes();
    let revision = state.view(Seat::Two).revision;
    let command = input(&state, 1, 1, PublicAction::Flag { cell: 8 });
    state.apply(Seat::One, command, 3001);
    assert_eq!(state.view(Seat::Two).revision, revision);
    let changes = state.take_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, Seat::One);
}
#[test]
fn exact_deadline_finishes_once_and_rejects_later_action() {
    let mut state = state();
    state.advance(243000);
    let view = state.view(Seat::One);
    assert_eq!(view.phase, GamePhase::Finished);
    assert!(view.result.unwrap().completed);
    state.take_changes();
    let command = input(&state, 1, 1, PublicAction::Open { cell: 8 });
    assert!(matches!(
        state.apply(Seat::One, command, 243000),
        OnlinePayload::Ack {
            status: AckStatus::Rejected,
            error: Some(PublicError::Finished),
            ..
        }
    ));
    assert!(state.take_changes().is_empty());
}
#[test]
fn replacing_connection_invalidates_old_epoch_and_foreign_match_input() {
    let mut state = state();
    let epoch = state.attach(Seat::One, 0).unwrap();
    assert_eq!(epoch, 2);
    let old = input(&state, 1, 1, PublicAction::Flag { cell: 8 });
    assert!(matches!(
        state.apply(Seat::One, old, 3000),
        OnlinePayload::Ack {
            error: Some(PublicError::InvalidEpoch),
            ..
        }
    ));
    let mut foreign = input(&state, 1, epoch, PublicAction::Flag { cell: 8 });
    foreign.match_id = Uuid::new_v4().to_string();
    assert!(matches!(
        state.apply(Seat::One, foreign, 3000),
        OnlinePayload::Error { .. }
    ));
    assert!(!state.view(Seat::One).own.cells[8].flagged);
}
