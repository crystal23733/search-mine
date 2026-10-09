use liar_core::{
    board::{Board, CellId},
    game::{EndReason, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
};

fn engine(created: u64) -> RuleEngine {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    RuleEngine::new(board, rules, created).unwrap()
}

#[test]
fn a_missing_initial_connection_cancels_at_the_logical_start_boundary_without_a_winner() {
    let mut game = engine(50000);
    assert_eq!(game.start_ms(), 53000);
    game.advance(52999).unwrap();
    game.cancel_before_start(53000).unwrap();
    game.advance(90000).unwrap();
    for seat in [Seat::One, Seat::Two] {
        let end = game.projection(seat).end.unwrap();
        assert_eq!(end.reason, EndReason::Cancelled);
        assert_eq!(end.at, 53000);
        assert_eq!(end.winner, None);
    }
}

#[test]
fn prestart_cancellation_is_terminal_and_cannot_be_rewritten_by_timeout_or_another_cancel() {
    let mut game = engine(0);
    game.cancel_before_start(1500).unwrap();
    assert_eq!(game.cancel_before_start(1700), Err(Rejection::Finished));
    game.advance(300000).unwrap();
    let end = game.projection(Seat::One).end.unwrap();
    assert_eq!(
        (end.reason, end.at, end.winner),
        (EndReason::Cancelled, 1500, None)
    );
}

#[test]
fn a_running_game_or_invalid_timestamp_cannot_use_initial_cancellation() {
    let mut game = engine(1000);
    game.advance(2500).unwrap();
    let before = game.projection(Seat::One);
    for at in [2499, 4001] {
        assert_eq!(game.cancel_before_start(at), Err(Rejection::InvalidTime));
        assert_eq!(game.projection(Seat::One), before);
    }
    game.advance(4000).unwrap();
    let running = game.projection(Seat::One);
    assert_eq!(game.cancel_before_start(4000), Err(Rejection::InvalidTime));
    assert_eq!(game.projection(Seat::One), running);
    game.advance(244000).unwrap();
    assert_eq!(game.cancel_before_start(244000), Err(Rejection::Finished));
    assert_eq!(
        game.projection(Seat::One).end.unwrap().reason,
        EndReason::Timeout
    );
}
