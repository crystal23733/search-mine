use liar_core::board::{Board, BoardSpec, Cell, CellId};
use liar_core::generator::{
    BoardGenerator, CertificationError, GenerationBudget, GenerationError, certify,
    verify_certificate,
};
use liar_core::random::{RNG_VERSION, RandomSource, SeededRng};
use liar_core::solver::SolverBudget;

#[test]
fn fixed_seed_generates_the_same_certified_zero_opening_board() {
    let spec = BoardSpec::default();
    let left = BoardGenerator::generate(spec, 42, GenerationBudget::default())
        .unwrap_or_else(|e| panic!("generation failed: {e:?}"));
    let right = BoardGenerator::generate(spec, 42, GenerationBudget::default())
        .unwrap_or_else(|e| panic!("generation failed: {e:?}"));
    assert!(left.board == right.board && left.certificate == right.certificate);
    assert_eq!(left.board.cell(spec.opening), Some(Cell::Number(0)));
    assert_eq!(left.board.safe_total(), 216);
    assert_eq!(left.certificate.rng_version, RNG_VERSION);
    assert!(verify_certificate(
        &left.board,
        &left.certificate,
        SolverBudget::default()
    ));
}

#[test]
fn candidate_limit_and_impossible_opening_fail_closed() {
    let budget = GenerationBudget {
        max_candidates: 0,
        ..GenerationBudget::default()
    };
    assert!(matches!(
        BoardGenerator::generate(BoardSpec::default(), 0, budget),
        Err(GenerationError::Exhausted { candidates: 0, .. })
    ));
    let spec = BoardSpec {
        width: 2,
        height: 2,
        mines: 1,
        opening: CellId(0),
    };
    assert!(matches!(
        BoardGenerator::generate(spec, 0, GenerationBudget::default()),
        Err(GenerationError::ImpossibleOpening)
    ));
}

#[test]
fn unsolved_truth_boards_and_forged_proofs_are_rejected() {
    let spec = BoardSpec {
        width: 4,
        height: 4,
        mines: 2,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(2), CellId(5)]).unwrap();
    assert_eq!(
        certify(&board, SolverBudget::default()).err(),
        Some(CertificationError::InvalidOpening)
    );
    let stuck_spec = BoardSpec { mines: 4, ..spec };
    let stuck =
        Board::from_mines(stuck_spec, &[CellId(2), CellId(7), CellId(9), CellId(14)]).unwrap();
    assert_eq!(
        certify(&stuck, SolverBudget::default()).err(),
        Some(CertificationError::Stalled)
    );
    let generated = BoardGenerator::generate(BoardSpec::default(), 42, GenerationBudget::default())
        .unwrap_or_else(|e| panic!("generation failed: {e:?}"));
    let mut proof = generated.certificate;
    proof.solver_version += 1;
    assert!(!verify_certificate(
        &generated.board,
        &proof,
        SolverBudget::default()
    ));
    proof.solver_version -= 1;
    proof.initial.push(CellId(255));
    assert!(!verify_certificate(
        &generated.board,
        &proof,
        SolverBudget::default()
    ));
}

#[test]
fn rng_v1_matches_the_fixed_integer_algorithm() {
    let mut random = SeededRng::new(0);
    assert_eq!(random.next_u64(), 0xe220a8397b1dcdaf);
    assert_eq!(random.next_u64(), 0x6e789e6aa1b965f4);
    assert_eq!(random.next_u64(), 0x06c45d188009454f);
}
