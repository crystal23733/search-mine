use liar_core::board::{Board, BoardSpec, CellId, Observation, ObservedCell};
use liar_core::policy::PublicCellUpdate;
use liar_core::policy::{PolicyError, PolicyKnowledge};
use liar_core::solver::SolverBudget;
fn setup() -> (Board, Observation, PolicyKnowledge) {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(5), CellId(7)]).unwrap();
    let mut view = Observation::closed(spec).unwrap();
    board.reveal(&mut view, CellId(0), &[false; 9]).unwrap();
    let ledger = PolicyKnowledge::start(&view, SolverBudget::default()).unwrap();
    (board, view, ledger)
}
#[test]
fn public_registered_strategy_preserves_safe_progress_and_identifies_two_lies() {
    let (board, mut view, mut ledger) = setup();
    assert_eq!(ledger.safe(), vec![CellId(8)]);
    assert_eq!(
        ledger.certificate(CellId(2), 1, 0).err(),
        Some(PolicyError::InvalidTarget)
    );
    board.reveal(&mut view, CellId(8), &[false; 9]).unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    assert_eq!(ledger.safe(), vec![CellId(2), CellId(6)]);
    let first = ledger.certificate(CellId(2), 1, 0).unwrap();
    let second = ledger.certificate(CellId(6), 1, 1).unwrap();
    assert!(first.matches(&ledger) && second.matches(&ledger));
    assert_eq!(first.expected_truth(), 1);
    board.reveal(&mut view, CellId(2), &[false; 9]).unwrap();
    board.reveal(&mut view, CellId(6), &[false; 9]).unwrap();
    view.set(CellId(2), ObservedCell::Number(first.displayed()))
        .unwrap();
    view.set(CellId(6), ObservedCell::Number(second.displayed()))
        .unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    assert_eq!(ledger.known_lies(), vec![CellId(2), CellId(6)]);
    assert_eq!(ledger.public_view(), &view);
    assert!(!first.matches(&ledger));
}
#[test]
fn correcting_a_lie_and_replaying_public_history_restore_the_same_knowledge() {
    let (board, initial, mut ledger) = setup();
    let mut view = initial.clone();
    board.reveal(&mut view, CellId(8), &[false; 9]).unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    let proof = ledger.certificate(CellId(2), 1, 0).unwrap();
    board.reveal(&mut view, CellId(2), &[false; 9]).unwrap();
    view.set(CellId(2), ObservedCell::Number(proof.displayed()))
        .unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    assert_eq!(ledger.known_lies(), vec![CellId(2)]);
    view.set(CellId(2), ObservedCell::Number(proof.expected_truth()))
        .unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    assert!(ledger.known_lies().is_empty());
    let restored =
        PolicyKnowledge::replay(&initial, ledger.public_history(), SolverBudget::default())
            .unwrap();
    assert!(ledger == restored);
}
#[test]
fn replay_rejects_oversized_empty_and_noop_history_before_analysis() {
    let (_, initial, ledger) = setup();
    let mut history = ledger.public_history().to_vec();
    history.push(Vec::new());
    assert_eq!(
        PolicyKnowledge::replay(&initial, &history, SolverBudget::default()).err(),
        Some(PolicyError::InvalidProjection)
    );
    history[1] = vec![PublicCellUpdate {
        cell: CellId(0),
        observed: ObservedCell::Number(0),
    }];
    assert_eq!(
        PolicyKnowledge::replay(&initial, &history, SolverBudget::default()).err(),
        Some(PolicyError::InvalidProjection)
    );
    history.resize(513, history[1].clone());
    assert_eq!(
        PolicyKnowledge::replay(&initial, &history, SolverBudget::default()).err(),
        Some(PolicyError::HistoryLimit)
    );
}

