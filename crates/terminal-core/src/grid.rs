use crate::{Cell, Cursor, CursorError, TerminalDimensions, VerticalScrollingMargins};

/// A bounded, row-major terminal screen grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenGrid {
    dimensions: TerminalDimensions,
    cells: Vec<Cell>,
    cursor: Cursor,
}

#[derive(Clone, Copy)]
enum ScrollDirection {
    Up,
    Down,
}

impl ScreenGrid {
    pub fn new(dimensions: TerminalDimensions) -> Self {
        Self {
            dimensions,
            cells: vec![Cell::default(); dimensions.cell_count()],
            cursor: Cursor::origin(),
        }
    }

    pub const fn dimensions(&self) -> TerminalDimensions {
        self.dimensions
    }

    /// Maps a zero-based `(row, column)` coordinate to its row-major index.
    pub fn index_of(&self, row: usize, column: usize) -> Option<usize> {
        if row >= self.dimensions.rows() || column >= self.dimensions.columns() {
            return None;
        }

        Some(row * self.dimensions.columns() + column)
    }

    /// Returns a cell for a zero-based `(row, column)` coordinate.
    pub fn cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.index_of(row, column)
            .and_then(|index| self.cells.get(index))
    }

    /// Returns a mutable cell for a zero-based `(row, column)` coordinate.
    pub fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        let index = self.index_of(row, column)?;
        self.cells.get_mut(index)
    }

    /// Returns the cursor, which is always inside the current dimensions.
    pub const fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Moves the cursor to an in-bounds zero-based coordinate.
    pub fn set_cursor_position(&mut self, row: usize, column: usize) -> Result<(), CursorError> {
        self.cursor.set_position(row, column, self.dimensions)
    }

    /// Moves the cursor by signed deltas, clamping at screen edges.
    pub fn move_cursor(&mut self, row_delta: isize, column_delta: isize) {
        self.cursor
            .move_by(row_delta, column_delta, self.dimensions);
    }

    /// Replaces every cell with the default blank cell without moving the cursor.
    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    pub(crate) fn insert_cell(&mut self, row: usize, column: usize, cell: Cell) {
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        let row_end = (row + 1) * self.dimensions.columns();

        if index + 1 < row_end {
            self.cells.copy_within(index..row_end - 1, index + 1);
        }
        self.cells[index] = cell;
    }

    pub(crate) fn erase_cells(&mut self, start: usize, end: usize) {
        self.cells[start..end].fill(Cell::default());
    }

    pub(crate) fn scroll_up(&mut self, rows: usize) {
        self.scroll_region_up(
            VerticalScrollingMargins::full_screen(self.dimensions.rows()),
            rows,
        );
    }

    pub(crate) fn scroll_down(&mut self, rows: usize) {
        self.scroll_region_down(
            VerticalScrollingMargins::full_screen(self.dimensions.rows()),
            rows,
        );
    }

    pub(crate) fn scroll_region_up(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.scroll_region(margins, rows, ScrollDirection::Up);
    }

    pub(crate) fn scroll_region_down(&mut self, margins: VerticalScrollingMargins, rows: usize) {
        self.scroll_region(margins, rows, ScrollDirection::Down);
    }

    fn scroll_region(
        &mut self,
        margins: VerticalScrollingMargins,
        rows: usize,
        direction: ScrollDirection,
    ) {
        if margins.bottom() >= self.dimensions.rows() {
            return;
        }

        let region_height = margins.bottom() - margins.top() + 1;
        let rows = rows.min(region_height);
        if rows == 0 {
            return;
        }

        let columns = self.dimensions.columns();
        let region_start = margins.top() * columns;
        let region_end = (margins.bottom() + 1) * columns;
        let shifted_cells = rows * columns;
        if rows == region_height {
            self.cells[region_start..region_end].fill(Cell::default());
            return;
        }

        match direction {
            ScrollDirection::Up => {
                self.cells
                    .copy_within(region_start + shifted_cells..region_end, region_start);
                self.cells[region_end - shifted_cells..region_end].fill(Cell::default());
            }
            ScrollDirection::Down => {
                self.cells.copy_within(
                    region_start..region_end - shifted_cells,
                    region_start + shifted_cells,
                );
                self.cells[region_start..region_start + shifted_cells].fill(Cell::default());
            }
        }
    }

    /// Resizes while preserving the top-left intersection of the old and new grids.
    ///
    /// Newly exposed cells are default blank cells. Cells below or to the right of
    /// the new dimensions are discarded. The cursor is clamped into the new grid.
    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        if dimensions == self.dimensions {
            return;
        }

        let old_columns = self.dimensions.columns();
        let preserved_rows = self.dimensions.rows().min(dimensions.rows());
        let preserved_columns = old_columns.min(dimensions.columns());
        let mut cells = vec![Cell::default(); dimensions.cell_count()];

        for row in 0..preserved_rows {
            let old_start = row * old_columns;
            let new_start = row * dimensions.columns();
            cells[new_start..new_start + preserved_columns]
                .clone_from_slice(&self.cells[old_start..old_start + preserved_columns]);
        }

        self.dimensions = dimensions;
        self.cells = cells;
        self.cursor.clamp_to(dimensions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CellAttributes, CellColor, InverseVideo, ItalicStyle, TextIntensity, UnderlineStyle,
    };

    fn labeled_grid() -> ScreenGrid {
        let mut grid = ScreenGrid::new(TerminalDimensions::new(2, 6).unwrap());
        for (row, text) in ["ab", "cd", "ef", "gh", "ij", "kl"].into_iter().enumerate() {
            for (column, character) in text.chars().enumerate() {
                *grid.cell_mut(row, column).unwrap() =
                    Cell::new(character, CellAttributes::default());
            }
        }
        grid
    }

    fn row_text(grid: &ScreenGrid, row: usize) -> String {
        (0..grid.dimensions().columns())
            .map(|column| grid.cell(row, column).unwrap().character())
            .collect()
    }

    #[test]
    fn scrolls_middle_region_up_and_down_without_touching_outside_rows() {
        let margins = VerticalScrollingMargins::new(1, 4, 6).unwrap();
        let mut up = labeled_grid();
        let up_above = [*up.cell(0, 0).unwrap(), *up.cell(0, 1).unwrap()];
        let up_below = [*up.cell(5, 0).unwrap(), *up.cell(5, 1).unwrap()];
        up.scroll_region_up(margins, 1);
        assert_eq!(
            (0..6).map(|row| row_text(&up, row)).collect::<Vec<_>>(),
            ["ab", "ef", "gh", "ij", "  ", "kl"]
        );
        assert_eq!([*up.cell(0, 0).unwrap(), *up.cell(0, 1).unwrap()], up_above);
        assert_eq!([*up.cell(5, 0).unwrap(), *up.cell(5, 1).unwrap()], up_below);

        let mut down = labeled_grid();
        down.scroll_region_down(margins, 1);
        assert_eq!(
            (0..6).map(|row| row_text(&down, row)).collect::<Vec<_>>(),
            ["ab", "  ", "cd", "ef", "gh", "kl"]
        );
    }

    #[test]
    fn region_scroll_preserves_complete_cells_and_uses_default_exposed_rows() {
        let mut grid = labeled_grid();
        let mut attributes = CellAttributes::new(CellColor::Indexed(196), CellColor::Indexed(22));
        attributes.set_intensity(TextIntensity::Bold);
        attributes.set_italic(ItalicStyle::Italic);
        attributes.set_underline(UnderlineStyle::Enabled);
        attributes.set_inverse(InverseVideo::Enabled);
        *grid.cell_mut(3, 0).unwrap() = Cell::new('X', attributes);
        let styled = *grid.cell(3, 0).unwrap();

        grid.scroll_region_up(VerticalScrollingMargins::new(1, 4, 6).unwrap(), 1);

        assert_eq!(grid.cell(2, 0), Some(&styled));
        for column in 0..2 {
            assert_eq!(grid.cell(4, column), Some(&Cell::default()));
        }
    }

    #[test]
    fn region_scroll_clamps_counts_and_handles_one_row_regions() {
        for scroll_up in [true, false] {
            for count in [4, 5, usize::MAX] {
                let mut grid = labeled_grid();
                let margins = VerticalScrollingMargins::new(1, 4, 6).unwrap();
                if scroll_up {
                    grid.scroll_region_up(margins, count);
                } else {
                    grid.scroll_region_down(margins, count);
                }
                assert_eq!(row_text(&grid, 0), "ab");
                assert_eq!(row_text(&grid, 5), "kl");
                for row in 1..=4 {
                    assert_eq!(row_text(&grid, row), "  ");
                }
            }
        }

        for scroll_up in [true, false] {
            let mut grid = labeled_grid();
            let margins = VerticalScrollingMargins::new(2, 2, 6).unwrap();
            if scroll_up {
                grid.scroll_region_up(margins, usize::MAX);
            } else {
                grid.scroll_region_down(margins, usize::MAX);
            }
            assert_eq!(row_text(&grid, 1), "cd");
            assert_eq!(row_text(&grid, 2), "  ");
            assert_eq!(row_text(&grid, 3), "gh");
        }
    }

    #[test]
    fn region_scroll_supports_ranges_touching_either_screen_edge() {
        let mut top = labeled_grid();
        top.scroll_region_up(VerticalScrollingMargins::new(0, 2, 6).unwrap(), 1);
        assert_eq!(
            (0..6).map(|row| row_text(&top, row)).collect::<Vec<_>>(),
            ["cd", "ef", "  ", "gh", "ij", "kl"]
        );

        let mut bottom = labeled_grid();
        bottom.scroll_region_down(VerticalScrollingMargins::new(3, 5, 6).unwrap(), 1);
        assert_eq!(
            (0..6).map(|row| row_text(&bottom, row)).collect::<Vec<_>>(),
            ["ab", "cd", "ef", "  ", "gh", "ij"]
        );
    }
}
