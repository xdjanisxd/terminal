//! App-owned URL matching and hint sessions over core-owned visible cells.
use terminal_core::{MouseTracking, ScreenKind, TerminalState};

pub const ALPHABET: &[u8; 7] = b"ASDFJKL";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub url: String,
    pub spans: Vec<Span>,
}

pub fn enabled(terminal: &TerminalState) -> bool {
    terminal.active_screen() == ScreenKind::Primary
        && terminal.input_modes().mouse_tracking() == MouseTracking::Off
}

/// Conservative HTTP(S) policy shared by both gestures and the opening boundary.
pub fn allowed(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '\\' | '"' | '<' | '>'))
    {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let host = authority;
    if let Some(stripped) = host.strip_prefix('[') {
        let Some((address, suffix)) = stripped.split_once(']') else {
            return false;
        };
        address.parse::<std::net::Ipv6Addr>().is_ok()
            && (suffix.is_empty() || suffix.strip_prefix(':').is_some_and(valid_port))
    } else {
        let (name, port) = host
            .split_once(':')
            .map_or((host, None), |(n, p)| (n, Some(p)));
        !name.is_empty()
            && !name.starts_with('.')
            && !name.ends_with('.')
            && !name.chars().any(|c| {
                matches!(
                    c,
                    '(' | ')' | '[' | ']' | '{' | '}' | ';' | ',' | '`' | '\'' | '$' | '%'
                )
            })
            && port.is_none_or(valid_port)
    }
}

fn valid_port(port: &str) -> bool {
    !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) && port.parse::<u16>().is_ok()
}

pub fn open_with(
    url: &str,
    opener: impl FnOnce(&str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if !allowed(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid HTTP/HTTPS URL",
        ));
    }
    opener(url)
}

pub fn detect(terminal: &TerminalState) -> Vec<Occurrence> {
    if !enabled(terminal) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut text = String::new();
    // One source cell span per UTF-8 byte, including combining scalars.
    let mut positions = Vec::new();
    let mut clipped_start = terminal.viewport_starts_in_wrapped_line();
    for row in 0..terminal.dimensions().rows() {
        let wrap = terminal.viewport_wrap_columns(row);
        let captured = terminal.viewport_row_columns(row);
        let right_clipped = captured > terminal.dimensions().columns();
        let width = if wrap == 0 {
            terminal.dimensions().columns()
        } else {
            wrap.min(terminal.dimensions().columns())
        };
        for column in 0..width {
            let Some(cell) = terminal.viewport_cell(row, column) else {
                text.push(' ');
                positions.push(Span {
                    row,
                    start: column,
                    end: column + 1,
                });
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let position = Span {
                row,
                start: column,
                end: (column + if cell.is_wide_lead() { 2 } else { 1 }).min(width),
            };
            for character in
                std::iter::once(cell.character()).chain(cell.combining_marks().iter().copied())
            {
                text.push(character);
                positions.extend(std::iter::repeat_n(position, character.len_utf8()));
            }
        }
        if wrap == 0 || row + 1 == terminal.dimensions().rows() || right_clipped {
            parse_line(
                &text,
                &positions,
                clipped_start,
                wrap != 0 || right_clipped,
                &mut found,
            );
            text.clear();
            positions.clear();
            clipped_start = right_clipped && wrap != 0;
        }
    }
    found
}

fn parse_line(
    text: &str,
    positions: &[Span],
    clipped_start: bool,
    clipped_end: bool,
    found: &mut Vec<Occurrence>,
) {
    let mut offset = 0;
    while offset < text.len() {
        let remaining = &text[offset..];
        let scheme = remaining
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
            || remaining
                .get(..8)
                .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
        if !scheme {
            offset += remaining.chars().next().unwrap().len_utf8();
            continue;
        }
        let boundary = offset == 0
            || text[..offset]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric() && !matches!(c, '_' | '/' | '.' | '-'));
        let raw_end = remaining
            .char_indices()
            .find(|(_, c)| {
                c.is_whitespace()
                    || c.is_control()
                    || matches!(c, '<' | '>' | '"' | '\'' | '`' | '\\')
            })
            .map_or(text.len(), |(i, _)| offset + i);
        let mut end = raw_end;
        loop {
            let url = &text[offset..end];
            let Some(last) = url.chars().next_back() else {
                break;
            };
            let trim = matches!(last, '.' | ',' | ';' | ':' | '!' | '?')
                || match last {
                    ')' => url.matches(')').count() > url.matches('(').count(),
                    ']' => url.matches(']').count() > url.matches('[').count(),
                    '}' => url.matches('}').count() > url.matches('{').count(),
                    _ => false,
                };
            if !trim {
                break;
            }
            end -= last.len_utf8();
        }
        if boundary
            && !(clipped_start && offset == 0)
            && !(clipped_end && raw_end == text.len())
            && allowed(&text[offset..end])
        {
            let mut spans: Vec<Span> = Vec::new();
            for position in &positions[offset..end] {
                if let Some(last) = spans.last_mut().filter(|last| last.row == position.row) {
                    last.end = position.end;
                } else {
                    spans.push(*position);
                }
            }
            found.push(Occurrence {
                url: text[offset..end].into(),
                spans,
            });
        }
        offset = raw_end.max(offset + 7);
    }
}

