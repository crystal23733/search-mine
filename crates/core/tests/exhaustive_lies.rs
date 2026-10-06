//! Independent visible-world oracle, including all zero/one/two-lie observations.
use liar_core::board::{BoardSpec, CellId, Observation, ObservedCell};
use liar_core::knowledge::{KnowledgeMemory, KnowledgeSolver};
use liar_core::lie::LieValidator;
use liar_core::solver::SolverBudget;
use proptest::prelude::*;

fn actual(layout: u16, cell: usize) -> ObservedCell {
    actual_grid(layout, cell, 6)
}
fn actual_grid(layout: u16, cell: usize, area: usize) -> ObservedCell {
    if layout & (1 << cell) != 0 {
        return ObservedCell::Mine;
    }
    let (x, y) = ((cell % 3) as i32, (cell / 3) as i32);
    let mines = (0..area)
        .filter(|&other| {
            let (ox, oy) = ((other % 3) as i32, (other / 3) as i32);
            other != cell
                && (ox - x).abs() <= 1
                && (oy - y).abs() <= 1
                && layout & (1 << other) != 0
        })
        .count();
    ObservedCell::Number(mines as u8)
}
fn possible(layout: u16, observed: &[ObservedCell]) -> bool {
    let mut lies = 0;
    for (i, &cell) in observed.iter().enumerate() {
        match (cell, actual_grid(layout, i, observed.len())) {
            (ObservedCell::Unknown, _) => {}
            (ObservedCell::Mine, ObservedCell::Mine) => {}
            (ObservedCell::Number(displayed), ObservedCell::Number(truth))
                if displayed == truth => {}
            (ObservedCell::Number(displayed), ObservedCell::Number(truth))
                if displayed > 0 && truth > 0 && displayed.abs_diff(truth) == 1 =>
            {
                lies += 1
            }
            _ => return false,
        }
    }
    lies <= 2
}
fn variants(
    base: &[ObservedCell],
    start: usize,
    active: usize,
    out: &mut Vec<(Vec<ObservedCell>, usize)>,
) {
    out.push((base.to_vec(), active));
    if active == 2 {
        return;
    }
    for i in start..base.len() {
        let ObservedCell::Number(truth @ 1..=8) = base[i] else {
            continue;
        };
        for delta in [-1i16, 1] {
            let displayed = i16::from(truth) + delta;
            if !(1..=8).contains(&displayed) {
                continue;
            }
            let mut changed = base.to_vec();
            changed[i] = ObservedCell::Number(displayed as u8);
            variants(&changed, i + 1, active + 1, out);
        }
    }
}

