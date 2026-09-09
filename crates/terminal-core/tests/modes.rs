use terminal_core::{
    AutoWrapMode, CharacterInsertionMode, CursorKeyMode, CursorVisibility, InputModes,
    TerminalModes,
};

#[test]
fn terminal_modes_use_standard_foundational_defaults() {
    let modes = TerminalModes::default();

    assert_eq!(modes.cursor_visibility(), CursorVisibility::Visible);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Replace);
}

#[test]
fn cursor_visibility_transitions_are_explicit_and_idempotent() {
    let mut modes = TerminalModes::default();

    modes.set_cursor_visibility(CursorVisibility::Hidden);
    modes.set_cursor_visibility(CursorVisibility::Hidden);
    assert_eq!(modes.cursor_visibility(), CursorVisibility::Hidden);

    modes.set_cursor_visibility(CursorVisibility::Visible);
    modes.set_cursor_visibility(CursorVisibility::Visible);
    assert_eq!(modes.cursor_visibility(), CursorVisibility::Visible);
}

#[test]
fn auto_wrap_transitions_are_explicit_and_idempotent() {
    let mut modes = TerminalModes::default();

    modes.set_auto_wrap(AutoWrapMode::Disabled);
    modes.set_auto_wrap(AutoWrapMode::Disabled);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Disabled);

    modes.set_auto_wrap(AutoWrapMode::Enabled);
    modes.set_auto_wrap(AutoWrapMode::Enabled);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Enabled);
}

#[test]
fn character_insertion_transitions_are_explicit_and_idempotent() {
    let mut modes = TerminalModes::default();

    modes.set_character_insertion(CharacterInsertionMode::Insert);
    modes.set_character_insertion(CharacterInsertionMode::Insert);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Insert);

    modes.set_character_insertion(CharacterInsertionMode::Replace);
    modes.set_character_insertion(CharacterInsertionMode::Replace);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Replace);
}

#[test]
fn input_modes_default_to_normal_cursor_keys() {
    let modes = InputModes::default();

    assert_eq!(modes.cursor_keys(), CursorKeyMode::Normal);
}

#[test]
fn cursor_key_transitions_are_explicit_and_idempotent() {
    let mut modes = InputModes::default();

    modes.set_cursor_keys(CursorKeyMode::Application);
    modes.set_cursor_keys(CursorKeyMode::Application);
    assert_eq!(modes.cursor_keys(), CursorKeyMode::Application);

    modes.set_cursor_keys(CursorKeyMode::Normal);
    modes.set_cursor_keys(CursorKeyMode::Normal);
    assert_eq!(modes.cursor_keys(), CursorKeyMode::Normal);
}

#[test]
fn changing_one_terminal_mode_does_not_change_unrelated_modes() {
    let mut modes = TerminalModes::default();

    modes.set_auto_wrap(AutoWrapMode::Disabled);

    assert_eq!(modes.cursor_visibility(), CursorVisibility::Visible);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Disabled);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Replace);
}

#[test]
fn changing_cursor_visibility_does_not_change_unrelated_modes() {
    let mut modes = TerminalModes::default();

    modes.set_cursor_visibility(CursorVisibility::Hidden);

    assert_eq!(modes.cursor_visibility(), CursorVisibility::Hidden);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Replace);
}

#[test]
fn changing_character_insertion_does_not_change_unrelated_modes() {
    let mut modes = TerminalModes::default();

    modes.set_character_insertion(CharacterInsertionMode::Insert);

    assert_eq!(modes.cursor_visibility(), CursorVisibility::Visible);
    assert_eq!(modes.auto_wrap(), AutoWrapMode::Enabled);
    assert_eq!(modes.character_insertion(), CharacterInsertionMode::Insert);
}

#[test]
fn input_modes_are_independent_from_terminal_modes() {
    let terminal = TerminalModes::default();
    let mut input = InputModes::default();

    input.set_cursor_keys(CursorKeyMode::Application);

    assert_eq!(input.cursor_keys(), CursorKeyMode::Application);
    assert_eq!(terminal.cursor_visibility(), CursorVisibility::Visible);
    assert_eq!(terminal.auto_wrap(), AutoWrapMode::Enabled);
}
