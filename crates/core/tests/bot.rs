use liar_core::{
    board::{Board, CellId},
    bot::{BotPolicy, Clock, Difficulty},
    game::{Action, RuleEngine, Seat},
    random::SeededRng,
    rules::RulesSnapshot,
};
struct FakeClock(u64);
impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}
fn game() -> RuleEngine {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut game = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    game.advance(3000).unwrap();
    game
}
#[test]
fn public_proof_selects_a_safe_action_and_difficulty_changes_decision_interval() {
    let view = game().projection(Seat::One);
    for (difficulty, interval) in [
        (Difficulty::Easy, 900),
        (Difficulty::Normal, 500),
        (Difficulty::Hard, 180),
    ] {
        let mut bot = BotPolicy::new(difficulty, &view).unwrap();
        let mut random = SeededRng::new(5);
        assert_eq!(
            bot.choose(&view, &FakeClock(3000), &mut random).unwrap(),
            Some(Action::Open(CellId(8)))
        );
        assert_eq!(
            bot.choose(&view, &FakeClock(3000 + interval - 1), &mut random)
                .unwrap(),
            None
        );
        assert_eq!(
            bot.choose(&view, &FakeClock(3000 + interval), &mut random)
                .unwrap(),
            Some(Action::Open(CellId(8)))
        );
    }
}
#[test]
fn invalid_bot_profile_does_not_produce_a_valid_rule_snapshot() {
    let source = include_str!("../../../config/game-rules.toml");
    assert!(RulesSnapshot::load(&source.replace("max_nodes = 5000\n", "max_nodes = 0\n")).is_err());
}
fn action(id: u128, seat: Seat, at: u64, action: Action) -> liar_core::game::Command {
    liar_core::game::Command {
        id,
        seq: id as u64,
        seat,
        received_at: at,
        epoch: 1,
        action,
    }
}
#[test]
fn different_hidden_boards_with_identical_public_views_produce_identical_intents() {
    let first = game();
    let rules = first.projection(Seat::One).rules.clone();
    let mut second = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(2), CellId(6)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    second.advance(3000).unwrap();
    let a = first.projection(Seat::One);
    let b = second.projection(Seat::One);
    assert_eq!(a, b);
    let mut first_bot = BotPolicy::new(Difficulty::Hard, &a).unwrap();
    let mut second_bot = BotPolicy::new(Difficulty::Hard, &b).unwrap();
    assert_eq!(
        first_bot
            .choose(&a, &FakeClock(3000), &mut SeededRng::new(12))
            .unwrap(),
        second_bot
            .choose(&b, &FakeClock(3000), &mut SeededRng::new(12))
            .unwrap()
    );
}
#[test]
fn canonical_history_identifies_a_lie_even_when_easy_planning_budget_is_tiny() {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    rules.gauge_capacity = 1;
    rules.bots.easy.max_constraints = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut g = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    for seat in [Seat::One, Seat::Two] {
        g.apply(action(1, seat, 3000, Action::Open(CellId(8))));
    }
    g.commit_proof(g.proof_work(Seat::Two).run().unwrap())
        .unwrap();
    g.apply(action(2, Seat::One, 3000, Action::Attack));
    g.apply(action(2, Seat::Two, 3000, Action::Open(CellId(2))));
    let view = g.projection(Seat::Two);
    let mut bot = BotPolicy::new(Difficulty::Easy, &view).unwrap();
    let mut rng = SeededRng::new(0);
    assert_eq!(bot.choose(&view, &FakeClock(3000), &mut rng).unwrap(), None);
    assert_eq!(bot.choose(&view, &FakeClock(4499), &mut rng).unwrap(), None);
    assert_eq!(
        bot.choose(&view, &FakeClock(4500), &mut rng).unwrap(),
        Some(Action::Accuse(CellId(2)))
    );
}
#[test]
fn flagged_safe_cell_is_unflagged_before_opening() {
    let mut g = game();
    g.apply(action(1, Seat::One, 3000, Action::ToggleFlag(CellId(8))));
    let mut bot = BotPolicy::new(Difficulty::Easy, &g.projection(Seat::One)).unwrap();
    let mut rng = SeededRng::new(4);
    assert_eq!(
        bot.choose(&g.projection(Seat::One), &FakeClock(3000), &mut rng)
            .unwrap(),
        Some(Action::ToggleFlag(CellId(8)))
    );
    g.apply(action(2, Seat::One, 3900, Action::ToggleFlag(CellId(8))));
    assert_eq!(
        bot.choose(&g.projection(Seat::One), &FakeClock(3900), &mut rng)
            .unwrap(),
        Some(Action::Open(CellId(8)))
    );
}
#[test]
fn failed_attack_does_not_block_safe_progress_and_public_progress_allows_a_retry() {
    let mut rules = game().projection(Seat::One).rules.rules;
    rules.gauge_capacity = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut g = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    g.apply(action(1, Seat::One, 3000, Action::Open(CellId(8))));
    let mut bot = BotPolicy::new(Difficulty::Normal, &g.projection(Seat::One)).unwrap();
    let mut rng = SeededRng::new(0);
    assert_eq!(
        bot.choose(&g.projection(Seat::One), &FakeClock(3000), &mut rng)
            .unwrap(),
        Some(Action::Attack)
    );
    g.apply(action(2, Seat::One, 3000, Action::Attack));
    g.advance(3500).unwrap();
    assert!(matches!(
        bot.choose(&g.projection(Seat::One), &FakeClock(3500), &mut rng)
            .unwrap(),
        Some(Action::Open(CellId(2) | CellId(6)))
    ));
    g.apply(action(1, Seat::Two, 4000, Action::Open(CellId(8))));
    assert_eq!(
        bot.choose(&g.projection(Seat::One), &FakeClock(4000), &mut rng)
            .unwrap(),
        Some(Action::Attack)
    );
}
#[test]
fn countdown_stun_terminal_and_time_reversal_never_produce_an_action() {
    use liar_core::bot::BotError;
    let mut g = game();
    let mut view = g.projection(Seat::One);
    let mut bot = BotPolicy::new(Difficulty::Hard, &view).unwrap();
    let mut rng = SeededRng::new(0);
    view.countdown_ms = 1;
    assert_eq!(bot.choose(&view, &FakeClock(3000), &mut rng).unwrap(), None);
    view.countdown_ms = 0;
    view.own.stun_ms = 1;
    assert_eq!(bot.choose(&view, &FakeClock(3001), &mut rng).unwrap(), None);
    assert_eq!(
        bot.choose(&view, &FakeClock(3000), &mut rng),
        Err(BotError::InvalidTime)
    );
    g.abort(3002).unwrap();
    assert_eq!(
        bot.choose(&g.projection(Seat::One), &FakeClock(3002), &mut rng)
            .unwrap(),
        None
    );
}
#[test]
fn malformed_or_regressed_public_history_is_rejected_without_replacing_knowledge() {
    use liar_core::{board::ObservedCell, bot::BotError};
    let g = game();
    let view = g.projection(Seat::One);
    let mut bad = view.clone();
    bad.own.flags.pop();
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    bad = view.clone();
    bad.rules.hash.clear();
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    bad = view.clone();
    bad.own.history.clear();
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    bad = view.clone();
    let update = bad.own.history[0][0];
    bad.own.history[0].push(update);
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    bad = view.clone();
    bad.own.history[0][0].cell = CellId(999);
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    bad = view.clone();
    bad.own
        .cells
        .set(CellId(8), ObservedCell::Number(2))
        .unwrap();
    assert!(BotPolicy::new(Difficulty::Hard, &bad).is_err());
    let mut bot = BotPolicy::new(Difficulty::Hard, &view).unwrap();
    let mut rng = SeededRng::new(0);
    bad = view.clone();
    bad.own.history.push(Vec::new());
    assert_eq!(
        bot.choose(&bad, &FakeClock(3000), &mut rng),
        Err(BotError::InvalidSnapshot)
    );
    bad = view.clone();
    bad.own.history[0][0].observed = ObservedCell::Unknown;
    assert_eq!(
        bot.choose(&bad, &FakeClock(3000), &mut rng),
        Err(BotError::InvalidSnapshot)
    );
    bad = view.clone();
    bad.rules.hash.clear();
    assert_eq!(
        bot.choose(&bad, &FakeClock(3000), &mut rng),
        Err(BotError::InvalidSnapshot)
    );
    assert_eq!(
        bot.choose(&view, &FakeClock(3000), &mut rng).unwrap(),
        Some(Action::Open(CellId(8)))
    );
}
#[test]
fn tiny_planning_budget_waits_and_an_ambiguous_board_never_guesses() {
    use liar_core::board::Observation;
    let mut view = game().projection(Seat::One);
    let mut rules = view.rules.rules.clone();
    rules.bots.easy.max_constraints = 1;
    view.rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut bot = BotPolicy::new(Difficulty::Easy, &view).unwrap();
    assert_eq!(
        bot.choose(&view, &FakeClock(3000), &mut SeededRng::new(0))
            .unwrap(),
        None
    );
    let mut rules = view.rules.rules;
    rules.width = 4;
    rules.height = 4;
    rules.mines = 4;
    view.rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(
        view.rules.rules.board_spec(),
        &[CellId(2), CellId(7), CellId(9), CellId(14)],
    )
    .unwrap();
    let mut initial = Observation::closed(board.spec()).unwrap();
    board.reveal(&mut initial, CellId(0), &[false; 16]).unwrap();
    let mut ledger = liar_core::policy::PolicyKnowledge::start(
        &initial,
        liar_core::solver::SolverBudget::default(),
    )
    .unwrap();
    while let Some(cell) = ledger.safe().first().copied() {
        board.reveal(&mut initial, cell, &[false; 16]).unwrap();
        ledger.observe(&initial).unwrap();
        ledger.refresh().unwrap();
    }
    assert!(initial.opened_safe() < board.safe_total());
    view.own.history = ledger.public_history().to_vec();
    view.own.cells = initial;
    view.own.flags = vec![false; 16];
    let mut bot = BotPolicy::new(Difficulty::Hard, &view).unwrap();
    let mut rng = SeededRng::new(0);
    for tick in [3000, 4000, 5000] {
        assert_eq!(bot.choose(&view, &FakeClock(tick), &mut rng).unwrap(), None);
    }
}
#[test]
fn hard_bot_completes_certified_boards_using_only_public_projections() {
    use liar_core::generator::{BoardGenerator, GenerationBudget};
    for seed in 0..16 {
        let rules = RulesSnapshot::bundled();
        let board =
            BoardGenerator::generate(rules.rules.board_spec(), seed, GenerationBudget::default())
                .unwrap()
                .board;
        let mut g = RuleEngine::new(board, rules, 0).unwrap();
        let mut bot = BotPolicy::new(Difficulty::Hard, &g.projection(Seat::One)).unwrap();
        let mut rng = SeededRng::new(seed + 4000);
        let mut seq = 1;
        for tick in (3000..243000).step_by(200) {
            g.advance(tick).unwrap();
            let view = g.projection(Seat::One);
            if view.end.is_some() {
                break;
            }
            if let Some(intent) = bot.choose(&view, &FakeClock(tick), &mut rng).unwrap() {
                let ack = g.apply(action(seq, Seat::One, tick, intent));
                seq += 1;
                assert!(!matches!(
                    ack.status,
                    liar_core::game::ActionStatus::Rejected(
                        liar_core::game::Rejection::InvalidBoard
                    )
                ));
            }
        }
        let end = g
            .projection(Seat::One)
            .end
            .expect("must finish before timeout");
        assert_eq!(end.reason, liar_core::game::EndReason::Clear, "seed {seed}");
        assert_eq!(end.winner, Some(Seat::One));
    }
}
