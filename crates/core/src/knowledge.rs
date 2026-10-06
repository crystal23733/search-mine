//! Public-knowledge inference allowing up to two hidden visible lies.
use crate::board::{BoardError, BoardSpec, CellId, Observation, ObservedCell};
use crate::solver::SolverBudget;
pub const KNOWLEDGE_VERSION: u16 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnowledgeError {
    Inconsistent,
    BudgetExceeded,
    InvalidContext,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KnownCell {
    Unknown,
    Safe,
    Mine,
}
#[derive(Clone, PartialEq, Eq)]
pub struct KnowledgeMemory {
    pub(crate) spec: BoardSpec,
    pub(crate) cells: Vec<KnownCell>,
    pub(crate) trusted: Vec<bool>,
}
impl KnowledgeMemory {
    pub fn new(spec: BoardSpec) -> Result<Self, BoardError> {
        let spec = spec.validate()?;
        Ok(Self {
            spec,
            cells: vec![KnownCell::Unknown; spec.area()],
            trusted: vec![false; spec.area()],
        })
    }
    /// Trust only the public automatic opening, which precedes all attack commands.
    /// Its immutable connected zero region can also be reconstructed on reconnect.
    pub fn from_initial_opening(view: &Observation) -> Result<Self, KnowledgeError> {
        let mut memory = Self::new(view.spec()).map_err(|_| KnowledgeError::InvalidContext)?;
        let opening = view.spec().opening;
        if view.cell(opening) != Some(ObservedCell::Number(0)) {
            return Err(KnowledgeError::InvalidContext);
        }
        let mut seen = vec![false; view.spec().area()];
        let mut pending = vec![opening];
        seen[usize::from(opening.0)] = true;
        while let Some(cell) = pending.pop() {
            memory.trusted[usize::from(cell.0)] = true;
            memory.cells[usize::from(cell.0)] = KnownCell::Safe;
            for neighbor in view.spec().neighbors(cell) {
                let Some(ObservedCell::Number(value)) = view.cell(neighbor) else {
                    return Err(KnowledgeError::InvalidContext);
                };
                let index = usize::from(neighbor.0);
                memory.trusted[index] = true;
                memory.cells[index] = KnownCell::Safe;
                if value == 0 && !seen[index] {
                    seen[index] = true;
                    pending.push(neighbor);
                }
            }
        }
        Ok(memory)
    }
    pub fn absorb(&mut self, proof: &Inference) -> Result<(), KnowledgeError> {
        if self.spec != proof.spec {
            return Err(KnowledgeError::InvalidContext);
        }
        let mut next = self.clone();
        for (i, &value) in proof.known.iter().enumerate() {
            if value == KnownCell::Unknown {
                continue;
            }
            if next.cells[i] != KnownCell::Unknown && next.cells[i] != value {
                return Err(KnowledgeError::Inconsistent);
            }
            next.cells[i] = value;
        }
        for &cell in &proof.truth {
            next.trusted[usize::from(cell.0)] = true;
        }
        *self = next;
        Ok(())
    }
    /// Called only after an authoritative accusation result, never from a user's flag.
    pub fn confirm_truth(
        &mut self,
        view: &Observation,
        cell: CellId,
    ) -> Result<(), KnowledgeError> {
        if self.spec != view.spec() || !matches!(view.cell(cell), Some(ObservedCell::Number(1..=8)))
        {
            return Err(KnowledgeError::InvalidContext);
        }
        self.trusted[usize::from(cell.0)] = true;
        Ok(())
    }
}
pub struct Inference {
    spec: BoardSpec,
    pub(crate) known: Vec<KnownCell>,
    safe: Vec<CellId>,
    mines: Vec<CellId>,
    lies: Vec<CellId>,
    truth: Vec<CellId>,
    pub nodes: usize,
}
impl Inference {
    pub fn safe(&self) -> &[CellId] {
        &self.safe
    }
    pub fn mines(&self) -> &[CellId] {
        &self.mines
    }
    pub fn lies(&self) -> &[CellId] {
        &self.lies
    }
    pub fn truth(&self) -> &[CellId] {
        &self.truth
    }
}
pub struct KnowledgeSolver;
impl KnowledgeSolver {
    pub fn default_budget() -> SolverBudget {
        SolverBudget {
            max_component_cells: 256,
            ..SolverBudget::default()
        }
    }
    pub fn deduce(
        view: &Observation,
        memory: &KnowledgeMemory,
        budget: SolverBudget,
    ) -> Result<Inference, KnowledgeError> {
        if view.spec() != memory.spec {
            return Err(KnowledgeError::InvalidContext);
        }
        let mut known = memory.cells.clone();
        for (i, cell) in view.cells().iter().enumerate() {
            match cell {
                ObservedCell::Number(_) => {
                    mark(&mut known, i, KnownCell::Safe)?;
                }
                ObservedCell::Mine => {
                    mark(&mut known, i, KnownCell::Mine)?;
                }
                ObservedCell::Unknown => {}
            }
        }
        let mut trusted = memory.trusted.clone();
        let (constraints, lies, truth) = loop {
            let mut changed = false;
            let unknown: Vec<_> = unknowns(&known);
            let remaining = remaining_mines(view, &known)?;
            if remaining > unknown.len() {
                return Err(KnowledgeError::Inconsistent);
            }
            if remaining == 0 || remaining == unknown.len() {
                let value = if remaining == 0 {
                    KnownCell::Safe
                } else {
                    KnownCell::Mine
                };
                for cell in unknown {
                    changed |= mark(&mut known, cell, value)?;
                }
            }
            let constraints = constraints(view, &known, &trusted);
            if constraints.len() > budget.max_constraints {
                return Err(KnowledgeError::BudgetExceeded);
            }
            let ranges: Vec<_> = constraints
                .iter()
                .map(|c| {
                    c.range(c.known_mines, c.known_mines + c.cells.len())
                        .ok_or(KnowledgeError::Inconsistent)
                })
                .collect::<Result<_, _>>()?;
            let forced_lies = ranges.iter().filter(|r| r.2).count();
            if forced_lies > 2 {
                return Err(KnowledgeError::Inconsistent);
            }
            let mut lies = Vec::new();
            let mut truth = Vec::new();
            for (constraint, &(lower, upper, lie)) in constraints.iter().zip(&ranges) {
                if lie {
                    lies.push(constraint.cell);
                }
                if !lie && (forced_lies == 2 || lower == upper) {
                    let i = usize::from(constraint.cell.0);
                    if !trusted[i] {
                        trusted[i] = true;
                        changed = true;
                    }
                    truth.push(constraint.cell);
                }
                if upper == constraint.known_mines
                    || lower == constraint.known_mines + constraint.cells.len()
                {
                    let value = if upper == constraint.known_mines {
                        KnownCell::Safe
                    } else {
                        KnownCell::Mine
                    };
                    for &cell in &constraint.cells {
                        changed |= mark(&mut known, cell, value)?;
                    }
                }
            }
            if !changed {
                break (constraints, lies, truth);
            }
        };
        let direct = inference(view, known.clone(), lies.clone(), truth.clone(), 0);
        if !direct.safe.is_empty() || !direct.lies.is_empty() {
            return Ok(direct);
        }
        model_inference(view, known, constraints, lies, truth, budget)
    }
}

fn mark(known: &mut [KnownCell], cell: usize, value: KnownCell) -> Result<bool, KnowledgeError> {
    if known[cell] == value {
        return Ok(false);
    }
    if known[cell] != KnownCell::Unknown {
        return Err(KnowledgeError::Inconsistent);
    }
    known[cell] = value;
    Ok(true)
}
fn unknowns(known: &[KnownCell]) -> Vec<usize> {
    known
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| (v == KnownCell::Unknown).then_some(i))
        .collect()
}
fn remaining_mines(view: &Observation, known: &[KnownCell]) -> Result<usize, KnowledgeError> {
    usize::from(view.spec().mines)
        .checked_sub(known.iter().filter(|&&v| v == KnownCell::Mine).count())
        .ok_or(KnowledgeError::Inconsistent)
}
fn inference(
    view: &Observation,
    known: Vec<KnownCell>,
    mut lies: Vec<CellId>,
    mut truth: Vec<CellId>,
    nodes: usize,
) -> Inference {
    let mut safe = Vec::new();
    let mut mines = Vec::new();
    for (i, &value) in known.iter().enumerate() {
        if view.cells()[i] != ObservedCell::Unknown {
            continue;
        }
        match value {
            KnownCell::Safe => safe.push(CellId(i as u16)),
            KnownCell::Mine => mines.push(CellId(i as u16)),
            KnownCell::Unknown => {}
        }
    }
    lies.sort_unstable();
    lies.dedup();
    truth.sort_unstable();
    truth.dedup();
    Inference {
        spec: view.spec(),
        known,
        safe,
        mines,
        lies,
        truth,
        nodes,
    }
}
#[derive(Clone)]
struct Clue {
    cell: CellId,
    cells: Vec<usize>,
    displayed: usize,
    known_mines: usize,
    trusted: bool,
}
impl Clue {
    fn range(&self, minimum: usize, maximum: usize) -> Option<(usize, usize, bool)> {
        if self.trusted {
            return (minimum <= self.displayed && self.displayed <= maximum).then_some((
                self.displayed,
                self.displayed,
                false,
            ));
        }
        let mut lower = usize::MAX;
        let mut upper = 0;
        for value in [
            self.displayed.saturating_sub(1),
            self.displayed,
            self.displayed + 1,
        ] {
            if (1..=8).contains(&value) && minimum <= value && value <= maximum {
                lower = lower.min(value);
                upper = upper.max(value);
            }
        }
        (lower != usize::MAX).then_some((
            lower,
            upper,
            self.displayed < minimum || self.displayed > maximum,
        ))
    }
}
fn constraints(view: &Observation, known: &[KnownCell], trusted: &[bool]) -> Vec<Clue> {
    view.cells()
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| {
            let ObservedCell::Number(displayed) = cell else {
                return None;
            };
            let adjacent = view.spec().neighbors(CellId(i as u16));
            let known_mines = adjacent
                .iter()
                .filter(|c| known[usize::from(c.0)] == KnownCell::Mine)
                .count();
            let cells = adjacent
                .into_iter()
                .map(|c| usize::from(c.0))
                .filter(|&c| known[c] == KnownCell::Unknown)
                .collect();
            Some(Clue {
                cell: CellId(i as u16),
                cells,
                displayed: usize::from(*displayed),
                known_mines,
                trusted: *displayed == 0 || trusted[i],
            })
        })
        .collect()
}

