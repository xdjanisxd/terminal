//! App-owned search state over the terminal's retained Primary rows.

use terminal_core::{ScreenKind, TerminalState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Match {
    pub row: usize,
    pub start_column: usize,
    pub end_column: usize,
    pub end_row: usize,
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

    pub fn markers(&self) -> Vec<(usize, bool)> {
        let mut markers: Vec<(usize, bool)> = Vec::new();
        for (index, found) in self.matches.iter().enumerate() {
            for row in found.row..=found.end_row {
                let active = Some(index) == self.selected;
                if let Some(last) = markers.last_mut().filter(|last| last.0 == row) {
                    last.1 |= active;
                } else {
                    markers.push((row, active));
                }
            }
        }
        markers
    }

    pub fn viewport_match(&self, terminal: &TerminalState) -> Option<(usize, usize)> {
        let found = self.selected_match()?;
        let top = terminal
            .scrollback_len()
            .saturating_sub(terminal.viewport_offset());
        if found.end_row < top {
            return None;
        }
        let row = found.row.saturating_sub(top);
        let column = if found.row < top {
            0
        } else {
            found.start_column
        };
        (row < terminal.dimensions().rows()).then_some((row, column))
    }

    pub fn viewport_matches(&self, terminal: &TerminalState) -> Vec<(usize, usize, usize, bool)> {
        let top = terminal
            .scrollback_len()
            .saturating_sub(terminal.viewport_offset());
        let bottom = top + terminal.dimensions().rows();
        let mut spans = Vec::new();
        for (index, found) in self.matches.iter().enumerate() {
            for row in found.row.max(top)..=found.end_row.min(bottom.saturating_sub(1)) {
                let start = if row == found.row {
                    found.start_column
                } else {
                    0
                };
                let end = if row == found.end_row {
                    found.end_column
                } else {
                    terminal.primary_row_wrap_columns(row)
                };
                spans.push((row - top, start, end, Some(index) == self.selected));
            }
        }
        spans
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
        let folded_query = self.query.to_lowercase();
        let query_chars = folded_query.chars().count();
        let mut line = String::new();
        let mut positions = Vec::new();
        for row in 0..history_len + rows {
            let cells = if row < history_len {
                terminal.scrollback_row(row)
            } else {
                None
            };
            let width = cells.map_or(terminal.dimensions().columns(), <[_]>::len);
            let wrap = terminal.primary_row_wrap_columns(row);
            let length = if wrap == 0 { width } else { wrap.min(width) };
            for column in 0..length {
                let cell = if let Some(cells) = cells {
                    cells.get(column)
                } else {
                    terminal.screen().cell(row - history_len, column)
                };
                if let Some(cell) = cell
                    && !cell.is_wide_continuation()
                {
                    for folded in std::iter::once(cell.character())
                        .chain(cell.grapheme_tail().iter().copied())
                        .flat_map(char::to_lowercase)
                    {
                        line.push(folded);
                        positions.push((
                            row,
                            column,
                            (column + usize::from(cell.is_wide_lead()) + 1).min(width),
                        ));
                    }
                }
            }
            let mut indexed_bytes = line.char_indices().map(|(byte, _)| byte).enumerate();
            let mut consumed = 0;
            for (byte, _) in line.match_indices(&folded_query) {
                let character = indexed_bytes.find(|&(_, offset)| offset == byte).unwrap().0;
                let end_character = character + query_chars - 1;
                self.matches.push(Match {
                    row: positions[character].0,
                    start_column: positions[character].1,
                    end_row: positions[end_character].0,
                    end_column: positions[end_character].2,
                });
                consumed = end_character + 1;
            }
            if wrap == 0 {
                line.clear();
                positions.clear();
            } else {
                // Retain only a possible prefix of a match crossing the next row.
                let discard = consumed.max(positions.len().saturating_sub(query_chars - 1));
                let byte = line
                    .char_indices()
                    .nth(discard)
                    .map_or(line.len(), |(i, _)| i);
                line.drain(..byte);
                positions.drain(..discard);
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
        if let Some(index) = self.selected
            && !self.matches.is_empty()
        {
            self.selected = Some(if direction < 0 {
                if index == 0 {
                    self.matches.len() - 1
                } else {
                    index - 1
                }
            } else if direction > 0 {
                (index + 1) % self.matches.len()
            } else {
                index
            });
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
        for text in ["alpha", "middle", "Alpha", "tail"] {
            for ch in text.chars() {
                terminal.print_character(ch).unwrap();
            }
            terminal.carriage_return();
            terminal.index();
        }
        terminal
    }

    #[test]
    fn finds_case_insensitive_matches_and_wraps_navigation() {
        let mut terminal = terminal();
        let mut search = Search::default();
        search.push_text("ALPHA", &mut terminal);
        assert_eq!(search.count(), 2);
        assert_eq!(search.selected_match().unwrap().row, 2);
        assert_eq!(search.selected_match().unwrap().start_column, 0);
        assert_eq!(search.selected_match().unwrap().end_column, 5);
        assert_eq!(search.position(), Some(2));
        search.navigate(&mut terminal, -1);
        assert_eq!(search.selected_match().unwrap().row, 0);
        assert_eq!(terminal.viewport_offset(), terminal.scrollback_len());
        search.navigate(&mut terminal, -1);
        assert_eq!(search.position(), Some(2));
        search.navigate(&mut terminal, 1);
        assert_eq!(search.position(), Some(1));
        search.navigate(&mut terminal, 1);
        assert_eq!(search.position(), Some(2));
    }

    #[test]
    fn searches_complete_graphemes_and_highlights_whole_cell_spans() {
        for query in ["👩‍💻", "🇺🇸", "e\u{301}"] {
            let mut terminal = TerminalState::new(TerminalDimensions::new(12, 2).unwrap());
            terminal_core::TerminalParser::new()
                .advance(&mut terminal, format!("A{query}B").as_bytes())
                .unwrap();
            let mut search = Search::default();
            search.push_text(query, &mut terminal);
            assert_eq!(search.count(), 1);
            let found = search.selected_match().unwrap();
            assert_eq!(found.start_column, 1);
            assert_eq!(found.end_column, if query.starts_with('e') { 2 } else { 3 });
        }
    }

    #[test]
    fn finds_text_across_soft_wrapped_rows() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
        terminal_core::TerminalParser::new()
            .advance(&mut terminal, b"abcneedle")
            .unwrap();
        let mut search = Search::default();
        search.push_text("needle", &mut terminal);
        assert_eq!(search.count(), 1);
        assert_eq!(
            search.viewport_matches(&terminal),
            vec![(0, 3, 5, true), (1, 0, 4, true)]
        );
        assert_eq!(search.markers(), vec![(0, true), (1, true)]);
        terminal.resize(TerminalDimensions::new(4, 3).unwrap());
        search.refresh(&mut terminal);
        assert_eq!(search.count(), 1);
    }

    #[test]
    fn does_not_match_across_hard_line_breaks() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(5, 3).unwrap());
        terminal_core::TerminalParser::new()
            .advance(&mut terminal, b"abcne\r\nedle")
            .unwrap();
        let mut search = Search::default();
        search.push_text("needle", &mut terminal);
        assert_eq!(search.count(), 0);
    }

    #[test]
    fn finds_wrapped_match_across_history_boundary() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(5, 2).unwrap());
        terminal_core::TerminalParser::new()
            .advance(&mut terminal, b"abcneedle123")
            .unwrap();
        assert_eq!(terminal.scrollback_len(), 1);
        let mut search = Search::default();
        search.push_text("needle", &mut terminal);
        assert_eq!(search.count(), 1);
        assert_eq!(search.selected_match().unwrap().row, 0);
        assert_eq!(search.selected_match().unwrap().end_row, 1);
        terminal.scroll_viewport_rows(-1);
        assert_eq!(search.viewport_match(&terminal), Some((0, 0)));
    }

    #[test]
    fn wrapped_search_preserves_non_overlapping_matches_and_unicode() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(3, 5).unwrap());
        terminal_core::TerminalParser::new()
            .advance(&mut terminal, "aaaaaaİxİx".as_bytes())
            .unwrap();
        let mut search = Search::default();
        search.push_text("aaaa", &mut terminal);
        assert_eq!(search.count(), 1);
        let mut search = Search::default();
        search.push_text("i\u{307}x", &mut terminal);
        assert_eq!(search.count(), 2);
    }

    #[test]
    fn wrapped_search_matches_literal_string_at_various_widths() {
        let text = "ababaabaababaabbababa";
        for width in 2..=8 {
            let mut terminal = TerminalState::new(TerminalDimensions::new(width, 2).unwrap());
            terminal_core::TerminalParser::new()
                .advance(&mut terminal, text.as_bytes())
                .unwrap();
            for query in ["a", "aba", "aabaa", "babaa", "missing"] {
                let mut search = Search::default();
                search.push_text(query, &mut terminal);
                let expected: Vec<_> = text
                    .match_indices(query)
                    .map(|(offset, _)| offset)
                    .collect();
                let actual: Vec<_> = search
                    .matches
                    .iter()
                    .map(|found| found.row * width + found.start_column)
                    .collect();
                assert_eq!(actual, expected, "width={width}, query={query}");
            }
        }
    }

    #[test]
    fn returns_every_visible_match_with_active_state_and_full_span() {
        let mut terminal = terminal();
        let mut search = Search::default();
        search.push_text("aLpHa", &mut terminal);
        search.navigate(&mut terminal, -1);
        assert_eq!(search.markers(), vec![(0, true), (2, false)]);
        assert_eq!(
            search.viewport_matches(&terminal),
            vec![(0, 0, 5, true), (2, 0, 5, false)]
        );
    }

    #[test]
    fn markers_merge_matches_on_one_row_and_keep_active_state() {
        let search = Search {
            matches: vec![
                Match {
                    row: 1,
                    start_column: 0,
                    end_column: 1,
                    end_row: 1,
                },
                Match {
                    row: 1,
                    start_column: 3,
                    end_column: 4,
                    end_row: 1,
                },
                Match {
                    row: 4,
                    start_column: 0,
                    end_column: 1,
                    end_row: 4,
                },
            ],
            selected: Some(1),
            ..Search::default()
        };
        assert_eq!(search.markers(), vec![(1, true), (4, false)]);
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

    #[test]
    fn searches_large_retained_history_after_scrolling_resize_and_screen_switch() {
        let mut terminal = TerminalState::new(TerminalDimensions::new(16, 3).unwrap());
        let mut parser = terminal_core::TerminalParser::new();
        parser
            .advance(&mut terminal, b"discarded\r\npad\r\npad\r\noldneedle\r\n")
            .unwrap();
        for _ in 0..terminal_core::MAX_SCROLLBACK_ROWS {
            parser.advance(&mut terminal, b"filler\r\n").unwrap();
        }
        parser.advance(&mut terminal, b"newneedle").unwrap();
        assert_eq!(
            terminal.scrollback_len(),
            terminal_core::MAX_SCROLLBACK_ROWS
        );

        let mut search = Search::default();
        search.push_text("discarded", &mut terminal);
        assert_eq!(search.count(), 0);
        search = Search::default();
        search.push_text("needle", &mut terminal);
        assert_eq!(search.count(), 2);
        assert!(search.viewport_match(&terminal).is_some());
        search.navigate(&mut terminal, -1);
        assert!(terminal.viewport_offset() > 9000);
        assert!(search.viewport_match(&terminal).is_some());
        assert!(search.viewport_matches(&terminal).iter().any(|m| m.3));
        terminal.scroll_viewport_rows(-20);
        search.refresh(&mut terminal);
        assert_eq!(search.count(), 2);
        assert!(search.viewport_match(&terminal).is_some());
        terminal.resize(TerminalDimensions::new(20, 4).unwrap());
        search.refresh(&mut terminal);
        assert_eq!(search.count(), 2);
        assert!(search.viewport_match(&terminal).is_some());
        terminal.switch_to_alternate_screen();
        search.refresh(&mut terminal);
        assert_eq!(search.count(), 0);
        terminal.switch_to_primary_screen();
        search.refresh(&mut terminal);
        assert_eq!(search.count(), 2);
    }
}
