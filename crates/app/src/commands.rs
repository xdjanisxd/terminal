//! App command definitions and palette selection state. Execution stays in Application.

use std::path::PathBuf;
use terminal_config::{Command, ProjectRoot};
use terminal_workspace::SplitAxis;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaletteAction {
    Command(Command),
    ProjectTab(PathBuf),
    ProjectSplit(PathBuf, SplitAxis),
}

#[derive(Clone, Debug)]
pub struct PaletteEntry {
    pub action: PaletteAction,
    pub name: String,
}

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
    CommandInfo {
        command: Command::NewTab,
        name: "New Tab",
        in_palette: true,
    },
    CommandInfo {
        command: Command::SplitHorizontal,
        name: "Split Horizontal",
        in_palette: true,
    },
    CommandInfo {
        command: Command::SplitVertical,
        name: "Split Vertical",
        in_palette: true,
    },
    CommandInfo {
        command: Command::NextTab,
        name: "Next Tab",
        in_palette: true,
    },
    CommandInfo {
        command: Command::PreviousTab,
        name: "Previous Tab",
        in_palette: true,
    },
    CommandInfo {
        command: Command::NextPane,
        name: "Next Pane",
        in_palette: true,
    },
    CommandInfo {
        command: Command::PreviousPane,
        name: "Previous Pane",
        in_palette: true,
    },
    CommandInfo {
        command: Command::ResizePaneLeft,
        name: "Resize Pane Left",
        in_palette: true,
    },
    CommandInfo {
        command: Command::ResizePaneRight,
        name: "Resize Pane Right",
        in_palette: true,
    },
    CommandInfo {
        command: Command::ResizePaneUp,
        name: "Resize Pane Up",
        in_palette: true,
    },
    CommandInfo {
        command: Command::ResizePaneDown,
        name: "Resize Pane Down",
        in_palette: true,
    },
    CommandInfo {
        command: Command::TogglePaneZoom,
        name: "Toggle Pane Zoom",
        in_palette: true,
    },
    CommandInfo {
        command: Command::ClosePane,
        name: "Close Pane",
        in_palette: true,
    },
    CommandInfo {
        command: Command::RenameTab,
        name: "Rename Tab",
        in_palette: true,
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

#[derive(Debug)]
pub struct Palette {
    query: String,
    selected: usize,
    entries: Vec<PaletteEntry>,
}

impl Default for Palette {
    fn default() -> Self {
        Self::with_projects(&[])
    }
}

impl Palette {
    pub fn with_projects(projects: &[ProjectRoot]) -> Self {
        let mut entries: Vec<_> = COMMANDS
            .iter()
            .filter(|info| info.in_palette)
            .map(|info| PaletteEntry {
                action: PaletteAction::Command(info.command),
                name: info.name.into(),
            })
            .collect();
        for project in projects {
            for (name, action) in [
                (
                    format!("New Tab: {}", project.name),
                    PaletteAction::ProjectTab(project.path.clone()),
                ),
                (
                    format!("Split Horizontal: {}", project.name),
                    PaletteAction::ProjectSplit(project.path.clone(), SplitAxis::Horizontal),
                ),
                (
                    format!("Split Vertical: {}", project.name),
                    PaletteAction::ProjectSplit(project.path.clone(), SplitAxis::Vertical),
                ),
            ] {
                entries.push(PaletteEntry { action, name });
            }
        }
        Self {
            query: String::new(),
            selected: 0,
            entries,
        }
    }
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn matches(&self) -> Vec<&PaletteEntry> {
        let query = self.query.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|info| info.name.to_ascii_lowercase().contains(&query))
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

    pub fn chosen(&self) -> Option<PaletteAction> {
        self.matches()
            .get(self.selected)
            .map(|info| info.action.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_filters_navigates_and_selects_registry_commands() {
        let mut palette = Palette::default();
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::Command(Command::Copy))
        );
        palette.push_text("PAGE");
        assert_eq!(palette.matches().len(), 2);
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::Command(Command::PageUp))
        );
        palette.move_selection(1);
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::Command(Command::PageDown))
        );
        palette.move_selection(1);
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::Command(Command::PageDown))
        );
        palette.backspace();
        assert_eq!(palette.selected(), 0);
        palette.push_text("zzzz");
        assert_eq!(palette.chosen(), None);
    }

    #[test]
    fn project_actions_are_searchable_and_typed() {
        let mut projects = vec![ProjectRoot {
            name: "Core".into(),
            path: "repo".into(),
        }];
        let mut palette = Palette::with_projects(&projects);
        projects[0].path = "changed".into();
        palette.push_text("vertical: core");
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::ProjectSplit(
                "repo".into(),
                SplitAxis::Vertical
            ))
        );
        palette.backspace();
        assert_eq!(
            palette.chosen(),
            Some(PaletteAction::ProjectSplit(
                "repo".into(),
                SplitAxis::Vertical
            ))
        );
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
