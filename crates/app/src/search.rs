//! App-owned search state over the terminal's retained Primary rows.

use terminal_core::{ScreenKind, TerminalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Match {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Search {
    query: String,
    matches: Vec<Match>,
    selected: Option<usize>,
}

impl Search {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn count(&self) -> usize {
        self.matches.len()
    }

    pub fn position(&self) -> Option<usize> {
        self.selected.map(|index| index + 1)
    }

    pub fn selected_match(&self) -> Option<Match> {
        self.selected
            .and_then(|index| self.matches.get(index))
            .copied()
    }

    pub fn viewport_match(&self, terminal: &TerminalState) -> Option<(usize, usize)> {
        let found = self.selected_match()?;
        let top = terminal
            .scrollback_len()
            .saturating_sub(terminal.viewport_offset());
        let row = found.row.checked_sub(top)?;
        (row < terminal.dimensions().rows()).then_some((row, found.column))
    }

    pub fn push_text(&mut self, text: &str, terminal: &mut TerminalState) {
        if text.chars().any(|character| character.is_control()) {
            return;
        }
        self.query.push_str(text);
        self.refresh(terminal);
    }

    pub fn backspace(&mut self, terminal: &mut TerminalState) {
        self.query.pop();
        self.refresh(terminal);
    }

    pub fn refresh(&mut self, terminal: &mut TerminalState) {
        let previous = self.selected_match();
        self.matches.clear();
        self.selected = None;
        if self.query.is_empty() || terminal.active_screen() != ScreenKind::Primary {
            return;
        }

        let history_len = terminal.scrollback_len();
        let rows = terminal.dimensions().rows();
        for row in 0..history_len + rows {
            let cells = if row < history_len {
                terminal.scrollback_row(row)
            } else {
                None
            };
            let width = cells.map_or(terminal.dimensions().columns(), <[_]>::len);
            let mut line = String::new();
            let mut columns = Vec::with_capacity(width);
            for column in 0..width {
                let cell = if let Some(cells) = cells {
                    cells.get(column)
                } else {
                    terminal.screen().cell(row - history_len, column)
                };
                if let Some(cell) = cell
                    && !cell.is_wide_continuation()
                {
                    line.push(cell.character());
                    columns.push(column);
                }
            }
            for (byte, _) in line.match_indices(&self.query) {
                let character = line[..byte].chars().count();
                self.matches.push(Match {
                    row,
                    column: columns[character],
                });
            }
        }
        if !self.matches.is_empty() {
            let top = history_len.saturating_sub(terminal.viewport_offset());
            self.selected = Some(
                previous
                    .and_then(|match_position| {
                        self.matches
                            .iter()
                            .position(|found| *found == match_position)
                    })
                    .or_else(|| self.matches.iter().position(|found| found.row >= top))
                    .unwrap_or(self.matches.len() - 1),
            );
            self.reveal(terminal);
        }
    }

    pub fn navigate(&mut self, terminal: &mut TerminalState, direction: isize) {
        if let Some(index) = self.selected {
            self.selected = Some(
                index
                    .saturating_add_signed(direction)
                    .min(self.matches.len() - 1),
            );
            self.reveal(terminal);
        }
    }

    fn reveal(&self, terminal: &mut TerminalState) {
        let Some(found) = self.selected_match() else {
            return;
        };
        let history_len = terminal.scrollback_len();
        let top = history_len.saturating_sub(terminal.viewport_offset());
        let rows = terminal.dimensions().rows();
        // Keep a match below the prompt's usual top row when history permits.
        // At the oldest retained row the prompt moves to the bottom instead.
        let visible = found.row > top && found.row < top + rows;
        if visible {
            return;
        }
        let desired = history_len.saturating_sub(found.row.saturating_sub(1));
        let delta = desired as i64 - terminal.viewport_offset() as i64;
        terminal.scroll_viewport_rows(delta.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::TerminalDimensions;

    fn terminal() -> TerminalState {
        let mut terminal = TerminalState::new(TerminalDimensions::new(12, 3).unwrap());
        for text in ["alpha", "middle", "alpha", "tail"] {
            for ch in text.chars() {
                terminal.print_character(ch).unwrap();
            }
            terminal.carriage_return();
            terminal.index();
        }
        terminal
    }

    #[test]
    fn finds_and_navigates_bounded_primary_matches() {
        let mut terminal = terminal();
        let mut search = Search::default();
        search.push_text("alpha", &mut terminal);
        assert_eq!(search.count(), 2);
        assert_eq!(search.selected_match().unwrap().row, 2);
        search.navigate(&mut terminal, -1);
        assert_eq!(search.selected_match().unwrap().row, 0);
        assert_eq!(terminal.viewport_offset(), terminal.scrollback_len());
        search.navigate(&mut terminal, -1);
        assert_eq!(search.position(), Some(1));
        search.navigate(&mut terminal, 1);
        assert_eq!(search.position(), Some(2));
        search.navigate(&mut terminal, 1);
        assert_eq!(search.position(), Some(2));
    }

    #[test]
    fn no_match_and_alternate_do_not_move_viewport() {
        let mut terminal = terminal();
        let mut search = Search::default();
        search.push_text("missing", &mut terminal);
        assert_eq!(search.count(), 0);
        assert_eq!(terminal.viewport_offset(), 0);
        terminal.switch_to_alternate_screen();
        search.push_text("alpha", &mut terminal);
        assert_eq!(search.count(), 0);
    }
}
