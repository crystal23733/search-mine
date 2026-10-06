use liar_core::board::{BoardSpec, CellId, Observation, ObservedCell};
use liar_core::knowledge::{KnowledgeMemory, KnowledgeSolver};
use liar_core::lie::{LieRejected, LieValidator, ProofAction};
use liar_core::solver::SolverBudget;

fn public_position() -> (Observation, KnowledgeMemory) {
    let spec = BoardSpec {
        width: 4,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(0), ObservedCell::Mine).unwrap();
    view.set(CellId(11), ObservedCell::Mine).unwrap();
    (view, KnowledgeMemory::new(spec).unwrap())
}
#[test]
fn two_lies_have_safe_open_and_definite_accusation_paths_using_only_public_information() {
    let (mut view, mut memory) = public_position();
    let prepared = LieValidator::prepare(&view, &memory, SolverBudget::default()).unwrap();
    let first = prepared.certificate(CellId(5), 1, 0).unwrap();
    assert_eq!(
        first.steps(),
        [
            ProofAction::OpenSafe(CellId(5)),
            ProofAction::AccuseLie(CellId(5))
        ]
    );
    assert_eq!(first.displayed(), 2);
    assert!(first.matches(&view, &memory));
    prepared.remember(&mut memory).unwrap();
    view.set(CellId(5), ObservedCell::Number(first.displayed()))
        .unwrap();
    assert!(!first.matches(&view, &memory));
    let second_prepared = LieValidator::prepare(&view, &memory, SolverBudget::default()).unwrap();
    let second = second_prepared.certificate(CellId(6), 1, 1).unwrap();
    second_prepared.remember(&mut memory).unwrap();
    view.set(CellId(6), ObservedCell::Number(second.displayed()))
        .unwrap();
    let knowledge = KnowledgeSolver::deduce(&view, &memory, SolverBudget::default()).unwrap();
    assert_eq!(knowledge.lies(), &[CellId(5), CellId(6)]);
}
#[test]
fn capacity_invalid_delta_open_cells_and_zero_are_rejected() {
    let (view, memory) = public_position();
    let prepared = LieValidator::prepare(&view, &memory, SolverBudget::default()).unwrap();
    assert_eq!(
        prepared.certificate(CellId(5), 1, 2).err(),
        Some(LieRejected::Capacity)
    );
    assert_eq!(
        prepared.certificate(CellId(5), 0, 0).err(),
        Some(LieRejected::InvalidDelta)
    );
    assert_eq!(
        prepared.certificate(CellId(0), 1, 0).err(),
        Some(LieRejected::InvalidTarget)
    );
    assert_eq!(
        prepared.certificate(CellId(12), 1, 0).err(),
        Some(LieRejected::InvalidTarget)
    );
    assert_eq!(
        prepared.certificate(CellId(3), 1, 0).err(),
        Some(LieRejected::Zero)
    );
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 8,
        opening: CellId(0),
    };
    let mut extreme = Observation::closed(spec).unwrap();
    for cell in (0..9).filter(|&i| i != 4) {
        extreme.set(CellId(cell), ObservedCell::Mine).unwrap();
    }
    let proof = LieValidator::prepare(
        &extreme,
        &KnowledgeMemory::new(spec).unwrap(),
        SolverBudget::default(),
    )
    .unwrap();
    assert_eq!(
        proof.certificate(CellId(4), 1, 0).err(),
        Some(LieRejected::InvalidDelta)
    );
    assert_eq!(proof.certificate(CellId(4), -1, 0).unwrap().displayed(), 7);
}

#[test]
fn certificate_is_valid_across_distinct_hidden_worlds_with_the_same_public_view() {
    use liar_core::board::{Board, Cell};
    let spec = BoardSpec {
        width: 7,
        height: 2,
        mines: 2,
        opening: CellId(0),
    };
    let mut view = Observation::closed(spec).unwrap();
    view.set(CellId(0), ObservedCell::Mine).unwrap();
    for (cell, number) in [(2, 0), (7, 1), (8, 1), (9, 0)] {
        view.set(CellId(cell), ObservedCell::Number(number))
            .unwrap();
    }
    let mut memory = KnowledgeMemory::new(spec).unwrap();
    let prepared =
        LieValidator::prepare(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    let proof = prepared.certificate(CellId(1), 1, 0).unwrap();
    assert_eq!(proof.expected_truth(), 1);
    for other_mine in [4, 5, 6, 11, 12, 13] {
        let world = Board::from_mines(spec, &[CellId(0), CellId(other_mine)]).unwrap();
        assert_eq!(
            world.cell(CellId(1)),
            Some(Cell::Number(proof.expected_truth()))
        );
        for (i, &observed) in view.cells().iter().enumerate() {
            if let ObservedCell::Number(number) = observed {
                assert_eq!(world.cell(CellId(i as u16)), Some(Cell::Number(number)));
            }
        }
    }
    prepared.remember(&mut memory).unwrap();
    view.set(CellId(1), ObservedCell::Number(proof.displayed()))
        .unwrap();
    let inference =
        KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    assert_eq!(inference.lies(), &[CellId(1)]);
}

#[test]
fn closed_lies_can_be_opened_in_either_order_and_stale_or_ambiguous_preparation_is_rejected() {
    let (mut view, mut memory) = public_position();
    let first_prepared =
        LieValidator::prepare(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    let first = first_prepared.certificate(CellId(5), 1, 0).unwrap();
    first_prepared.remember(&mut memory).unwrap();
    assert!(!first.matches(&view, &memory));
    let second_prepared =
        LieValidator::prepare(&view, &memory, KnowledgeSolver::default_budget()).unwrap();
    let second = second_prepared.certificate(CellId(6), 1, 1).unwrap();
    second_prepared.remember(&mut memory).unwrap();
    view.set(CellId(6), ObservedCell::Number(second.displayed()))
        .unwrap();
    assert_eq!(
        KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget())
            .unwrap()
            .lies(),
        &[CellId(6)]
    );
    view.set(CellId(5), ObservedCell::Number(first.displayed()))
        .unwrap();
    assert_eq!(
        KnowledgeSolver::deduce(&view, &memory, KnowledgeSolver::default_budget())
            .unwrap()
            .lies(),
        &[CellId(5), CellId(6)]
    );
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let mut ambiguous = Observation::closed(spec).unwrap();
    ambiguous.set(CellId(4), ObservedCell::Number(2)).unwrap();
    let empty = KnowledgeMemory::new(spec).unwrap();
    let proof =
        LieValidator::prepare(&ambiguous, &empty, KnowledgeSolver::default_budget()).unwrap();
    assert_eq!(
        proof.certificate(CellId(0), 1, 0).err(),
        Some(LieRejected::Ambiguous)
    );
    assert_eq!(
        LieValidator::prepare(
            &ambiguous,
            &empty,
            SolverBudget {
                max_nodes: 1,
                ..KnowledgeSolver::default_budget()
            }
        )
        .err(),
        Some(LieRejected::BudgetExceeded)
    );
    assert_eq!(
        LieValidator::prepare(
            &ambiguous,
            &KnowledgeMemory::new(BoardSpec { mines: 1, ..spec }).unwrap(),
            KnowledgeSolver::default_budget()
        )
        .err(),
        Some(LieRejected::InvalidContext)
    );
}
