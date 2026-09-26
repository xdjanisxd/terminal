//! App-managed reusable workspace definitions. No terminal or PTY state is stored here.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use terminal_workspace::{Workspace, WorkspaceDefinition};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedWorkspace {
    pub name: String,
    pub definition: WorkspaceDefinition,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedWorkspaces {
    pub workspaces: Vec<SavedWorkspace>,
}

impl SavedWorkspaces {
    pub fn load(path: &Path) -> Result<Self, String> {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let backup = path.with_extension("toml.bak");
                match fs::read_to_string(backup) {
                    Ok(source) => source,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(Self::default());
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
            Err(error) => return Err(error.to_string()),
        };
        let store: Self = toml::from_str(&source).map_err(|error| error.to_string())?;
        let mut names = std::collections::HashSet::new();
        for item in &store.workspaces {
            if item.name.trim() != item.name || item.name.is_empty() || !names.insert(&item.name) {
                return Err("invalid or duplicate saved workspace name".into());
            }
            Workspace::from_definition(&item.definition).map_err(|error| error.to_string())?;
        }
        Ok(store)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.workspaces.iter().any(|item| item.name == name)
    }

    pub fn save(&mut self, name: String, definition: WorkspaceDefinition) -> Result<(), String> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Err("workspace name must not be empty".into());
        }
        Workspace::from_definition(&definition).map_err(|error| error.to_string())?;
        if let Some(item) = self.workspaces.iter_mut().find(|item| item.name == name) {
            item.definition = definition;
        } else {
            self.workspaces.push(SavedWorkspace { name, definition });
        }
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> bool {
        let old_len = self.workspaces.len();
        self.workspaces.retain(|item| item.name != name);
        self.workspaces.len() != old_len
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let source = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        let parent = path.parent().ok_or("workspace state path has no parent")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = temporary_path(path);
        let result = (|| -> io::Result<()> {
            let mut file = File::create(&temporary)?;
            file.write_all(source.as_bytes())?;
            file.sync_all()?;
            // Windows cannot rename onto an existing file. Preserve a recoverable
            // previous copy across the short replacement interval.
            if path.exists() {
                let backup = path.with_extension("toml.bak");
                if backup.exists() {
                    fs::remove_file(&backup)?;
                }
                fs::rename(path, &backup)?;
                if let Err(error) = fs::rename(&temporary, path) {
                    let _ = fs::rename(&backup, path);
                    return Err(error);
                }
                let _ = fs::remove_file(backup);
            } else {
                fs::rename(&temporary, path)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(|error| error.to_string())
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("toml.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty_and_interrupted_replace_recovers_backup() {
        let path = std::env::temp_dir().join(format!(
            "terminal-saved-workspaces-{}-{:?}.toml",
            std::process::id(),
            std::thread::current().id()
        ));
        assert!(SavedWorkspaces::load(&path).unwrap().workspaces.is_empty());
        let mut store = SavedWorkspaces::default();
        store
            .save("Mine".into(), WorkspaceDefinition::default())
            .unwrap();
        store.write(&path).unwrap();
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("startup_command")
        );
        assert!(SavedWorkspaces::load(&path).unwrap().contains("Mine"));
        let backup = path.with_extension("toml.bak");
        fs::rename(&path, &backup).unwrap();
        assert!(SavedWorkspaces::load(&path).unwrap().contains("Mine"));
        fs::remove_file(backup).unwrap();
    }
}