#[test]
fn every_small_world_lie_combination_and_every_accepted_future_branch_is_safe() {
    let spec = BoardSpec {
        width: 3,
        height: 2,
        mines: 2,
        opening: CellId(0),
    };
    let worlds: Vec<_> = (0u16..64).filter(|mask| mask.count_ones() == 2).collect();
    let mut observations = 0u64;
    let mut certificates = 0u64;
    let mut future_worlds = 0u64;
    for &layout in &worlds {
        for reveal in 0u16..64 {
            let base: Vec<_> = (0..6)
                .map(|i| {
                    if reveal & (1 << i) != 0 {
                        actual(layout, i)
                    } else {
                        ObservedCell::Unknown
                    }
                })
                .collect();
            let mut displays = Vec::new();
            variants(&base, 0, 0, &mut displays);
            for (cells, active) in displays {
                let compatible: Vec<_> = worlds
                    .iter()
                    .copied()
                    .filter(|&world| possible(world, &cells))
                    .collect();
                assert!(!compatible.is_empty());
                let view = Observation::new(spec, cells).unwrap();
                let memory = KnowledgeMemory::new(spec).unwrap();
                let proof =
                    KnowledgeSolver::deduce(&view, &memory, SolverBudget::default()).unwrap();
                for &cell in proof.safe() {
                    assert!(
                        compatible
                            .iter()
                            .all(|&w| actual(w, usize::from(cell.0)) != ObservedCell::Mine)
                    );
                }
                for &cell in proof.mines() {
                    assert!(
                        compatible
                            .iter()
                            .all(|&w| actual(w, usize::from(cell.0)) == ObservedCell::Mine)
                    );
                }
                for &cell in proof.lies() {
                    assert!(
                        compatible
                            .iter()
                            .all(|&w| actual(w, usize::from(cell.0)) != view.cell(cell).unwrap())
                    );
                }
                for &cell in proof.truth() {
                    assert!(
                        compatible
                            .iter()
                            .all(|&w| actual(w, usize::from(cell.0)) == view.cell(cell).unwrap())
                    );
                }
                let prepared =
                    LieValidator::prepare(&view, &memory, SolverBudget::default()).unwrap();
                for target in 0..6 {
                    for delta in [-1, 1] {
                        let Ok(certificate) = prepared.certificate(CellId(target), delta, active)
                        else {
                            continue;
                        };
                        assert!(compatible.iter().all(|&w| actual(w, usize::from(target))
                            == ObservedCell::Number(certificate.expected_truth())));
                        let mut after = view.clone();
                        after
                            .set(
                                CellId(target),
                                ObservedCell::Number(certificate.displayed()),
                            )
                            .unwrap();
                        let future: Vec<_> = worlds
                            .iter()
                            .copied()
                            .filter(|&w| possible(w, after.cells()))
                            .collect();
                        assert!(!future.is_empty());
                        for &world in &future {
                            assert_ne!(
                                actual(world, usize::from(target)),
                                after.cell(CellId(target)).unwrap()
                            );
                            assert_ne!(actual(world, usize::from(target)), ObservedCell::Mine);
                        }
                        let mut next_memory = memory.clone();
                        prepared.remember(&mut next_memory).unwrap();
                        let after_proof =
                            KnowledgeSolver::deduce(&after, &next_memory, SolverBudget::default())
                                .unwrap();
                        assert!(
                            after_proof.lies().contains(&CellId(target)),
                            "accepted certificate lacks a definite accusation path"
                        );
                        certificates += 1;
                        future_worlds += future.len() as u64;
                    }
                }
                observations += 1;
            }
        }
    }
    assert!(observations > 1000 && certificates > 0 && future_worlds > 0);
    println!(
        "E4 observations={observations}, certificates={certificates}, future_worlds={future_worlds}, counterexamples=0"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn larger_partial_observations_and_two_lies_never_change_a_certain_fact(
        world_index in 0usize..36, reveal in 0u16..512, first in 0usize..9, second in 0usize..9,
        first_up in any::<bool>(), second_up in any::<bool>()
    ) {
        let worlds: Vec<_> = (0u16..512).filter(|m| m.count_ones() == 2).collect();
        let actual_world = worlds[world_index];
        let spec = BoardSpec { width: 3, height: 3, mines: 2, opening: CellId(0) };
        let mut cells: Vec<_> = (0..9).map(|i| if reveal & (1 << i) != 0 { actual_grid(actual_world, i, 9) } else { ObservedCell::Unknown }).collect();
        let mut changed = [false; 9];
        for (index, up) in [(first, first_up), (second, second_up)] {
            if changed[index] { continue; }
            let ObservedCell::Number(truth @ 1..=8) = cells[index] else { continue; };
            let displayed = i16::from(truth) + if up { 1 } else { -1 };
            if !(1..=8).contains(&displayed) { continue; }
            cells[index] = ObservedCell::Number(displayed as u8); changed[index] = true;
        }
        let models: Vec<_> = worlds.into_iter().filter(|&w| possible(w, &cells)).collect();
        prop_assert!(!models.is_empty());
        let view = Observation::new(spec, cells).unwrap();
        let proof = KnowledgeSolver::deduce(&view, &KnowledgeMemory::new(spec).unwrap(), KnowledgeSolver::default_budget()).unwrap();
        for &cell in proof.safe() { prop_assert!(models.iter().all(|&w| actual_grid(w, usize::from(cell.0), 9) != ObservedCell::Mine)); }
        for &cell in proof.mines() { prop_assert!(models.iter().all(|&w| actual_grid(w, usize::from(cell.0), 9) == ObservedCell::Mine)); }
        for &cell in proof.lies() { prop_assert!(models.iter().all(|&w| actual_grid(w, usize::from(cell.0), 9) != view.cell(cell).unwrap())); }
        for &cell in proof.truth() { prop_assert!(models.iter().all(|&w| actual_grid(w, usize::from(cell.0), 9) == view.cell(cell).unwrap())); }
    }
}
