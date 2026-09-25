use crate::{CellOccupancy, ScreenKind, TerminalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionPoint {
    row: usize,
    column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionUnit {
    Cell,
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selection {
    screen: ScreenKind,
    anchor_start: SelectionPoint,
    anchor_end: SelectionPoint,
    focus_start: SelectionPoint,
    focus_end: SelectionPoint,
    unit: SelectionUnit,
    active: bool,
}

impl TerminalState {
    pub fn begin_selection(&mut self, row: usize, column: usize) -> bool {
        self.start_selection(row, column, SelectionUnit::Cell)
    }

    pub fn select_word(&mut self, row: usize, column: usize) -> bool {
        self.start_selection(row, column, SelectionUnit::Word)
    }

    pub fn select_line(&mut self, row: usize, column: usize) -> bool {
        self.start_selection(row, column, SelectionUnit::Line)
    }

    fn start_selection(&mut self, row: usize, column: usize, unit: SelectionUnit) -> bool {
        let Some(point) = self.selection_point(row, column) else {
            return false;
        };
        let (start, end) = self.unit_bounds(point, unit);
        self.selection = Some(Selection {
            screen: self.active_screen(),
            anchor_start: start,
            anchor_end: end,
            focus_start: start,
            focus_end: end,
            unit,
            active: unit != SelectionUnit::Cell,
        });
        true
    }

    pub fn extend_selection(&mut self, row: usize, column: usize) -> bool {
        let Some(point) = self.selection_point(row, column) else {
            return false;
        };
        let Some(selection) = self.selection else {
            return false;
        };
        if selection.screen != self.active_screen() {
            self.selection = None;
            return false;
        }
        let (start, end) = self.unit_bounds(point, selection.unit);
        let active = selection.unit != SelectionUnit::Cell || start != selection.anchor_start;
        let changed = selection.focus_start != start
            || selection.focus_end != end
            || selection.active != active;
        if let Some(selection) = &mut self.selection {
            selection.focus_start = start;
            selection.focus_end = end;
            selection.active = active;
        }
        changed
    }

    /// Scrolls the viewport during a drag and extends the same selection into the new view.
    pub fn scroll_selection_viewport_rows(&mut self, rows: i32, row: usize, column: usize) -> bool {
        if self.selection.is_none() || !self.scroll_viewport_rows(rows) {
            return false;
        }
        self.extend_selection(row, column);
        true
    }

    pub fn has_selection(&self) -> bool {
        self.selection
            .is_some_and(|selection| selection.screen == self.active_screen())
    }

    pub fn clear_selection(&mut self) -> bool {
        self.selection.take().is_some()
    }

    pub fn is_selected(&self, row: usize, column: usize) -> bool {
        let Some(selection) = self
            .selection
            .filter(|selection| selection.active && selection.screen == self.active_screen())
        else {
            return false;
        };
        let Some(point) = self.selection_point(row, column) else {
            return false;
        };
        let (start, end) = selection_bounds(selection);
        if end.row < self.selection_history_origin() {
            return false;
        }
        start <= point && point <= end
    }

    pub fn selected_text(&self) -> Option<String> {
        let selection = self
            .selection
            .filter(|selection| selection.active && selection.screen == self.active_screen())?;
        let (mut start, end) = selection_bounds(selection);
        if start.row < self.selection_history_origin() {
            start.row = self.selection_history_origin();
            start.column = 0;
        }
        if start > end {
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
                let Some(cell) = self.selection_cell(row, column) else {
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

    fn selection_point(&self, row: usize, column: usize) -> Option<SelectionPoint> {
        if row >= self.dimensions().rows() || column >= self.dimensions().columns() {
            return None;
        }
        let history = if self.active_screen() == ScreenKind::Primary {
            self.scrollback_len()
        } else {
            0
        };
        let row = self.selection_history_origin() + history + row - self.viewport_offset();
        let column = if column > 0
            && self
                .selection_cell(row, column)
                .is_some_and(|cell| cell.is_wide_continuation())
        {
            column - 1
        } else {
            column
        };
        Some(SelectionPoint { row, column })
    }

    fn unit_bounds(
        &self,
        point: SelectionPoint,
        unit: SelectionUnit,
    ) -> (SelectionPoint, SelectionPoint) {
        match unit {
            SelectionUnit::Cell => (point, point),
            SelectionUnit::Line => (
                SelectionPoint { column: 0, ..point },
                SelectionPoint {
                    column: self.dimensions().columns() - 1,
                    ..point
                },
            ),
            SelectionUnit::Word => {
                let class = self.word_class(point.row, point.column);
                let mut left = point.column;
                while left > 0 && self.word_class(point.row, left - 1) == class {
                    left -= 1;
                }
                let mut right = point.column;
                let last = self.dimensions().columns() - 1;
                while right < last && self.word_class(point.row, right + 1) == class {
                    right += 1;
                }
                (
                    SelectionPoint {
                        column: left,
                        ..point
                    },
                    SelectionPoint {
                        column: right,
                        ..point
                    },
                )
            }
        }
    }

    fn word_class(&self, row: usize, column: usize) -> u8 {
        let Some(cell) = self.selection_cell(row, column) else {
            return 0;
        };
        let character = if cell.is_wide_continuation() && column > 0 {
            self.selection_cell(row, column - 1)
                .map_or(' ', |lead| lead.character())
        } else {
            cell.character()
        };
        if character.is_alphanumeric()
            || matches!(
                character,
                '_' | '.' | '/' | '\\' | ':' | '~' | '@' | '-' | '$' | '%' | '+'
            )
        {
            1
        } else if character.is_whitespace() {
            0
        } else {
            2
        }
    }
}

fn selection_bounds(selection: Selection) -> (SelectionPoint, SelectionPoint) {
    (
        selection.anchor_start.min(selection.focus_start),
        selection.anchor_end.max(selection.focus_end),
    )
}

#[cfg(test)]
mod tests {
    use crate::{TerminalDimensions, TerminalState};

    fn print(state: &mut TerminalState, text: &str) {
        for character in text.chars() {
            state.print_character(character).unwrap();
        }
    }

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
    fn selection_reads_scrollback_projection_and_survives_navigation() {
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
        assert_eq!(state.selected_text().as_deref(), Some("one\ntwo"));
        assert!(state.is_selected(0, 0));
        assert!(!state.is_selected(1, 0));
        assert!(state.page_up());
        assert!(state.is_selected(0, 0));
        assert!(state.return_to_live_viewport());
        assert_eq!(state.selected_text().as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn viewport_movement_preserves_selection_spanning_history_and_live_rows() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        for (index, word) in ["one", "two", "tri", "for"].into_iter().enumerate() {
            state.set_cursor_position(1, 0).unwrap();
            if index > 0 {
                state.index();
            }
            print(&mut state, word);
        }
        assert!(state.scroll_viewport_rows(1));
        assert!(state.begin_selection(0, 0));
        assert!(state.extend_selection(1, 2));
        assert_eq!(state.selected_text().as_deref(), Some("two\ntri"));

        assert!(state.scroll_viewport_rows(1));
        assert_eq!(state.selected_text().as_deref(), Some("two\ntri"));
        assert!(!state.is_selected(0, 0));
        assert!(state.is_selected(1, 0));

        assert!(state.scroll_viewport_rows(-2));
        assert_eq!(state.selected_text().as_deref(), Some("two\ntri"));
        assert!(state.is_selected(0, 0));
        assert!(!state.is_selected(1, 0));
    }

    #[test]
    fn word_selection_uses_terminal_identifier_and_path_boundaries() {
        let mut state = TerminalState::new(TerminalDimensions::new(28, 2).unwrap());
        print(&mut state, "go src/my_file.rs:12! next");
        assert!(state.select_word(0, 7));
        assert_eq!(state.selected_text().as_deref(), Some("src/my_file.rs:12"));
        assert!(state.select_word(0, 20));
        assert_eq!(state.selected_text().as_deref(), Some("!"));
        assert!(state.select_word(0, 21));
        assert_eq!(state.selected_text().as_deref(), Some(""));
    }

    #[test]
    fn line_selection_and_extension_keep_complete_rows() {
        let mut state = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
        print(&mut state, "first");
        state.set_cursor_position(1, 0).unwrap();
        print(&mut state, "two");
        assert!(state.select_line(0, 2));
        assert_eq!(state.selected_text().as_deref(), Some("first"));
        assert!(state.extend_selection(1, 1));
        assert_eq!(state.selected_text().as_deref(), Some("first\ntwo"));
        assert!(state.begin_selection(0, 1));
        assert!(state.has_selection());
        assert_eq!(state.selected_text(), None);
        assert!(state.extend_selection(1, 1));
        assert_eq!(state.selected_text().as_deref(), Some("irst\ntw"));
    }

    #[test]
    fn drag_autoscroll_extends_selection_from_live_rows_into_scrollback() {
        let mut state = TerminalState::new(TerminalDimensions::new(3, 2).unwrap());
        for (index, word) in ["one", "two", "tri", "for"].into_iter().enumerate() {
            if index == 1 {
                state.set_cursor_position(1, 0).unwrap();
            } else if index > 1 {
                state.set_cursor_position(1, 0).unwrap();
                state.index();
            }
            print(&mut state, word);
        }
        assert!(state.begin_selection(1, 2));
        assert!(state.scroll_selection_viewport_rows(1, 0, 0));
        assert_eq!(state.selected_text().as_deref(), Some("two\ntri\nfor"));
        assert!(state.scroll_selection_viewport_rows(1, 0, 0));
        assert_eq!(state.selected_text().as_deref(), Some("one\ntwo\ntri\nfor"));
        assert!(state.is_selected(0, 0));
        assert!(!state.scroll_selection_viewport_rows(1, 0, 0));
        assert!(state.scroll_selection_viewport_rows(-1, 1, 2));
        assert_eq!(state.selected_text().as_deref(), Some("i\nfor"));
    }

    #[test]
    fn wide_and_combining_cells_are_selected_as_whole_characters() {
        let mut state = TerminalState::new(TerminalDimensions::new(8, 2).unwrap());
        print(&mut state, "界e\u{301}/z !");
        assert!(state.select_word(0, 1));
        assert_eq!(state.selected_text().as_deref(), Some("界e\u{301}/z"));
        assert!(state.is_selected(0, 0));
        assert!(state.is_selected(0, 1));
        assert!(state.select_word(0, 2));
        assert_eq!(state.selected_text().as_deref(), Some("界e\u{301}/z"));
    }

    #[test]
    fn retained_selection_survives_scrollback_eviction() {
        let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
        state.set_cursor_position(1, 0).unwrap();
        for _ in 0..crate::MAX_SCROLLBACK_ROWS {
            state.index();
        }
        state.set_cursor_position(0, 0).unwrap();
        print(&mut state, "ab");
        assert!(state.select_word(0, 0));
        state.set_cursor_position(1, 0).unwrap();
        state.index();
        assert_eq!(state.selected_text().as_deref(), Some("ab"));
    }
}
