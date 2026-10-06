//! Conservative public-information certificate. Never serialize these server-private artifacts.
use crate::board::{CellId, Observation};
use crate::knowledge::{
    Inference, KNOWLEDGE_VERSION, KnowledgeError, KnowledgeMemory, KnowledgeSolver, KnownCell,
};
use crate::solver::SolverBudget;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LieRejected {
    InvalidTarget,
    InvalidDelta,
    Zero,
    Ambiguous,
    Capacity,
    BudgetExceeded,
    Inconsistent,
    InvalidContext,
}
struct Evidence {
    view: Observation,
    memory: KnowledgeMemory,
    inference: Inference,
}
pub struct PreparedProof {
    evidence: Arc<Evidence>,
}
pub struct LieCertificate {
    evidence: Arc<Evidence>,
    target: CellId,
    delta: i8,
    truth: u8,
    displayed: u8,
    version: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofAction {
    OpenSafe(CellId),
    AccuseLie(CellId),
}
impl PreparedProof {
    pub fn certificate(
        &self,
        target: CellId,
        delta: i8,
        active_lies: usize,
    ) -> Result<LieCertificate, LieRejected> {
        if active_lies >= 2 {
            return Err(LieRejected::Capacity);
        }
        if delta != -1 && delta != 1 {
            return Err(LieRejected::InvalidDelta);
        }
        if self.evidence.view.cell(target) != Some(crate::board::ObservedCell::Unknown) {
            return Err(LieRejected::InvalidTarget);
        }
        if self.evidence.inference.known[usize::from(target.0)] != KnownCell::Safe {
            return Err(LieRejected::Ambiguous);
        }
        let mut truth = 0u8;
        for neighbor in self.evidence.view.spec().neighbors(target) {
            match self.evidence.inference.known[usize::from(neighbor.0)] {
                KnownCell::Mine => truth += 1,
                KnownCell::Safe => {}
                KnownCell::Unknown => return Err(LieRejected::Ambiguous),
            }
        }
        if truth == 0 {
            return Err(LieRejected::Zero);
        }
        let displayed = i16::from(truth) + i16::from(delta);
        if !(1..=8).contains(&displayed) {
            return Err(LieRejected::InvalidDelta);
        }
        Ok(LieCertificate {
            evidence: self.evidence.clone(),
            target,
            delta,
            truth,
            displayed: displayed as u8,
            version: KNOWLEDGE_VERSION,
        })
    }
    pub fn nodes(&self) -> usize {
        self.evidence.inference.nodes
    }
    pub fn inference(&self) -> &Inference {
        &self.evidence.inference
    }
    pub fn remember(&self, memory: &mut KnowledgeMemory) -> Result<(), KnowledgeError> {
        memory.absorb(&self.evidence.inference)
    }
}
impl LieCertificate {
    pub fn target(&self) -> CellId {
        self.target
    }
    pub fn delta(&self) -> i8 {
        self.delta
    }
    pub fn displayed(&self) -> u8 {
        self.displayed
    }
    /// Internal proof support. Never include it in a public protocol response.
    pub fn expected_truth(&self) -> u8 {
        self.truth
    }
    pub fn steps(&self) -> [ProofAction; 2] {
        [
            ProofAction::OpenSafe(self.target),
            ProofAction::AccuseLie(self.target),
        ]
    }
    pub fn matches(&self, view: &Observation, memory: &KnowledgeMemory) -> bool {
        self.version == KNOWLEDGE_VERSION
            && self.evidence.view == *view
            && self.evidence.memory == *memory
    }
}
pub struct LieValidator;
impl LieValidator {
    pub fn prepare(
        view: &Observation,
        memory: &KnowledgeMemory,
        budget: SolverBudget,
    ) -> Result<PreparedProof, LieRejected> {
        let proof = KnowledgeSolver::deduce(view, memory, budget).map_err(|error| match error {
            KnowledgeError::Inconsistent => LieRejected::Inconsistent,
            KnowledgeError::BudgetExceeded => LieRejected::BudgetExceeded,
            KnowledgeError::InvalidContext => LieRejected::InvalidContext,
        })?;
        Ok(PreparedProof {
            evidence: Arc::new(Evidence {
                view: view.clone(),
                memory: memory.clone(),
                inference: proof,
            }),
        })
    }
}
