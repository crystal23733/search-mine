use liar_core::{
    board::{Board, CellId},
    game::{Action, Command, EndReason, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
};
fn rules() -> RulesSnapshot {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    RulesSnapshot::from_rules(rules).unwrap()
}
fn board() -> Board {
    Board::from_mines(rules().rules.board_spec(), &[CellId(5), CellId(7)]).unwrap()
}

#[test]
fn prepared_work_starts_a_fresh_full_countdown_at_admission_time() {
    let prepared =
        RuleEngine::prepare(board(), rules()).unwrap_or_else(|error| panic!("{error:?}"));
    let mut engine = prepared.start(50000).unwrap();
    assert_eq!(engine.projection(Seat::One).countdown_ms, 3000);
    assert_eq!(engine.projection(Seat::Two).remaining_ms, 240000);
    assert!(engine.projection(Seat::One).end.is_none());
    engine.advance(53000).unwrap();
    assert_eq!(engine.projection(Seat::One).countdown_ms, 0);
    assert_eq!(engine.projection(Seat::One).remaining_ms, 240000);
}

#[test]
fn legacy_constructor_and_prepared_start_keep_the_same_projection_and_transition() {
    let mut old = RuleEngine::new(board(), rules(), 9000).unwrap();
    let mut prepared = RuleEngine::prepare(board(), rules())
        .unwrap_or_else(|error| panic!("{error:?}"))
        .start(9000)
        .unwrap();
    for at in [9000, 12000, 12100] {
        old.advance(at).unwrap();
        prepared.advance(at).unwrap();
        for seat in [Seat::One, Seat::Two] {
            assert_eq!(old.projection(seat), prepared.projection(seat));
        }
    }
    let command = Command {
        id: 1,
        seq: 1,
        epoch: 1,
        seat: Seat::One,
        received_at: 12100,
        action: Action::Open(CellId(8)),
    };
    assert_eq!(old.apply(command), prepared.apply(command));
    for seat in [Seat::One, Seat::Two] {
        assert_eq!(old.projection(seat), prepared.projection(seat));
    }
}

#[test]
fn preparation_rejects_tampered_rules_and_nonzero_or_mismatched_board() {
    let mut invalid = rules();
    invalid.hash.push('x');
    assert!(matches!(
        RuleEngine::prepare(board(), invalid),
        Err(Rejection::InvalidRules)
    ));
    let bad_opening =
        Board::from_mines(rules().rules.board_spec(), &[CellId(0), CellId(7)]).unwrap();
    assert!(matches!(
        RuleEngine::prepare(bad_opening, rules()),
        Err(Rejection::InvalidBoard)
    ));
    let mut different = rules().rules;
    different.width = 4;
    different.height = 4;
    let different = RulesSnapshot::from_rules(different).unwrap();
    let different_board =
        Board::from_mines(different.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    assert!(matches!(
        RuleEngine::prepare(different_board, rules()),
        Err(Rejection::InvalidBoard)
    ));
}

#[test]
fn start_checks_clock_overflow_and_the_last_representable_deadline() {
    let prepared =
        RuleEngine::prepare(board(), rules()).unwrap_or_else(|error| panic!("{error:?}"));
    assert!(matches!(
        prepared.start(u64::MAX),
        Err(Rejection::InvalidTime)
    ));
    let mut engine = RuleEngine::prepare(board(), rules())
        .unwrap_or_else(|error| panic!("{error:?}"))
        .start(u64::MAX - 243000)
        .unwrap();
    assert_eq!(engine.projection(Seat::One).countdown_ms, 3000);
    engine.advance(u64::MAX).unwrap();
    assert_eq!(
        engine.projection(Seat::One).end.unwrap().reason,
        EndReason::Timeout
    );
}