pub fn hints(count: usize) -> Vec<String> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    // Alphabet ranks give the specified ordering rather than ASCII ordering.
    // The heap expands shortest leaves first, later alphabet leaves on ties.
    let mut leaves: BinaryHeap<_> = (0..count.min(ALPHABET.len()))
        .map(|rank| (Reverse(1), vec![rank]))
        .collect();
    while leaves.len() < count {
        let (_, parent) = leaves.pop().unwrap();
        let children = (count - leaves.len()).min(ALPHABET.len());
        for rank in 0..children {
            let mut child = parent.clone();
            child.push(rank);
            leaves.push((Reverse(child.len()), child));
        }
    }
    let mut ordered: Vec<_> = leaves.into_iter().map(|(_, leaf)| leaf).collect();
    ordered.sort_unstable();
    ordered
        .into_iter()
        .map(|leaf| {
            leaf.into_iter()
                .map(|rank| ALPHABET[rank] as char)
                .collect()
        })
        .collect()
}

#[derive(Debug)]
pub struct Session {
    pub candidates: Vec<Occurrence>,
    pub hints: Vec<String>,
    pub prefix: String,
    viewport: usize,
    dimensions: terminal_core::TerminalDimensions,
}

impl Session {
    pub fn new(terminal: &TerminalState) -> Option<Self> {
        let candidates = detect(terminal);
        (!candidates.is_empty()).then(|| Self {
            hints: hints(candidates.len()),
            candidates,
            prefix: String::new(),
            viewport: terminal.viewport_offset(),
            dimensions: terminal.dimensions(),
        })
    }
    pub fn visible(&self) -> impl Iterator<Item = (&Occurrence, &str)> {
        self.candidates
            .iter()
            .zip(&self.hints)
            .filter(|(_, h)| h.starts_with(&self.prefix))
            .map(|(c, h)| (c, h.as_str()))
    }
    pub fn push(&mut self, text: &str) -> Option<String> {
        if text.chars().count() != 1 {
            return None;
        }
        let character = text.chars().next()?.to_ascii_uppercase();
        if !ALPHABET.contains(&(character as u32).try_into().ok()?) {
            return None;
        }
        let next = format!("{}{character}", self.prefix);
        if !self.hints.iter().any(|h| h.starts_with(&next)) {
            return None;
        }
        self.prefix = next;
        self.hints
            .iter()
            .position(|h| *h == self.prefix)
            .map(|i| self.candidates[i].url.clone())
    }
    pub fn backspace(&mut self) {
        self.prefix.pop();
    }
    pub fn valid(&self, terminal: &TerminalState) -> bool {
        self.viewport == terminal.viewport_offset()
            && self.dimensions == terminal.dimensions()
            && self.candidates == detect(terminal)
    }
}

pub fn at(terminal: &TerminalState, row: usize, column: usize) -> Option<Occurrence> {
    detect(terminal).into_iter().find(|c| {
        c.spans
            .iter()
            .any(|s| s.row == row && column >= s.start && column < s.end)
    })
}

