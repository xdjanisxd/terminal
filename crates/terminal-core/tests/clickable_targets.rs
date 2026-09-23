use terminal_core::{TerminalDimensions, TerminalParser, TerminalState};

fn terminal(columns: usize, rows: usize) -> TerminalState {
    TerminalState::new(TerminalDimensions::new(columns, rows).unwrap())
}

#[test]
fn osc8_spans_are_cell_metadata_and_plain_text_is_not_inferred() {
    let mut parser = TerminalParser::new();
    let mut state = terminal(32, 2);
    parser
        .advance(&mut state, b"https://plain.example ")
        .unwrap();
    assert_eq!(state.target_at_viewport(0, 0), None);

    parser
        .advance(&mut state, b"\x1b]8;id=one;https://example.org/a;b\x1b\\A")
        .unwrap();
    assert_eq!(
        state.target_at_viewport(0, 22),
        Some("https://example.org/a;b")
    );
    parser.advance(&mut state, b"\x1b]8;;\x07B").unwrap();
    assert_eq!(state.target_at_viewport(0, 23), None);
}

#[test]
fn osc8_chunking_wide_cells_and_overwrite_follow_visible_cells() {
    let mut parser = TerminalParser::new();
    let mut state = terminal(6, 2);
    parser
        .advance(&mut state, b"\x1b]8;;https://example.org\x1b")
        .unwrap();
    parser.advance(&mut state, b"\\").unwrap();
    parser.advance(&mut state, "界".as_bytes()).unwrap();
    assert_eq!(state.target_at_viewport(0, 0), Some("https://example.org"));
    assert_eq!(state.target_at_viewport(0, 1), Some("https://example.org"));
    parser.advance(&mut state, b"\x1b]8;;\x1b\\\rZ").unwrap();
    assert_eq!(state.target_at_viewport(0, 0), None);
    assert_eq!(state.target_at_viewport(0, 1), None);
}

#[test]
fn primary_history_and_alternate_screen_keep_separate_targets() {
    let mut parser = TerminalParser::new();
    let mut state = terminal(4, 2);
    parser
        .advance(
            &mut state,
            b"\x1b]8;;https://primary.example\x07P\x1b]8;;\x07\r\n\r\n",
        )
        .unwrap();
    assert_eq!(state.target_at_viewport(0, 0), None);
    assert!(state.page_up());
    assert_eq!(
        state.target_at_viewport(0, 0),
        Some("https://primary.example")
    );

    parser
        .advance(
            &mut state,
            b"\x1b[?1047h\x1b]8;;https://alt.example\x07A\x1b]8;;\x07",
        )
        .unwrap();
    assert_eq!(state.target_at_viewport(0, 0), Some("https://alt.example"));
    assert!(!state.page_up());
    parser.advance(&mut state, b"\x1b[?1047l").unwrap();
    assert_eq!(
        state.target_at_viewport(0, 0),
        Some("https://primary.example")
    );
    parser.advance(&mut state, b"\x1b[?1047h").unwrap();
    assert_eq!(state.target_at_viewport(0, 0), None);

    parser
        .advance(
            &mut state,
            b"\x1b]8;;https://retained.example\x07R\x1b]8;;\x07\x1b[?47l\x1b[?47h",
        )
        .unwrap();
    assert_eq!(
        state.target_at_viewport(0, 0),
        Some("https://retained.example")
    );
}
