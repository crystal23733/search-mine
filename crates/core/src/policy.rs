//! Public known-neighborhood-v1 strategy. Inference consumes only the owner's public history.
use crate::board::{CellId, Observation, ObservedCell};
use crate::knowledge::{KnowledgeMemory, KnownCell};
use crate::solver::{NoGuessSolver, SOLVER_VERSION, SolverBudget, SolverError};

pub const ATTACK_STRATEGY: &str = "known-neighborhood-v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    InvalidInitial,
    InvalidProjection,
    Inconsistent,
    HistoryLimit,
    InvalidTarget,
    InvalidDelta,
    Capacity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicCellUpdate {
    pub cell: CellId,
    pub observed: ObservedCell,
}

// No Serialize/Debug: these artifacts include internal proof support, never public DTOs.
#[derive(Clone, PartialEq, Eq)]
pub struct PolicyKnowledge {
    public: Observation,
    logical: Observation,
    facts: Vec<KnownCell>,
    registered: Vec<Option<u8>>,
    history: Vec<Vec<PublicCellUpdate>>,
    budget: SolverBudget,
    analysis: Option<SolverError>,
}
pub struct PolicyCertificate {
    source: PolicyKnowledge,
    target: CellId,
    delta: i8,
    truth: u8,
    displayed: u8,
    version: u16,
}
impl PolicyCertificate {
    pub fn target(&self) -> CellId {
        self.target
    }
    pub fn delta(&self) -> i8 {
        self.delta
    }
    pub fn expected_truth(&self) -> u8 {
        self.truth
    }
    pub fn displayed(&self) -> u8 {
        self.displayed
    }
    pub fn matches(&self, current: &PolicyKnowledge) -> bool {
        self.version == SOLVER_VERSION && self.source == *current
    }
}
impl PolicyKnowledge {
    pub fn start(initial: &Observation, budget: SolverBudget) -> Result<Self, PolicyError> {
        let opening = KnowledgeMemory::from_initial_opening(initial)
            .map_err(|_| PolicyError::InvalidInitial)?;
        if initial
            .cells()
            .iter()
            .enumerate()
            .any(|(i, cell)| match cell {
                ObservedCell::Number(_) => !opening.trusted[i],
                ObservedCell::Mine => true,
                ObservedCell::Unknown => false,
            })
        {
            return Err(PolicyError::InvalidInitial);
        }
        let mut ledger = Self {
            public: initial.clone(),
            logical: initial.clone(),
            facts: opening.cells,
            registered: vec![None; initial.spec().area()],
            history: vec![updates(initial)],
            budget,
            analysis: None,
        };
        ledger.refresh()?;
        Ok(ledger)
    }
    pub fn public_view(&self) -> &Observation {
        &self.public
    }
    pub fn public_history(&self) -> &[Vec<PublicCellUpdate>] {
        &self.history
    }
    pub fn analysis(&self) -> Option<SolverError> {
        self.analysis
    }
    pub fn safe(&self) -> Vec<CellId> {
        self.facts
            .iter()
            .enumerate()
            .filter_map(|(i, &fact)| {
                (fact == KnownCell::Safe && self.public.cells()[i] == ObservedCell::Unknown)
                    .then_some(CellId(i as u16))
            })
            .collect()
    }
    /// Difficulty budgets limit safe planning after canonical public-history classification.
    pub fn limited_safe(&self, budget: SolverBudget) -> Result<Vec<CellId>, SolverError> {
        Ok(NoGuessSolver::deduce(&self.logical, budget)?.safe)
    }
    pub fn known_lies(&self) -> Vec<CellId> {
        self.public
            .cells()
            .iter()
            .zip(self.logical.cells())
            .enumerate()
            .filter_map(|(i, (&shown, &logical))| {
                (matches!(shown, ObservedCell::Number(_)) && shown != logical)
                    .then_some(CellId(i as u16))
            })
            .collect()
    }
    /// Public projection changes are committed separately from bounded analysis work.
    pub fn observe(&mut self, projection: &Observation) -> Result<(), PolicyError> {
        if projection.spec() != self.public.spec() {
            return Err(PolicyError::InvalidProjection);
        }
        let mut next = self.clone();
        let mut changes = Vec::new();
        for (i, (&old, &observed)) in self
            .public
            .cells()
            .iter()
            .zip(projection.cells())
            .enumerate()
        {
            if old == observed {
                continue;
            }
            let cell = CellId(i as u16);
            let logical = match (old, observed) {
                (ObservedCell::Unknown, ObservedCell::Mine) if next.facts[i] != KnownCell::Safe => {
                    ObservedCell::Mine
                }
                (ObservedCell::Unknown, ObservedCell::Number(displayed))
                    if next.facts[i] != KnownCell::Mine =>
                {
                    let truth = next.registered[i].unwrap_or(displayed);
                    if truth != displayed
                        && (truth == 0 || displayed == 0 || truth.abs_diff(displayed) != 1)
                    {
                        return Err(PolicyError::InvalidProjection);
                    }
                    ObservedCell::Number(truth)
                }
                (ObservedCell::Number(_), ObservedCell::Number(_))
                    if observed == self.logical.cells()[i] =>
                {
                    observed
                }
                _ => return Err(PolicyError::InvalidProjection),
            };
            next.logical
                .set(cell, logical)
                .map_err(|_| PolicyError::InvalidProjection)?;
            next.facts[i] = if logical == ObservedCell::Mine {
                KnownCell::Mine
            } else {
                KnownCell::Safe
            };
            changes.push(PublicCellUpdate { cell, observed });
        }
        next.public = projection.clone();
        if next.known_lies().len() > 2 {
            return Err(PolicyError::Inconsistent);
        }
        if !changes.is_empty() {
            if next.history.len() >= 512 {
                return Err(PolicyError::HistoryLimit);
            }
            next.history.push(changes);
        }
        *self = next;
        Ok(())
    }
    /// Run in bounded analysis work; a budget rejection retains earlier immutable facts.
    pub fn refresh(&mut self) -> Result<(), PolicyError> {
        let proof = match NoGuessSolver::deduce(&self.logical, self.budget) {
            Ok(proof) => {
                self.analysis = None;
                proof
            }
            Err(SolverError::BudgetExceeded) => {
                self.analysis = Some(SolverError::BudgetExceeded);
                return Ok(());
            }
            Err(SolverError::Inconsistent) => return Err(PolicyError::Inconsistent),
        };
        for &cell in &proof.safe {
            self.merge(cell, KnownCell::Safe)?;
        }
        for &cell in &proof.mines {
            self.merge(cell, KnownCell::Mine)?;
        }
        for i in 0..self.public.spec().area() {
            if self.public.cells()[i] != ObservedCell::Unknown || self.facts[i] != KnownCell::Safe {
                continue;
            }
            let neighbors = self.public.spec().neighbors(CellId(i as u16));
            if neighbors
                .iter()
                .any(|c| self.facts[usize::from(c.0)] == KnownCell::Unknown)
            {
                continue;
            }
            let truth = neighbors
                .iter()
                .filter(|c| self.facts[usize::from(c.0)] == KnownCell::Mine)
                .count() as u8;
            if truth > 0 {
                self.registered[i] = Some(truth);
            }
        }
        Ok(())
    }
    fn merge(&mut self, cell: CellId, value: KnownCell) -> Result<(), PolicyError> {
        let previous = self.facts[usize::from(cell.0)];
        if previous != KnownCell::Unknown && previous != value {
            return Err(PolicyError::Inconsistent);
        }
        self.facts[usize::from(cell.0)] = value;
        Ok(())
    }
    pub fn certificate(
        &self,
        target: CellId,
        delta: i8,
        active: usize,
    ) -> Result<PolicyCertificate, PolicyError> {
        if active >= 2 {
            return Err(PolicyError::Capacity);
        }
        if delta != -1 && delta != 1 {
            return Err(PolicyError::InvalidDelta);
        }
        if self.public.cell(target) != Some(ObservedCell::Unknown) {
            return Err(PolicyError::InvalidTarget);
        }
        let truth = self.registered[usize::from(target.0)].ok_or(PolicyError::InvalidTarget)?;
        let displayed = i16::from(truth) + i16::from(delta);
        if !(1..=8).contains(&displayed) {
            return Err(PolicyError::InvalidDelta);
        }
        Ok(PolicyCertificate {
            source: self.clone(),
            target,
            delta,
            truth,
            displayed: displayed as u8,
            version: SOLVER_VERSION,
        })
    }
    pub fn replay(
        initial: &Observation,
        history: &[Vec<PublicCellUpdate>],
        budget: SolverBudget,
    ) -> Result<Self, PolicyError> {
        if history.len() > 512 {
            return Err(PolicyError::HistoryLimit);
        }
        if history.first() != Some(&updates(initial)) {
            return Err(PolicyError::InvalidInitial);
        }
        let mut ledger = Self::start(initial, budget)?;
        for batch in history.iter().skip(1) {
            ledger.replay_batch(batch)?;
        }
        Ok(ledger)
    }
    pub fn replay_batch(&mut self, batch: &[PublicCellUpdate]) -> Result<(), PolicyError> {
        if batch.is_empty() {
            return Err(PolicyError::InvalidProjection);
        }
        let mut projection = self.public.clone();
        let mut seen = vec![false; self.public.spec().area()];
        for update in batch {
            let i = usize::from(update.cell.0);
            if i >= seen.len()
                || seen[i]
                || update.observed == ObservedCell::Unknown
                || projection.cell(update.cell) == Some(update.observed)
            {
                return Err(PolicyError::InvalidProjection);
            }
            seen[i] = true;
            projection
                .set(update.cell, update.observed)
                .map_err(|_| PolicyError::InvalidProjection)?;
        }
        let mut next = self.clone();
        next.observe(&projection)?;
        next.refresh()?;
        *self = next;
        Ok(())
    }
}
fn updates(view: &Observation) -> Vec<PublicCellUpdate> {
    view.cells()
        .iter()
        .enumerate()
        .filter_map(|(i, &observed)| {
            (observed != ObservedCell::Unknown).then_some(PublicCellUpdate {
                cell: CellId(i as u16),
                observed,
            })
        })
        .collect()
}
