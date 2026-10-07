use liar_core::{
    board::{Board, CellId, ObservedCell},
    daily::{DailyError, DailyMetadata, DailyPuzzle},
    game::{Action, ActionStatus, Command, EndReason, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
    solver::{NoGuessSolver, SolverBudget},
};
fn input(id: u32, action: Action, time: u64) -> Command {
    Command {
        id: id.into(),
        seq: id.into(),
        epoch: 1,
        seat: Seat::One,
        received_at: time,
        action,
    }
}
#[test]
fn public_daily_metadata_is_date_and_version_bound_and_rejects_noncanonical_dates() {
    let rules = RulesSnapshot::bundled();
    let first = DailyMetadata::derive("2026-10-07", 1, &rules).unwrap();
    assert_eq!(
        first,
        DailyMetadata::derive("2026-10-07", 1, &rules).unwrap()
    );
    assert_ne!(
        first.seed,
        DailyMetadata::derive("2026-10-08", 1, &rules).unwrap().seed
    );
    assert_eq!(first.mode, "solo-v1");
    assert_eq!(first.rules_hash, rules.hash);
    assert!(first.seed.parse::<u64>().is_ok());
    assert!(first.id().starts_with("2026-10-07:v1:"));
    for date in [
        "2026-2-03",
        "2026-02-30",
        "1900-02-29",
        "2100-02-29",
        "2026/10/07",
        "2026-10-07Z",
        "0000-01-01",
        "２０２６-10-07",
    ] {
        assert_eq!(
            DailyMetadata::derive(date, 1, &rules).err(),
            Some(DailyError::InvalidDate)
        );
    }
    for date in ["2000-02-29", "2024-02-29", "2026-12-31", "2027-01-01"] {
        assert!(DailyMetadata::derive(date, 1, &rules).is_ok());
    }
    assert_eq!(
        DailyMetadata::derive("2026-10-07", 2, &rules).err(),
        Some(DailyError::UnsupportedVersion)
    );
    let mut tampered = rules;
    tampered.hash = "forged".into();
    assert_eq!(
        DailyMetadata::derive("2026-10-07", 1, &tampered).err(),
        Some(DailyError::InvalidRules)
    );
}
#[test]
fn daily_uses_one_shared_engine_with_cached_solo_rejections_and_timeout_is_not_clear() {
    let mut puzzle = DailyPuzzle::new("2026-10-07", 1).unwrap();
    let other = DailyPuzzle::new("2026-10-07", 1).unwrap();
    assert_eq!(puzzle.projection().own.cells, other.projection().own.cells);
    let forbidden = input(1, Action::Attack, 3000);
    assert_eq!(
        puzzle.apply(forbidden).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert!(puzzle.apply(forbidden).duplicate);
    assert_eq!(
        puzzle
            .apply(input(1, Action::Accuse(CellId(0)), 3000))
            .status,
        ActionStatus::Rejected(Rejection::CommandConflict)
    );
    assert_eq!(
        puzzle
            .apply(input(2, Action::Accuse(CellId(0)), 3000))
            .status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert_eq!(puzzle.projection().own.stats.accusation_attempts, 0);
    puzzle.advance(243000).unwrap();
    assert!(!puzzle.complete());
    assert_eq!(puzzle.elapsed_ms(), 240000);
    assert_eq!(puzzle.advance(242999), Err(Rejection::InvalidTime));
}
#[test]
fn public_no_lie_solver_clears_the_solo_daily_and_completion_time_freezes() {
    let mut puzzle = DailyPuzzle::new("2026-10-07", 1).unwrap();
    let mut serial = 0;
    while !puzzle.complete() {
        let view = puzzle.projection();
        let deduction = NoGuessSolver::deduce(&view.own.cells, SolverBudget::default()).unwrap();
        let cell = deduction
            .safe
            .into_iter()
            .find(|&cell| view.own.cells.cell(cell) == Some(ObservedCell::Unknown))
            .expect("Certified public daily must make progress");
        serial += 1;
        assert!(serial < 216);
        assert_eq!(
            puzzle
                .apply(input(
                    serial,
                    Action::Open(cell),
                    3000 + u64::from(serial) * 50
                ))
                .status,
            ActionStatus::Applied
        );
    }
    assert_eq!(puzzle.projection().own.cells.opened_safe(), 216);
    assert_eq!(puzzle.elapsed_ms(), serial * 50);
    puzzle.advance(250000).unwrap();
    assert_eq!(puzzle.elapsed_ms(), serial * 50);
}
#[test]
fn automatic_zero_clear_in_solo_mode_is_completion_and_opponent_inputs_are_rejected() {
    let mut puzzle = DailyPuzzle::new("2026-10-07", 1).unwrap();
    let mut wrong_seat = input(1, Action::Open(CellId(2)), 3000);
    wrong_seat.seat = Seat::Two;
    assert_eq!(
        puzzle.apply(wrong_seat).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 4;
    rules.height = 4;
    rules.mines = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut engine = RuleEngine::new_solo(
        Board::from_mines(rules.rules.board_spec(), &[CellId(15)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    assert_eq!(engine.projection(Seat::One).own.cells.opened_safe(), 15);
    engine.advance(3000).unwrap();
    let end = engine.projection(Seat::One).end.unwrap();
    assert_eq!(end.reason, EndReason::Clear);
    assert_eq!(end.winner, Some(Seat::One));
}
