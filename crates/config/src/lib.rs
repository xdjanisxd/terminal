//! Typed, validated persistent configuration. Runtime state stays in the app.

use serde::Deserialize;
use std::{collections::HashSet, fmt, fs, path::Path};

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
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: Rgb(230, 230, 230),
            background: Rgb(0, 0, 0),
            cursor: Rgb(230, 230, 230),
            selection_foreground: Rgb(255, 255, 255),
            selection_background: Rgb(46, 89, 166),
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
    OpenPalette,
    OpenTarget,
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
    pub theme: Theme,
    pub bindings: Vec<Binding>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            bindings: [
                ("Ctrl+Shift+C", Command::Copy),
                ("Ctrl+Shift+V", Command::Paste),
                ("PageUp", Command::PageUp),
                ("PageDown", Command::PageDown),
                ("Ctrl+Shift+P", Command::OpenPalette),
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
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        let raw: RawConfig =
            toml::from_str(source).map_err(|error| ConfigError(error.to_string()))?;
        let mut config = Self::default();
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
        Self::parse(&source).map_err(|error| ConfigError(format!("{}: {error}", path.display())))
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
    theme: Option<RawTheme>,
    bindings: Option<Vec<RawBinding>>,
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
                    "pageup" => "PageUp".into(),
                    "pagedown" => "PageDown".into(),
                    "home" => "Home".into(),
                    "end" => "End".into(),
                    "insert" => "Insert".into(),
                    "delete" => "Delete".into(),
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
    fn defaults_and_partial_theme() {
        let config = Config::parse("[theme]\nforeground = '#123abc'").unwrap();
        assert_eq!(config.theme.foreground, Rgb(0x12, 0x3a, 0xbc));
        assert_eq!(config.theme.background, Theme::default().background);
        assert_eq!(config.bindings, Config::default().bindings);
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
}
