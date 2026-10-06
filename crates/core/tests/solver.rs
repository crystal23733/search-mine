use liar_core::board::{BoardSpec, CellId, Observation, ObservedCell};
use liar_core::solver::{NoGuessSolver, SolverBudget, SolverError};

#[test]
fn safe_cells_are_deduced_from_zero_without_a_secret_board() {
    let spec = BoardSpec {
        width: 3,
        height: 2,
        mines: 1,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(0), ObservedCell::Number(0)).unwrap();
    let proof = NoGuessSolver::deduce(&view, SolverBudget::default()).unwrap();
    assert_eq!(proof.safe, vec![CellId(1), CellId(3), CellId(4)]);
    assert!(proof.mines.is_empty());
}

#[test]
fn ambiguity_and_inconsistent_observations_do_not_claim_safety() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(4), ObservedCell::Number(2)).unwrap();
    let proof = NoGuessSolver::deduce(&view, SolverBudget::default()).unwrap();
    assert!(proof.safe.is_empty() && proof.mines.is_empty());
    view.set(CellId(0), ObservedCell::Number(8)).unwrap();
    assert_eq!(
        NoGuessSolver::deduce(&view, SolverBudget::default()),
        Err(SolverError::Inconsistent)
    );
}

#[test]
fn partial_model_enumeration_is_never_used_as_a_proof() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(4), ObservedCell::Number(2)).unwrap();
    let budget = SolverBudget {
        max_nodes: 2,
        ..SolverBudget::default()
    };
    assert_eq!(
        NoGuessSolver::deduce(&view, budget),
        Err(SolverError::BudgetExceeded)
    );
    let component_limit = SolverBudget {
        max_component_cells: 2,
        ..SolverBudget::default()
    };
    assert_eq!(
        NoGuessSolver::deduce(&view, component_limit),
        Err(SolverError::BudgetExceeded)
    );
    let constraint_limit = SolverBudget {
        max_constraints: 0,
        ..SolverBudget::default()
    };
    assert_eq!(
        NoGuessSolver::deduce(&view, constraint_limit),
        Err(SolverError::BudgetExceeded)
    );
}

#[test]
fn subset_constraints_identify_the_middle_safe_cell_of_a_121_frontier() {
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
    let proof = NoGuessSolver::deduce(&view, SolverBudget::default()).unwrap();
    assert_eq!(proof.safe, vec![CellId(4)]);
    assert_eq!(proof.mines, vec![CellId(3), CellId(5)]);
}
