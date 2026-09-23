//! App command definitions and palette selection state. Execution stays in Application.

use terminal_config::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandInfo {
    pub command: Command,
    pub name: &'static str,
    pub in_palette: bool,
}

pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        command: Command::Copy,
        name: "Copy",
        in_palette: true,
    },
    CommandInfo {
        command: Command::Paste,
        name: "Paste",
        in_palette: true,
    },
    CommandInfo {
        command: Command::PageUp,
        name: "Page Up",
        in_palette: true,
    },
    CommandInfo {
        command: Command::PageDown,
        name: "Page Down",
        in_palette: true,
    },
    CommandInfo {
        command: Command::OpenPalette,
        name: "Command Palette",
        in_palette: false,
    },
    CommandInfo {
        command: Command::OpenTarget,
        name: "Open Target",
        in_palette: false,
    },
];

/// Terminal output is untrusted. Only an explicit user gesture may pass an
/// OSC 8 target to the OS, and only web URLs are eligible.
pub fn allowed_target(uri: &str) -> bool {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https")
        || rest.is_empty()
        || uri
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '\\' | '"' | '\''))
    {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty()
        && authority
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-:[]".contains(&byte))
}

#[derive(Debug, Default)]
pub struct Palette {
    query: String,
    selected: usize,
}

impl Palette {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn matches(&self) -> Vec<CommandInfo> {
        let query = self.query.to_ascii_lowercase();
        COMMANDS
            .iter()
            .copied()
            .filter(|info| info.in_palette && info.name.to_ascii_lowercase().contains(&query))
            .collect()
    }

    pub fn push_text(&mut self, text: &str) {
        // Command names are ASCII; this also keeps the cell overlay width exact.
        self.query.extend(
            text.chars()
                .filter(|character| character.is_ascii() && !character.is_ascii_control()),
        );
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, delta: isize) {
        let count = self.matches().len();
        if count > 0 {
            self.selected = self.selected.saturating_add_signed(delta).min(count - 1);
        }
    }

    pub fn chosen(&self) -> Option<Command> {
        self.matches().get(self.selected).map(|info| info.command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_filters_navigates_and_selects_registry_commands() {
        let mut palette = Palette::default();
        assert_eq!(palette.chosen(), Some(Command::Copy));
        palette.push_text("PAGE");
        assert_eq!(palette.matches().len(), 2);
        assert_eq!(palette.chosen(), Some(Command::PageUp));
        palette.move_selection(1);
        assert_eq!(palette.chosen(), Some(Command::PageDown));
        palette.move_selection(1);
        assert_eq!(palette.chosen(), Some(Command::PageDown));
        palette.backspace();
        assert_eq!(palette.selected(), 0);
        palette.push_text("zzzz");
        assert_eq!(palette.chosen(), None);
    }

    #[test]
    fn untrusted_targets_have_a_narrow_web_url_allowlist() {
        assert!(allowed_target("https://example.org/path?q=1"));
        assert!(allowed_target("http://localhost:8080/"));
        for uri in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://",
            "https://user@example.org/",
            "https://example.org/\rcommand",
            "https://example.org\\bad",
            "https://example.org\"/bad",
            "https://evil.example@trusted.example/",
            "HTTPS://example.org",
        ] {
            assert!(!allowed_target(uri), "{uri}");
        }
    }
}
