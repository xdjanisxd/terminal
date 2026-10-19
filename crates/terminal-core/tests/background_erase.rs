use terminal_core::{
    CellAttributes, CellColor, CellOccupancy, TerminalDimensions, TerminalParser, TerminalState,
};

fn parse(bytes: &[u8]) -> TerminalState {
    let mut state = TerminalState::new(TerminalDimensions::new(8, 4).unwrap());
    TerminalParser::new().advance(&mut state, bytes).unwrap();
    state
}

#[test]
fn erased_and_exposed_cells_use_current_background_without_text_styles() {
    // Expected coordinate in each operation's newly blank region.
    let operations: &[(&[u8], usize, usize)] = &[
        (b"\x1b[K", 1, 7),
        (b"\x1b[1K", 1, 0),
        (b"\x1b[2K", 1, 7),
        (b"\x1b[J", 3, 7),
        (b"\x1b[1J", 0, 0),
        (b"\x1b[2J", 0, 0),
        (b"\x1b[2X", 1, 2),
        (b"\x1b[2@", 1, 2),
        (b"\x1b[2P", 1, 7),
        (b"\x1b[L", 1, 7),
        (b"\x1b[M", 3, 7),
        (b"\x1b[S", 3, 7),
        (b"\x1b[T", 0, 7),
        (b"\x1b[4;1H\n", 3, 7),
        (b"\x1b[4;1H\x1bD", 3, 7),
        (b"\x1b[4;1H\x1bE", 3, 7),
        (b"\x1b[1;1H\x1bM", 0, 7),
        (b"\x1b[4;8HXY", 3, 7),
        (b"\x1b[99S", 0, 0),
        (b"\x1b[99T", 3, 7),
    ];
    for (sgr, background) in [
        ("49", CellColor::Default),
        ("44", CellColor::Indexed(4)),
        ("48;5;200", CellColor::Indexed(200)),
        (
            "48;2;32;32;32",
            CellColor::Rgb {
                red: 32,
                green: 32,
                blue: 32,
            },
        ),
    ] {
        for &(operation, row, column) in operations {
            for alternate in [false, true] {
                let mut bytes = if alternate {
                    b"\x1b[?1049h".to_vec()
                } else {
                    Vec::new()
                };
                bytes.extend_from_slice(
                    format!("\x1b[1;3;4;7;38;2;120;169;255;{sgr}m\x1b[2;3H").as_bytes(),
                );
                bytes.extend_from_slice(operation);
                let state = parse(&bytes);
                let cell = state.screen().cell(row, column).unwrap();
                assert_eq!(cell.character(), ' ', "{sgr} {operation:?}");
                assert_eq!(
                    *cell.attributes(),
                    CellAttributes::new(CellColor::Default, background),
                    "{sgr} {operation:?} alternate={alternate}"
                );
                assert_eq!(cell.occupancy(), CellOccupancy::Single);
                assert!(cell.combining_marks().is_empty());
                assert!(cell.target_id().is_none());
            }
        }
    }
}

#[test]
fn background_reset_and_full_rendition_reset_restore_default_erasure() {
    for reset in [b"\x1b[49m".as_slice(), b"\x1b[0m"] {
        let mut bytes = b"\x1b[48;2;57;57;57m\x1b[2J".to_vec();
        bytes.extend_from_slice(reset);
        bytes.extend_from_slice(b"\x1b[2;3H\x1b[K");
        let state = parse(&bytes);
        assert_eq!(
            state.screen().cell(1, 2).unwrap().attributes().background(),
            CellColor::Default
        );
        assert_eq!(
            state.screen().cell(1, 1).unwrap().attributes().background(),
            CellColor::Rgb {
                red: 57,
                green: 57,
                blue: 57
            }
        );
    }
}

#[test]
fn partial_wide_erasure_uses_background_and_removes_target_and_combining_marks() {
    let state = parse("\x1b]8;;https://example.com\x1b\\界e\u{301}\x1b]8;;\x1b\\\x1b[48;2;68;68;68m\x1b[1;2H\x1b[2X".as_bytes());
    for column in 0..3 {
        let cell = state.screen().cell(0, column).unwrap();
        assert_eq!(
            *cell.attributes(),
            CellAttributes::new(
                CellColor::Default,
                CellColor::Rgb {
                    red: 68,
                    green: 68,
                    blue: 68
                }
            )
        );
        assert_eq!(cell.occupancy(), CellOccupancy::Single);
        assert!(cell.target_id().is_none());
        assert!(cell.combining_marks().is_empty());
    }
}

#[test]
fn wide_character_edits_fill_orphaned_halves_with_the_current_background() {
    for operation in ["\x1b[@", "\x1b[P", "X", "界"] {
        let state = parse(format!("界abcdef\x1b[48;5;200m\x1b[1;2H{operation}").as_bytes());
        let cell = state.screen().cell(0, 0).unwrap();
        assert_eq!(cell.character(), ' ');
        assert_eq!(cell.attributes().background(), CellColor::Indexed(200));
        assert_eq!(cell.occupancy(), CellOccupancy::Single);
    }
}

#[test]
fn scrolling_keeps_original_attributes_in_history_and_fills_only_the_exposed_row() {
    let state = parse(b"\x1b[48;5;4mTOP\x1b[48;5;200m\x1b[4;1H\n");
    assert_eq!(state.scrollback_len(), 1);
    let cell = &state.scrollback_row(0).unwrap()[0];
    assert_eq!(cell.character(), 'T');
    assert_eq!(cell.attributes().background(), CellColor::Indexed(4));
    assert_eq!(
        state.screen().cell(3, 0).unwrap().attributes().background(),
        CellColor::Indexed(200)
    );
}
