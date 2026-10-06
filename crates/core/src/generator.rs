//! Bounded no-guess generation. Certificates and boards are server-private.
use crate::board::{Board, BoardError, BoardSpec, Cell, CellId, Observation};
use crate::random::{RNG_VERSION, SeededRng, shuffle};
use crate::solver::{Deduction, NoGuessSolver, SOLVER_VERSION, SolverBudget, SolverError};

#[derive(Clone, Copy, Debug)]
pub struct GenerationBudget {
    pub max_candidates: u16,
    pub solver: SolverBudget,
}
impl Default for GenerationBudget {
    fn default() -> Self {
        Self {
            max_candidates: 256,
            solver: SolverBudget::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationError {
    InvalidSpec(BoardError),
    ImpossibleOpening,
    Exhausted {
        candidates: u16,
        stalled: u16,
        budget_rejections: u16,
    },
    UnsoundInference,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificationError {
    InvalidOpening,
    Stalled,
    BudgetExceeded,
    UnsoundInference,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SolutionCertificate {
    pub rng_version: u16,
    pub solver_version: u16,
    pub candidate: u16,
    pub initial: Vec<CellId>,
    pub steps: Vec<Deduction>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationStats {
    pub candidates: u16,
    pub stalled: u16,
    pub budget_rejections: u16,
}
pub struct GeneratedBoard {
    pub board: Board,
    pub certificate: SolutionCertificate,
    pub stats: GenerationStats,
}
pub struct BoardGenerator;
impl BoardGenerator {
    pub fn generate(
        spec: BoardSpec,
        seed: u64,
        budget: GenerationBudget,
    ) -> Result<GeneratedBoard, GenerationError> {
        let spec = spec.validate().map_err(GenerationError::InvalidSpec)?;
        let mut excluded = spec.neighbors(spec.opening);
        excluded.push(spec.opening);
        let available: Vec<_> = (0..spec.area())
            .map(|i| CellId(i as u16))
            .filter(|cell| !excluded.contains(cell))
            .collect();
        if available.len() < usize::from(spec.mines) {
            return Err(GenerationError::ImpossibleOpening);
        }
        let mut random = SeededRng::new(seed);
        let mut stalled = 0;
        let mut budget_rejections = 0;
        for candidate in 0..budget.max_candidates {
            let mut cells = available.clone();
            shuffle(&mut cells, &mut random);
            let board = Board::from_mines(spec, &cells[..usize::from(spec.mines)])
                .map_err(GenerationError::InvalidSpec)?;
            match certify(&board, budget.solver) {
                Ok(mut certificate) => {
                    certificate.candidate = candidate;
                    return Ok(GeneratedBoard {
                        board,
                        certificate,
                        stats: GenerationStats {
                            candidates: candidate + 1,
                            stalled,
                            budget_rejections,
                        },
                    });
                }
                Err(CertificationError::Stalled) => stalled += 1,
                Err(CertificationError::BudgetExceeded) => budget_rejections += 1,
                Err(_) => return Err(GenerationError::UnsoundInference),
            }
        }
        Err(GenerationError::Exhausted {
            candidates: budget.max_candidates,
            stalled,
            budget_rejections,
        })
    }
}
pub fn certify(
    board: &Board,
    budget: SolverBudget,
) -> Result<SolutionCertificate, CertificationError> {
    if board.cell(board.spec().opening) != Some(Cell::Number(0)) {
        return Err(CertificationError::InvalidOpening);
    }
    let mut view =
        Observation::closed(board.spec()).map_err(|_| CertificationError::InvalidOpening)?;
    let flags = vec![false; board.spec().area()];
    let initial = board
        .reveal(&mut view, board.spec().opening, &flags)
        .map_err(|_| CertificationError::InvalidOpening)?;
    let mut steps = Vec::new();
    while view.opened_safe() < board.safe_total() {
        let deduction = NoGuessSolver::deduce(&view, budget).map_err(|error| match error {
            SolverError::BudgetExceeded => CertificationError::BudgetExceeded,
            SolverError::Inconsistent => CertificationError::UnsoundInference,
        })?;
        if deduction.safe.is_empty() {
            return Err(CertificationError::Stalled);
        }
        for &mine in &deduction.mines {
            if board.cell(mine) != Some(Cell::Mine) {
                return Err(CertificationError::UnsoundInference);
            }
        }
        for &safe in &deduction.safe {
            if !matches!(board.cell(safe), Some(Cell::Number(_))) {
                return Err(CertificationError::UnsoundInference);
            }
            board
                .reveal(&mut view, safe, &flags)
                .map_err(|_| CertificationError::UnsoundInference)?;
        }
        steps.push(deduction);
    }
    Ok(SolutionCertificate {
        rng_version: RNG_VERSION,
        solver_version: SOLVER_VERSION,
        candidate: 0,
        initial,
        steps,
    })
}
pub fn verify_certificate(
    board: &Board,
    certificate: &SolutionCertificate,
    budget: SolverBudget,
) -> bool {
    match certify(board, budget) {
        Ok(actual) => {
            actual.rng_version == certificate.rng_version
                && actual.solver_version == certificate.solver_version
                && actual.initial == certificate.initial
                && actual.steps == certificate.steps
        }
        Err(_) => false,
    }
}
