use terminal_core::{
    CellAttributes, CellColor, MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser,
    TerminalState,
};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(16, 4).unwrap())
}

fn take_reply(terminal: &mut TerminalState) -> Vec<u8> {
    terminal
        .take_reply()
        .unwrap()
        .as_bytes()
        .as_slice()
        .to_vec()
}

#[test]
fn answers_default_color_queries_with_the_request_terminator_at_every_split() {
    for (query, expected) in [
        (
            b"\x1b]10;?\x07".as_slice(),
            b"\x1b]10;rgb:e6e6/e6e6/e6e6\x07".as_slice(),
        ),
        (b"\x1b]10;?\x1b\\", b"\x1b]10;rgb:e6e6/e6e6/e6e6\x1b\\"),
        (b"\x1b]11;?\x07", b"\x1b]11;rgb:0000/0000/0000\x07"),
        (b"\x1b]11;?\x1b\\", b"\x1b]11;rgb:0000/0000/0000\x1b\\"),
    ] {
        for split in 0..query.len() {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            parser.advance(&mut terminal, &query[..split]).unwrap();
            assert_eq!(terminal.take_reply(), None);
            parser.advance(&mut terminal, &query[split..]).unwrap();
            assert_eq!(take_reply(&mut terminal), expected);
            assert_eq!(terminal.take_reply(), None);
        }
    }
}

#[test]
fn configured_colors_survive_resets_and_are_independent_of_rendition() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    terminal.set_default_colors([1, 0x23, 0xff], [0x34, 0x56, 0x78]);
    terminal.reset();
    for reset in [b"".as_slice(), b"\x1b[!p", b"\x1bc"] {
        parser.advance(&mut terminal, reset).unwrap();
        assert_eq!(terminal.current_rendition(), &CellAttributes::default());
        parser
            .advance(&mut terminal, b"\x1b[31;44m\x1b]10;?\x07\x1b]11;?\x07")
            .unwrap();
        assert_eq!(take_reply(&mut terminal), b"\x1b]10;rgb:0101/2323/ffff\x07");
        assert_eq!(take_reply(&mut terminal), b"\x1b]11;rgb:3434/5656/7878\x07");
        assert_eq!(
            terminal.current_rendition().foreground(),
            CellColor::Indexed(1)
        );
        assert_eq!(
            terminal.current_rendition().background(),
            CellColor::Indexed(4)
        );
    }
}

#[test]
fn color_sets_and_canceled_or_malformed_queries_do_not_generate_replies() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    for input in [
        b"\x1b]10;#123456\x07".as_slice(),
        b"\x1b]11;rgb:12/34/56\x1b\\",
        b"\x1b]10;?;extra\x07",
        b"\x1b]11;??\x07",
        b"\x1b]10;?\x18",
        b"\x1b]11;?\x1a",
        b"\x1b]10;?\x1bc",
        b"\x1b]11;?\x1b\x18",
        b"\x1b]10;?\x1b\x1b\\",
    ] {
        for byte in input {
            parser.advance(&mut terminal, &[*byte]).unwrap();
            assert_eq!(terminal.take_reply(), None, "{input:?}");
        }
    }
    parser
        .advance(&mut terminal, b"\x1b]10;?\x07\x1b]11;?\x07")
        .unwrap();
    assert_eq!(take_reply(&mut terminal), b"\x1b]10;rgb:e6e6/e6e6/e6e6\x07");
    assert_eq!(take_reply(&mut terminal), b"\x1b]11;rgb:0000/0000/0000\x07");
}

#[test]
fn color_replies_capture_colors_and_respect_the_existing_reply_queue() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    parser.advance(&mut terminal, b"\x1b[5n").unwrap();
    for _ in 1..MAX_PENDING_REPLIES {
        parser.advance(&mut terminal, b"\x1b]10;?\x07").unwrap();
    }
    terminal.set_default_colors([0xab; 3], [0xcd; 3]);
    parser.advance(&mut terminal, b"\x1b]11;?\x1b\\").unwrap();
    assert_eq!(terminal.pending_reply_count(), MAX_PENDING_REPLIES);
    assert_eq!(take_reply(&mut terminal), b"\x1b[0n");
    parser.advance(&mut terminal, b"\x1b]11;?\x1b\\").unwrap();
    for _ in 1..MAX_PENDING_REPLIES {
        assert_eq!(take_reply(&mut terminal), b"\x1b]10;rgb:e6e6/e6e6/e6e6\x07");
    }
    assert_eq!(
        take_reply(&mut terminal),
        b"\x1b]11;rgb:cdcd/cdcd/cdcd\x1b\\"
    );
    assert_eq!(terminal.take_reply(), None);
}
