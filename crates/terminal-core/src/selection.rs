use crate::{CellOccupancy, ScreenKind, TerminalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectionPoint {
    row: usize,
    column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selection {
    screen: ScreenKind,
    viewport_offset: usize,
    anchor: SelectionPoint,
    focus: SelectionPoint,
}

impl TerminalState {
    pub fn begin_selection(&mut self, row: usize, column: usize) -> bool {
        if row >= self.dimensions().rows() || column >= self.dimensions().columns() {
            return false;
        }
        let point = SelectionPoint { row, column };
        self.selection = Some(Selection {
            screen: self.active_screen(),
            viewport_offset: self.viewport_offset(),
            anchor: point,
            focus: point,
        });
        true
    }

    pub fn extend_selection(&mut self, row: usize, column: usize) -> bool {
        if row >= self.dimensions().rows() || column >= self.dimensions().columns() {
            return false;
        }
        let screen = self.active_screen();
        let viewport_offset = self.viewport_offset();
        let Some(selection) = self.selection.as_mut() else {
            return false;
        };
        if selection.screen != screen || selection.viewport_offset != viewport_offset {
            self.selection = None;
            return false;
        }
        let point = SelectionPoint { row, column };
        if selection.focus == point {
            return false;
        }
        selection.focus = point;
        true
    }

    pub fn clear_selection(&mut self) -> bool {
        self.selection.take().is_some()
    }

    pub fn is_selected(&self, row: usize, column: usize) -> bool {
        let Some(selection) = self.selection else {
            return false;
        };
        if selection.screen != self.active_screen()
            || selection.viewport_offset != self.viewport_offset()
        {
            return false;
        }
        let (start, end) = ordered(selection.anchor, selection.focus);
        let point = SelectionPoint { row, column };
        point_key(start) <= point_key(point) && point_key(point) <= point_key(end) && start != end
    }

    pub fn selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        if selection.screen != self.active_screen()
            || selection.viewport_offset != self.viewport_offset()
        {
            return None;
        }
        let (start, end) = ordered(selection.anchor, selection.focus);
        if start == end {
            return None;
        }
        let mut result = String::new();
        for row in start.row..=end.row {
            if row != start.row {
                result.push('\n');
            }
            let left = if row == start.row { start.column } else { 0 };
            let right = if row == end.row {
                end.column
            } else {
                self.dimensions().columns() - 1
            };
            let mut line = String::new();
            for column in left..=right {
                let Some(cell) = self.viewport_cell(row, column) else {
                    continue;
                };
                if cell.occupancy() == CellOccupancy::WideContinuation {
                    continue;
                }
                line.push(cell.character());
                line.extend(cell.combining_marks().iter().copied());
            }
            result.push_str(line.trim_end_matches(' '));
        }
        Some(result)
    }
}

fn point_key(point: SelectionPoint) -> (usize, usize) {
    (point.row, point.column)
}

fn ordered(a: SelectionPoint, b: SelectionPoint) -> (SelectionPoint, SelectionPoint) {
    if point_key(a) <= point_key(b) {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use crate::{TerminalDimensions, TerminalState};

    #[test]
    fn selection_extracts_viewport_text_and_clears_across_screen_switch() {
        let mut state = TerminalState::new(TerminalDimensions::new(4, 2).unwrap());
        for c in "ab  ".chars() {
            state.print_character(c).unwrap();
        }
        state.set_cursor_position(1, 0).unwrap();
        for c in "界e\u{301}".chars() {
            state.print_character(c).unwrap();
        }
        assert!(state.begin_selection(0, 0));
        assert!(state.extend_selection(1, 3));
        assert_eq!(state.selected_text().as_deref(), Some("ab\n界e\u{301}"));
        state.switch_to_alternate_screen();
        assert_eq!(state.selected_text(), None);
    }

    #[test]
    fn selection_reads_scrollback_projection_and_clears_on_navigation() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        for c in "one".chars() {
            state.print_character(c).unwrap();
        }
        state.set_cursor_position(1, 0).unwrap();
        for c in "two".chars() {
            state.print_character(c).unwrap();
        }
        state.set_cursor_position(1, 0).unwrap();
        state.index();
        assert!(state.page_up());
        assert!(state.begin_selection(0, 0));
        assert!(state.extend_selection(1, 2));
        assert_eq!(state.selected_text().as_deref(), Some("one\ntwo"));
        assert!(state.page_down());
        assert_eq!(state.selected_text(), None);
    }
}
