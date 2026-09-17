use terminal_core::{
    AutoWrapMode, CellOccupancy, CharacterInsertionMode, CursorKeyMode, CursorVisibility,
    ScreenKind, TerminalDimensions, TerminalState, VerticalScrollingMargins,
};

const ACUTE: char = '\u{0301}';

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(6, 3).unwrap())
}

fn row_text(state: &TerminalState, row: usize) -> String {
    (0..state.dimensions().columns())
        .map(|column| state.screen().cell(row, column).unwrap().character())
        .collect()
}

#[test]
fn starts_primary_and_switches_between_independent_contents_and_cursors() {
    let mut state = state();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    state.print_character('p').unwrap();
    state.set_cursor_position(2, 4).unwrap();

    state.switch_to_alternate_screen();
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(row_text(&state, 0), "      ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (0, 0));
    state.print_character('a').unwrap();
    state.set_cursor_position(1, 2).unwrap();

    state.switch_to_primary_screen();
    assert_eq!(row_text(&state, 0), "p     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (2, 4));
    state.switch_to_alternate_screen();
    assert_eq!(row_text(&state, 0), "a     ");
    assert_eq!((state.cursor().row(), state.cursor().column()), (1, 2));
}

#[test]
fn margins_and_pending_wrap_are_screen_local() {
    let mut state = state();
    assert!(state.set_vertical_scrolling_margins(1, 2));
    state.set_cursor_position(0, 5).unwrap();
    state.print_character('p').unwrap();

    state.switch_to_alternate_screen();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::full_screen(3)
    );
    state.print_character('a').unwrap();
    assert_eq!(row_text(&state, 0), "a     ");
    assert!(state.set_vertical_scrolling_margins(0, 1));

    state.switch_to_primary_screen();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(1, 2, 3).unwrap()
    );
    state.print_character('x').unwrap();
    assert_eq!(state.screen().cell(1, 0).unwrap().character(), 'x');
    state.switch_to_alternate_screen();
    assert_eq!(
        state.vertical_scrolling_margins(),
        VerticalScrollingMargins::new(0, 1, 3).unwrap()
    );
}

#[test]
fn modes_rendition_tabs_and_replies_are_shared_across_switching() {
    let mut state = state();
    state.set_character_insertion(CharacterInsertionMode::Insert);
    state.set_auto_wrap(AutoWrapMode::Disabled);
    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_cursor_position(0, 3).unwrap();
    state.set_horizontal_tab_stop();
    assert!(state.request_primary_device_attributes());

    state.switch_to_alternate_screen();
    assert_eq!(
        state.terminal_modes().character_insertion(),
        CharacterInsertionMode::Insert
    );
    assert_eq!(state.terminal_modes().auto_wrap(), AutoWrapMode::Disabled);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
    assert!(state.has_horizontal_tab_stop(3));
    assert_eq!(state.pending_reply_count(), 1);
}

#[test]
fn wide_combining_primary_payload_survives_round_trip_and_resize_updates_both_screens() {
    let mut state = state();
    state.print_character('界').unwrap();
    state.print_character(ACUTE).unwrap();
    state.switch_to_alternate_screen();
    state.print_character('a').unwrap();
    state.resize(TerminalDimensions::new(4, 2).unwrap());
    assert_eq!(state.active_screen(), ScreenKind::Alternate);
    assert_eq!(state.dimensions(), TerminalDimensions::new(4, 2).unwrap());

    state.switch_to_primary_screen();
    assert_eq!(state.dimensions(), TerminalDimensions::new(4, 2).unwrap());
    assert_eq!(
        state.screen().cell(0, 0).unwrap().occupancy(),
        CellOccupancy::WideLead
    );
    assert_eq!(
        state.screen().cell(0, 0).unwrap().combining_marks(),
        [ACUTE]
    );
    assert_eq!(
        state.screen().cell(0, 1).unwrap().occupancy(),
        CellOccupancy::WideContinuation
    );
}

#[test]
fn switching_is_idempotent_and_reset_blanks_both_screens_on_primary() {
    let mut state = state();
    state.print_character('p').unwrap();
    state.switch_to_primary_screen();
    assert_eq!(row_text(&state, 0), "p     ");
    state.switch_to_alternate_screen();
    state.print_character('a').unwrap();
    state.switch_to_alternate_screen();
    assert_eq!(row_text(&state, 0), "a     ");

    state.reset();
    assert_eq!(state.active_screen(), ScreenKind::Primary);
    assert_eq!(row_text(&state, 0), "      ");
    state.switch_to_alternate_screen();
    assert_eq!(row_text(&state, 0), "      ");
}