/// Keep trimmed prose punctuation visible between the URL and its label.
pub fn label_end(terminal: &TerminalState, span: Span) -> usize {
    let mut end = span.end;
    while terminal.viewport_cell(span.row, end).is_some_and(|cell| {
        matches!(
            cell.character(),
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '\'' | '"' | '`'
        )
    }) {
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_core::{TerminalDimensions, TerminalParser};
    fn terminal(text: &str, columns: usize, rows: usize) -> TerminalState {
        let mut terminal = TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
        TerminalParser::new()
            .advance(&mut terminal, text.as_bytes())
            .unwrap();
        terminal
    }
    #[test]
    fn hint_examples_and_prefix_freedom() {
        assert_eq!(hints(8), ["A", "S", "D", "F", "J", "K", "LA", "LS"]);
        assert_eq!(&hints(50)[48..], ["LLA", "LLS"]);
        for count in 0..400 {
            let assigned = hints(count);
            assert_eq!(assigned.len(), count);
            assert_eq!(assigned, hints(count));
            for (i, hint) in assigned.iter().enumerate() {
                assert!(hint.bytes().all(|c| ALPHABET.contains(&c)));
                for (j, other) in assigned.iter().enumerate() {
                    if i != j {
                        assert!(!other.starts_with(hint));
                    }
                }
            }
        }
    }
    #[test]
    fn large_hint_sets_support_deep_prefix_free_codes() {
        let mut assigned = hints(10_000);
        assert_eq!(assigned.len(), 10_000);
        assert!(assigned.iter().any(|hint| hint.len() >= 5));
        assigned.sort_unstable();
        // Prefixes form adjacent groups under any character ordering.
        assert!(
            assigned
                .windows(2)
                .all(|pair| !pair[1].starts_with(&pair[0]))
        );
    }

    #[test]
    fn wrapped_unicode_punctuation_and_occurrences() {
        let t = terminal(
            "https://例え.test/e\u{301}_(x). https://a.test https://a.test",
            19,
            5,
        );
        let urls = detect(&t);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0].url, "https://例え.test/e\u{301}_(x)");
        assert!(urls[0].spans.len() > 1);
        let span = urls[0].spans[1];
        assert_eq!(at(&t, span.row, span.start), Some(urls[0].clone()));
        assert_eq!(detect(&terminal("https:\r\n//a.test", 30, 3)).len(), 0);
        assert_eq!(
            detect(&terminal("URL:https://a.test;", 30, 3))[0].url,
            "https://a.test"
        );
        let prose = terminal("https://a.test). next", 30, 3);
        let span = detect(&prose)[0].spans[0];
        assert_eq!(label_end(&prose, span), span.end + 2);
    }
    #[test]
    fn filtering_backtracking_and_stable_assignment() {
        let text = (0..8)
            .map(|i| format!("https://{i}.test\r\n"))
            .collect::<String>();
        let mut session = Session::new(&terminal(&text, 40, 10)).unwrap();
        let original = session.hints.clone();
        assert_eq!(session.push("l"), None);
        assert_eq!(session.visible().count(), 2);
        assert_eq!(session.push("z"), None);
        assert_eq!(session.prefix, "L");
        session.backspace();
        assert_eq!(session.visible().count(), 8);
        assert_eq!(session.hints, original);
        session.push("L");
        assert_eq!(session.push("S"), Some("https://7.test".into()));
    }
    #[test]
    fn safe_open_policy_and_failure() {
        for url in [
            "file:///tmp/a",
            "https://",
            "https://a\n.test",
            "https://a:bogus",
            "https://[bad]",
            "https://user@example.test",
        ] {
            assert!(open_with(url, |_| panic!("opener called")).is_err());
        }
        let url = "https://example.test/$(echo;pwn)?q=é";
        open_with(url, |actual| {
            assert_eq!(actual, url);
            Ok(())
        })
        .unwrap();
        assert!(
            open_with("http://localhost:8080", |_| Err(std::io::Error::other(
                "fixture"
            )))
            .is_err()
        );
        assert!(allowed("https://[::1]:443/a"));
    }
    #[test]
    fn resized_history_does_not_open_a_truncated_url() {
        let mut t = terminal("https://example.test/hidden-path\r\nblank\r\nmore", 40, 2);
        t.resize(TerminalDimensions::new(14, 2).unwrap());
        t.page_up();
        assert_eq!(t.viewport_row_columns(0), 14);
        assert!(detect(&t).is_empty());
    }

    #[test]
    fn clipped_and_alternate_content() {
        assert!(detect(&terminal("https://a.test/long-long-long", 10, 1)).is_empty());
        let mut t = terminal("https://a.test", 30, 3);
        TerminalParser::new()
            .advance(&mut t, b"\x1b[?1049hhttps://b.test")
            .unwrap();
        assert!(detect(&t).is_empty());
        TerminalParser::new()
            .advance(&mut t, b"\x1b[?1049l\x1b[?1000h")
            .unwrap();
        assert!(detect(&t).is_empty());
    }
}
