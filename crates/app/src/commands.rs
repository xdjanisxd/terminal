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
];

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
}
