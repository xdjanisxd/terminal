use terminal_core::{MAX_PENDING_REPLIES, TerminalDimensions, TerminalParser, TerminalState};

fn state() -> TerminalState {
    TerminalState::new(TerminalDimensions::new(16, 4).unwrap())
}

fn assert_reply(terminal: &mut TerminalState, expected: &[u8]) {
    assert_eq!(
        terminal.take_reply().unwrap().as_bytes().as_slice(),
        expected,
    );
}

#[test]
fn indexed_color_queries_report_the_default_ansi_and_fixed_xterm_palette() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    for (query, reply) in [
        (
            b"\x1b]4;0;?\x07".as_slice(),
            b"\x1b]4;0;rgb:0000/0000/0000\x07".as_slice(),
        ),
        (b"\x1b]4;01;?\x07", b"\x1b]4;1;rgb:cdcd/3131/3131\x07"),
        (b"\x1b]4;15;?\x07", b"\x1b]4;15;rgb:ffff/ffff/ffff\x07"),
        (b"\x1b]4;16;?\x07", b"\x1b]4;16;rgb:0000/0000/0000\x07"),
        (b"\x1b]4;67;?\x07", b"\x1b]4;67;rgb:5f5f/8787/afaf\x07"),
        (b"\x1b]4;231;?\x07", b"\x1b]4;231;rgb:ffff/ffff/ffff\x07"),
        (b"\x1b]4;232;?\x07", b"\x1b]4;232;rgb:0808/0808/0808\x07"),
        (b"\x1b]4;255;?\x07", b"\x1b]4;255;rgb:eeee/eeee/eeee\x07"),
    ] {
        parser.advance(&mut terminal, query).unwrap();
        assert_reply(&mut terminal, reply);
        assert_eq!(terminal.take_reply(), None);
    }
}

#[test]
fn indexed_color_queries_support_multiple_pairs_and_both_terminators_at_every_split() {
    for terminator in [b"\x07".as_slice(), b"\x1b\\"] {
        let mut query = b"\x1b]4;255;?;256;?;1;#abcdef;1;?;0;?;2;?;15;?".to_vec();
        query.extend_from_slice(terminator);
        for split in 0..query.len() {
            let mut terminal = state();
            let mut parser = TerminalParser::new();
            parser.advance(&mut terminal, &query[..split]).unwrap();
            assert_eq!(terminal.take_reply(), None);
            parser.advance(&mut terminal, &query[split..]).unwrap();
            for mut reply in [
                b"\x1b]4;255;rgb:eeee/eeee/eeee".to_vec(),
                b"\x1b]4;1;rgb:cdcd/3131/3131".to_vec(),
                b"\x1b]4;0;rgb:0000/0000/0000".to_vec(),
                b"\x1b]4;2;rgb:0d0d/bcbc/7979".to_vec(),
                b"\x1b]4;15;rgb:ffff/ffff/ffff".to_vec(),
            ] {
                reply.extend_from_slice(terminator);
                assert_reply(&mut terminal, &reply);
            }
            assert_eq!(terminal.take_reply(), None);
        }
    }
}

#[test]
fn malformed_canceled_and_unsupported_palette_sequences_have_no_effect() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    for sequence in [
        b"\x1b]4;1;#abcdef\x07".as_slice(),
        b"\x1b]104;1\x07",
        b"\x1b]4;+1;?\x07",
        b"\x1b]4;-1;?\x07",
        b"\x1b]4;;?\x07",
        b"\x1b]4;1;??\x07",
        b"\x1b]4;1;?;2\x07",
        b"\x1b]4;1;?;2;?;3;?;4;?;5;?;6;?;7;?;8;?\x07",
        b"\x1b]4;1;?\x18",
        b"\x1b]4;1;?\x1a",
        b"\x1b]4;1;?\x1bc",
        b"\x1b]4;1;?\x1b\x1b\\",
    ] {
        for byte in sequence {
            parser.advance(&mut terminal, &[*byte]).unwrap();
            assert_eq!(terminal.take_reply(), None, "{sequence:?}");
        }
    }
    parser.advance(&mut terminal, b"\x1b]4;1;?\x07").unwrap();
    assert_reply(&mut terminal, b"\x1b]4;1;rgb:cdcd/3131/3131\x07");
}

#[test]
fn configured_colors_survive_resets_and_osc_palette_writes() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    let mut ansi = [[0; 3]; 16];
    ansi[1] = [0x12, 0x34, 0x56];
    terminal.set_ansi_colors(ansi);
    terminal.reset();
    for reset in [b"".as_slice(), b"\x1b[!p", b"\x1bc"] {
        parser.advance(&mut terminal, reset).unwrap();
        parser
            .advance(
                &mut terminal,
                b"\x1b]4;1;#abcdef\x07\x1b]104;1\x07\x1b]4;1;?;67;?\x07",
            )
            .unwrap();
        assert_reply(&mut terminal, b"\x1b]4;1;rgb:1212/3434/5656\x07");
        assert_reply(&mut terminal, b"\x1b]4;67;rgb:5f5f/8787/afaf\x07");
        assert_eq!(terminal.take_reply(), None);
    }
}

#[test]
fn indexed_color_replies_share_the_bounded_fifo_and_capture_colors() {
    let mut terminal = state();
    let mut parser = TerminalParser::new();
    for _ in 1..MAX_PENDING_REPLIES {
        parser.advance(&mut terminal, b"\x1b[5n").unwrap();
    }
    parser
        .advance(&mut terminal, b"\x1b]4;1;?;2;?\x07")
        .unwrap();
    terminal.set_ansi_colors([[0x12, 0x34, 0x56]; 16]);
    assert_eq!(terminal.pending_reply_count(), MAX_PENDING_REPLIES);
    assert_reply(&mut terminal, b"\x1b[0n");
    parser.advance(&mut terminal, b"\x1b]4;2;?\x07").unwrap();
    for _ in 2..MAX_PENDING_REPLIES {
        assert_reply(&mut terminal, b"\x1b[0n");
    }
    assert_reply(&mut terminal, b"\x1b]4;1;rgb:cdcd/3131/3131\x07");
    assert_reply(&mut terminal, b"\x1b]4;2;rgb:1212/3434/5656\x07");
    assert_eq!(terminal.take_reply(), None);
}
