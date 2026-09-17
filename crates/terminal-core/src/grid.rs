use crate::{
    Cell, CellAttributes, CellOccupancy, Cursor, CursorError, TerminalDimensions,
    VerticalScrollingMargins,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CombiningMarkAttachment {
    Attached,
    NoBase,
    Full,
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

    /// Replaces a cell through the bounded grid API, clearing any intersected wide pair.
    ///
    /// The supplied cell is normalized to single-cell occupancy so continuation
    /// metadata cannot be copied into an unrelated coordinate.
    pub fn set_cell(&mut self, row: usize, column: usize, cell: Cell) -> bool {
        if self.index_of(row, column).is_none() {
            return false;
        }
        self.write_single(row, column, cell.character(), *cell.attributes());
        true
    }

    #[cfg(test)]
    pub(crate) fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
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

    pub(crate) fn append_combining_mark(
        &mut self,
        row: usize,
        column: usize,
        character: char,
    ) -> CombiningMarkAttachment {
        let Some(index) = self.index_of(row, column) else {
            return CombiningMarkAttachment::NoBase;
        };
        let lead_index = match self.cells[index].occupancy() {
            CellOccupancy::WideContinuation if column > 0 => index - 1,
            CellOccupancy::WideContinuation => return CombiningMarkAttachment::NoBase,
            CellOccupancy::Single | CellOccupancy::WideLead => index,
        };
        match self.cells[lead_index].append_combining_mark(character) {
            Ok(true) => CombiningMarkAttachment::Attached,
            Ok(false) => CombiningMarkAttachment::NoBase,
            Err(()) => CombiningMarkAttachment::Full,
        }
    }

    /// Writes a single-column printable cell, clearing any intersected wide pair.
    pub(crate) fn write_single(
        &mut self,
        row: usize,
        column: usize,
        character: char,
        attributes: CellAttributes,
    ) {
        self.clear_wide_at(row, column);
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        self.cells[index] = Cell::new(character, attributes);
        self.normalize_row(row);
    }

    /// Writes one complete two-column character at an in-bounds leading column.
    pub(crate) fn write_wide(
        &mut self,
        row: usize,
        column: usize,
        character: char,
        attributes: CellAttributes,
    ) {
        debug_assert!(column + 1 < self.dimensions.columns());
        self.clear_wide_at(row, column);
        self.clear_wide_at(row, column + 1);
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        self.cells[index] = Cell::wide_lead(character, attributes);
        self.cells[index + 1] = Cell::wide_continuation();
        self.normalize_row(row);
    }

    pub(crate) fn insert_cells(&mut self, row: usize, column: usize, count: usize, cell: Cell) {
        self.clear_wide_at(row, column);
        self.shift_right(row, column, count, cell);
    }

    pub(crate) fn insert_wide(
        &mut self,
        row: usize,
        column: usize,
        character: char,
        attributes: CellAttributes,
    ) {
        self.clear_wide_at(row, column);
        if column + 1 < self.dimensions.columns() {
            self.clear_wide_at(row, column + 1);
        }
        self.shift_right(row, column, 2, Cell::default());
        if column + 1 < self.dimensions.columns() {
            self.write_wide(row, column, character, attributes);
        }
    }

    pub(crate) fn delete_cells(&mut self, row: usize, column: usize, count: usize) {
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        let row_end = (row + 1) * self.dimensions.columns();
        let count = count.min(row_end - index);
        if count == 0 {
            return;
        }

        if count < row_end - index {
            self.cells.copy_within(index + count..row_end, index);
        }
        self.cells[row_end - count..row_end].fill(Cell::default());
        self.normalize_row(row);
    }

    pub(crate) fn erase_cells(&mut self, start: usize, end: usize) {
        self.cells[start..end].fill(Cell::default());
        let columns = self.dimensions.columns();
        let first_row = start / columns;
        let last_row = (end - 1) / columns;
        for row in first_row..=last_row {
            self.normalize_row(row);
        }
    }

    fn shift_right(&mut self, row: usize, column: usize, count: usize, cell: Cell) {
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        let row_end = (row + 1) * self.dimensions.columns();
        let count = count.min(row_end - index);
        if count == 0 {
            return;
        }

        if count < row_end - index {
            self.cells
                .copy_within(index..row_end - count, index + count);
        }
        self.cells[index..index + count].fill(cell);
        self.normalize_row(row);
    }

    fn clear_wide_at(&mut self, row: usize, column: usize) {
        let index = self
            .index_of(row, column)
            .expect("terminal state keeps cell writes in bounds");
        match self.cells[index].occupancy() {
            CellOccupancy::Single => {}
            CellOccupancy::WideLead => {
                self.cells[index] = Cell::default();
                if column + 1 < self.dimensions.columns() {
                    self.cells[index + 1] = Cell::default();
                }
            }
            CellOccupancy::WideContinuation => {
                self.cells[index] = Cell::default();
                if column > 0 {
                    self.cells[index - 1] = Cell::default();
                }
            }
        }
    }

    fn normalize_row(&mut self, row: usize) {
        let columns = self.dimensions.columns();
        let start = row * columns;
        let end = start + columns;
        let mut column = 0;
        while column < columns {
            match self.cells[start + column].occupancy() {
                CellOccupancy::Single => column += 1,
                CellOccupancy::WideLead => {
                    if column + 1 >= columns
                        || self.cells[start + column + 1].occupancy()
                            != CellOccupancy::WideContinuation
                    {
                        self.cells[start + column] = Cell::default();
                    }
                    column += 1;
                }
                CellOccupancy::WideContinuation => {
                    if column == 0
                        || self.cells[start + column - 1].occupancy() != CellOccupancy::WideLead
                    {
                        self.cells[start + column] = Cell::default();
                    }
                    column += 1;
                }
            }
        }
        debug_assert!(self.wide_cells_are_valid());
        debug_assert_eq!(end, start + columns);
    }

    fn wide_cells_are_valid(&self) -> bool {
        (0..self.dimensions.rows()).all(|row| {
            (0..self.dimensions.columns()).all(|column| {
                match self.cell(row, column).unwrap().occupancy() {
                    CellOccupancy::Single => true,
                    CellOccupancy::WideLead => {
                        column + 1 < self.dimensions.columns()
                            && self.cell(row, column + 1).unwrap().occupancy()
                                == CellOccupancy::WideContinuation
                    }
                    CellOccupancy::WideContinuation => {
                        column > 0
                            && self.cell(row, column - 1).unwrap().occupancy()
                                == CellOccupancy::WideLead
                    }
                }
            })
        })
    }

    pub(crate) fn scroll_up(&mut self, rows: usize) {
        self.scroll_region_up(
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
        for row in 0..dimensions.rows() {
            self.normalize_row(row);
        }
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
    fn delete_cells_shifts_complete_row_cells_left_and_clamps() {
        let mut grid = ScreenGrid::new(TerminalDimensions::new(5, 3).unwrap());
        for (row, text) in ["abcde", "fghij", "klmno"].into_iter().enumerate() {
            for (column, character) in text.chars().enumerate() {
                *grid.cell_mut(row, column).unwrap() =
                    Cell::new(character, CellAttributes::default());
            }
        }
        let mut attributes = CellAttributes::new(CellColor::Indexed(196), CellColor::Indexed(22));
        attributes.set_intensity(TextIntensity::Bold);
        *grid.cell_mut(1, 4).unwrap() = Cell::new('X', attributes);
        let styled = *grid.cell(1, 4).unwrap();

        grid.delete_cells(1, 2, 2);

        assert_eq!(row_text(&grid, 1), "fgX  ");
        assert_eq!(grid.cell(1, 2), Some(&styled));
        assert_eq!(grid.cell(1, 3), Some(&Cell::default()));
        assert_eq!(grid.cell(1, 4), Some(&Cell::default()));
        assert_eq!(row_text(&grid, 0), "abcde");
        assert_eq!(row_text(&grid, 2), "klmno");

        grid.delete_cells(1, 0, usize::MAX);
        assert_eq!(row_text(&grid, 1), "     ");
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
