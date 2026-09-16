use terminal_core::{
    AutoWrapMode, CellAttributes, CellColor, CharacterInsertionMode, CursorKeyMode,
    CursorVisibility, InverseVideo, ItalicStyle, TerminalDimensions, TerminalParser, TerminalReply,
    TerminalState, TextIntensity, UnderlineStyle,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(8, 3).unwrap())
}

fn rgb(red: u8, green: u8, blue: u8) -> CellColor {
    CellColor::Rgb { red, green, blue }
}

#[test]
fn truecolor_foreground_and_background_are_exact_and_preserve_styles() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    terminal.set_text_intensity(TextIntensity::Bold);
    terminal.set_italic_style(ItalicStyle::Italic);
    terminal.set_underline_style(UnderlineStyle::Enabled);
    terminal.set_inverse_video(InverseVideo::Enabled);
    terminal.set_foreground_color(CellColor::Indexed(4));
    terminal.set_background_color(CellColor::Indexed(12));

    parser
        .advance(&mut terminal, b"\x1b[38;2;255;0;128m\x1b[48;2;1;2;3m")
        .unwrap();
    assert_eq!(terminal.current_rendition().foreground(), rgb(255, 0, 128));
    assert_eq!(terminal.current_rendition().background(), rgb(1, 2, 3));

    parser
        .advance(&mut terminal, b"\x1b[38;2;17;34;51;48;2;68;85;102m")
        .unwrap();
    let attributes = *terminal.current_rendition();
    assert_eq!(attributes.foreground(), rgb(17, 34, 51));
    assert_eq!(attributes.background(), rgb(68, 85, 102));
    assert_eq!(attributes.intensity(), TextIntensity::Bold);
    assert_eq!(attributes.italic(), ItalicStyle::Italic);
    assert_eq!(attributes.underline(), UnderlineStyle::Enabled);
    assert_eq!(attributes.inverse(), InverseVideo::Enabled);

    parser.advance(&mut terminal, b"A").unwrap();
    assert_eq!(
        terminal.screen().cell(0, 0).unwrap().attributes(),
        &attributes
    );
}

#[test]
fn truecolor_minimum_maximum_replacement_and_resets_work_per_channel() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();

    parser.advance(&mut terminal, b"\x1b[38;2;0;0;0m").unwrap();
    assert_eq!(terminal.current_rendition().foreground(), rgb(0, 0, 0));
    parser
        .advance(&mut terminal, b"\x1b[48;2;255;255;255m")
        .unwrap();
    assert_eq!(
        terminal.current_rendition().background(),
        rgb(255, 255, 255)
    );

    parser.advance(&mut terminal, b"\x1b[32;45m").unwrap();
    assert_eq!(
        terminal.current_rendition().foreground(),
        CellColor::Indexed(2)
    );
    assert_eq!(
        terminal.current_rendition().background(),
        CellColor::Indexed(5)
    );
    parser
        .advance(&mut terminal, b"\x1b[38;5;199;48;5;200m")
        .unwrap();
    assert_eq!(
        terminal.current_rendition().foreground(),
        CellColor::Indexed(199)
    );
    assert_eq!(
        terminal.current_rendition().background(),
        CellColor::Indexed(200)
    );
    parser
        .advance(&mut terminal, b"\x1b[38;2;4;5;6m\x1b[48;2;7;8;9m")
        .unwrap();
    assert_eq!(terminal.current_rendition().foreground(), rgb(4, 5, 6));
    assert_eq!(terminal.current_rendition().background(), rgb(7, 8, 9));

    parser.advance(&mut terminal, b"\x1b[39m").unwrap();
    assert_eq!(
        terminal.current_rendition().foreground(),
        CellColor::Default
    );
    assert_eq!(terminal.current_rendition().background(), rgb(7, 8, 9));
    parser.advance(&mut terminal, b"\x1b[49m").unwrap();
    assert_eq!(
        terminal.current_rendition().background(),
        CellColor::Default
    );
    parser
        .advance(&mut terminal, b"\x1b[38;2;1;2;3;48;2;4;5;6m\x1b[0m")
        .unwrap();
    assert_eq!(terminal.current_rendition(), &CellAttributes::default());
}

