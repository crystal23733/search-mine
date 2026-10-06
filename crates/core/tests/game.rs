use liar_core::{
    board::{Board, CellId},
    game::{Action, ActionStatus, Command, Rejection, RuleEngine, Seat},
    rules::RulesSnapshot,
};
fn game() -> RuleEngine {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap()
}
fn command(id: u128, seat: Seat, at: u64, action: Action) -> Command {
    Command {
        id,
        seq: id as u64,
        epoch: 1,
        seat,
        received_at: at,
        action,
    }
}
#[test]
fn initial_zero_has_no_gauge_and_open_adds_only_new_safe_cells() {
    let mut g = game();
    let initial = g.projection(Seat::One);
    assert_eq!(initial.own.gauge, 0);
    assert_eq!(initial.own.cells.opened_safe(), 4);
    let c = command(1, Seat::One, 3000, Action::Open(CellId(8)));
    assert_eq!(g.apply(c).status, ActionStatus::Applied);
    assert_eq!(g.projection(Seat::One).own.gauge, 1);
    assert_eq!(g.projection(Seat::Two).own.gauge, 0);
    assert!(g.apply(c).duplicate);
    assert_eq!(g.projection(Seat::One).own.gauge, 1);
    assert_eq!(
        g.apply(command(2, Seat::One, 3001, Action::Open(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::AlreadyOpen)
    );
}
#[test]
fn mine_and_false_accusation_stun_until_the_exact_boundary() {
    let mut g = game();
    assert_eq!(
        g.apply(command(1, Seat::One, 3000, Action::Open(CellId(5))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::One).own.gauge, 0);
    assert_eq!(g.projection(Seat::One).own.stun_ms, 3000);
    assert_eq!(
        g.apply(command(2, Seat::One, 5999, Action::Open(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::Stunned)
    );
    assert_eq!(
        g.apply(command(3, Seat::One, 6000, Action::Open(CellId(8))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.apply(command(4, Seat::One, 6001, Action::Accuse(CellId(8))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::One).own.stun_ms, 3000);
}
#[test]
fn finished_match_clock_freezes_at_the_terminal_commit() {
    let mut g = game();
    for (id, cell) in [(1, 8), (2, 2), (3, 6)] {
        assert_eq!(
            g.apply(command(
                id,
                Seat::One,
                3000 + id as u64,
                Action::Open(CellId(cell))
            ))
            .status,
            ActionStatus::Applied
        );
    }
    let frozen = g.projection(Seat::One).remaining_ms;
    assert!(g.projection(Seat::One).end.is_some());
    g.advance(10000).unwrap();
    assert_eq!(g.projection(Seat::One).remaining_ms, frozen);
}

fn tuned_game(capacity: u16, max_commands: u16) -> RuleEngine {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    rules.gauge_capacity = capacity;
    rules.max_commands_per_seat = max_commands;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap(),
        rules,
        0,
    )
    .unwrap()
}
#[test]
fn candidate_failure_keeps_full_gauge_and_two_attacks_reflect_on_correct_accusation() {
    let mut g = tuned_game(1, 8192);
    g.apply(command(1, Seat::One, 3000, Action::Open(CellId(8))));
    assert_eq!(
        g.apply(command(2, Seat::One, 3001, Action::Attack)).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert_eq!(g.projection(Seat::One).own.gauge, 1);
    g.apply(command(1, Seat::Two, 3002, Action::Open(CellId(8))));
    let job = g.proof_work(Seat::Two).run().unwrap();
    g.commit_proof(job).unwrap();
    let before = g.projection(Seat::Two).own.clone();
    assert_eq!(
        g.apply(command(3, Seat::One, 3003, Action::Attack)).status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::Two).own, before);
    assert_eq!(g.projection(Seat::One).own.gauge, 0);
    g.apply(command(4, Seat::One, 3004, Action::Open(CellId(2))));
    assert_eq!(
        g.apply(command(5, Seat::One, 3005, Action::Attack)).status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.apply(command(2, Seat::Two, 3006, Action::Open(CellId(2))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.projection(Seat::Two).own.cells.cell(CellId(2)),
        Some(liar_core::board::ObservedCell::Number(2))
    );
    let accuse = command(3, Seat::Two, 3007, Action::Accuse(CellId(2)));
    assert_eq!(g.apply(accuse).status, ActionStatus::Applied);
    assert_eq!(
        g.projection(Seat::Two).own.cells.cell(CellId(2)),
        Some(liar_core::board::ObservedCell::Number(1))
    );
    assert_eq!(g.projection(Seat::One).own.stun_ms, 2000);
    assert_eq!(g.projection(Seat::One).own.gauge, 0);
    assert!(
        g.apply(Command {
            received_at: 3008,
            ..accuse
        })
        .duplicate
    );
    assert_eq!(g.projection(Seat::One).own.stun_ms, 1999);
    assert_eq!(
        g.apply(command(6, Seat::One, 5006, Action::ToggleFlag(CellId(6))))
            .status,
        ActionStatus::Rejected(Rejection::Stunned)
    );
    assert_eq!(
        g.apply(command(7, Seat::One, 5007, Action::ToggleFlag(CellId(6))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.apply(command(4, Seat::Two, 5008, Action::Accuse(CellId(2))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::Two).own.stun_ms, 3000);
}
#[test]
fn flags_are_untrusted_memory_and_flood_gain_is_capped() {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 4;
    rules.height = 4;
    rules.mines = 2;
    rules.gauge_capacity = 2;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut g = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(6), CellId(9)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    assert_eq!(
        g.apply(command(1, Seat::One, 3000, Action::ToggleFlag(CellId(10))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.apply(command(2, Seat::One, 3001, Action::Open(CellId(10))))
            .status,
        ActionStatus::Rejected(Rejection::Flagged)
    );
    let initial = g.projection(Seat::One).own.cells.opened_safe();
    assert_eq!(
        g.apply(command(3, Seat::One, 3002, Action::Open(CellId(15))))
            .status,
        ActionStatus::Applied
    );
    let p = g.projection(Seat::One);
    assert!(p.own.cells.opened_safe() > initial + 2);
    assert_eq!(p.own.gauge, 2);
    assert_eq!(
        p.own.cells.cell(CellId(10)),
        Some(liar_core::board::ObservedCell::Unknown)
    );
    assert_eq!(
        g.apply(command(4, Seat::One, 3003, Action::ToggleFlag(CellId(10))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        g.apply(command(5, Seat::One, 3004, Action::Open(CellId(10))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::One).own.gauge, 2);
}
#[test]
fn timeout_equality_rejects_actions_and_progress_ties_draw() {
    use liar_core::game::EndReason;
    let mut g = game();
    g.apply(command(1, Seat::One, 3000, Action::Open(CellId(8))));
    assert_eq!(
        g.apply(command(2, Seat::One, 243000, Action::Open(CellId(2))))
            .status,
        ActionStatus::Rejected(Rejection::Finished)
    );
    let end = g.projection(Seat::One).end.unwrap();
    assert_eq!(end.reason, EndReason::Timeout);
    assert_eq!(end.winner, Some(Seat::One));
    assert_eq!(end.at, 243000);
    let mut tied = game();
    tied.advance(243000).unwrap();
    assert_eq!(tied.projection(Seat::One).end.unwrap().winner, None);
}
#[test]
fn equal_tick_batch_commits_seat_order_then_stops_other_actions() {
    let mut g = game();
    for seat in [Seat::One, Seat::Two] {
        for (id, cell) in [(1, 8), (2, 2)] {
            g.apply(command(id, seat, 3000, Action::Open(CellId(cell))));
        }
    }
    let acks = g.apply_batch(vec![
        command(3, Seat::Two, 3001, Action::Open(CellId(6))),
        command(3, Seat::One, 3001, Action::Open(CellId(6))),
    ]);
    assert_eq!(acks[0].status, ActionStatus::Applied);
    assert_eq!(acks[1].status, ActionStatus::Rejected(Rejection::Finished));
    assert_eq!(g.projection(Seat::One).end.unwrap().winner, Some(Seat::One));
}
#[test]
fn cached_rejections_sequence_epoch_conflicts_and_command_limits_are_bounded() {
    let mut g = tuned_game(20, 3);
    let too_early = command(1, Seat::One, 2999, Action::Open(CellId(8)));
    assert_eq!(
        g.apply(too_early).status,
        ActionStatus::Rejected(Rejection::NotStarted)
    );
    assert!(
        g.apply(Command {
            received_at: 3000,
            ..too_early
        })
        .duplicate
    );
    assert_eq!(
        g.apply(Command {
            action: Action::Attack,
            received_at: 3000,
            ..too_early
        })
        .status,
        ActionStatus::Rejected(Rejection::CommandConflict)
    );
    assert_eq!(
        g.apply(Command {
            id: 99,
            received_at: 3000,
            ..too_early
        })
        .status,
        ActionStatus::Rejected(Rejection::InvalidSequence)
    );
    assert_eq!(
        g.apply(Command {
            epoch: 2,
            ..command(2, Seat::One, 3000, Action::Attack)
        })
        .status,
        ActionStatus::Rejected(Rejection::InvalidEpoch)
    );
    assert_eq!(
        g.apply(command(2, Seat::One, 3000, Action::Open(CellId(999))))
            .status,
        ActionStatus::Rejected(Rejection::InvalidCell)
    );
    assert_eq!(
        g.apply(command(3, Seat::One, 3000, Action::Accuse(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::InvalidCell)
    );
    assert_eq!(
        g.apply(command(4, Seat::One, 3000, Action::ToggleFlag(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::CommandLimit)
    );
    assert_eq!(
        g.apply(command(5, Seat::One, 2999, Action::Attack)).status,
        ActionStatus::Rejected(Rejection::InvalidTime)
    );
    assert_eq!(g.advance(2999), Err(Rejection::InvalidTime));
}
#[test]
fn reconnect_before_grace_rotates_epoch_and_retransmits_the_same_command() {
    let mut g = game();
    let original = command(1, Seat::One, 3000, Action::Open(CellId(8)));
    let ack = g.apply(original);
    g.disconnect(Seat::One, 3001).unwrap();
    g.disconnect(Seat::One, 3002).unwrap();
    assert_eq!(
        g.apply(command(2, Seat::One, 3003, Action::Open(CellId(2))))
            .status,
        ActionStatus::Rejected(Rejection::Disconnected)
    );
    assert_eq!(g.resume(Seat::One, 1, 3004), Err(Rejection::InvalidEpoch));
    g.resume(Seat::One, 2, 3005).unwrap();
    let replay = g.apply(Command {
        received_at: 3006,
        epoch: 2,
        ..original
    });
    assert!(replay.duplicate);
    assert_eq!(replay.revision, ack.revision);
    assert_eq!(replay.status, ack.status);
    assert_eq!(
        g.apply(command(3, Seat::One, 3007, Action::Attack)).status,
        ActionStatus::Rejected(Rejection::InvalidEpoch)
    );
    assert_eq!(g.projection(Seat::One).own.gauge, 1);
}
#[test]
fn disconnect_terminal_time_precedes_timeout_and_both_expired_abandon() {
    use liar_core::game::EndReason;
    let mut g = game();
    g.disconnect(Seat::One, 3000).unwrap();
    g.advance(300000).unwrap();
    let end = g.projection(Seat::One).end.unwrap();
    assert_eq!(end.reason, EndReason::Forfeit);
    assert_eq!(end.winner, Some(Seat::Two));
    assert_eq!(end.at, 33000);
    assert_eq!(g.resume(Seat::One, 2, 300001), Err(Rejection::Finished));
    assert_eq!(g.disconnect(Seat::Two, 300001), Err(Rejection::Finished));
    let mut both = game();
    both.disconnect(Seat::One, 3000).unwrap();
    both.disconnect(Seat::Two, 4000).unwrap();
    both.advance(33000).unwrap();
    assert_eq!(both.projection(Seat::One).end, None);
    assert_eq!(
        both.resume(Seat::One, 2, 33000),
        Err(Rejection::Disconnected)
    );
    both.advance(34000).unwrap();
    let end = both.projection(Seat::One).end.unwrap();
    assert_eq!(end.reason, EndReason::Abandoned);
    assert_eq!(end.winner, None);
    let mut second = game();
    second.disconnect(Seat::Two, 3000).unwrap();
    second.advance(33000).unwrap();
    assert_eq!(
        second.projection(Seat::One).end.unwrap().winner,
        Some(Seat::One)
    );
}
#[test]
fn abort_and_countdown_cancel_are_terminal_and_analysis_is_revision_bound() {
    use liar_core::game::EndReason;
    let mut g = game();
    let work = g.proof_work(Seat::One).run().unwrap();
    g.apply(command(1, Seat::One, 3000, Action::Open(CellId(8))));
    assert_eq!(g.commit_proof(work), Err(Rejection::AnalysisStale));
    g.commit_proof(g.proof_work(Seat::One).run().unwrap())
        .unwrap();
    g.abort(3001).unwrap();
    assert_eq!(
        g.projection(Seat::One).end.unwrap().reason,
        EndReason::ServerFailure
    );
    let end = g.projection(Seat::One).end;
    g.abort(3002).unwrap();
    assert_eq!(g.projection(Seat::One).end, end);
    let mut cancelled = game();
    cancelled.disconnect(Seat::One, 2999).unwrap();
    assert_eq!(
        cancelled.projection(Seat::One).end.unwrap().reason,
        EndReason::Cancelled
    );
}
#[test]
fn two_closed_overlays_reject_a_third_attack_without_consuming_full_gauge() {
    use liar_core::{
        generator::{BoardGenerator, GenerationBudget},
        policy::PolicyKnowledge,
        solver::SolverBudget,
    };
    let mut rules = RulesSnapshot::bundled().rules;
    rules.gauge_capacity = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = BoardGenerator::generate(rules.rules.board_spec(), 42, GenerationBudget::default())
        .unwrap()
        .board;
    let mut g = RuleEngine::new(board, rules, 0).unwrap();
    let initial = g.projection(Seat::Two).own.cells;
    let mut defender = PolicyKnowledge::start(&initial, SolverBudget::default()).unwrap();
    let mut seq = 1;
    loop {
        let eligible = (0..256)
            .filter(|&i| {
                [-1, 1]
                    .into_iter()
                    .any(|d| defender.certificate(CellId(i), d, 0).is_ok())
            })
            .count();
        if eligible >= 3 {
            break;
        }
        let cell = *defender
            .safe()
            .first()
            .expect("certified board still progresses");
        assert_eq!(
            g.apply(command(
                seq,
                Seat::Two,
                3000 + seq as u64,
                Action::Open(cell)
            ))
            .status,
            ActionStatus::Applied
        );
        defender
            .observe(&g.projection(Seat::Two).own.cells)
            .unwrap();
        defender.refresh().unwrap();
        seq += 1;
    }
    g.commit_proof(g.proof_work(Seat::Two).run().unwrap())
        .unwrap();
    let mut attacker =
        PolicyKnowledge::start(&g.projection(Seat::One).own.cells, SolverBudget::default())
            .unwrap();
    for attempt in 0..3 {
        let cell = *attacker.safe().first().unwrap();
        assert_eq!(
            g.apply(command(
                seq,
                Seat::One,
                3000 + seq as u64,
                Action::Open(cell)
            ))
            .status,
            ActionStatus::Applied
        );
        seq += 1;
        attacker
            .observe(&g.projection(Seat::One).own.cells)
            .unwrap();
        attacker.refresh().unwrap();
        let attack = g.apply(command(seq, Seat::One, 3000 + seq as u64, Action::Attack));
        seq += 1;
        if attempt < 2 {
            assert_eq!(attack.status, ActionStatus::Applied);
            assert_eq!(g.projection(Seat::One).own.gauge, 0);
        } else {
            assert_eq!(
                attack.status,
                ActionStatus::Rejected(Rejection::AttackUnavailable)
            );
            assert_eq!(g.projection(Seat::One).own.gauge, 1);
        }
    }
}
#[test]
fn shorter_reflection_preserves_a_longer_existing_mine_stun() {
    let mut g = tuned_game(1, 8192);
    for seat in [Seat::One, Seat::Two] {
        g.apply(command(1, seat, 3000, Action::Open(CellId(8))));
    }
    g.commit_proof(g.proof_work(Seat::Two).run().unwrap())
        .unwrap();
    g.apply(command(2, Seat::One, 3001, Action::Attack));
    g.apply(command(3, Seat::One, 3002, Action::Open(CellId(5))));
    g.apply(command(2, Seat::Two, 3003, Action::Open(CellId(2))));
    g.apply(command(3, Seat::Two, 3004, Action::Accuse(CellId(2))));
    assert_eq!(g.projection(Seat::One).own.stun_ms, 2998);
}
#[test]
fn invalid_startup_and_tampered_snapshot_fail_closed_and_automatic_clear_draws() {
    let rules = RulesSnapshot::bundled();
    let small = game().projection(Seat::One).rules;
    let board = Board::from_mines(small.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    assert_eq!(
        RuleEngine::new(board.clone(), rules, 0).err(),
        Some(Rejection::InvalidBoard)
    );
    assert_eq!(
        RuleEngine::new(board.clone(), small.clone(), u64::MAX).err(),
        Some(Rejection::InvalidTime)
    );
    let mut tampered = small.clone();
    tampered.rules.duration_ms += 1;
    assert_eq!(
        RuleEngine::new(board, tampered, 0).err(),
        Some(Rejection::InvalidRules)
    );
    let nonzero = Board::from_mines(small.rules.board_spec(), &[CellId(1), CellId(7)]).unwrap();
    assert_eq!(
        RuleEngine::new(nonzero, small, 0).err(),
        Some(Rejection::InvalidBoard)
    );
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 4;
    rules.height = 4;
    rules.mines = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let mut automatic = RuleEngine::new(
        Board::from_mines(rules.rules.board_spec(), &[CellId(15)]).unwrap(),
        rules,
        0,
    )
    .unwrap();
    automatic.advance(3000).unwrap();
    let end = automatic.projection(Seat::One).end.unwrap();
    assert_eq!(end.reason, liar_core::game::EndReason::Clear);
    assert_eq!(end.winner, None);
}
#[test]
fn zero_opening_alone_does_not_authorize_an_uncertified_stalled_board() {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 4;
    rules.height = 4;
    rules.mines = 4;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(
        rules.rules.board_spec(),
        &[CellId(2), CellId(7), CellId(9), CellId(14)],
    )
    .unwrap();
    assert_eq!(
        RuleEngine::new(board, rules, 0).err(),
        Some(Rejection::InvalidBoard)
    );
}
#[test]
fn hidden_attack_and_opponent_flags_do_not_change_the_defenders_public_revision() {
    let mut g = tuned_game(1, 8192);
    for seat in [Seat::One, Seat::Two] {
        g.apply(command(1, seat, 3000, Action::Open(CellId(8))));
    }
    g.commit_proof(g.proof_work(Seat::Two).run().unwrap())
        .unwrap();
    let before = g.projection(Seat::Two);
    assert_eq!(
        g.apply(command(2, Seat::One, 3000, Action::Attack)).status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::Two), before);
    assert_eq!(
        g.apply(command(3, Seat::One, 3000, Action::ToggleFlag(CellId(6))))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(g.projection(Seat::Two), before);
}
