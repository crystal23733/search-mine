use liar_core::board::{Board, BoardError, BoardSpec, Cell, CellId, Observation, ObservedCell};

#[test]
fn adjacent_numbers_do_not_wrap_rows_and_include_diagonals() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(2), CellId(4)]).unwrap();
    let expected = [
        Cell::Number(1),
        Cell::Number(2),
        Cell::Mine,
        Cell::Number(1),
        Cell::Mine,
        Cell::Number(2),
        Cell::Number(1),
        Cell::Number(1),
        Cell::Number(1),
    ];
    for (index, cell) in expected.into_iter().enumerate() {
        assert_eq!(board.cell(CellId(index as u16)), Some(cell));
    }
    assert_eq!(board.safe_total(), 7);
    assert_eq!(board.cell(CellId(9)), None);
}

#[test]
fn malformed_board_inputs_are_rejected() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 2,
        opening: CellId(0),
    };
    assert!(matches!(
        Board::from_mines(spec, &[CellId(1), CellId(1)]),
        Err(BoardError::DuplicateMine)
    ));
    assert!(matches!(
        Board::from_mines(spec, &[CellId(1), CellId(9)]),
        Err(BoardError::InvalidCell)
    ));
    assert!(matches!(
        Board::from_mines(spec, &[]),
        Err(BoardError::InvalidMineCount)
    ));
    assert_eq!(
        BoardSpec { width: 0, ..spec }.validate(),
        Err(BoardError::InvalidDimensions)
    );
    assert_eq!(
        BoardSpec {
            width: 17,
            height: 17,
            ..spec
        }
        .validate(),
        Err(BoardError::InvalidDimensions)
    );
    assert_eq!(
        BoardSpec { mines: 9, ..spec }.validate(),
        Err(BoardError::InvalidMineCount)
    );
    assert_eq!(
        BoardSpec {
            opening: CellId(9),
            ..spec
        }
        .validate(),
        Err(BoardError::InvalidCell)
    );
}

#[test]
fn zero_flood_opens_every_reachable_safe_cell_once_and_skips_flags() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 1,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(8)]).unwrap();
    let mut view = Observation::closed(spec).unwrap();
    let mut flags = vec![false; 9];
    flags[1] = true;
    let opened = board.reveal(&mut view, CellId(0), &flags).unwrap();
    assert_eq!(
        opened,
        vec![CellId(0), CellId(3), CellId(4), CellId(6), CellId(7)]
    );
    assert_eq!(view.cell(CellId(1)), Some(ObservedCell::Unknown));
    assert_eq!(view.cell(CellId(8)), Some(ObservedCell::Unknown));
    assert!(
        board
            .reveal(&mut view, CellId(0), &flags)
            .unwrap()
            .is_empty()
    );
    flags[1] = false;
    assert_eq!(
        board.reveal(&mut view, CellId(1), &flags).unwrap(),
        vec![CellId(1), CellId(2), CellId(5)]
    );
    assert_eq!(view.opened_safe(), 8);
    assert!(
        board
            .reveal(&mut view, CellId(8), &flags)
            .unwrap()
            .is_empty()
    );
    assert_eq!(view.cell(CellId(8)), Some(ObservedCell::Mine));
}

#[test]
fn public_observation_and_reveal_reject_malformed_input_without_mutation() {
    let spec = BoardSpec {
        width: 3,
        height: 3,
        mines: 1,
        opening: CellId(0),
    };
    let board = Board::from_mines(spec, &[CellId(8)]).unwrap();
    let mut view = Observation::closed(spec).unwrap();
    let original = view.clone();
    assert_eq!(
        board.reveal(&mut view, CellId(9), &[false; 9]),
        Err(BoardError::InvalidCell)
    );
    assert_eq!(
        board.reveal(&mut view, CellId(0), &[false; 8]),
        Err(BoardError::ObservationMismatch)
    );
    assert_eq!(view, original);
    assert_eq!(
        view.set(CellId(9), ObservedCell::Number(1)),
        Err(BoardError::InvalidCell)
    );
    assert_eq!(
        view.set(CellId(0), ObservedCell::Number(9)),
        Err(BoardError::ObservationMismatch)
    );
    assert_eq!(
        Observation::new(spec, vec![ObservedCell::Number(9); 9]),
        Err(BoardError::ObservationMismatch)
    );
    assert_eq!(
        Observation::new(spec, vec![]),
        Err(BoardError::ObservationMismatch)
    );
    assert!(
        BoardSpec { width: 0, ..spec }
            .neighbors(CellId(0))
            .is_empty()
    );
}
