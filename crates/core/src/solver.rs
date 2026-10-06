//! Truth-only observations for board certification. Never use this solver for live lies.
use crate::board::{CellId, Observation, ObservedCell};
use std::collections::BTreeMap;

pub const SOLVER_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug)]
pub struct SolverBudget {
    pub max_nodes: usize,
    pub max_component_cells: usize,
    pub max_constraints: usize,
}
impl Default for SolverBudget {
    fn default() -> Self {
        Self {
            max_nodes: 200_000,
            max_component_cells: 22,
            max_constraints: 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverError {
    Inconsistent,
    BudgetExceeded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeductionKind {
    Direct,
    Subset,
    Models,
    Stalled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deduction {
    pub safe: Vec<CellId>,
    pub mines: Vec<CellId>,
    pub kind: DeductionKind,
    pub nodes: usize,
}
pub struct NoGuessSolver;
impl NoGuessSolver {
    pub fn deduce(view: &Observation, budget: SolverBudget) -> Result<Deduction, SolverError> {
        let mut known: Vec<_> = view
            .cells()
            .iter()
            .map(|cell| match cell {
                ObservedCell::Unknown => Known::Unknown,
                ObservedCell::Mine => Known::Mine,
                ObservedCell::Number(_) => Known::Safe,
            })
            .collect();
        let mut constraints = local_constraints(view, &known)?;
        let observed_mines = known.iter().filter(|&&v| v == Known::Mine).count();
        let remaining = usize::from(view.spec().mines)
            .checked_sub(observed_mines)
            .ok_or(SolverError::Inconsistent)?;
        constraints.push(Constraint {
            cells: unknown_cells(&known),
            mines: remaining,
        });
        let mut kind = DeductionKind::Direct;
        loop {
            constraints = normalize(constraints, &known, budget.max_constraints)?;
            let mut changed = false;
            for constraint in &constraints {
                if constraint.mines == 0 || constraint.mines == constraint.cells.len() {
                    let value = if constraint.mines == 0 {
                        Known::Safe
                    } else {
                        Known::Mine
                    };
                    for &cell in &constraint.cells {
                        changed |= assign(&mut known, cell, value)?;
                    }
                }
            }
            if changed {
                continue;
            }
            let current = result(view, &known, kind, 0);
            if !current.safe.is_empty() {
                return Ok(current);
            }
            let mut additions = BTreeMap::new();
            let mut forced = false;
            'pairs: for smaller in &constraints {
                for larger in &constraints {
                    if let Some(cells) = subset_difference(&smaller.cells, &larger.cells) {
                        let mines = larger
                            .mines
                            .checked_sub(smaller.mines)
                            .ok_or(SolverError::Inconsistent)?;
                        if mines > cells.len() {
                            return Err(SolverError::Inconsistent);
                        }
                        if mines == 0 || mines == cells.len() {
                            let value = if mines == 0 { Known::Safe } else { Known::Mine };
                            for cell in cells {
                                assign(&mut known, cell, value)?;
                            }
                            forced = true;
                            break 'pairs;
                        }
                        let derived = Constraint { cells, mines };
                        if !constraints.contains(&derived) {
                            if let Some(previous) = additions.insert(derived.cells, derived.mines)
                                && previous != mines
                            {
                                return Err(SolverError::Inconsistent);
                            }
                            if constraints.len() + additions.len() > budget.max_constraints {
                                return Err(SolverError::BudgetExceeded);
                            }
                        }
                    }
                }
            }
            if forced {
                kind = DeductionKind::Subset;
                continue;
            }
            if additions.is_empty() {
                break;
            }
            constraints.extend(
                additions
                    .into_iter()
                    .map(|(cells, mines)| Constraint { cells, mines }),
            );
        }
        let nodes = enumerate_components(view, &mut known, budget)?;
        Ok(result(view, &known, DeductionKind::Models, nodes))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Known {
    Unknown,
    Safe,
    Mine,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Constraint {
    cells: Vec<usize>,
    mines: usize,
}

fn unknown_cells(known: &[Known]) -> Vec<usize> {
    known
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| (v == Known::Unknown).then_some(i))
        .collect()
}
fn assign(known: &mut [Known], cell: usize, value: Known) -> Result<bool, SolverError> {
    match known[cell] {
        Known::Unknown => {
            known[cell] = value;
            Ok(true)
        }
        previous if previous == value => Ok(false),
        _ => Err(SolverError::Inconsistent),
    }
}
fn result(view: &Observation, known: &[Known], kind: DeductionKind, nodes: usize) -> Deduction {
    let mut safe = Vec::new();
    let mut mines = Vec::new();
    for (i, &value) in known.iter().enumerate() {
        if view.cells()[i] == ObservedCell::Unknown {
            match value {
                Known::Safe => safe.push(CellId(i as u16)),
                Known::Mine => mines.push(CellId(i as u16)),
                Known::Unknown => {}
            }
        }
    }
    let kind = if safe.is_empty() && mines.is_empty() {
        DeductionKind::Stalled
    } else {
        kind
    };
    Deduction {
        safe,
        mines,
        kind,
        nodes,
    }
}
fn local_constraints(view: &Observation, known: &[Known]) -> Result<Vec<Constraint>, SolverError> {
    let mut constraints = Vec::new();
    for (index, cell) in view.cells().iter().enumerate() {
        if let ObservedCell::Number(value) = cell {
            let neighbors = view.spec().neighbors(CellId(index as u16));
            let mines = neighbors
                .iter()
                .filter(|c| known[usize::from(c.0)] == Known::Mine)
                .count();
            let remaining = usize::from(*value)
                .checked_sub(mines)
                .ok_or(SolverError::Inconsistent)?;
            let cells = neighbors
                .into_iter()
                .map(|c| usize::from(c.0))
                .filter(|&i| known[i] == Known::Unknown)
                .collect();
            constraints.push(Constraint {
                cells,
                mines: remaining,
            });
        }
    }
    normalize(constraints, known, usize::MAX)
}
fn normalize(
    constraints: Vec<Constraint>,
    known: &[Known],
    maximum: usize,
) -> Result<Vec<Constraint>, SolverError> {
    let mut unique = BTreeMap::new();
    for constraint in constraints {
        let known_mines = constraint
            .cells
            .iter()
            .filter(|&&i| known[i] == Known::Mine)
            .count();
        let mines = constraint
            .mines
            .checked_sub(known_mines)
            .ok_or(SolverError::Inconsistent)?;
        let cells: Vec<_> = constraint
            .cells
            .into_iter()
            .filter(|&i| known[i] == Known::Unknown)
            .collect();
        if mines > cells.len() {
            return Err(SolverError::Inconsistent);
        }
        if cells.is_empty() {
            continue;
        }
        if let Some(previous) = unique.insert(cells, mines)
            && previous != mines
        {
            return Err(SolverError::Inconsistent);
        }
        if unique.len() > maximum {
            return Err(SolverError::BudgetExceeded);
        }
    }
    Ok(unique
        .into_iter()
        .map(|(cells, mines)| Constraint { cells, mines })
        .collect())
}
fn subset_difference(smaller: &[usize], larger: &[usize]) -> Option<Vec<usize>> {
    if smaller.len() >= larger.len() {
        return None;
    }
    let mut index = 0;
    let mut difference = Vec::with_capacity(larger.len() - smaller.len());
    for &cell in larger {
        if smaller.get(index) == Some(&cell) {
            index += 1;
        } else {
            difference.push(cell);
        }
    }
    (index == smaller.len()).then_some(difference)
}

struct Possibility {
    mine: Vec<bool>,
    safe: Vec<bool>,
}
struct Component {
    cells: Vec<usize>,
    counts: Vec<Option<Possibility>>,
}
struct Enumerator<'a> {
    constraints: Vec<Constraint>,
    affects: Vec<Vec<usize>>,
    sums: Vec<usize>,
    remaining: Vec<usize>,
    assignment: Vec<bool>,
    counts: Vec<Option<Possibility>>,
    nodes: &'a mut usize,
    limit: usize,
}
impl Enumerator<'_> {
    fn visit(&mut self, position: usize, mine_count: usize) -> Result<(), SolverError> {
        if *self.nodes >= self.limit {
            return Err(SolverError::BudgetExceeded);
        }
        *self.nodes += 1;
        for (i, constraint) in self.constraints.iter().enumerate() {
            if self.sums[i] > constraint.mines
                || self.sums[i] + self.remaining[i] < constraint.mines
            {
                return Ok(());
            }
        }
        if position == self.assignment.len() {
            let size = self.assignment.len();
            let possible = self.counts[mine_count].get_or_insert_with(|| Possibility {
                mine: vec![false; size],
                safe: vec![false; size],
            });
            for (i, &value) in self.assignment.iter().enumerate() {
                if value {
                    possible.mine[i] = true;
                } else {
                    possible.safe[i] = true;
                }
            }
            return Ok(());
        }
        for value in [false, true] {
            self.assignment[position] = value;
            for &constraint in &self.affects[position] {
                self.remaining[constraint] -= 1;
                self.sums[constraint] += usize::from(value);
            }
            self.visit(position + 1, mine_count + usize::from(value))?;
            for &constraint in &self.affects[position] {
                self.remaining[constraint] += 1;
                self.sums[constraint] -= usize::from(value);
            }
        }
        Ok(())
    }
}

fn enumerate_components(
    view: &Observation,
    known: &mut [Known],
    budget: SolverBudget,
) -> Result<usize, SolverError> {
    let constraints = local_constraints(view, known)?;
    let mut frontier = vec![false; known.len()];
    for constraint in &constraints {
        for &cell in &constraint.cells {
            frontier[cell] = true;
        }
    }
    let free: Vec<_> = unknown_cells(known)
        .into_iter()
        .filter(|&cell| !frontier[cell])
        .collect();
    let mut seen = vec![false; known.len()];
    let mut components = Vec::new();
    let mut nodes = 0;
    for start in 0..known.len() {
        if !frontier[start] || seen[start] {
            continue;
        }
        let mut cells = vec![start];
        seen[start] = true;
        let mut cursor = 0;
        while cursor < cells.len() {
            for constraint in &constraints {
                if constraint.cells.contains(&cells[cursor]) {
                    for &cell in &constraint.cells {
                        if !seen[cell] {
                            cells.push(cell);
                            seen[cell] = true;
                        }
                    }
                }
            }
            cursor += 1;
        }
        cells.sort_unstable();
        if cells.len() > budget.max_component_cells {
            return Err(SolverError::BudgetExceeded);
        }
        let mut indices = vec![None; known.len()];
        for (i, &cell) in cells.iter().enumerate() {
            indices[cell] = Some(i);
        }
        let local: Vec<_> = constraints
            .iter()
            .filter(|c| indices[c.cells[0]].is_some())
            .map(|c| Constraint {
                cells: c
                    .cells
                    .iter()
                    .map(|&cell| {
                        indices[cell]
                            .expect("connected component contains every constraint variable")
                    })
                    .collect(),
                mines: c.mines,
            })
            .collect();
        let mut affects = vec![Vec::new(); cells.len()];
        for (i, c) in local.iter().enumerate() {
            for &cell in &c.cells {
                affects[cell].push(i);
            }
        }
        let mut enumerator = Enumerator {
            sums: vec![0; local.len()],
            remaining: local.iter().map(|c| c.cells.len()).collect(),
            constraints: local,
            affects,
            assignment: vec![false; cells.len()],
            counts: (0..=cells.len()).map(|_| None).collect(),
            nodes: &mut nodes,
            limit: budget.max_nodes,
        };
        enumerator.visit(0, 0)?;
        if enumerator.counts.iter().all(Option::is_none) {
            return Err(SolverError::Inconsistent);
        }
        components.push(Component {
            cells,
            counts: enumerator.counts,
        });
    }
    let remaining = usize::from(view.spec().mines)
        .checked_sub(known.iter().filter(|&&v| v == Known::Mine).count())
        .ok_or(SolverError::Inconsistent)?;
    for (index, component) in components.iter().enumerate() {
        let other = reachable(&components, Some(index), free.len(), remaining);
        let mut can_mine = vec![false; component.cells.len()];
        let mut can_safe = vec![false; component.cells.len()];
        let mut valid = false;
        for (count, possible) in component.counts.iter().enumerate() {
            if let Some(possible) = possible {
                if count > remaining || !other[remaining - count] {
                    continue;
                }
                valid = true;
                for cell in 0..component.cells.len() {
                    can_mine[cell] |= possible.mine[cell];
                    can_safe[cell] |= possible.safe[cell];
                }
            }
        }
        if !valid {
            return Err(SolverError::Inconsistent);
        }
        for (index, &cell) in component.cells.iter().enumerate() {
            if !can_mine[index] {
                assign(known, cell, Known::Safe)?;
            } else if !can_safe[index] {
                assign(known, cell, Known::Mine)?;
            }
        }
    }
    let group_counts = reachable(&components, None, 0, remaining);
    let valid_free: Vec<_> = (0..=free.len())
        .filter(|&count| count <= remaining && group_counts[remaining - count])
        .collect();
    if valid_free.is_empty() {
        return Err(SolverError::Inconsistent);
    }
    if valid_free.iter().all(|&v| v == 0) {
        for cell in free {
            assign(known, cell, Known::Safe)?;
        }
    } else if valid_free.iter().all(|&v| v == free.len()) {
        for cell in free {
            assign(known, cell, Known::Mine)?;
        }
    }
    Ok(nodes)
}
fn reachable(
    components: &[Component],
    skip: Option<usize>,
    free: usize,
    limit: usize,
) -> Vec<bool> {
    let mut possible = vec![false; limit + 1];
    possible[..=free.min(limit)].fill(true);
    for (index, component) in components.iter().enumerate() {
        if Some(index) == skip {
            continue;
        }
        let mut next = vec![false; limit + 1];
        for (previous, &exists) in possible.iter().enumerate() {
            if !exists {
                continue;
            }
            for (count, models) in component.counts.iter().enumerate() {
                if models.is_some() && previous + count <= limit {
                    next[previous + count] = true;
                }
            }
        }
        possible = next;
    }
    possible
}
