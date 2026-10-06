use liar_core::board::{BoardSpec, CellId, Observation, ObservedCell};
use liar_core::knowledge::{KnowledgeError, KnowledgeMemory, KnowledgeSolver};
use liar_core::solver::SolverBudget;

#[test]
fn a_normal_no_guess_cell_is_not_safe_when_an_unknown_lie_explains_the_same_view() {
    let spec = BoardSpec {
        width: 3,
        height: 2,
        mines: 2,
        opening: CellId(0),
    };
    let view = Observation::new(
        spec,
        vec![
            ObservedCell::Number(1),
            ObservedCell::Number(2),
            ObservedCell::Number(1),
            ObservedCell::Unknown,
            ObservedCell::Unknown,
            ObservedCell::Unknown,
        ],
    )
    .unwrap();
    let proof = KnowledgeSolver::deduce(
        &view,
        &KnowledgeMemory::new(spec).unwrap(),
        SolverBudget::default(),
    )
    .unwrap();
    assert!(proof.safe().is_empty() && proof.mines().is_empty() && proof.lies().is_empty());
}

#[test]
fn visible_mines_and_total_count_prove_a_lie_without_any_secret_board() {
    let spec = BoardSpec {
        width: 3,
        height: 2,
        mines: 1,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(0), ObservedCell::Mine).unwrap();
    view.set(CellId(1), ObservedCell::Number(2)).unwrap();
    let mut memory = KnowledgeMemory::new(spec).unwrap();
    let proof = KnowledgeSolver::deduce(&view, &memory, SolverBudget::default()).unwrap();
    assert_eq!(proof.lies(), &[CellId(1)]);
    assert_eq!(proof.safe(), &[CellId(2), CellId(3), CellId(4), CellId(5)]);
    memory.absorb(&proof).unwrap();
    view.set(CellId(1), ObservedCell::Number(1)).unwrap();
    memory.confirm_truth(&view, CellId(1)).unwrap();
    let corrected = KnowledgeSolver::deduce(&view, &memory, SolverBudget::default()).unwrap();
    assert!(corrected.lies().is_empty());
    assert_eq!(corrected.safe(), proof.safe());
}

#[test]
fn knowledge_model_budget_and_invalid_memory_context_fail_closed() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(4), ObservedCell::Number(2)).unwrap();
    let memory = KnowledgeMemory::new(spec).unwrap();
    let limit = SolverBudget {
        max_nodes: 2,
        ..SolverBudget::default()
    };
    assert_eq!(
        KnowledgeSolver::deduce(&view, &memory, limit).err(),
        Some(KnowledgeError::BudgetExceeded)
    );
    let wrong = KnowledgeMemory::new(BoardSpec { mines: 1, ..spec }).unwrap();
    assert_eq!(
        KnowledgeSolver::deduce(&view, &wrong, SolverBudget::default()).err(),
        Some(KnowledgeError::InvalidContext)
    );
}

#[test]
fn global_lie_budget_couples_disconnected_frontiers_and_rejects_three_required_lies() {
    let spec = BoardSpec {
        width: 9,
        height: 1,
        mines: 3,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    for cell in [1, 4, 7] {
        view.set(CellId(cell), ObservedCell::Number(2)).unwrap();
    }
    assert_eq!(
        KnowledgeSolver::deduce(
            &view,
            &KnowledgeMemory::new(spec).unwrap(),
            KnowledgeSolver::default_budget()
        )
        .err(),
        Some(KnowledgeError::Inconsistent)
    );
    let valid_spec = BoardSpec { mines: 4, ..spec };
    let valid = Observation::new(valid_spec, view.cells().to_vec()).unwrap();
    let proof = KnowledgeSolver::deduce(
        &valid,
        &KnowledgeMemory::new(valid_spec).unwrap(),
        KnowledgeSolver::default_budget(),
    )
    .unwrap();
    assert!(proof.safe().is_empty() && proof.mines().is_empty() && proof.lies().is_empty());
}

#[test]
fn impossible_public_numbers_and_invalid_truth_or_memory_updates_are_rejected() {
    let spec = BoardSpec {
        width: 4,
        height: 2,
        mines: 1,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(0), ObservedCell::Mine).unwrap();
    for cell in [1, 4, 5] {
        view.set(CellId(cell), ObservedCell::Number(2)).unwrap();
    }
    let mut memory = KnowledgeMemory::new(spec).unwrap();
    assert_eq!(
        KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget()).err(),
        Some(KnowledgeError::Inconsistent)
    );
    assert_eq!(
        memory.confirm_truth(&view, CellId(0)),
        Err(KnowledgeError::InvalidContext)
    );
    assert_eq!(
        memory.confirm_truth(&view, CellId(8)),
        Err(KnowledgeError::InvalidContext)
    );
    view.set(CellId(4), ObservedCell::Unknown).unwrap();
    view.set(CellId(5), ObservedCell::Unknown).unwrap();
    let proof = KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    let mut wrong_memory = KnowledgeMemory::new(BoardSpec { mines: 2, ..spec }).unwrap();
    assert_eq!(
        wrong_memory.absorb(&proof),
        Err(KnowledgeError::InvalidContext)
    );
    memory.confirm_truth(&view, CellId(1)).unwrap();
    assert_eq!(
        KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget()).err(),
        Some(KnowledgeError::Inconsistent)
    );
}

#[test]
fn automatic_opening_is_trusted_by_public_order_but_other_snapshot_numbers_are_not() {
    use liar_core::board::Board;
    let spec = BoardSpec {
        width: 4,
        height: 4,
        mines: 4,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(6), CellId(7), CellId(9), CellId(14)]).unwrap();
    let mut view = Observation::closed(spec).unwrap();
    assert_eq!(
        KnowledgeMemory::from_initial_opening(&view).err(),
        Some(KnowledgeError::InvalidContext)
    );
    board.reveal(&mut view, CellId(0), &[false; 16]).unwrap();
    view.set(CellId(15), ObservedCell::Number(1)).unwrap();
    let memory = KnowledgeMemory::from_initial_opening(&view).unwrap();
    let proof = KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    for cell in [0, 1, 4, 5] {
        assert!(proof.truth().contains(&CellId(cell)));
    }
    assert!(!proof.truth().contains(&CellId(15)));
    let mut incomplete = view.clone();
    incomplete.set(CellId(1), ObservedCell::Unknown).unwrap();
    assert_eq!(
        KnowledgeMemory::from_initial_opening(&incomplete).err(),
        Some(KnowledgeError::InvalidContext)
    );
}