#[test]
fn malformed_projection_rolls_back_and_certificates_fail_closed() {
    let (board, initial, mut ledger) = setup();
    let original = ledger.clone();
    let mut bad = initial.clone();
    bad.set(CellId(8), ObservedCell::Mine).unwrap();
    assert_eq!(ledger.observe(&bad), Err(PolicyError::InvalidProjection));
    assert!(ledger == original);
    bad = initial.clone();
    bad.set(CellId(1), ObservedCell::Unknown).unwrap();
    assert_eq!(ledger.observe(&bad), Err(PolicyError::InvalidProjection));
    let other = Observation::closed(BoardSpec::default()).unwrap();
    assert_eq!(ledger.observe(&other), Err(PolicyError::InvalidProjection));
    assert!(ledger == original);
    assert_eq!(
        PolicyKnowledge::start(&bad, SolverBudget::default()).err(),
        Some(PolicyError::InvalidInitial)
    );
    let mut view = initial;
    board.reveal(&mut view, CellId(8), &[false; 9]).unwrap();
    ledger.observe(&view).unwrap();
    ledger.refresh().unwrap();
    assert_eq!(
        ledger.certificate(CellId(2), 1, 2).err(),
        Some(PolicyError::Capacity)
    );
    for delta in [0, 2, -2, -1] {
        assert_eq!(
            ledger.certificate(CellId(2), delta, 0).err(),
            Some(PolicyError::InvalidDelta)
        );
    }
    for target in [CellId(999), CellId(0), CellId(5)] {
        assert_eq!(
            ledger.certificate(target, 1, 0).err(),
            Some(PolicyError::InvalidTarget)
        );
    }
    let proof = ledger.certificate(CellId(2), 1, 0).unwrap();
    assert_eq!(proof.target(), CellId(2));
    assert_eq!(proof.delta(), 1);
    bad = view.clone();
    bad.set(CellId(2), ObservedCell::Number(4)).unwrap();
    let before = ledger.clone();
    assert_eq!(ledger.observe(&bad), Err(PolicyError::InvalidProjection));
    assert!(ledger == before);
    let mut history = ledger.public_history().to_vec();
    history.push(vec![PublicCellUpdate {
        cell: CellId(999),
        observed: ObservedCell::Mine,
    }]);
    assert_eq!(
        PolicyKnowledge::replay(ledger.public_view(), &history, SolverBudget::default()).err(),
        Some(PolicyError::InvalidInitial)
    );
}

#[test]
fn valid_observation_survives_analysis_budget_rejection_and_duplicate_refresh() {
    let (_, initial, _) = setup();
    let budget = SolverBudget {
        max_constraints: 0,
        ..SolverBudget::default()
    };
    let mut ledger = PolicyKnowledge::start(&initial, budget).unwrap();
    assert_eq!(
        ledger.analysis(),
        Some(liar_core::solver::SolverError::BudgetExceeded)
    );
    assert!(ledger.safe().is_empty());
    let unchanged = ledger.clone();
    ledger.observe(&initial).unwrap();
    ledger.refresh().unwrap();
    assert!(ledger == unchanged);
    assert_eq!(ledger.public_history().len(), 1);
}

#[test]
fn generated_boards_complete_with_two_overlays_and_public_history_only() {
    use liar_core::board::Cell;
    use liar_core::generator::{BoardGenerator, GenerationBudget};
    for seed in 0..32 {
        let spec = BoardSpec::default();
        let board = BoardGenerator::generate(spec, seed, GenerationBudget::default())
            .unwrap()
            .board;
        let flags = vec![false; spec.area()];
        let mut view = Observation::closed(spec).unwrap();
        board.reveal(&mut view, spec.opening, &flags).unwrap();
        let initial = view.clone();
        let mut ledger = PolicyKnowledge::start(&initial, SolverBudget::default()).unwrap();
        let mut overlays = Vec::new();
        for _ in 0..spec.area() {
            if view.opened_safe() == board.safe_total() {
                break;
            }
            for i in 0..spec.area() {
                let cell = CellId(i as u16);
                if overlays.len() >= 2 {
                    break;
                }
                if overlays.iter().any(|p: &(CellId, u8, u8)| p.0 == cell) {
                    continue;
                }
                if let Ok(proof) = ledger.certificate(cell, 1, overlays.len()) {
                    assert_eq!(board.cell(cell), Some(Cell::Number(proof.expected_truth())));
                    overlays.push((cell, proof.expected_truth(), proof.displayed()));
                }
            }
            let safe = ledger.safe();
            assert!(!safe.is_empty(), "seed {seed}");
            for cell in safe {
                assert!(matches!(board.cell(cell), Some(Cell::Number(_))));
                board.reveal(&mut view, cell, &flags).unwrap();
            }
            for &(cell, _, displayed) in &overlays {
                if matches!(view.cell(cell), Some(ObservedCell::Number(_))) {
                    view.set(cell, ObservedCell::Number(displayed)).unwrap();
                }
            }
            ledger.observe(&view).unwrap();
            ledger.refresh().unwrap();
            for cell in ledger.known_lies() {
                let index = overlays
                    .iter()
                    .position(|p| p.0 == cell)
                    .expect("no false positive");
                let (_, truth, _) = overlays.remove(index);
                view.set(cell, ObservedCell::Number(truth)).unwrap();
            }
            ledger.observe(&view).unwrap();
            ledger.refresh().unwrap();
        }
        assert_eq!(view.opened_safe(), board.safe_total());
        let replayed =
            PolicyKnowledge::replay(&initial, ledger.public_history(), SolverBudget::default())
                .unwrap();
        assert!(ledger == replayed);
    }
}

