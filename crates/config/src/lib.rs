//! Typed, validated persistent configuration. Runtime state stays in the app.

use serde::Deserialize;
use std::{collections::HashSet, fmt, fs, path::Path};
use terminal_workspace::{LayoutDefinition, Workspace, WorkspaceDefinition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Theme {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub selection_foreground: Rgb,
    pub selection_background: Rgb,
    pub ansi: [Rgb; 16],
    pub ui_background: Option<Rgb>,
    pub ui_foreground: Option<Rgb>,
    pub ui_selected_background: Option<Rgb>,
    pub ui_muted: Option<Rgb>,
    pub ui_accent: Option<Rgb>,
    pub ui_border: Option<Rgb>,
    pub search_match_background: Option<Rgb>,
    pub search_active_background: Option<Rgb>,
    pub scrollbar_thumb: Option<Rgb>,
    pub scrollbar_thumb_hover: Option<Rgb>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: Rgb(230, 230, 230),
            background: Rgb(0, 0, 0),
            cursor: Rgb(230, 230, 230),
            selection_foreground: Rgb(255, 255, 255),
            selection_background: Rgb(46, 89, 166),
            ui_background: None,
            ui_foreground: None,
            ui_selected_background: None,
            ui_muted: None,
            ui_accent: None,
            ui_border: None,
            search_match_background: None,
            search_active_background: None,
            scrollbar_thumb: None,
            scrollbar_thumb_hover: None,
            ansi: [
                Rgb(0, 0, 0),
                Rgb(205, 49, 49),
                Rgb(13, 188, 121),
                Rgb(229, 229, 16),
                Rgb(36, 114, 200),
                Rgb(188, 63, 188),
                Rgb(17, 168, 205),
                Rgb(229, 229, 229),
                Rgb(102, 102, 102),
                Rgb(241, 76, 76),
                Rgb(35, 209, 139),
                Rgb(245, 245, 67),
                Rgb(59, 142, 234),
                Rgb(214, 112, 214),
                Rgb(41, 184, 219),
                Rgb(255, 255, 255),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Copy,
    Paste,
    PageUp,
    PageDown,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    OpenPalette,
    OpenTabPicker,
    SaveCurrentWorkspace,
    SetPaneStartupCommand,
    ClearPaneStartupCommand,
    OpenWorkspacePicker,
    DeleteWorkspace,
    OpenTarget,
    NewTab,
    SplitHorizontal,
    SplitVertical,
    NextTab,
    PreviousTab,
    NextPane,
    PreviousPane,
    ResizePaneLeft,
    ResizePaneRight,
    ResizePaneUp,
    ResizePaneDown,
    FocusPaneLeft,
    FocusPaneRight,
    FocusPaneUp,
    FocusPaneDown,
    TogglePaneZoom,
    ClosePane,
    RenameTab,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct KeyChord {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    /// Winit physical key name, e.g. `KeyC` or `PageUp`.
    pub key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Binding {
    pub key: KeyChord,
    pub command: Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub font: FontConfig,
    pub theme: Theme,
    pub bindings: Vec<Binding>,
    pub workspace: WorkspaceDefinition,
    pub projects: Vec<ProjectRoot>,
}

/// Terminal font selection and size in logical pixels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FontConfig {
    pub family: Option<String>,
    pub size: u16,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: None,
            size: 16,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRoot {
    pub name: String,
    pub path: std::path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            theme: Theme::default(),
            workspace: WorkspaceDefinition::default(),
            projects: Vec::new(),
            bindings: [
                ("Ctrl+Shift+C", Command::Copy),
                ("Ctrl+Shift+V", Command::Paste),
                ("PageUp", Command::PageUp),
                ("PageDown", Command::PageDown),
                ("Ctrl+=", Command::IncreaseFontSize),
                ("Ctrl+Shift+=", Command::IncreaseFontSize),
                ("Ctrl+-", Command::DecreaseFontSize),
                ("Ctrl+0", Command::ResetFontSize),
                ("Ctrl+Shift+P", Command::OpenPalette),
                ("Ctrl+Shift+Space", Command::OpenTabPicker),
                ("Ctrl+Shift+T", Command::NewTab),
                ("Ctrl+Shift+E", Command::SplitVertical),
                ("Ctrl+Shift+O", Command::SplitHorizontal),
                ("Ctrl+Tab", Command::NextTab),
                ("Ctrl+Shift+Tab", Command::PreviousTab),
                ("Ctrl+Shift+ArrowRight", Command::NextPane),
                ("Ctrl+Shift+ArrowLeft", Command::PreviousPane),
                ("Ctrl+Shift+H", Command::FocusPaneLeft),
                ("Ctrl+Shift+L", Command::FocusPaneRight),
                ("Ctrl+Shift+K", Command::FocusPaneUp),
                ("Ctrl+Shift+J", Command::FocusPaneDown),
                ("Ctrl+Alt+H", Command::ResizePaneLeft),
                ("Ctrl+Alt+L", Command::ResizePaneRight),
                ("Ctrl+Alt+K", Command::ResizePaneUp),
                ("Ctrl+Alt+J", Command::ResizePaneDown),
                ("Ctrl+Shift+Z", Command::TogglePaneZoom),
                ("Ctrl+Shift+W", Command::ClosePane),
            ]
            .into_iter()
            .map(|(key, command)| Binding {
                key: parse_chord(key).expect("valid default key"),
                command,
            })
            .collect(),
        }
    }
}

impl Config {
    pub fn load_workspace_file(path: &Path) -> Result<Self, ConfigError> {
        let source = fs::read_to_string(path)
            .map_err(|error| ConfigError(format!("{}: {error}", path.display())))?;
        let raw: RawConfig = toml::from_str(&source)
            .map_err(|error| ConfigError(format!("{}: {error}", path.display())))?;
        if raw.workspace.is_none() {
            return Err(ConfigError(format!(
                "{}: missing [workspace] definition",
                path.display()
            )));
        }
        Self::load(path)
    }
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig =
            toml::from_str(source).map_err(|error| ConfigError(error.to_string()))?;
        let mut config = Self::default();
        if let Some(font) = raw.font {
            if let Some(family) = font.family {
                if family.trim().is_empty() {
                    return Err(ConfigError(
                        "font.family: expected a nonempty font family".into(),
                    ));
                }
                config.font.family = Some(family);
            }
            if let Some(size) = font.size {
                if !(1..=256).contains(&size) {
                    return Err(ConfigError(
                        "font.size: expected 1..=256 logical pixels".into(),
                    ));
                }
                config.font.size = size as u16;
            }
        }
        if let Some(workspace) = raw.workspace {
            Workspace::from_definition(&workspace)
                .map_err(|error| ConfigError(format!("workspace: {error}")))?;
            config.workspace = workspace;
        }
        if let Some(projects) = raw.projects {
            let mut names = HashSet::new();
            for project in &projects {
                if project.name.trim().is_empty() || project.path.as_os_str().is_empty() {
                    return Err(ConfigError("projects need a name and path".into()));
                }
                if !names.insert(project.name.clone()) {
                    return Err(ConfigError(format!("duplicate project {:?}", project.name)));
                }
            }
            config.projects = projects;
        }
        if let Some(theme) = raw.theme {
            for (name, value, slot) in [
                (
                    "theme.foreground",
                    theme.foreground,
                    &mut config.theme.foreground,
                ),
                (
                    "theme.background",
                    theme.background,
                    &mut config.theme.background,
                ),
                ("theme.cursor", theme.cursor, &mut config.theme.cursor),
                (
                    "theme.selection_foreground",
                    theme.selection_foreground,
                    &mut config.theme.selection_foreground,
                ),
                (
                    "theme.selection_background",
                    theme.selection_background,
                    &mut config.theme.selection_background,
                ),
            ] {
                if let Some(value) = value {
                    *slot = parse_color(&value)
                        .map_err(|reason| ConfigError(format!("{name}: {reason}")))?;
                }
            }
            for (name, value, slot) in [
                (
                    "theme.ui_background",
                    theme.ui_background,
                    &mut config.theme.ui_background,
                ),
                (
                    "theme.ui_foreground",
                    theme.ui_foreground,
                    &mut config.theme.ui_foreground,
                ),
                (
                    "theme.ui_selected_background",
                    theme.ui_selected_background,
                    &mut config.theme.ui_selected_background,
                ),
                ("theme.ui_muted", theme.ui_muted, &mut config.theme.ui_muted),
                (
                    "theme.ui_accent",
                    theme.ui_accent,
                    &mut config.theme.ui_accent,
                ),
                (
                    "theme.ui_border",
                    theme.ui_border,
                    &mut config.theme.ui_border,
                ),
                (
                    "theme.search_match_background",
                    theme.search_match_background,
                    &mut config.theme.search_match_background,
                ),
                (
                    "theme.search_active_background",
                    theme.search_active_background,
                    &mut config.theme.search_active_background,
                ),
                (
                    "theme.scrollbar_thumb",
                    theme.scrollbar_thumb,
                    &mut config.theme.scrollbar_thumb,
                ),
                (
                    "theme.scrollbar_thumb_hover",
                    theme.scrollbar_thumb_hover,
                    &mut config.theme.scrollbar_thumb_hover,
                ),
            ] {
                if let Some(value) = value {
                    *slot = Some(
                        parse_color(&value)
                            .map_err(|reason| ConfigError(format!("{name}: {reason}")))?,
                    );
                }
            }
            if let Some(ansi) = theme.ansi {
                if ansi.len() != 16 {
                    return Err(ConfigError(format!(
                        "theme.ansi: expected 16 colors, got {}",
                        ansi.len()
                    )));
                }
                for (index, value) in ansi.iter().enumerate() {
                    config.theme.ansi[index] = parse_color(value)
                        .map_err(|reason| ConfigError(format!("theme.ansi[{index}]: {reason}")))?;
                }
            }
        }
        if let Some(bindings) = raw.bindings {
            let mut seen = HashSet::new();
            config.bindings.clear();
            for (index, binding) in bindings.into_iter().enumerate() {
                let key = parse_chord(&binding.key)
                    .map_err(|reason| ConfigError(format!("bindings[{index}].key: {reason}")))?;
                if !seen.insert(key.clone()) {
                    return Err(ConfigError(format!(
                        "bindings[{index}].key: duplicate chord {}",
                        binding.key
                    )));
                }
                config.bindings.push(Binding {
                    key,
                    command: binding.command,
                });
            }
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let source = fs::read_to_string(path)
            .map_err(|error| ConfigError(format!("{}: {error}", path.display())))?;
        let mut config = Self::parse(&source)
            .map_err(|error| ConfigError(format!("{}: {error}", path.display())))?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let resolve = |root: &mut std::path::PathBuf| {
            if root.is_relative() {
                *root = base.join(&*root);
            }
        };
        if let Some(root) = &mut config.workspace.project_root {
            resolve(root);
        }
        for tab in &mut config.workspace.tabs {
            if let Some(root) = &mut tab.project_root {
                resolve(root);
            }
            fn resolve_layout(layout: &mut LayoutDefinition, base: &Path) {
                match layout {
                    LayoutDefinition::Pane {
                        project_root: Some(root),
                        ..
                    } if root.is_relative() => {
                        *root = base.join(&*root);
                    }
                    LayoutDefinition::Split { first, second, .. } => {
                        resolve_layout(first, base);
                        resolve_layout(second, base);
                    }
                    _ => {}
                }
            }
            resolve_layout(&mut tab.layout, base);
        }
        for project in &mut config.projects {
            resolve(&mut project.path);
        }
        Ok(config)
    }
}

#[derive(Debug)]
pub struct ConfigError(pub String);
impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for ConfigError {}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    font: Option<RawFont>,
    theme: Option<RawTheme>,
    bindings: Option<Vec<RawBinding>>,
    workspace: Option<WorkspaceDefinition>,
    projects: Option<Vec<ProjectRoot>>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawFont {
    family: Option<String>,
    size: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawTheme {
    foreground: Option<String>,
    background: Option<String>,
    cursor: Option<String>,
    selection_foreground: Option<String>,
    selection_background: Option<String>,
    ansi: Option<Vec<String>>,
    ui_background: Option<String>,
    ui_foreground: Option<String>,
    ui_selected_background: Option<String>,
    ui_muted: Option<String>,
    ui_accent: Option<String>,
    ui_border: Option<String>,
    search_match_background: Option<String>,
    search_active_background: Option<String>,
    scrollbar_thumb: Option<String>,
    scrollbar_thumb_hover: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBinding {
    key: String,
    #[serde(alias = "action")]
    command: Command,
}

fn parse_color(value: &str) -> Result<Rgb, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' {
        return Err(format!("expected #RRGGBB, got {value:?}"));
    }
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(
            std::str::from_utf8(&bytes[range])
                .map_err(|_| format!("expected #RRGGBB, got {value:?}"))?,
            16,
        )
        .map_err(|_| format!("expected #RRGGBB, got {value:?}"))
    };
    Ok(Rgb(channel(1..3)?, channel(3..5)?, channel(5..7)?))
}

fn parse_chord(value: &str) -> Result<KeyChord, String> {
    let value = if value.trim().eq_ignore_ascii_case("Ctrl++") {
        "Ctrl+Shift+="
    } else {
        value
    };
    let mut chord = KeyChord {
        control: false,
        shift: false,
        alt: false,
        key: String::new(),
    };
    for part in value.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" if !chord.control && chord.key.is_empty() => chord.control = true,
            "shift" if !chord.shift && chord.key.is_empty() => chord.shift = true,
            "alt" if !chord.alt && chord.key.is_empty() => chord.alt = true,
            key if chord.key.is_empty() => {
                chord.key = match key {
                    "=" => "Equal".into(),
                    "-" => "Minus".into(),
                    "pageup" => "PageUp".into(),
                    "pagedown" => "PageDown".into(),
                    "home" => "Home".into(),
                    "end" => "End".into(),
                    "insert" => "Insert".into(),
                    "delete" => "Delete".into(),
                    "backspace" => "Backspace".into(),
                    "space" => "Space".into(),
                    "tab" => "Tab".into(),
                    "arrowright" => "ArrowRight".into(),
                    "arrowleft" => "ArrowLeft".into(),
                    "arrowup" => "ArrowUp".into(),
                    "arrowdown" => "ArrowDown".into(),
                    _ if key.len() == 1 && key.bytes().all(|byte| byte.is_ascii_alphabetic()) => {
                        format!("Key{}", key.to_ascii_uppercase())
                    }
                    _ if key.len() == 1 && key.bytes().all(|byte| byte.is_ascii_digit()) => {
                        format!("Digit{key}")
                    }
                    _ => return Err(format!("unknown key or modifier {part:?}")),
                };
            }
            _ => return Err(format!("duplicate or misplaced key/modifier {part:?}")),
        }
    }
    if chord.key.is_empty() {
        return Err("missing key".into());
    }
    if !chord.control
        && !chord.shift
        && !chord.alt
        && (chord.key.starts_with("Key") || chord.key.starts_with("Digit"))
    {
        return Err("plain text keys cannot be bound".into());
    }
    Ok(chord)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_startup_and_project_roots_parse() {
        let source = "[workspace]\nproject_root = 'repo'\nactive_tab = 0\n[[workspace.tabs]]\ntitle = 'Build'\nproject_root = 'tab'\nactive_pane = 0\n[workspace.tabs.layout]\nkind = 'pane'\nproject_root = 'pane'\n[workspace.tabs.layout.session.command]\nprogram = 'cargo'\nargs = ['watch']\n[[projects]]\nname = 'Core'\npath = 'core'";
        let config = Config::parse(source).unwrap();
        assert_eq!(config.projects[0].name, "Core");
        assert_eq!(config.workspace.tabs[0].project_root, Some("tab".into()));
        assert!(matches!(
            config.workspace.tabs[0].layout,
            LayoutDefinition::Pane {
                session: terminal_workspace::SessionDefinition::Command { .. },
                ..
            }
        ));
        assert!(
            Config::parse(
                "[[projects]]\nname = 'x'\npath = 'a'\n[[projects]]\nname = 'x'\npath = 'b'"
            )
            .is_err()
        );
    }
    #[test]
    fn loaded_roots_resolve_relative_to_definition_file() {
        let directory = std::env::temp_dir().join(format!(
            "terminal-config-roots-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("workspace.toml");
        std::fs::write(&file, "[workspace]\nproject_root = 'root'\n[[workspace.tabs]]\ntitle = 'Build'\nproject_root = 'tab'\n[workspace.tabs.layout]\nkind = 'pane'\nproject_root = 'pane'\nsession = 'local_shell'\n[[projects]]\nname = 'Core'\npath = 'core'").unwrap();
        let config = Config::load_workspace_file(&file).unwrap();
        assert_eq!(config.workspace.project_root, Some(directory.join("root")));
        assert_eq!(
            config.workspace.tabs[0].project_root,
            Some(directory.join("tab"))
        );
        assert!(
            matches!(&config.workspace.tabs[0].layout, LayoutDefinition::Pane { project_root: Some(root), .. } if root == &directory.join("pane"))
        );
        assert_eq!(config.projects[0].path, directory.join("core"));
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn defaults_and_partial_theme() {
        let config = Config::parse("[theme]\nforeground = '#123abc'").unwrap();
        assert_eq!(config.theme.foreground, Rgb(0x12, 0x3a, 0xbc));
        assert_eq!(config.theme.background, Theme::default().background);
        assert_eq!(config.theme.ui_accent, None);
        assert_eq!(config.bindings, Config::default().bindings);
    }
    #[test]
    fn old_theme_fields_and_partial_ui_override_parse_together() {
        let config = Config::parse(
            "[theme]\nforeground = '#123abc'\nbackground = '#010203'\ncursor = '#040506'\nselection_foreground = '#070809'\nselection_background = '#0a0b0c'\nui_accent = '#ff79c6'",
        )
        .unwrap();
        assert_eq!(config.theme.foreground, Rgb(0x12, 0x3a, 0xbc));
        assert_eq!(config.theme.background, Rgb(1, 2, 3));
        assert_eq!(config.theme.cursor, Rgb(4, 5, 6));
        assert_eq!(config.theme.selection_foreground, Rgb(7, 8, 9));
        assert_eq!(config.theme.selection_background, Rgb(10, 11, 12));
        assert_eq!(config.theme.ui_accent, Some(Rgb(255, 121, 198)));
        assert_eq!(config.theme.ui_background, None);
        assert_eq!(config.theme.search_match_background, None);
        assert_eq!(config.theme.ansi, Theme::default().ansi);
    }
    #[test]
    fn invalid_ui_colors_name_the_field() {
        for field in ["ui_accent", "search_match_background", "scrollbar_thumb"] {
            let source = format!("[theme]\n{field} = 'red'");
            let error = Config::parse(&source).unwrap_err().to_string();
            assert!(error.contains(&format!("theme.{field}: expected #RRGGBB")));
        }
    }
    #[test]
    fn repository_example_loads_through_config_file_path() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.example.toml");
        let config = Config::load(&path).unwrap();
        assert_eq!(config, Config::default());
    }
    #[test]
    fn font_defaults_and_partial_settings() {
        assert_eq!(Config::parse("").unwrap().font, FontConfig::default());
        let family = Config::parse("[font]\nfamily = 'Cascadia Mono'").unwrap();
        assert_eq!(family.font.family.as_deref(), Some("Cascadia Mono"));
        assert_eq!(family.font.size, 16);
        let size = Config::parse("[font]\nsize = 20").unwrap();
        assert_eq!(size.font.family, None);
        assert_eq!(size.font.size, 20);
    }
    #[test]
    fn invalid_font_settings_name_the_field() {
        for source in [
            "[font]\nsize = -1",
            "[font]\nsize = 0",
            "[font]\nsize = 257",
        ] {
            assert!(
                Config::parse(source)
                    .unwrap_err()
                    .to_string()
                    .contains("font.size")
            );
        }
        assert!(Config::parse("[font]\nsize = 16.5").is_err());
        assert!(
            Config::parse("[font]\nfamily = '  '")
                .unwrap_err()
                .to_string()
                .contains("font.family")
        );
        assert!(Config::parse("[font]\nfallback = 'Other'").is_err());
    }
    #[test]
    fn bindings_replace_defaults_and_are_typed() {
        let config = Config::parse("[[bindings]]\nkey = 'Alt+PageUp'\naction = 'page_up'").unwrap();
        assert_eq!(config.bindings.len(), 1);
        assert_eq!(config.bindings[0].key.key, "PageUp");
        assert_eq!(config.bindings[0].command, Command::PageUp);
        let config =
            Config::parse("[[bindings]]\nkey = 'Ctrl+P'\ncommand = 'open_palette'").unwrap();
        assert_eq!(config.bindings[0].command, Command::OpenPalette);
        assert!(
            Config::default()
                .bindings
                .iter()
                .any(|binding| binding.command == Command::OpenPalette)
        );
    }

    #[test]
    fn font_size_shortcuts_are_configurable_physical_chords() {
        let config = Config::parse(
            "[[bindings]]\nkey = 'Ctrl++'\ncommand = 'increase_font_size'\n[[bindings]]\nkey = 'Ctrl+-'\ncommand = 'decrease_font_size'\n[[bindings]]\nkey = 'Ctrl+0'\ncommand = 'reset_font_size'",
        )
        .unwrap();
        assert_eq!(config.bindings[0].key.key, "Equal");
        assert!(config.bindings[0].key.shift);
        assert_eq!(config.bindings[0].command, Command::IncreaseFontSize);
        assert_eq!(config.bindings[1].key.key, "Minus");
        assert_eq!(config.bindings[1].command, Command::DecreaseFontSize);
        assert_eq!(config.bindings[2].command, Command::ResetFontSize);

        let defaults = Config::default();
        for command in [
            Command::IncreaseFontSize,
            Command::DecreaseFontSize,
            Command::ResetFontSize,
        ] {
            assert!(
                defaults
                    .bindings
                    .iter()
                    .any(|binding| binding.command == command)
            );
        }
    }
    #[test]
    fn backspace_is_a_supported_physical_binding_key() {
        let config =
            Config::parse("[[bindings]]\nkey = 'Ctrl+Shift+Backspace'\ncommand = 'close_pane'")
                .unwrap();
        assert_eq!(config.bindings.len(), 1);
        assert_eq!(config.bindings[0].key.key, "Backspace");
        assert!(config.bindings[0].key.control);
        assert!(config.bindings[0].key.shift);
        assert_eq!(config.bindings[0].command, Command::ClosePane);
    }
    #[test]
    fn validation_names_the_bad_field() {
        for (source, field) in [
            ("[theme]\nbackground = 'red'", "theme.background"),
            ("[theme]\nansi = ['#000000']", "theme.ansi"),
            (
                "[[bindings]]\nkey = 'Ctrl+Nope'\naction = 'copy'",
                "bindings[0].key",
            ),
            (
                "[[bindings]]\nkey = 'A'\naction = 'copy'",
                "bindings[0].key",
            ),
            (
                "[[bindings]]\nkey = 'Ctrl+C'\naction = 'copy'\n[[bindings]]\nkey = 'Control+C'\naction = 'paste'",
                "bindings[1].key",
            ),
        ] {
            assert!(
                Config::parse(source)
                    .unwrap_err()
                    .to_string()
                    .contains(field)
            );
        }
        assert!(Config::parse("[them]\nx = 1").is_err());
    }

    #[test]
    fn workspace_definition_parses_and_validates_focus() {
        let source = "[workspace]\nactive_tab = 0\n[[workspace.tabs]]\ntitle = 'Development'\nactive_pane = 1\nlayout = { kind = 'split', axis = 'vertical', first = { kind = 'pane', session = 'local_shell' }, second = { kind = 'pane', session = 'local_shell' } }";
        let config = Config::parse(source).unwrap();
        assert_eq!(
            Workspace::from_definition(&config.workspace)
                .unwrap()
                .panes()
                .len(),
            2
        );
        assert_eq!(config.workspace.tabs[0].title, "Development");
        assert!(
            Config::parse(&source.replace("active_pane = 1", "active_pane = 2"))
                .unwrap_err()
                .to_string()
                .contains("active_pane")
        );
    }

    #[test]
    fn default_workspace_bindings_use_supported_physical_keys() {
        let config = Config::default();
        assert_eq!(config.workspace, WorkspaceDefinition::default());
        let mut chords = HashSet::new();
        for binding in &config.bindings {
            assert!(
                chords.insert(binding.key.clone()),
                "duplicate default chord"
            );
        }
        for command in [
            Command::NewTab,
            Command::OpenTabPicker,
            Command::SplitHorizontal,
            Command::SplitVertical,
            Command::NextTab,
            Command::PreviousTab,
            Command::NextPane,
            Command::PreviousPane,
            Command::ResizePaneLeft,
            Command::ResizePaneRight,
            Command::ResizePaneUp,
            Command::ResizePaneDown,
            Command::FocusPaneLeft,
            Command::FocusPaneRight,
            Command::FocusPaneUp,
            Command::FocusPaneDown,
            Command::TogglePaneZoom,
            Command::ClosePane,
        ] {
            assert!(
                config
                    .bindings
                    .iter()
                    .any(|binding| binding.command == command)
            );
        }
        let mut chords = std::collections::HashSet::new();
        assert!(
            config
                .bindings
                .iter()
                .all(|binding| chords.insert(&binding.key))
        );
    }

    #[test]
    fn directional_pane_commands_accept_configured_up_and_down_chords() {
        let config = Config::parse(
            "[[bindings]]\nkey = 'Ctrl+ArrowUp'\ncommand = 'focus_pane_up'\n[[bindings]]\nkey = 'Ctrl+ArrowDown'\ncommand = 'focus_pane_down'",
        )
        .unwrap();
        assert_eq!(config.bindings.len(), 2);
        assert_eq!(config.bindings[0].key.key, "ArrowUp");
        assert_eq!(config.bindings[0].command, Command::FocusPaneUp);
        assert_eq!(config.bindings[1].key.key, "ArrowDown");
        assert_eq!(config.bindings[1].command, Command::FocusPaneDown);
    }
}