struct Possibility {
    mine: Vec<bool>,
    safe: Vec<bool>,
    lie: Vec<bool>,
    truth: Vec<bool>,
}
struct Component {
    cells: Vec<usize>,
    clues: Vec<CellId>,
    counts: Vec<Vec<Option<Possibility>>>,
}
struct ModelSearch<'a> {
    clues: Vec<Clue>,
    affects: Vec<Vec<usize>>,
    sums: Vec<usize>,
    remaining: Vec<usize>,
    assignment: Vec<bool>,
    counts: Vec<Vec<Option<Possibility>>>,
    nodes: &'a mut usize,
    limit: usize,
    lie_limit: usize,
    mine_min: usize,
    mine_max: usize,
}
impl ModelSearch<'_> {
    fn visit(&mut self, position: usize, mines: usize) -> Result<(), KnowledgeError> {
        if *self.nodes >= self.limit {
            return Err(KnowledgeError::BudgetExceeded);
        }
        *self.nodes += 1;
        if mines > self.mine_max || mines + self.assignment.len() - position < self.mine_min {
            return Ok(());
        }
        let mut lie_count = 0;
        for (i, clue) in self.clues.iter().enumerate() {
            let minimum = clue.known_mines + self.sums[i];
            let Some(range) = clue.range(minimum, minimum + self.remaining[i]) else {
                return Ok(());
            };
            lie_count += usize::from(range.2);
        }
        if lie_count > self.lie_limit {
            return Ok(());
        }
        if position == self.assignment.len() {
            let size = self.assignment.len();
            let clue_count = self.clues.len();
            let possible = self.counts[mines][lie_count].get_or_insert_with(|| Possibility {
                mine: vec![false; size],
                safe: vec![false; size],
                lie: vec![false; clue_count],
                truth: vec![false; clue_count],
            });
            for (i, &value) in self.assignment.iter().enumerate() {
                if value {
                    possible.mine[i] = true;
                } else {
                    possible.safe[i] = true;
                }
            }
            for (i, clue) in self.clues.iter().enumerate() {
                if clue.known_mines + self.sums[i] == clue.displayed {
                    possible.truth[i] = true;
                } else {
                    possible.lie[i] = true;
                }
            }
            return Ok(());
        }
        for value in [false, true] {
            self.assignment[position] = value;
            for &index in &self.affects[position] {
                self.sums[index] += usize::from(value);
                self.remaining[index] -= 1;
            }
            self.visit(position + 1, mines + usize::from(value))?;
            for &index in &self.affects[position] {
                self.sums[index] -= usize::from(value);
                self.remaining[index] += 1;
            }
        }
        Ok(())
    }
}
fn model_inference(
    view: &Observation,
    mut known: Vec<KnownCell>,
    constraints: Vec<Clue>,
    mut lies: Vec<CellId>,
    mut truth: Vec<CellId>,
    budget: SolverBudget,
) -> Result<Inference, KnowledgeError> {
    let fixed_lies = constraints
        .iter()
        .filter(|c| c.cells.is_empty() && c.known_mines != c.displayed)
        .count();
    let lie_limit = 2usize
        .checked_sub(fixed_lies)
        .ok_or(KnowledgeError::Inconsistent)?;
    let clues: Vec<_> = constraints
        .into_iter()
        .filter(|c| !c.cells.is_empty())
        .collect();
    let mut frontier = vec![false; known.len()];
    for clue in &clues {
        for &cell in &clue.cells {
            frontier[cell] = true;
        }
    }
    let free: Vec<_> = unknowns(&known)
        .into_iter()
        .filter(|&c| !frontier[c])
        .collect();
    let mut seen = vec![false; known.len()];
    let mut components = Vec::new();
    let mut nodes = 0;
    let global_remaining = remaining_mines(view, &known)?;
    let total_unknown = unknowns(&known).len();
    for start in 0..known.len() {
        if !frontier[start] || seen[start] {
            continue;
        }
        let mut cells = vec![start];
        seen[start] = true;
        let mut cursor = 0;
        while cursor < cells.len() {
            for clue in &clues {
                if clue.cells.contains(&cells[cursor]) {
                    for &cell in &clue.cells {
                        if !seen[cell] {
                            seen[cell] = true;
                            cells.push(cell);
                        }
                    }
                }
            }
            cursor += 1;
        }
        cells.sort_unstable();
        if cells.len() > budget.max_component_cells {
            return Err(KnowledgeError::BudgetExceeded);
        }
        let mut indices = vec![None; known.len()];
        for (i, &cell) in cells.iter().enumerate() {
            indices[cell] = Some(i);
        }
        let local: Vec<_> = clues
            .iter()
            .filter(|c| indices[c.cells[0]].is_some())
            .map(|clue| Clue {
                cells: clue
                    .cells
                    .iter()
                    .map(|&c| indices[c].expect("clue is entirely within its connected component"))
                    .collect(),
                ..clue.clone()
            })
            .collect();
        let mut affects = vec![Vec::new(); cells.len()];
        for (i, clue) in local.iter().enumerate() {
            for &cell in &clue.cells {
                affects[cell].push(i);
            }
        }
        let clue_cells = local.iter().map(|c| c.cell).collect();
        let mut search = ModelSearch {
            sums: vec![0; local.len()],
            remaining: local.iter().map(|c| c.cells.len()).collect(),
            clues: local,
            affects,
            assignment: vec![false; cells.len()],
            counts: (0..=cells.len())
                .map(|_| (0..=lie_limit).map(|_| None).collect())
                .collect(),
            nodes: &mut nodes,
            limit: budget.max_nodes,
            lie_limit,
            mine_min: global_remaining.saturating_sub(total_unknown - cells.len()),
            mine_max: global_remaining.min(cells.len()),
        };
        search.visit(0, 0)?;
        if search
            .counts
            .iter()
            .all(|row| row.iter().all(Option::is_none))
        {
            return Err(KnowledgeError::Inconsistent);
        }
        components.push(Component {
            cells,
            clues: clue_cells,
            counts: search.counts,
        });
    }
    let remaining = remaining_mines(view, &known)?;
    for (index, component) in components.iter().enumerate() {
        let other = reachable(&components, Some(index), free.len(), remaining, lie_limit);
        let mut can_mine = vec![false; component.cells.len()];
        let mut can_safe = can_mine.clone();
        let mut can_lie = vec![false; component.clues.len()];
        let mut can_truth = can_lie.clone();
        let mut exists = false;
        for (count, row) in component.counts.iter().enumerate() {
            if count > remaining {
                continue;
            }
            for (lie_count, possible) in row.iter().enumerate() {
                let Some(possible) = possible else {
                    continue;
                };
                if !other[remaining - count][..=lie_limit - lie_count]
                    .iter()
                    .any(|&v| v)
                {
                    continue;
                }
                exists = true;
                for i in 0..can_mine.len() {
                    can_mine[i] |= possible.mine[i];
                    can_safe[i] |= possible.safe[i];
                }
                for i in 0..can_lie.len() {
                    can_lie[i] |= possible.lie[i];
                    can_truth[i] |= possible.truth[i];
                }
            }
        }
        if !exists {
            return Err(KnowledgeError::Inconsistent);
        }
        for (i, &cell) in component.cells.iter().enumerate() {
            if !can_mine[i] {
                mark(&mut known, cell, KnownCell::Safe)?;
            } else if !can_safe[i] {
                mark(&mut known, cell, KnownCell::Mine)?;
            }
        }
        for (i, &cell) in component.clues.iter().enumerate() {
            if !can_truth[i] {
                lies.push(cell);
            } else if !can_lie[i] {
                truth.push(cell);
            }
        }
    }
    let group = reachable(&components, None, 0, remaining, lie_limit);
    let possible_free: Vec<_> = (0..=free.len())
        .filter(|&n| n <= remaining && group[remaining - n].iter().any(|&v| v))
        .collect();
    if possible_free.is_empty() {
        return Err(KnowledgeError::Inconsistent);
    }
    if possible_free.iter().all(|&v| v == 0) {
        for cell in free {
            mark(&mut known, cell, KnownCell::Safe)?;
        }
    } else if possible_free.iter().all(|&v| v == free.len()) {
        for cell in free {
            mark(&mut known, cell, KnownCell::Mine)?;
        }
    }
    Ok(inference(view, known, lies, truth, nodes))
}
fn reachable(
    components: &[Component],
    skip: Option<usize>,
    free: usize,
    mines: usize,
    lies: usize,
) -> Vec<Vec<bool>> {
    let mut possible = vec![vec![false; lies + 1]; mines + 1];
    for row in possible.iter_mut().take(free.min(mines) + 1) {
        row[0] = true;
    }
    for (index, component) in components.iter().enumerate() {
        if Some(index) == skip {
            continue;
        }
        let mut next = vec![vec![false; lies + 1]; mines + 1];
        for (previous_mines, row) in possible.iter().enumerate() {
            for (previous_lies, &exists) in row.iter().enumerate() {
                if !exists {
                    continue;
                }
                for (count, row) in component.counts.iter().enumerate() {
                    if previous_mines + count > mines {
                        continue;
                    }
                    for (cost, models) in row.iter().enumerate() {
                        if models.is_some() && previous_lies + cost <= lies {
                            next[previous_mines + count][previous_lies + cost] = true;
                        }
                    }
                }
            }
        }
        possible = next;
    }
    possible
}