#[test]
fn every_small_legal_history_certificate_holds_in_all_independent_worlds() {
    use liar_core::board::Cell;
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut worlds = Vec::new();
    for a in 0..9 {
        for b in a + 1..9 {
            worlds.push(Board::from_mines(spec, &[CellId(a), CellId(b)]).unwrap());
        }
    }
    let mut checked = 0;
    for actual in &worlds {
        if actual.cell(spec.opening) != Some(Cell::Number(0)) {
            continue;
        }
        let mut view = Observation::closed(spec).unwrap();
        actual.reveal(&mut view, spec.opening, &[false; 9]).unwrap();
        let mut possible: Vec<_> = worlds.iter().filter(|world| view.cells().iter().enumerate().all(|(i,&c)| {
            c == ObservedCell::Unknown || matches!((c,world.cell(CellId(i as u16))), (ObservedCell::Number(x),Some(Cell::Number(y))) if x==y)
        })).collect();
        let mut ledger = PolicyKnowledge::start(&view, SolverBudget::default()).unwrap();
        let mut registered = [None; 9];
        for _ in 0..9 {
            for i in 0..9 {
                let cell = CellId(i);
                for delta in [-1, 1] {
                    if let Ok(proof) = ledger.certificate(cell, delta, 0) {
                        assert!(!possible.is_empty());
                        assert!(
                            possible.iter().all(|world| world.cell(cell)
                                == Some(Cell::Number(proof.expected_truth())))
                        );
                        for neighbor in spec.neighbors(cell) {
                            let mine = possible[0].cell(neighbor) == Some(Cell::Mine);
                            assert!(possible.iter().all(|world| (world.cell(neighbor)==Some(Cell::Mine))==mine));
                        }
                        registered[usize::from(i)] =
                            Some((proof.expected_truth(), proof.displayed()));
                        checked += possible.len();
                    }
                }
            }
            let Some(cell) = ledger.safe().first().copied() else {
                break;
            };
            assert!(
                possible
                    .iter()
                    .all(|world| matches!(world.cell(cell), Some(Cell::Number(_))))
            );
            let old = view.clone();
            actual.reveal(&mut view, cell, &[false; 9]).unwrap();
            let mut truth_updates = Vec::new();
            let mut active = ledger.known_lies().len();
            for (i, &registration) in registered.iter().enumerate() {
                if old.cells()[i] != ObservedCell::Unknown
                    || view.cells()[i] == ObservedCell::Unknown
                {
                    continue;
                }
                let ObservedCell::Number(truth) = view.cells()[i] else {
                    panic!("unsafe open");
                };
                let inferred = if let Some((expected, displayed)) = registration {
                    assert_eq!(truth, expected);
                    if active < 2 {
                        view.set(CellId(i as u16), ObservedCell::Number(displayed))
                            .unwrap();
                        active += 1;
                    }
                    expected
                } else {
                    truth
                };
                truth_updates.push((CellId(i as u16), inferred));
            }
            possible.retain(|world| {
                truth_updates
                    .iter()
                    .all(|&(c, t)| world.cell(c) == Some(Cell::Number(t)))
            });
            assert!(!possible.is_empty());
            ledger.observe(&view).unwrap();
            ledger.refresh().unwrap();
            for accused in ledger.known_lies() {
                assert!(possible.iter().all(|world| world.cell(accused)
                    != Some(Cell::Number(match view.cell(accused).unwrap() {
                        ObservedCell::Number(n) => n,
                        _ => unreachable!(),
                    }))));
            }
        }
    }
    assert!(checked > 0);
}