#[test]
fn truecolor_cells_snapshot_and_move_with_complete_attributes() {
    let mut parser = TerminalParser::new();
    let mut terminal = state();
    parser
        .advance(&mut terminal, b"A\x1b[38;2;9;8;7mB")
        .unwrap();
    assert_eq!(
        terminal.screen().cell(0, 0).unwrap().attributes(),
        &CellAttributes::default()
    );
    assert_eq!(
        terminal
            .screen()
            .cell(0, 1)
            .unwrap()
            .attributes()
            .foreground(),
        rgb(9, 8, 7)
    );

    terminal.set_cursor_position(1, 0).unwrap();
    terminal.set_text_intensity(TextIntensity::Bold);
    terminal.print_character('R').unwrap();
    terminal.scroll_up(1);
    let moved = terminal.screen().cell(0, 0).unwrap();
    assert_eq!(moved.character(), 'R');
    assert_eq!(moved.attributes().foreground(), rgb(9, 8, 7));
    assert_eq!(moved.attributes().intensity(), TextIntensity::Bold);
}

#[test]
fn truecolor_is_chunk_safe_and_preserves_unrelated_terminal_state_and_replies() {
    let input = b"X\x1b[38;2;11;22;33mY\x1b[48;2;44;55;66mZ";
    let mut expected_parser = TerminalParser::new();
    let mut expected = state();
    expected_parser.advance(&mut expected, input).unwrap();

    for split in 0..=input.len() {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        terminal.set_auto_wrap(AutoWrapMode::Disabled);
        terminal.set_character_insertion(CharacterInsertionMode::Insert);
        terminal.set_cursor_visibility(CursorVisibility::Hidden);
        terminal.set_cursor_key_mode(CursorKeyMode::Application);
        terminal.set_horizontal_tab_stop();
        assert!(terminal.set_vertical_scrolling_margins(1, 2));
        assert!(terminal.request_primary_device_attributes());
        terminal.set_cursor_position(0, 0).unwrap();
        assert!(terminal.request_cursor_position_report());
        let modes = *terminal.terminal_modes();
        let input_modes = *terminal.input_modes();
        let margins = terminal.vertical_scrolling_margins();

        parser.advance(&mut terminal, &input[..split]).unwrap();
        parser.advance(&mut terminal, &input[split..]).unwrap();

        assert_eq!(terminal.screen(), expected.screen(), "split {split}");
        assert_eq!(terminal.current_rendition(), expected.current_rendition());
        assert_eq!(*terminal.terminal_modes(), modes);
        assert_eq!(*terminal.input_modes(), input_modes);
        assert_eq!(terminal.vertical_scrolling_margins(), margins);
        assert_eq!(terminal.cursor(), expected.cursor());
        assert_eq!(
            terminal.take_reply(),
            Some(TerminalReply::PrimaryDeviceAttributes)
        );
        assert_eq!(
            terminal.take_reply(),
            Some(TerminalReply::CursorPosition { row: 1, column: 1 })
        );
        assert_eq!(terminal.take_reply(), None);
    }
}

#[test]
fn malformed_truecolor_groups_are_atomic_and_colon_forms_stay_unsupported() {
    for input in [
        b"\x1b[38;2m".as_slice(),
        b"\x1b[38;2;255m",
        b"\x1b[38;2;255;0m",
        b"\x1b[48;2m",
        b"\x1b[48;2;1;2m",
        b"\x1b[38;2;256;0;1m",
        b"\x1b[48;2;1;256;3m",
        b"\x1b[38:2:1:2:3m",
        b"\x1b[48:2:1:2:3m",
    ] {
        let mut parser = TerminalParser::new();
        let mut terminal = state();
        terminal.set_foreground_color(CellColor::Indexed(5));
        terminal.set_background_color(CellColor::Indexed(6));
        parser.advance(&mut terminal, input).unwrap();
        assert_eq!(
            terminal.current_rendition().foreground(),
            CellColor::Indexed(5),
            "{input:?}"
        );
        assert_eq!(
            terminal.current_rendition().background(),
            CellColor::Indexed(6),
            "{input:?}"
        );
    }

    let mut parser = TerminalParser::new();
    let mut terminal = state();
    parser
        .advance(&mut terminal, b"\x1b[1;38;2;256;0;1;7m")
        .unwrap();
    assert_eq!(
        terminal.current_rendition().foreground(),
        CellColor::Default
    );
    assert_eq!(
        terminal.current_rendition().intensity(),
        TextIntensity::Bold
    );
    assert_eq!(
        terminal.current_rendition().inverse(),
        InverseVideo::Enabled
    );
}
