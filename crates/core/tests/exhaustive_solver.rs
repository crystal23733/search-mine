//! Independent possible-world oracle; it never calls the board's adjacency implementation.
use liar_core::board::{BoardSpec, CellId, Observation, ObservedCell};
use liar_core::solver::{NoGuessSolver, SolverBudget};

fn value(layout: u16, cell: usize) -> ObservedCell {
    if layout & (1 << cell) != 0 {
        return ObservedCell::Mine;
    }
    let (x, y) = ((cell % 3) as i32, (cell / 3) as i32);
    let count = (0..9)
        .filter(|&other| {
            let (ox, oy) = ((other % 3) as i32, (other / 3) as i32);
            other != cell
                && (ox - x).abs() <= 1
                && (oy - y).abs() <= 1
                && layout & (1 << other) != 0
        })
        .count();
    ObservedCell::Number(count as u8)
}

#[test]
fn every_deduction_is_true_in_all_small_board_worlds_for_every_reveal_subset() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let worlds: Vec<_> = (0u16..512)
        .filter(|mask| mask.count_ones() == 2)
        .map(|layout| {
            (
                layout,
                (0..9).map(|cell| value(layout, cell)).collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut checked = 0;
    for (_, truth) in &worlds {
        for revealed in 0u16..512 {
            let cells: Vec<_> = truth
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    if revealed & (1 << i) != 0 {
                        value
                    } else {
                        ObservedCell::Unknown
                    }
                })
                .collect();
            let possible: Vec<_> = worlds
                .iter()
                .filter(|(_, candidate)| {
                    cells.iter().zip(candidate).all(|(&observed, &actual)| {
                        observed == ObservedCell::Unknown || observed == actual
                    })
                })
                .collect();
            assert!(!possible.is_empty());
            let view = Observation::new(spec, cells).unwrap();
            let proof = NoGuessSolver::deduce(&view, SolverBudget::default()).unwrap();
            for cell in proof.safe {
                assert_eq!(view.cell(cell), Some(ObservedCell::Unknown));
                assert!(
                    possible
                        .iter()
                        .all(|(layout, _)| layout & (1 << cell.0) == 0),
                    "unsafe inference in observable worlds"
                );
            }
            for cell in proof.mines {
                assert_eq!(view.cell(cell), Some(ObservedCell::Unknown));
                assert!(
                    possible
                        .iter()
                        .all(|(layout, _)| layout & (1 << cell.0) != 0),
                    "mine inference is not certain"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 18_432);
}
