use liar_core::{
    board::{Board, CellId, ObservedCell},
    game::{Action, ActionStatus, Command, EndReason, Rejection, multiplayer::PreparedRace},
    random::RandomSource,
    rules::RulesSnapshot,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct Draws {
    values: VecDeque<u64>,
    count: Arc<AtomicUsize>,
}
impl RandomSource for Draws {
    fn next_u64(&mut self) -> u64 {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.values.pop_front().expect("unexpected target draw")
    }
}
fn fixture() -> (Board, RulesSnapshot) {
    let mut rules = RulesSnapshot::bundled().rules;
    rules.width = 3;
    rules.height = 3;
    rules.mines = 2;
    rules.gauge_capacity = 1;
    let rules = RulesSnapshot::from_rules(rules).unwrap();
    let board = Board::from_mines(rules.rules.board_spec(), &[CellId(5), CellId(7)]).unwrap();
    (board, rules)
}
fn game(
    players: usize,
    draws: impl IntoIterator<Item = u64>,
) -> (liar_core::game::multiplayer::RaceEngine, Arc<AtomicUsize>) {
    let (board, rules) = fixture();
    let count = Arc::new(AtomicUsize::new(0));
    let game = PreparedRace::prepare(board, rules, players)
        .unwrap()
        .start(
            0,
            Draws {
                values: draws.into_iter().collect(),
                count: count.clone(),
            },
        )
        .unwrap();
    (game, count)
}
fn command(seat: usize, seq: u64, at: u64, action: Action) -> Command<usize> {
    Command {
        id: u128::from(seq),
        seq,
        epoch: 1,
        seat,
        received_at: at,
        action,
    }
}

#[test]
fn player_count_is_bounded_and_equal_opening_is_private_to_each_seat() {
    for invalid in [0, 1, 5, usize::MAX] {
        let (board, rules) = fixture();
        assert!(PreparedRace::prepare(board, rules, invalid).is_err());
    }
    for size in 2..=4 {
        let (mut game, _) = game(size, []);
        let initial = game.projection(0).unwrap().own;
        for seat in 1..size {
            let view = game.projection(seat).unwrap();
            assert_eq!(view.own, initial);
            assert_eq!(view.opponents.len(), size - 1);
            assert!(view.opponents.iter().all(|entry| entry.seat != seat));
        }
        assert!(game.projection(size).is_none());
        assert!(matches!(
            game.apply(command(size, 1, 3000, Action::Attack)).status,
            ActionStatus::Rejected(_)
        ));
        assert_eq!(game.projection(0).unwrap().countdown_ms, 3000);
        assert_eq!(
            game.apply(command(size - 1, 1, 3000, Action::Open(CellId(8))))
                .status,
            ActionStatus::Applied
        );
        assert_eq!(game.projection(0).unwrap().own, initial);
        assert_eq!(
            game.projection(size - 1).unwrap().own.cells.opened_safe(),
            5
        );
    }
}

#[test]
fn first_clear_freezes_other_progress_and_competition_ranks() {
    let (mut game, _) = game(4, []);
    for seat in [1, 2] {
        game.apply(command(seat, 1, 3000, Action::Open(CellId(8))));
    }
    for (seq, cell) in [(1, 8), (2, 2), (3, 6)] {
        assert_eq!(
            game.apply(command(3, seq, 3000 + seq, Action::Open(CellId(cell))))
                .status,
            ActionStatus::Applied
        );
    }
    let result = game.result().unwrap().clone();
    assert_eq!(result.reason, EndReason::Clear);
    assert_eq!(result.at, 3003);
    assert_eq!(result.winner, Some(3));
    let ranks: Vec<_> = result
        .standings
        .iter()
        .map(|p| (p.seat, p.rank, p.opened_safe))
        .collect();
    assert_eq!(
        ranks,
        [
            (3, Some(1), 7),
            (1, Some(2), 5),
            (2, Some(2), 5),
            (0, Some(4), 4)
        ]
    );
    assert_eq!(
        game.apply(command(0, 1, 3004, Action::Open(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::Finished)
    );
    game.advance(243000).unwrap();
    assert_eq!(game.result(), Some(&result));
}

#[test]
fn timeout_ties_first_and_forfeits_do_not_rank_above_active_players() {
    let (mut game, _) = game(4, []);
    game.apply(command(0, 1, 3000, Action::Open(CellId(8))));
    game.apply(command(0, 2, 3000, Action::Open(CellId(2))));
    game.forfeit(0, 3001).unwrap();
    assert!(game.result().is_none());
    assert_eq!(
        game.apply(command(0, 3, 3002, Action::Open(CellId(6))))
            .status,
        ActionStatus::Rejected(Rejection::Disconnected)
    );
    for seat in [1, 2] {
        game.apply(command(seat, 1, 3003, Action::Open(CellId(8))));
    }
    assert_eq!(
        game.apply(command(3, 1, 243000, Action::Open(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::Finished)
    );
    let result = game.result().unwrap();
    assert_eq!(result.reason, EndReason::Timeout);
    assert_eq!(result.winner, None);
    let ranks: Vec<_> = result.standings.iter().map(|p| (p.seat, p.rank)).collect();
    assert_eq!(ranks, [(1, Some(1)), (2, Some(1)), (3, Some(3)), (0, None)]);
}

#[test]
fn expiration_batches_all_seats_before_declaring_the_last_player() {
    let (mut abandoned, _) = game(3, []);
    for seat in 0..3 {
        abandoned.disconnect(seat, 3000).unwrap();
    }
    abandoned.advance(33000).unwrap();
    assert_eq!(abandoned.result().unwrap().reason, EndReason::Abandoned);
    assert!(
        abandoned
            .result()
            .unwrap()
            .standings
            .iter()
            .all(|p| p.rank.is_none())
    );

    let (mut survivor, _) = game(3, []);
    for seat in [0, 2] {
        survivor.disconnect(seat, 3000).unwrap();
    }
    survivor.advance(50000).unwrap();
    let result = survivor.result().unwrap();
    assert_eq!(
        (result.reason, result.winner, result.at),
        (EndReason::Forfeit, Some(1), 33000)
    );
}

#[test]
fn random_attack_is_sampled_once_and_reflects_to_the_actual_source() {
    // For seat 1 among four players, draw 4 selects seat 2.
    let (mut game, draws) = game(4, [4]);
    let mut time = 3000;
    for seat in 0..4 {
        time += 1;
        game.apply(command(seat, 1, time, Action::Open(CellId(8))));
        game.commit_proof(game.proof_work(seat).unwrap().run().unwrap())
            .unwrap();
    }
    let before = game.projection(2).unwrap().own;
    let attack = command(1, 2, 3005, Action::Attack);
    assert_eq!(game.apply(attack).status, ActionStatus::Applied);
    assert_eq!(game.projection(2).unwrap().own, before);
    game.resume(1, 2, 3006).unwrap();
    assert!(
        game.apply(Command {
            epoch: 2,
            received_at: 3006,
            ..attack
        })
        .duplicate
    );
    assert_eq!(draws.load(Ordering::Relaxed), 1);
    game.apply(command(2, 2, 3007, Action::Open(CellId(2))));
    assert_eq!(
        game.projection(2).unwrap().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(2))
    );
    game.apply(command(2, 3, 3008, Action::Accuse(CellId(2))));
    assert_eq!(
        game.projection(2).unwrap().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(1))
    );
    assert_eq!(game.projection(1).unwrap().own.stun_ms, 2000);
    assert_eq!(game.projection(0).unwrap().own.stun_ms, 0);
    assert_eq!(game.projection(3).unwrap().own.stun_ms, 0);
}

#[test]
fn no_candidate_does_not_reroll_or_spend_gauge_and_dnf_is_never_sampled() {
    let (mut game, draws) = game(4, [2, 5]);
    game.forfeit(2, 3000).unwrap();
    for seat in [0, 3] {
        game.apply(command(seat, 1, 3001, Action::Open(CellId(8))));
        game.commit_proof(game.proof_work(seat).unwrap().run().unwrap())
            .unwrap();
    }
    let failure = command(0, 2, 3002, Action::Attack);
    assert_eq!(
        game.apply(failure).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert!(game.apply(failure).duplicate);
    assert_eq!(draws.load(Ordering::Relaxed), 1);
    assert_eq!(game.projection(0).unwrap().own.gauge, 1);
    assert_eq!(
        game.apply(command(0, 3, 3003, Action::Attack)).status,
        ActionStatus::Applied
    );
    game.apply(command(3, 2, 3004, Action::Open(CellId(2))));
    assert_eq!(
        game.projection(3).unwrap().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(2))
    );
}

#[test]
fn three_attackers_cannot_stack_a_third_lie_on_the_same_player() {
    let (mut game, draws) = game(4, [5, 5, 5]);
    for seat in 0..4 {
        assert_eq!(
            game.apply(command(seat, 1, 3000, Action::Open(CellId(8))))
                .status,
            ActionStatus::Applied
        );
    }
    game.commit_proof(game.proof_work(3).unwrap().run().unwrap())
        .unwrap();
    let before = game.projection(3).unwrap();
    let acks = game.apply_batch(
        (0..3)
            .rev()
            .map(|seat| command(seat, 2, 3000, Action::Attack))
            .collect(),
    );
    assert_eq!(draws.load(Ordering::Relaxed), 3);
    assert_eq!(
        acks.iter().map(|ack| ack.status).collect::<Vec<_>>(),
        [
            ActionStatus::Applied,
            ActionStatus::Applied,
            ActionStatus::Rejected(Rejection::AttackUnavailable)
        ]
    );
    assert_eq!(game.projection(3).unwrap(), before);
    assert_eq!(game.projection(2).unwrap().own.gauge, 1);
    let first_lie = CellId(2);
    game.apply(command(3, 2, 3002, Action::Open(first_lie)));
    assert_eq!(
        game.projection(3).unwrap().own.cells.cell(first_lie),
        Some(ObservedCell::Number(2))
    );
    assert_eq!(
        game.apply(command(3, 3, 3003, Action::Accuse(first_lie)))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(game.projection(0).unwrap().own.stun_ms, 2000);
    assert_eq!(game.projection(1).unwrap().own.stun_ms, 0);
    // Opening the second lie also completes this fixture, so it ends the race immediately.
    let second_lie = CellId(6);
    assert_eq!(
        game.apply(command(3, 4, 3004, Action::Open(second_lie)))
            .status,
        ActionStatus::Applied
    );
    assert_eq!(
        game.projection(3).unwrap().own.cells.cell(second_lie),
        Some(ObservedCell::Number(2))
    );
    assert_eq!(game.result().unwrap().winner, Some(3));
}

#[test]
fn connection_changes_are_visible_without_changing_any_board() {
    let (mut game, _) = game(3, []);
    let before = game.projection(1).unwrap();
    game.disconnect(2, 3000).unwrap();
    let grace = game.projection(1).unwrap();
    assert!(grace.revision > before.revision);
    assert_eq!(grace.own, before.own);
    assert_eq!(grace.opponents[1].progress.reconnect_ms, Some(30000));
    game.resume(2, 2, 32999).unwrap();
    let restored = game.projection(1).unwrap();
    assert!(restored.revision > grace.revision);
    assert_eq!(restored.opponents[1].progress.reconnect_ms, None);
    assert!(!restored.opponents[1].forfeited);
    assert_eq!(game.last_sequence(2), Some(0));
    assert_eq!(game.last_sequence(3), None);
    assert_eq!(game.disconnect(3, 32999), Err(Rejection::InvalidSeat));
    assert_eq!(game.resume(3, 2, 32999), Err(Rejection::InvalidSeat));
    assert_eq!(game.forfeit(3, 32999), Err(Rejection::InvalidSeat));
    assert!(matches!(game.proof_work(3), Err(Rejection::InvalidSeat)));
}

#[test]
fn departed_source_still_allows_correction_but_cannot_reenter() {
    let (mut game, _) = game(3, [1]);
    for seat in [0, 2] {
        game.apply(command(seat, 1, 3000, Action::Open(CellId(8))));
    }
    game.commit_proof(game.proof_work(2).unwrap().run().unwrap())
        .unwrap();
    assert_eq!(
        game.apply(command(0, 2, 3001, Action::Attack)).status,
        ActionStatus::Applied
    );
    game.forfeit(0, 3002).unwrap();
    assert_eq!(game.resume(0, 2, 3002), Err(Rejection::Disconnected));
    game.apply(command(2, 2, 3003, Action::Open(CellId(2))));
    assert_eq!(
        game.projection(2).unwrap().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(2))
    );
    game.apply(command(2, 3, 3004, Action::Accuse(CellId(2))));
    assert_eq!(
        game.projection(2).unwrap().own.cells.cell(CellId(2)),
        Some(ObservedCell::Number(1))
    );
    assert!(game.projection(0).unwrap().forfeited);
    assert!(game.result().is_none());
}

#[test]
fn last_disconnected_player_has_a_chance_to_return_before_the_result() {
    let (mut game, _) = game(3, []);
    game.disconnect(0, 3000).unwrap();
    game.disconnect(1, 4000).unwrap();
    game.disconnect(2, 5000).unwrap();
    game.advance(34000).unwrap();
    assert!(game.result().is_none());
    game.resume(2, 2, 34999).unwrap();
    let result = game.result().unwrap();
    assert_eq!(
        (result.reason, result.winner, result.at),
        (EndReason::Forfeit, Some(2), 34999)
    );
}

#[test]
fn cancelled_and_aborted_races_do_not_assign_places() {
    for reason in [EndReason::Cancelled, EndReason::ServerFailure] {
        let (mut game, _) = game(2, []);
        match reason {
            EndReason::Cancelled => game.cancel_before_start(game.start_ms()).unwrap(),
            _ => game.abort(3001).unwrap(),
        }
        assert_eq!(game.result().unwrap().reason, reason);
        assert_eq!(game.result().unwrap().winner, None);
        assert!(
            game.result()
                .unwrap()
                .standings
                .iter()
                .all(|entry| entry.rank.is_none())
        );
        assert_eq!(game.projection(1).unwrap().result.as_ref(), game.result());
        assert_eq!(game.forfeit(0, 3002), Err(Rejection::Finished));
    }
}

#[test]
fn biased_boundaries_have_a_fixed_budget_without_charging_the_attacker() {
    let (mut game, draws) = game(4, [0; 8]);
    assert_eq!(
        game.apply(command(0, 1, 3000, Action::Attack)).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert_eq!(draws.load(Ordering::Relaxed), 0);
    game.apply(command(0, 2, 3000, Action::Open(CellId(8))));
    assert_eq!(
        game.apply(command(0, 3, 3000, Action::Attack)).status,
        ActionStatus::Rejected(Rejection::AttackUnavailable)
    );
    assert_eq!(draws.load(Ordering::Relaxed), 8);
    assert_eq!(game.projection(0).unwrap().own.gauge, 1);
}

#[test]
fn late_proof_is_checked_against_the_seats_current_observation_and_end() {
    let (mut game, _) = game(3, []);
    let stale = game.proof_work(2).unwrap().run().unwrap();
    game.apply(command(2, 1, 3000, Action::Open(CellId(8))));
    assert_eq!(game.commit_proof(stale), Err(Rejection::AnalysisStale));
    let after_open = game.proof_work(2).unwrap().run().unwrap();
    game.abort(3001).unwrap();
    assert_eq!(game.commit_proof(after_open), Err(Rejection::Finished));
}

#[test]
fn recognized_start_tick_and_original_rules_hash_come_from_the_prepared_round() {
    let (board, rules) = fixture();
    let hash = rules.hash.clone();
    let prepared = PreparedRace::prepare(board, rules, 3).unwrap();
    assert_eq!(prepared.rules_hash(), hash);
    let mut game = prepared
        .start(9000, liar_core::random::SeededRng::new(42))
        .unwrap();
    assert_eq!(game.start_ms(), 12000);
    assert_eq!(game.projection(2).unwrap().rules.hash, hash);
    assert_eq!(
        game.apply(command(2, 1, 11999, Action::Open(CellId(8))))
            .status,
        ActionStatus::Rejected(Rejection::NotStarted)
    );
}
