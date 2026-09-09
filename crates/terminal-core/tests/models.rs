use terminal_core::{
    Cell, CellAttributes, CellColor, CursorError, DimensionsError, MAX_COLUMNS, MAX_GRID_CELLS,
    MAX_ROWS, ScreenGrid, TerminalDimensions,
};

#[test]
fn constructs_valid_dimensions_and_default_cell() {
    let dimensions = TerminalDimensions::new(80, 24).expect("80x24 is valid");

    assert_eq!(dimensions.columns(), 80);
    assert_eq!(dimensions.rows(), 24);
    assert_eq!(dimensions.cell_count(), 1_920);

    let cell = Cell::default();
    assert_eq!(cell.character(), ' ');
    assert_eq!(cell.attributes(), &CellAttributes::default());
    assert_eq!(cell.attributes().foreground(), CellColor::Default);
    assert_eq!(cell.attributes().background(), CellColor::Default);
}

#[test]
fn rejects_zero_columns() {
    assert_eq!(
        TerminalDimensions::new(0, 24),
        Err(terminal_core::DimensionsError::ZeroColumns)
    );
}

#[test]
fn rejects_zero_rows() {
    assert_eq!(
        TerminalDimensions::new(80, 0),
        Err(terminal_core::DimensionsError::ZeroRows)
    );
}

#[test]
fn rejects_columns_above_resource_limit() {
    assert_eq!(
        TerminalDimensions::new(MAX_COLUMNS + 1, 1),
        Err(DimensionsError::TooManyColumns {
            requested: MAX_COLUMNS + 1,
            maximum: MAX_COLUMNS,
        })
    );
}

#[test]
fn rejects_rows_above_resource_limit() {
    assert_eq!(
        TerminalDimensions::new(1, MAX_ROWS + 1),
        Err(DimensionsError::TooManyRows {
            requested: MAX_ROWS + 1,
            maximum: MAX_ROWS,
        })
    );
}

#[test]
fn rejects_total_cell_count_above_resource_limit() {
    assert_eq!(
        TerminalDimensions::new(MAX_COLUMNS, MAX_ROWS),
        Err(DimensionsError::TooManyCells {
            columns: MAX_COLUMNS,
            rows: MAX_ROWS,
            maximum: MAX_GRID_CELLS,
        })
    );
}

#[test]
fn accepts_dimensions_at_each_resource_boundary() {
    assert!(TerminalDimensions::new(MAX_COLUMNS, 1).is_ok());
    assert!(TerminalDimensions::new(1, MAX_ROWS).is_ok());

    let square_side = 1_024;
    assert_eq!(square_side * square_side, MAX_GRID_CELLS);
    assert!(TerminalDimensions::new(square_side, square_side).is_ok());
}

#[test]
fn constructs_cell_with_explicit_attributes() {
    let attributes = CellAttributes::new(
        CellColor::Indexed(4),
        CellColor::Rgb {
            red: 10,
            green: 20,
            blue: 30,
        },
    );
    let cell = Cell::new('x', attributes);

    assert_eq!(cell.character(), 'x');
    assert_eq!(cell.attributes(), &attributes);
}

#[test]
fn indexes_cells_in_zero_based_row_major_order() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let grid = ScreenGrid::new(dimensions);

    assert_eq!(grid.dimensions(), dimensions);
    assert_eq!(grid.index_of(0, 0), Some(0));
    assert_eq!(grid.index_of(0, 2), Some(2));
    assert_eq!(grid.index_of(1, 0), Some(3));
    assert_eq!(grid.index_of(1, 2), Some(5));
}

#[test]
fn accesses_and_mutates_cells_by_row_and_column() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);
    let marked = Cell::new('x', CellAttributes::default());

    *grid.cell_mut(1, 0).unwrap() = marked;

    assert_eq!(grid.cell(1, 0), Some(&marked));
    assert_eq!(grid.cell(0, 1), Some(&Cell::default()));
}

#[test]
fn returns_none_for_out_of_bounds_cells() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);

    assert_eq!(grid.index_of(2, 0), None);
    assert_eq!(grid.index_of(0, 3), None);
    assert_eq!(grid.cell(2, 0), None);
    assert_eq!(grid.cell(0, 3), None);
    assert_eq!(grid.cell_mut(2, 0), None);
    assert_eq!(grid.cell_mut(0, 3), None);
}

