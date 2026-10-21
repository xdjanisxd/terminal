use terminal_core::{CellColor, ScreenKind, TerminalDimensions, TerminalParser, TerminalState};

fn state(rows: usize, columns: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

fn feed(state: &mut TerminalState, bytes: &[u8]) {
    TerminalParser::new().advance(state, bytes).unwrap();
}

fn text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

fn assert_prefix(state: &TerminalState, row: usize, expected: &str) {
    assert_eq!(
        text(state, row)
            .chars()
            .take(expected.chars().count())
            .collect::<String>(),
        expected
    );
}

fn history_state() -> TerminalState {
    let mut terminal = state(3, 8);
    feed(&mut terminal, b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
    assert_eq!(terminal.scrollback_len(), 2);
    terminal
}

#[test]
fn dec_special_graphics_maps_the_full_range_and_leaves_unicode_unchanged_at_every_split() {
    let bytes = "\x1b(0^_`abcdefghijklmnopqrstuvwxyz{|}~é界e\u{301}\x1b(Bq".as_bytes();
    for split in 0..=bytes.len() {
        let mut terminal = state(3, 48);
        let mut parser = TerminalParser::new();
        parser.advance(&mut terminal, &bytes[..split]).unwrap();
        parser.advance(&mut terminal, &bytes[split..]).unwrap();
        assert_prefix(&terminal, 0, "^ ◆▒␉␌␍␊°±␤␋┘┐┌└┼⎺⎻─⎼⎽├┤┴┬│≤≥π≠£·é界");
        // DEC graphics maps the ASCII 'e', while UTF-8 scalars bypass translation.
        assert_eq!(terminal.screen().cell(0, 36).unwrap().character(), '␊');
        assert_eq!(
            terminal.screen().cell(0, 36).unwrap().grapheme_tail(),
            &['\u{301}']
        );
        assert_eq!(terminal.screen().cell(0, 37).unwrap().character(), 'q');
    }
}

#[test]
fn g0_g1_designation_and_si_so_invocation_are_independent_at_every_split() {
    let bytes = b"q\x0eq\x1b)0q\x0fq\x1b(0q\x0eq\x1b)Bq\x0fq\x1b(Bq";
    for split in 0..=bytes.len() {
        let mut terminal = state(2, 16);
        let mut parser = TerminalParser::new();
        parser.advance(&mut terminal, &bytes[..split]).unwrap();
        parser.advance(&mut terminal, &bytes[split..]).unwrap();
        assert_prefix(&terminal, 0, "qq─q──q─q");
    }
}

#[test]
fn unsupported_extra_intermediate_and_canceled_designations_preserve_active_mapping() {
    for invalid in [
        b"\x1b(A".as_slice(),
        b"\x1b(1",
        b"\x1b((B",
        b"\x1b(\x18",
        b"\x1b(\x1a",
        b"\x1b*B",
        b"\x1b(B\x18",
    ] {
        let mut terminal = state(2, 8);
        feed(&mut terminal, b"\x1b(0");
        // The last form is a completed ASCII designation followed by CAN.
        feed(&mut terminal, invalid);
        feed(&mut terminal, b"q");
        assert_prefix(
            &terminal,
            0,
            if invalid == b"\x1b(B\x18" { "q" } else { "─" },
        );
    }
}

#[test]
fn character_sets_are_terminal_global_but_each_screens_saved_slot_is_independent() {
    let mut terminal = state(3, 16);
    feed(
        &mut terminal,
        b"\x1b)0\x0e\x1b7\x1b[?47h\x1b)B\x1b7\x1b)0\x1b[?47lq",
    );
    assert_prefix(&terminal, 0, "─");
    feed(&mut terminal, b"\x1b)B\x1b8q");
    assert_prefix(&terminal, 0, "─");
    feed(&mut terminal, b"\x1b[?47h\x1b8q");
    assert_prefix(&terminal, 0, "q");
    feed(&mut terminal, b"\x1b[?47l\x1b8q");
    assert_prefix(&terminal, 0, "─");
}

#[test]
fn mode_1049_restores_primary_designation_and_invocation_even_after_resize() {
    let mut terminal = state(3, 16);
    feed(&mut terminal, b"\x1b)0\x0e\x1b[?1049h\x0f\x1b)Bq");
    terminal.resize(TerminalDimensions::new(12, 4).unwrap());
    feed(&mut terminal, b"\x1b[?1049lq");
    assert_eq!(terminal.active_screen(), ScreenKind::Primary);
    assert_prefix(&terminal, 0, "─");
}

#[test]
fn soft_and_hard_reset_restore_both_designations_invocation_and_saved_state() {
    for reset in [b"\x1b[!p".as_slice(), b"\x1bc"] {
        let mut terminal = state(3, 16);
        feed(&mut terminal, b"\x1b(0\x1b)0\x0e\x1b7");
        feed(&mut terminal, reset);
        feed(&mut terminal, b"q\x0eq\x0fq\x1b8q");
        assert_prefix(&terminal, 0, "qqq");
    }
}

#[test]
fn graphics_survive_wrap_scrollback_copy_and_reflow_as_unicode_cells() {
    let mut terminal = state(2, 4);
    feed(&mut terminal, b"\x1b(0lqqkx\x1b(B");
    assert_prefix(&terminal, 0, "┌──┐");
    assert_prefix(&terminal, 1, "│");
    assert!(terminal.begin_selection(0, 0));
    assert!(terminal.extend_selection(1, 0));
    assert_eq!(terminal.selected_text().as_deref(), Some("┌──┐│"));
    terminal.resize(TerminalDimensions::new(8, 2).unwrap());
    assert_prefix(&terminal, 0, "┌──┐│");
    assert_eq!(terminal.selected_text().as_deref(), Some("┌──┐│"));
    feed(&mut terminal, b"\r\n\r\n");
    assert_eq!(terminal.scrollback_row(0).unwrap()[0].character(), '┌');
}

#[test]
fn ed3_clears_history_and_scrolled_selection_but_preserves_live_grid_and_cursor_at_every_split() {
    for split in 0..=4 {
        let mut terminal = history_state();
        assert!(terminal.page_up());
        assert!(terminal.begin_selection(0, 0));
        assert!(terminal.extend_selection(0, 2));
        let live = terminal.screen().clone();
        let cursor = terminal.cursor();
        let mut parser = TerminalParser::new();
        parser.advance(&mut terminal, &b"\x1b[3J"[..split]).unwrap();
        parser.advance(&mut terminal, &b"\x1b[3J"[split..]).unwrap();
        assert_eq!(terminal.scrollback_len(), 0);
        assert_eq!(terminal.viewport_offset(), 0);
        assert_eq!(terminal.screen(), &live);
        assert_eq!(terminal.cursor(), cursor);
        assert_eq!(terminal.selected_text(), None);
        feed(&mut terminal, b"\x1b[3J");
        // Growing the viewport must not resurrect deleted rows.
        terminal.resize(TerminalDimensions::new(8, 5).unwrap());
        assert_eq!(terminal.scrollback_len(), 0);
        assert_prefix(&terminal, 0, "three");
    }
}

#[test]
fn ed3_preserves_live_selection_across_future_output_and_reflow() {
    let mut terminal = history_state();
    assert!(terminal.begin_selection(0, 0));
    assert!(terminal.extend_selection(0, 4));
    let selected = terminal.selected_text();
    assert_eq!(selected.as_deref(), Some("three"));
    feed(&mut terminal, b"\x1b[3J");
    assert_eq!(terminal.selected_text(), selected);
    feed(&mut terminal, b"\r\nsix");
    assert_eq!(terminal.selected_text(), selected);
    terminal.resize(TerminalDimensions::new(6, 4).unwrap());
    assert_eq!(terminal.selected_text(), selected);
}

#[test]
fn ed3_drops_selections_spanning_removed_history_and_live_rows() {
    let mut terminal = history_state();
    terminal.page_up();
    terminal.begin_selection(0, 0);
    terminal.extend_selection(2, 2);
    assert!(terminal.selected_text().is_some());
    feed(&mut terminal, b"\x1b[3J");
    assert_eq!(terminal.selected_text(), None);
}

#[test]
fn ed3_preserves_rendition_pending_wrap_saved_cursor_and_reply_fifo() {
    let mut terminal = history_state();
    feed(&mut terminal, b"\x1b[31m\x1b[1;8HZ\x1b7\x1b[6n\x1b[3JX");
    assert_eq!(terminal.screen().cell(1, 0).unwrap().character(), 'X');
    assert_eq!(
        terminal
            .screen()
            .cell(1, 0)
            .unwrap()
            .attributes()
            .foreground(),
        CellColor::Indexed(1)
    );
    assert_eq!(terminal.pending_reply_count(), 1);
    feed(&mut terminal, b"\x1b[32m\x1b8Y");
    assert_eq!(terminal.screen().cell(0, 7).unwrap().character(), 'Y');
    assert_eq!(
        terminal
            .screen()
            .cell(0, 7)
            .unwrap()
            .attributes()
            .foreground(),
        CellColor::Indexed(1)
    );
}

#[test]
fn ed3_is_primary_only_and_rejects_private_malformed_extra_and_canceled_parameters() {
    let mut terminal = history_state();
    let history = terminal.scrollback_len();
    feed(&mut terminal, b"\x1b[?1049h\x1b[3J\x1b[?1049l");
    assert_eq!(terminal.scrollback_len(), history);
    for malformed in [
        b"\x1b[?3J".as_slice(),
        b"\x1b[3;0J",
        b"\x1b[3:1J",
        b"\x1b[3 J",
        b"\x1b[3\x18J",
        b"\x1b[65535J",
    ] {
        feed(&mut terminal, malformed);
        assert_eq!(terminal.scrollback_len(), history);
    }
}

#[test]
fn vt_and_form_feed_match_lf_on_both_screens_at_margins_one_row_and_pending_wrap() {
    for rows in [1, 4] {
        for alternate in [false, true] {
            for control in *b"\x0b\x0c" {
                for setup in [
                    b"abcd".as_slice(),
                    b"A\r\nB\r\nC\r\nD\x1b[2;4r\x1b[4;3H",
                    b"\x1b[1;3H",
                ] {
                    let mut actual = state(rows, 4);
                    let mut expected = state(rows, 4);
                    for terminal in [&mut actual, &mut expected] {
                        if alternate {
                            feed(terminal, b"\x1b[?1049h");
                        }
                        feed(terminal, setup);
                    }
                    feed(&mut actual, &[control, b'X']);
                    feed(&mut expected, b"\nX");
                    assert_eq!(actual.screen(), expected.screen());
                    assert_eq!(actual.cursor(), expected.cursor());
                    assert_eq!(actual.scrollback_len(), expected.scrollback_len());
                    for row in 0..actual.scrollback_len() {
                        assert_eq!(actual.scrollback_row(row), expected.scrollback_row(row));
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires installed xterm-256color terminfo, tput, and ncurses clear"]
fn native_linux_terminfo_acs_and_clear_replay() {
    fn output(program: &str, arguments: &[&str]) -> Vec<u8> {
        let output = std::process::Command::new(program)
            .args(arguments)
            .env("TERM", "xterm-256color")
            .output()
            .unwrap();
        assert!(output.status.success(), "{program}: {:?}", output.stderr);
        output.stdout
    }
    let mut terminal = history_state();
    feed(&mut terminal, b"\x1b[H");
    let mut bytes = output("tput", &["smacs"]);
    bytes.extend_from_slice(b"lqqkx");
    bytes.extend(output("tput", &["rmacs"]));
    bytes.push(b'q');
    let mut parser = TerminalParser::new();
    for chunk in bytes.chunks(2) {
        parser.advance(&mut terminal, chunk).unwrap();
    }
    assert_prefix(&terminal, 0, "┌──┐│q");
    let clear = output("clear", &[]);
    assert!(clear.windows(4).any(|part| part == b"\x1b[3J"));
    parser.advance(&mut terminal, &clear).unwrap();
    assert_eq!(terminal.scrollback_len(), 0);
    assert_eq!(terminal.viewport_offset(), 0);
    assert_prefix(&terminal, 0, "        ");
}
