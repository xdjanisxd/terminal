use terminal_core::{ScreenKind, TerminalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Point {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Motion {
    Left,
    Down,
    Up,
    Right,
    WordForward,
    WordBack,
    WordEnd,
    LineStart,
    LineEnd,
    Top,
    Bottom,
    HalfUp,
    HalfDown,
}

#[derive(Debug)]
pub(crate) struct CopyMode {
    pub cursor: Point,
    pub visual: bool,
    pub search_input: bool,
    pub pending_g: bool,
}

fn bounds(terminal: &TerminalState) -> (usize, usize) {
    let first = terminal.scrollback_origin();
    let last = first + terminal.scrollback_len() + terminal.dimensions().rows() - 1;
    (first, last)
}

fn row_width(terminal: &TerminalState, row: usize) -> usize {
    let index = row.saturating_sub(terminal.scrollback_origin());
    terminal
        .scrollback_row(index)
        .map_or(terminal.dimensions().columns(), <[_]>::len)
        .max(1)
}

fn cell(terminal: &TerminalState, point: Point) -> Option<terminal_core::Cell> {
    let index = point.row.checked_sub(terminal.scrollback_origin())?;
    if index < terminal.scrollback_len() {
        terminal.scrollback_row(index)?.get(point.column).copied()
    } else {
        terminal
            .screen()
            .cell(index - terminal.scrollback_len(), point.column)
            .copied()
    }
}

fn normalize(terminal: &TerminalState, mut point: Point) -> Point {
    let (first, last) = bounds(terminal);
    point.row = point.row.clamp(first, last);
    point.column = point.column.min(row_width(terminal, point.row) - 1);
    if point.column > 0 && cell(terminal, point).is_some_and(|cell| cell.is_wide_continuation()) {
        point.column -= 1;
    }
    point
}

fn next(terminal: &TerminalState, point: Point) -> Option<Point> {
    let (_, last) = bounds(terminal);
    let mut column = point.column + 1;
    while column < row_width(terminal, point.row) {
        let candidate = Point { column, ..point };
        if !cell(terminal, candidate).is_some_and(|cell| cell.is_wide_continuation()) {
            return Some(candidate);
        }
        column += 1;
    }
    (point.row < last).then_some(Point {
        row: point.row + 1,
        column: 0,
    })
}

fn previous(terminal: &TerminalState, point: Point) -> Option<Point> {
    let (first, _) = bounds(terminal);
    let mut candidate = if point.column > 0 {
        Point {
            column: point.column - 1,
            ..point
        }
    } else if point.row > first {
        Point {
            row: point.row - 1,
            column: row_width(terminal, point.row - 1) - 1,
        }
    } else {
        return None;
    };
    if candidate.column > 0
        && cell(terminal, candidate).is_some_and(|cell| cell.is_wide_continuation())
    {
        candidate.column -= 1;
    }
    Some(candidate)
}

fn class(terminal: &TerminalState, point: Point) -> u8 {
    match cell(terminal, point)
        .map(|cell| cell.character())
        .unwrap_or(' ')
    {
        c if c.is_whitespace() => 0,
        c if c.is_alphanumeric() || c == '_' => 1,
        _ => 2,
    }
}

fn wraps_from_previous(terminal: &TerminalState, row: usize) -> bool {
    row.checked_sub(terminal.scrollback_origin())
        .and_then(|index| terminal.primary_row_wraps_from_previous(index))
        .unwrap_or(false)
}

impl CopyMode {
    pub fn enter(terminal: &TerminalState) -> Option<Self> {
        (terminal.active_screen() == ScreenKind::Primary).then(|| {
            let top = terminal
                .scrollback_len()
                .saturating_sub(terminal.viewport_offset());
            let row = if terminal.viewport_offset() == 0 {
                terminal.scrollback_len() + terminal.cursor().row()
            } else {
                top + terminal.dimensions().rows() - 1
            };
            Self {
                cursor: normalize(
                    terminal,
                    Point {
                        row: terminal.scrollback_origin() + row,
                        column: terminal.cursor().column(),
                    },
                ),
                visual: false,
                search_input: false,
                pending_g: false,
            }
        })
    }

    pub fn reconcile(&mut self, terminal: &mut TerminalState) {
        self.cursor = normalize(terminal, self.cursor);
        self.reveal(terminal);
        if self.visual {
            self.extend(terminal);
        }
    }

    pub fn viewport_position(&self, terminal: &TerminalState) -> Option<(usize, usize)> {
        let top =
            terminal.scrollback_origin() + terminal.scrollback_len() - terminal.viewport_offset();
        let row = self.cursor.row.checked_sub(top)?;
        (row < terminal.dimensions().rows()).then_some((row, self.cursor.column))
    }

    fn reveal(&self, terminal: &mut TerminalState) {
        let top =
            terminal.scrollback_origin() + terminal.scrollback_len() - terminal.viewport_offset();
        let rows = terminal.dimensions().rows();
        if self.cursor.row >= top && self.cursor.row < top + rows {
            return;
        }
        let desired_top = if self.cursor.row < top {
            self.cursor.row
        } else {
            self.cursor.row + 1 - rows
        };
        let delta = top as i64 - desired_top as i64;
        terminal.scroll_viewport_rows(delta.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }

    pub fn start_visual(&mut self, terminal: &mut TerminalState) {
        self.reconcile(terminal);
        if let Some((row, column)) = self.viewport_position(terminal) {
            terminal.begin_visual_selection(row, column);
            self.visual = true;
        }
    }

    pub fn cancel_visual(&mut self, terminal: &mut TerminalState) {
        self.visual = false;
        terminal.clear_selection();
    }

    fn extend(&self, terminal: &mut TerminalState) {
        if let Some((row, column)) = self.viewport_position(terminal) {
            terminal.extend_selection(row, column);
        }
    }

    pub fn set_cursor(&mut self, terminal: &mut TerminalState, point: Point) {
        self.cursor = normalize(terminal, point);
        self.reveal(terminal);
        if self.visual {
            self.extend(terminal);
        }
    }

    pub fn move_cursor(&mut self, terminal: &mut TerminalState, motion: Motion) {
        self.reconcile(terminal);
        let mut point = self.cursor;
        let (first, last) = bounds(terminal);
        match motion {
            Motion::Left => {
                point = previous(terminal, point)
                    .filter(|p| p.row == point.row)
                    .unwrap_or(point)
            }
            Motion::Right => {
                point = next(terminal, point)
                    .filter(|p| p.row == point.row)
                    .unwrap_or(point)
            }
            Motion::Up => point.row = point.row.saturating_sub(1).max(first),
            Motion::Down => point.row = point.row.saturating_add(1).min(last),
            Motion::Top => {
                point.row = first;
                point.column = 0;
            }
            Motion::Bottom => {
                point.row = last;
                point.column = 0;
            }
            Motion::HalfUp => {
                point.row = point
                    .row
                    .saturating_sub((terminal.dimensions().rows() / 2).max(1))
                    .max(first)
            }
            Motion::HalfDown => {
                point.row = point
                    .row
                    .saturating_add((terminal.dimensions().rows() / 2).max(1))
                    .min(last)
            }
            Motion::LineStart => {
                while point.row > first && wraps_from_previous(terminal, point.row) {
                    point.row -= 1;
                }
                point.column = 0;
            }
            Motion::LineEnd => {
                while point.row < last && wraps_from_previous(terminal, point.row + 1) {
                    point.row += 1;
                }
                point.column = (0..row_width(terminal, point.row))
                    .rev()
                    .find(|&column| {
                        cell(terminal, Point { column, ..point }).is_some_and(|cell| {
                            !cell.is_wide_continuation() && cell.character() != ' '
                        })
                    })
                    .unwrap_or(0);
            }
            Motion::WordForward => {
                let original = class(terminal, point);
                while let Some(candidate) = next(terminal, point) {
                    point = candidate;
                    if class(terminal, point) != original {
                        break;
                    }
                }
                while class(terminal, point) == 0 {
                    let Some(candidate) = next(terminal, point) else {
                        break;
                    };
                    point = candidate;
                }
            }
            Motion::WordBack => {
                while let Some(candidate) = previous(terminal, point) {
                    point = candidate;
                    if class(terminal, point) != 0 {
                        break;
                    }
                }
                let kind = class(terminal, point);
                while let Some(candidate) = previous(terminal, point) {
                    if class(terminal, candidate) != kind {
                        break;
                    }
                    point = candidate;
                }
            }
            Motion::WordEnd => {
                if class(terminal, point) == 0 {
                    while let Some(candidate) = next(terminal, point) {
                        point = candidate;
                        if class(terminal, point) != 0 {
                            break;
                        }
                    }
                } else if let Some(candidate) = next(terminal, point) {
                    point = candidate;
                    if class(terminal, point) == 0 {
                        while let Some(candidate) = next(terminal, point) {
                            point = candidate;
                            if class(terminal, point) != 0 {
                                break;
                            }
                        }
                    }
                }
                let kind = class(terminal, point);
                if kind != 0 {
                    while let Some(candidate) = next(terminal, point) {
                        if class(terminal, candidate) != kind {
                            break;
                        }
                        point = candidate;
                    }
                }
            }
        }
        self.set_cursor(terminal, point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::TerminalDimensions;

    fn terminal() -> TerminalState {
        let mut terminal = TerminalState::new(TerminalDimensions::new(12, 3).unwrap());
        for line in ["old", "alpha beta", "界e\u{301} x", "tail"] {
            for character in line.chars() {
                terminal.print_character(character).unwrap();
            }
            terminal.carriage_return();
            terminal.index();
        }
        terminal
    }

    #[test]
    fn vertical_and_horizontal_movement_clamp_to_retained_rows() {
        let mut terminal = terminal();
        let mut mode = CopyMode::enter(&terminal).unwrap();
        mode.move_cursor(&mut terminal, Motion::Top);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin());
        mode.move_cursor(&mut terminal, Motion::Up);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin());
        mode.move_cursor(&mut terminal, Motion::Right);
        assert_eq!(mode.cursor.column, 1);
        mode.move_cursor(&mut terminal, Motion::Left);
        assert_eq!(mode.cursor.column, 0);
        mode.move_cursor(&mut terminal, Motion::Down);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin() + 1);
        mode.move_cursor(&mut terminal, Motion::Bottom);
        let bottom = mode.cursor.row;
        mode.move_cursor(&mut terminal, Motion::Down);
        assert_eq!(mode.cursor.row, bottom);
    }

    #[test]
    fn word_line_and_half_page_motions() {
        let mut terminal = terminal();
        let mut mode = CopyMode::enter(&terminal).unwrap();
        let row = terminal.scrollback_origin() + 1;
        mode.set_cursor(&mut terminal, Point { row, column: 0 });
        mode.move_cursor(&mut terminal, Motion::WordForward);
        assert_eq!(mode.cursor.column, 6);
        mode.move_cursor(&mut terminal, Motion::WordEnd);
        assert_eq!(mode.cursor.column, 9);
        mode.move_cursor(&mut terminal, Motion::WordBack);
        assert_eq!(mode.cursor.column, 6);
        mode.move_cursor(&mut terminal, Motion::LineEnd);
        assert_eq!(mode.cursor.column, 9);
        mode.move_cursor(&mut terminal, Motion::LineStart);
        assert_eq!(mode.cursor.column, 0);
        mode.move_cursor(&mut terminal, Motion::HalfDown);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin() + 2);
        mode.move_cursor(&mut terminal, Motion::HalfUp);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin() + 1);
    }

    #[test]
    fn line_motions_cross_soft_wraps_into_scrollback() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(5, 2).unwrap());
        for character in "abcdefgh".chars() {
            terminal.print_character(character).unwrap();
        }
        let mut mode = CopyMode::enter(&terminal).unwrap();
        mode.move_cursor(&mut terminal, Motion::LineStart);
        assert_eq!(mode.cursor.column, 0);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin());
        mode.move_cursor(&mut terminal, Motion::LineEnd);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin() + 1);
        assert_eq!(mode.cursor.column, 2);

        terminal.index();
        mode.reconcile(&mut terminal);
        mode.move_cursor(&mut terminal, Motion::LineStart);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin());
        mode.move_cursor(&mut terminal, Motion::LineEnd);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin() + 1);
        assert_eq!(mode.cursor.column, 2);
    }

    #[test]
    fn visual_selection_preserves_wide_and_combining_cells() {
        let mut terminal = terminal();
        let mut mode = CopyMode::enter(&terminal).unwrap();
        let row = terminal.scrollback_origin() + 2;
        mode.set_cursor(&mut terminal, Point { row, column: 0 });
        mode.start_visual(&mut terminal);
        assert_eq!(terminal.selected_text().as_deref(), Some("界"));
        mode.move_cursor(&mut terminal, Motion::Right);
        assert_eq!(mode.cursor.column, 2);
        assert_eq!(terminal.selected_text().as_deref(), Some("界e\u{301}"));
        mode.cancel_visual(&mut terminal);
        assert!(terminal.selected_text().is_none());
    }

    #[test]
    fn output_and_eviction_keep_cursor_in_retained_history() {
        let mut terminal = terminal();
        let mut mode = CopyMode::enter(&terminal).unwrap();
        mode.move_cursor(&mut terminal, Motion::Top);
        let original = mode.cursor;
        terminal.print_character('z').unwrap();
        mode.reconcile(&mut terminal);
        assert_eq!(mode.cursor, original);
        for _ in 0..10_100 {
            terminal.index();
        }
        mode.reconcile(&mut terminal);
        assert_eq!(mode.cursor.row, terminal.scrollback_origin());
        assert!(mode.viewport_position(&terminal).is_some());
    }
}
