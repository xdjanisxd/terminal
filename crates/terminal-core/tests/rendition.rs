use terminal_core::{
    AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, InverseVideo, ItalicStyle, TerminalDimensions, TerminalState, TextIntensity,
    UnderlineStyle,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(8, 2).unwrap())
}

#[test]
fn current_rendition_uses_documented_defaults() {
    let state = state();

    assert_eq!(state.current_rendition(), &CellAttributes::default());
    assert_eq!(state.current_rendition().foreground(), CellColor::Default);
    assert_eq!(state.current_rendition().background(), CellColor::Default);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Upright);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Disabled
    );
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Disabled);
}

#[test]
fn text_intensity_can_be_set_and_cleared_independently() {
    let mut state = state();
    state.set_italic_style(ItalicStyle::Italic);

    state.set_text_intensity(TextIntensity::Bold);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);

    state.set_text_intensity(TextIntensity::Normal);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Normal);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
}

#[test]
fn italic_can_be_set_and_cleared_independently() {
    let mut state = state();
    state.set_text_intensity(TextIntensity::Bold);

    state.set_italic_style(ItalicStyle::Italic);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Italic);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);

    state.set_italic_style(ItalicStyle::Upright);
    assert_eq!(state.current_rendition().italic(), ItalicStyle::Upright);
    assert_eq!(state.current_rendition().intensity(), TextIntensity::Bold);
}

#[test]
fn underline_can_be_set_and_cleared_independently() {
    let mut state = state();
    state.set_inverse_video(InverseVideo::Enabled);

    state.set_underline_style(UnderlineStyle::Enabled);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Enabled
    );
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Enabled);

    state.set_underline_style(UnderlineStyle::Disabled);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Disabled
    );
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Enabled);
}

#[test]
fn inverse_video_can_be_set_and_cleared_independently() {
    let mut state = state();
    state.set_underline_style(UnderlineStyle::Enabled);

    state.set_inverse_video(InverseVideo::Enabled);
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Enabled);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Enabled
    );

    state.set_inverse_video(InverseVideo::Disabled);
    assert_eq!(state.current_rendition().inverse(), InverseVideo::Disabled);
    assert_eq!(
        state.current_rendition().underline(),
        UnderlineStyle::Enabled
    );
}

#[test]
fn foreground_and_background_channels_change_independently() {
    let mut state = state();

    state.set_foreground_color(CellColor::Indexed(3));
    state.set_background_color(CellColor::Indexed(12));
    assert_eq!(
        state.current_rendition().foreground(),
        CellColor::Indexed(3)
    );
    assert_eq!(
        state.current_rendition().background(),
        CellColor::Indexed(12)
    );

    state.set_foreground_color(CellColor::Default);
    assert_eq!(state.current_rendition().foreground(), CellColor::Default);
    assert_eq!(
        state.current_rendition().background(),
        CellColor::Indexed(12)
    );

    state.set_background_color(CellColor::Default);
    assert_eq!(state.current_rendition(), &CellAttributes::default());
}

#[test]
fn printed_cells_snapshot_the_current_rendition_without_retroactive_changes() {
    let mut state = state();
    state.print_character('a').unwrap();

    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_foreground_color(CellColor::Indexed(2));
    state.set_background_color(CellColor::Indexed(11));
    let styled = *state.current_rendition();
    state.print_character('b').unwrap();

    state.reset_rendition();
    state.print_character('c').unwrap();

    assert_eq!(
        state.screen().cell(0, 0).unwrap().attributes(),
        &CellAttributes::default()
    );
    assert_eq!(state.screen().cell(0, 1).unwrap().attributes(), &styled);
    assert_eq!(
        state.screen().cell(0, 2).unwrap().attributes(),
        &CellAttributes::default()
    );
}

#[test]
fn terminal_reset_restores_rendition_defaults() {
    let mut state = state();
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_foreground_color(CellColor::Indexed(5));
    state.set_background_color(CellColor::Indexed(14));

    state.reset();

    assert_eq!(state.current_rendition(), &CellAttributes::default());
}

#[test]
fn cursor_modes_tabs_erase_and_delayed_wrap_preserve_current_rendition() {
    let mut state = TerminalState::new(TerminalDimensions::new(2, 2).unwrap());
    state.set_text_intensity(TextIntensity::Bold);
    state.set_italic_style(ItalicStyle::Italic);
    state.set_underline_style(UnderlineStyle::Enabled);
    state.set_inverse_video(InverseVideo::Enabled);
    state.set_foreground_color(CellColor::Indexed(1));
    state.set_background_color(CellColor::Indexed(10));
    let rendition = *state.current_rendition();

    state.set_cursor_visibility(CursorVisibility::Hidden);
    state.set_auto_wrap(AutoWrapMode::Enabled);
    state.set_character_insertion(CharacterInsertionMode::Replace);
    state.set_cursor_key_mode(CursorKeyMode::Application);
    state.set_horizontal_tab_stop();
    state.clear_horizontal_tab_stop();
    state.print_character('a').unwrap();
    state.print_character('b').unwrap();
    state.print_character('c').unwrap();

    assert_eq!(state.screen().cell(0, 0).unwrap().attributes(), &rendition);
    assert_eq!(state.screen().cell(0, 1).unwrap().attributes(), &rendition);
    assert_eq!(state.screen().cell(1, 0).unwrap().attributes(), &rendition);
    assert_eq!(state.current_rendition(), &rendition);

    state.erase(
        terminal_core::EraseRegion::Line,
        terminal_core::EraseDirection::CursorToEnd,
    );
    assert_eq!(state.current_rendition(), &rendition);
    assert_eq!(
        state.terminal_modes().cursor_visibility(),
        CursorVisibility::Hidden
    );
    assert_eq!(
        state.input_modes().cursor_keys(),
        CursorKeyMode::Application
    );
}
