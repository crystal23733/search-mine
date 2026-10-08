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
    state_at(0)
}
fn state_at(at: u64) -> MatchState {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    MatchState::new(
        Uuid::new_v4(),
        RuleEngine::new(board, rules, at).unwrap(),
        [Some(Uuid::from_u128(1)), Some(Uuid::from_u128(2))],
        [46; 8],
        at,
    )
    .unwrap()
}
#[test]
fn stored_duration_is_independent_of_server_uptime() {
    let mut state = state_at(9000);
    state.advance(252000);
    assert_eq!(state.finished().unwrap().ended_ms, 243000);
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
fn retried_ack_keeps_its_original_revision_after_later_private_changes() {
    let mut state = state();
    let first = input(&state, 1, 1, PublicAction::Open { cell: 8 });
    let ack = state.apply(Seat::One, first.clone(), 3000);
    let second = input(&state, 2, 1, PublicAction::Flag { cell: 5 });
    state.apply(Seat::One, second, 3001);
    let repeated = state.apply(Seat::One, first, 3002);
    let OnlinePayload::Ack {
        revision: original, ..
    } = ack
    else {
        panic!("Expected ack")
    };
    let OnlinePayload::Ack {
        revision,
        duplicate,
        ..
    } = repeated
    else {
        panic!("Expected duplicate ack")
    };
    assert!(duplicate);
    assert!(state.view(Seat::One).revision > u64::from(original));
    assert_eq!(revision, original);
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
#[test]
fn invalid_commands_do_not_mutate_the_board_and_future_revision_is_rejected() {
    let mut state = state();
    state.advance(3000);
    let before = state.view(Seat::One).own;
    for command_id in ["broken".to_string(), Uuid::nil().to_string()] {
        let mut command = input(&state, 1, 1, PublicAction::Open { cell: 8 });
        command.command_id = command_id;
        assert!(matches!(
            state.apply(Seat::One, command, 3000),
            OnlinePayload::Error { .. }
        ));
    }
    let mut future = input(&state, 1, 1, PublicAction::Open { cell: 8 });
    future.known_revision = u32::MAX;
    assert!(matches!(
        state.apply(Seat::One, future, 3000),
        OnlinePayload::Ack {
            error: Some(PublicError::Stale),
            ..
        }
    ));
    assert_eq!(state.view(Seat::One).own, before);
    assert_eq!(state.seat(Uuid::from_u128(1)).unwrap(), Seat::One);
    assert_eq!(state.seat(Uuid::from_u128(2)).unwrap(), Seat::Two);
    assert!(state.seat(Uuid::new_v4()).is_err());
    for action in [PublicAction::Attack, PublicAction::Accuse { cell: 8 }] {
        let command = input(&state, 2, 1, action);
        assert!(matches!(
            state.apply(Seat::One, command, 3000),
            OnlinePayload::Ack {
                status: AckStatus::Rejected,
                ..
            }
        ));
    }
}
#[test]
fn proof_results_are_committed_only_for_the_same_observation_revision() {
    let mut state = state();
    let stale = state.proof_work(Seat::One).run().unwrap();
    let command = input(&state, 1, 1, PublicAction::Open { cell: 8 });
    state.apply(Seat::One, command, 3000);
    assert!(!state.commit_proof(stale));
    let fresh = state.proof_work(Seat::One).run().unwrap();
    assert!(state.commit_proof(fresh));
}
#[test]
fn disconnect_and_resume_obey_the_grace_boundary_and_stale_epoch_cannot_disconnect() {
    let mut state = state();
    let epoch = state.attach(Seat::One, 0).unwrap();
    state.disconnect(Seat::One, epoch - 1, 3000);
    state.advance(34000);
    assert_eq!(state.view(Seat::One).phase, GamePhase::Playing);
    state.disconnect(Seat::One, epoch, 34000);
    let next = state.attach(Seat::One, 63999).unwrap();
    state.disconnect(Seat::One, next, 64000);
    assert!(state.attach(Seat::One, 94000).is_err());
    assert_eq!(state.view(Seat::One).phase, GamePhase::Finished);
    assert!(state.finished().is_some());
}
#[test]
fn mine_stun_uses_an_absolute_deadline_and_does_not_emit_time_only_deltas() {
    let mut state = state();
    let command = input(&state, 1, 1, PublicAction::Open { cell: 5 });
    state.apply(Seat::One, command, 3000);
    assert!(state.view(Seat::One).own.stun_ms > 0);
    assert!(state.view(Seat::Two).opponent.stun_ms > 0);
    state.take_changes();
    state.advance(3001);
    assert!(state.take_changes().is_empty());
    state.advance(10000);
    assert_eq!(state.view(Seat::One).own.stun_ms, 0);
    assert_eq!(state.take_changes().len(), 2);
}
#[test]
fn invalid_match_identity_and_participants_are_rejected() {
    for (id, players) in [
        (Uuid::nil(), [Some(Uuid::new_v4()), None]),
        (Uuid::new_v4(), [None, None]),
        (Uuid::new_v4(), [Some(Uuid::nil()), None]),
        (
            Uuid::new_v4(),
            [Some(Uuid::from_u128(1)), Some(Uuid::from_u128(1))],
        ),
    ] {
        let mut rules = RulesSnapshot::bundled().rules;
        rules.width = 3;
        rules.height = 3;
        rules.mines = 2;
        let rules = RulesSnapshot::from_rules(rules).unwrap();
        let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
        assert!(
            MatchState::new(
                id,
                RuleEngine::new(board, rules, 0).unwrap(),
                players,
                [46; 8],
                0
            )
            .is_err()
        );
    }
}