#[test]
fn cursor_starts_at_origin_and_accepts_in_bounds_positions() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);

    assert_eq!((grid.cursor().row(), grid.cursor().column()), (0, 0));
    assert_eq!(grid.set_cursor_position(1, 2), Ok(()));
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 2));
}

#[test]
fn cursor_rejects_out_of_bounds_positions_without_moving() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);
    grid.set_cursor_position(1, 1).unwrap();

    assert_eq!(
        grid.set_cursor_position(2, 1),
        Err(CursorError::RowOutOfBounds { row: 2, rows: 2 })
    );
    assert_eq!(
        grid.set_cursor_position(1, 3),
        Err(CursorError::ColumnOutOfBounds {
            column: 3,
            columns: 3,
        })
    );
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 1));
}

#[test]
fn relative_cursor_movement_clamps_to_screen_bounds() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);

    grid.move_cursor(20, 20);
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 2));

    grid.move_cursor(-20, -20);
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (0, 0));
}

#[test]
fn extreme_relative_cursor_movement_remains_bounded() {
    let dimensions = TerminalDimensions::new(3, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);

    grid.move_cursor(isize::MAX, isize::MAX);
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 2));

    grid.move_cursor(isize::MIN, isize::MIN);
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (0, 0));
}

#[test]
fn clearing_blanks_every_cell_without_moving_the_cursor() {
    let dimensions = TerminalDimensions::new(2, 2).unwrap();
    let mut grid = ScreenGrid::new(dimensions);
    let marked = Cell::new(
        'x',
        CellAttributes::new(CellColor::Indexed(1), CellColor::Indexed(2)),
    );
    *grid.cell_mut(0, 0).unwrap() = marked;
    *grid.cell_mut(1, 1).unwrap() = marked;
    grid.set_cursor_position(1, 1).unwrap();

    grid.clear();

    for row in 0..dimensions.rows() {
        for column in 0..dimensions.columns() {
            assert_eq!(grid.cell(row, column), Some(&Cell::default()));
        }
    }
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 1));
}

#[test]
fn growing_resize_preserves_top_left_cells_and_blanks_new_cells() {
    let mut grid = ScreenGrid::new(TerminalDimensions::new(2, 2).unwrap());
    let first = Cell::new('a', CellAttributes::default());
    let second = Cell::new(
        'b',
        CellAttributes::new(CellColor::Indexed(3), CellColor::Default),
    );
    *grid.cell_mut(0, 0).unwrap() = first;
    *grid.cell_mut(1, 1).unwrap() = second;

    let grown = TerminalDimensions::new(4, 3).unwrap();
    grid.resize(grown);

    assert_eq!(grid.dimensions(), grown);
    assert_eq!(grid.cell(0, 0), Some(&first));
    assert_eq!(grid.cell(1, 1), Some(&second));
    assert_eq!(grid.cell(0, 2), Some(&Cell::default()));
    assert_eq!(grid.cell(2, 3), Some(&Cell::default()));
}

#[test]
fn shrinking_resize_keeps_top_left_intersection_and_clamps_cursor() {
    let mut grid = ScreenGrid::new(TerminalDimensions::new(3, 3).unwrap());
    let kept = Cell::new('k', CellAttributes::default());
    let dropped = Cell::new('d', CellAttributes::default());
    *grid.cell_mut(1, 1).unwrap() = kept;
    *grid.cell_mut(2, 2).unwrap() = dropped;
    grid.set_cursor_position(2, 2).unwrap();

    let shrunk = TerminalDimensions::new(2, 2).unwrap();
    grid.resize(shrunk);

    assert_eq!(grid.dimensions(), shrunk);
    assert_eq!(grid.cell(1, 1), Some(&kept));
    assert_eq!(grid.cell(2, 2), None);
    assert_eq!((grid.cursor().row(), grid.cursor().column()), (1, 1));
}

#[test]
fn mixed_axis_resize_preserves_only_the_top_left_intersection() {
    let mut grid = ScreenGrid::new(TerminalDimensions::new(3, 2).unwrap());
    let kept = Cell::new('k', CellAttributes::default());
    let dropped = Cell::new('d', CellAttributes::default());
    *grid.cell_mut(1, 1).unwrap() = kept;
    *grid.cell_mut(0, 2).unwrap() = dropped;

    let taller_and_narrower = TerminalDimensions::new(2, 4).unwrap();
    grid.resize(taller_and_narrower);

    assert_eq!(grid.cell(1, 1), Some(&kept));
    assert_eq!(grid.cell(0, 2), None);
    assert_eq!(grid.cell(3, 1), Some(&Cell::default()));
}
