use terminal_core::{
    AutoWrapMode, Cell, CellColor, CellOccupancy, MAX_GRAPHEME_SCALARS, MAX_SCROLLBACK_ROWS,
    PrintError, TerminalDimensions, TerminalParser, TerminalState,
};
use unicode_width::UnicodeWidthChar;

fn state(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn feed(state: &mut TerminalState, parser: &mut TerminalParser, text: &str) {
    parser.advance(state, text.as_bytes()).unwrap();
}

fn row_text(state: &TerminalState, row: usize) -> String {
    let mut result = String::new();
    for column in 0..state.dimensions().columns() {
        let cell = state.screen().cell(row, column).unwrap();
        if cell.occupancy() != CellOccupancy::WideContinuation {
            result.push(cell.character());
            result.extend(cell.grapheme_tail());
        }
    }
    result
}

fn assert_wide_invariants(state: &TerminalState) {
    for row in 0..state.dimensions().rows() {
        for column in 0..state.dimensions().columns() {
            let cell = state.screen().cell(row, column).unwrap();
            match cell.occupancy() {
                CellOccupancy::Single => {}
                CellOccupancy::WideLead => assert_eq!(
                    state.screen().cell(row, column + 1).unwrap().occupancy(),
                    CellOccupancy::WideContinuation
                ),
                CellOccupancy::WideContinuation => {
                    assert!(column > 0);
                    assert_eq!(
                        state.screen().cell(row, column - 1).unwrap().occupancy(),
                        CellOccupancy::WideLead
                    );
                    assert!(cell.grapheme_tail().is_empty());
                }
            }
        }
    }
}

fn history_contains_cluster(state: &TerminalState, cluster: &str) -> bool {
    (0..state.scrollback_len()).any(|row| {
        state.scrollback_row(row).unwrap().iter().any(|cell| {
            if cell.character() != cluster.chars().next().unwrap() {
                return false;
            }
            let mut actual = String::new();
            actual.push(cell.character());
            actual.extend(cell.grapheme_tail());
            actual == cluster
        })
    })
}

fn assert_cluster(state: &TerminalState, row: usize, column: usize, expected: &str, width: usize) {
    let cell = state.screen().cell(row, column).unwrap();
    let mut actual = String::new();
    actual.push(cell.character());
    actual.extend(cell.grapheme_tail());
    assert_eq!(actual, expected, "row={row}, column={column}");
    assert_eq!(
        cell.occupancy(),
        if width == 2 {
            CellOccupancy::WideLead
        } else {
            CellOccupancy::Single
        },
        "actual={actual:?}, row={row}, column={column}"
    );
}

#[test]
fn extended_graphemes_keep_all_suffix_scalars_and_expected_cell_widths() {
    let fixtures = [
        ("e\u{301}", 1),
        ("✈\u{fe0e}", 1),
        ("✈\u{fe0f}", 2),
        ("👍🏽", 2),
        ("🇺🇸", 2),
        ("1️⃣", 2),
        ("👩\u{200d}💻", 2),
        ("👨\u{200d}👩\u{200d}👧\u{200d}👦", 2),
    ];
    for (cluster, width) in fixtures {
        let mut state = state(12, 2);
        let mut parser = TerminalParser::new();
        feed(&mut state, &mut parser, cluster);
        assert_cluster(&state, 0, 0, cluster, width);
        if width == 2 {
            assert_wide_invariants(&state);
        }
    }
}

#[test]
fn grapheme_boundaries_survive_every_utf8_chunk_split() {
    let fixtures = [
        "e\u{301}",
        "👍🏽",
        "🇺🇸",
        "1️⃣",
        "👩\u{200d}💻",
        "👨\u{200d}👩\u{200d}👧\u{200d}👦",
    ];
    for fixture in fixtures {
        let bytes = fixture.as_bytes();
        for split in 1..bytes.len() {
            let mut state = state(16, 2);
            let mut parser = TerminalParser::new();
            parser.advance(&mut state, &bytes[..split]).unwrap();
            parser.advance(&mut state, &bytes[split..]).unwrap();
            let mut actual = String::new();
            let cell = state.screen().cell(0, 0).unwrap();
            actual.push(cell.character());
            actual.extend(cell.grapheme_tail());
            assert_eq!(actual, fixture, "split={split}, fixture={fixture:?}");
        }
    }
}

#[test]
fn width_growth_at_margin_wraps_the_complete_cluster() {
    let mut terminal = state(3, 2);
    let mut parser = TerminalParser::new();
    feed(&mut terminal, &mut parser, "ab✈\u{fe0f}");
    assert_eq!(terminal.screen().cell(0, 2).unwrap(), &Cell::default());
    assert_cluster(&terminal, 1, 0, "✈\u{fe0f}", 2);
    assert_wide_invariants(&terminal);

    let mut no_wrap = state(3, 2);
    no_wrap.set_auto_wrap(AutoWrapMode::Disabled);
    let mut parser = TerminalParser::new();
    feed(&mut no_wrap, &mut parser, "ab✈");
    let before = *no_wrap.screen().cell(0, 2).unwrap();
    let result = parser.advance(&mut no_wrap, "\u{fe0f}".as_bytes());
    assert!(result.is_err());
    assert_eq!(no_wrap.screen().cell(0, 2).unwrap(), &before);
    assert!(
        no_wrap
            .screen()
            .cell(0, 2)
            .unwrap()
            .grapheme_tail()
            .is_empty()
    );
    assert_wide_invariants(&no_wrap);
}

#[test]
fn variation_selector_width_shrink_restores_last_column_cursor_state() {
    let mut terminal = state(3, 2);
    let mut parser = TerminalParser::new();
    feed(&mut terminal, &mut parser, "ab⌚\u{fe0e}");
    assert_cluster(&terminal, 1, 0, "⌚\u{fe0e}", 1);
    assert_eq!(
        (terminal.cursor().row(), terminal.cursor().column()),
        (1, 1)
    );
    assert_wide_invariants(&terminal);
}

#[test]
fn insert_mode_width_changes_preserve_adjacent_text() {
    for (cluster, width, b_column) in [("⌚\u{fe0f}", 2, 3), ("⌚\u{fe0e}", 1, 2)] {
        let mut terminal = state(8, 2);
        let mut parser = TerminalParser::new();
        feed(&mut terminal, &mut parser, &format!("\x1b[4hA{cluster}B"));
        assert_cluster(&terminal, 0, 1, cluster, width);
        assert_eq!(terminal.screen().cell(0, 0).unwrap().character(), 'A');
        assert_eq!(
            terminal.screen().cell(0, b_column).unwrap().character(),
            'B'
        );
        assert_wide_invariants(&terminal);
    }
}

#[test]
fn edits_and_selection_preserve_cluster_integrity() {
    let mut state = state(12, 2);
    let mut parser = TerminalParser::new();
    feed(&mut state, &mut parser, "👩\u{200d}💻B");
    state.set_cursor_position(0, 1).unwrap();
    state.extend_keyboard_selection(true, false);
    assert_eq!(state.selected_text().as_deref(), Some("👩\u{200d}💻"));
    state.set_cursor_position(0, 0).unwrap();
    state.delete_characters(2);
    assert_eq!(row_text(&state, 0).trim_end(), "B");
    assert_wide_invariants(&state);
}

#[test]
fn cursor_edits_and_screen_switches_do_not_join_old_positive_width_anchors() {
    let mut terminal = state(12, 2);
    let mut parser = TerminalParser::new();
    feed(&mut terminal, &mut parser, "👩");
    terminal.set_cursor_position(1, 4).unwrap();
    feed(&mut terminal, &mut parser, "💻");
    assert_cluster(&terminal, 0, 0, "👩", 2);
    assert_cluster(&terminal, 1, 4, "💻", 2);

    terminal.set_cursor_position(1, 8).unwrap();
    feed(&mut terminal, &mut parser, "🇺");
    terminal.switch_to_alternate_screen();
    feed(&mut terminal, &mut parser, "🇸");
    terminal.switch_to_primary_screen();
    feed(&mut terminal, &mut parser, "🇸");
    assert_cluster(&terminal, 1, 8, "🇺", 1);
    assert_cluster(&terminal, 1, 9, "🇸", 1);
    assert_cluster(&terminal, 0, 0, "👩", 2);
    assert_wide_invariants(&terminal);

    let mut moved = state(12, 2);
    let mut parser = TerminalParser::new();
    moved.set_cursor_position(0, 6).unwrap();
    feed(&mut moved, &mut parser, "👩\u{200d}");
    moved.set_cursor_position(1, 4).unwrap();
    feed(&mut moved, &mut parser, "💻");
    assert_cluster(&moved, 0, 6, "👩\u{200d}", 2);
    assert_cluster(&moved, 1, 4, "💻", 2);
    assert_wide_invariants(&moved);
}

#[test]
fn rejected_or_unchanged_state_operations_preserve_open_grapheme_anchor() {
    let noops: [fn(&mut TerminalState); 3] = [
        |terminal| assert!(terminal.set_cursor_position(usize::MAX, 0).is_err()),
        |terminal| assert!(!terminal.set_vertical_scrolling_margins(1, 0)),
        |terminal| terminal.resize(TerminalDimensions::new(12, 2).unwrap()),
    ];
    for noop in noops {
        let mut terminal = state(12, 2);
        let mut parser = TerminalParser::new();
        feed(&mut terminal, &mut parser, "👩\u{200d}");
        noop(&mut terminal);
        feed(&mut terminal, &mut parser, "💻");
        assert_cluster(&terminal, 0, 0, "👩\u{200d}💻", 2);
        assert_wide_invariants(&terminal);
    }
}

#[test]
fn sgr_inside_cluster_preserves_first_base_rendition() {
    let mut terminal = state(8, 2);
    let mut parser = TerminalParser::new();
    feed(&mut terminal, &mut parser, "✈\x1b[31m\u{fe0f}");
    let cell = terminal.screen().cell(0, 0).unwrap();
    assert_cluster(&terminal, 0, 0, "✈\u{fe0f}", 2);
    assert_eq!(cell.attributes().foreground(), CellColor::Default);
    assert_eq!(
        terminal.screen().cell(0, 1).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
}

#[test]
fn reflow_and_history_eviction_keep_clusters_whole_and_bounded() {
    let mut state = state(12, 2);
    let mut parser = TerminalParser::new();
    let cluster = "👨\u{200d}👩\u{200d}👧\u{200d}👦";
    feed(&mut state, &mut parser, cluster);
    state.set_cursor_position(0, 1).unwrap();
    state.extend_keyboard_selection(true, false);
    assert_eq!(state.selected_text().as_deref(), Some(cluster));
    state.resize(TerminalDimensions::new(1, 2).unwrap());
    assert_eq!(state.selected_text().as_deref(), Some(cluster));
    assert_wide_invariants(&state);
    state.resize(TerminalDimensions::new(12, 2).unwrap());
    assert_eq!(state.selected_text().as_deref(), Some(cluster));
    assert_wide_invariants(&state);
    feed(&mut state, &mut parser, "\r\nnext\r\nthird\r\nfourth");
    assert!(state.scrollback_len() > 0);
    assert!(history_contains_cluster(&state, cluster));
    feed(
        &mut state,
        &mut parser,
        &"line\r\n".repeat(MAX_SCROLLBACK_ROWS + 2),
    );
    assert!(state.scrollback_len() <= MAX_SCROLLBACK_ROWS);
    assert!(!history_contains_cluster(&state, cluster));
    for row in 0..state.scrollback_len() {
        let cells = state.scrollback_row(row).unwrap();
        for (column, cell) in cells.iter().enumerate() {
            if cell.occupancy() == CellOccupancy::WideLead {
                assert_eq!(
                    cells[column + 1].occupancy(),
                    CellOccupancy::WideContinuation
                );
            }
        }
    }
}

#[test]
fn oversized_cluster_reports_legacy_overflow_and_parser_resumes() {
    let mut state = state(8, 2);
    let mut parser = TerminalParser::new();
    let oversized = format!("a{}b", "\u{301}".repeat(MAX_GRAPHEME_SCALARS));
    let error = parser
        .advance(&mut state, oversized.as_bytes())
        .unwrap_err();
    assert!(error.bytes_consumed() <= oversized.len());
    assert!(matches!(
        error.semantic_error(),
        PrintError::CombiningMarkOverflow(_)
    ));
    parser.advance(&mut state, b"z").unwrap();
    assert!(row_text(&state, 0).contains('z'));
    assert_wide_invariants(&state);
}

#[test]
fn interrupted_incomplete_utf8_after_zwj_recovers_and_keeps_later_ascii() {
    let mut terminal = state(12, 2);
    let mut parser = TerminalParser::new();
    parser
        .advance(&mut terminal, "👩\u{200d}".as_bytes())
        .unwrap();
    parser.advance(&mut terminal, &[0xf0, 0x9f]).unwrap();
    let interrupted = parser.advance(&mut terminal, b"x").unwrap_err();
    assert_eq!(interrupted.bytes_consumed(), 0);
    parser
        .advance(&mut terminal, &b"x"[interrupted.bytes_consumed()..])
        .unwrap();
    assert!(row_text(&terminal, 0).contains('x'));
    assert_wide_invariants(&terminal);
}

#[test]
fn single_column_input_keeps_zwj_cluster_when_widened() {
    let mut terminal = state(1, 2);
    let mut parser = TerminalParser::new();
    let cluster = "👩\u{200d}💻";
    feed(&mut terminal, &mut parser, cluster);
    terminal.resize(TerminalDimensions::new(8, 2).unwrap());
    assert_cluster(&terminal, 0, 0, cluster, 2);
    assert_wide_invariants(&terminal);
}

#[test]
fn partial_wide_delete_and_erase_never_leave_orphan_continuations() {
    for erase in [false, true] {
        let mut terminal = state(8, 2);
        let mut parser = TerminalParser::new();
        feed(&mut terminal, &mut parser, "A界B");
        terminal.set_cursor_position(0, 2).unwrap();
        if erase {
            terminal.erase_characters(1);
        } else {
            terminal.delete_characters(1);
        }
        assert_wide_invariants(&terminal);
    }
}

#[test]
fn representative_shell_and_tui_unicode_fixtures_remain_copyable() {
    for fixture in [
        "$ echo 'cafe\u{301} 👩\u{200d}💻 🇺🇳'",
        "界 e\u{301} 👍🏽 1️⃣",
        "👨\u{200d}👩\u{200d}👧\u{200d}👦 ok",
    ] {
        let mut state = state(24, 3);
        let mut parser = TerminalParser::new();
        feed(&mut state, &mut parser, fixture);
        let last_column = fixture
            .chars()
            .map(|character| UnicodeWidthChar::width(character).unwrap_or(0))
            .sum::<usize>()
            - 1;
        state.begin_selection(0, 0);
        state.extend_selection(0, last_column);
        assert_eq!(state.selected_text().as_deref(), Some(fixture));
        assert_wide_invariants(&state);
    }
}
