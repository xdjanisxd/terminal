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
        let paths = PersistencePaths::new(path);
        let source = match paths.observe("read destination", || fs::read_to_string(path)) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match paths.observe("read backup", || fs::read_to_string(&paths.backup)) {
                    Ok(source) => source,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(Self::default());
                    }
                    Err(error) => return Err(paths.error("read backup", error)),
                }
            }
            Err(error) => return Err(paths.error("read destination", error)),
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
        self.write_with_install(path, |temporary, destination| {
            fs::rename(temporary, destination)
        })
    }

    // Keep the final rename injectable so tests can deterministically exercise
    // restoration after the previous destination has been moved to its backup.
    fn write_with_install(
        &self,
        path: &Path,
        install: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), String> {
        let source = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        let parent = path.parent().ok_or("workspace state path has no parent")?;
        let paths = PersistencePaths::new(path);
        paths.run("create_dir_all(parent)", || fs::create_dir_all(parent))?;
        let result = (|| -> Result<(), String> {
            let mut file = paths.run("create temporary file", || File::create(&paths.temporary))?;
            paths.run("write temporary file", || file.write_all(source.as_bytes()))?;
            paths.run("sync_all temporary file", || file.sync_all())?;
            paths.run("close temporary file", || {
                drop(file);
                Ok(())
            })?;
            // Windows cannot rename onto an existing file. Preserve a recoverable
            // previous copy across the short replacement interval.
            if paths.run("check destination existence", || path.try_exists())? {
                if paths.run("check backup existence", || paths.backup.try_exists())? {
                    paths.run("remove previous backup", || fs::remove_file(&paths.backup))?;
                }
                paths.run("rename destination -> backup", || {
                    fs::rename(path, &paths.backup)
                })?;
                if let Err(mut error) = paths.run("rename temporary -> destination", || {
                    install(&paths.temporary, path)
                }) {
                    if let Err(restore_error) = paths.run("restore backup -> destination", || {
                        fs::rename(&paths.backup, path)
                    }) {
                        error.push_str(&format!("; {restore_error}"));
                    }
                    return Err(error);
                }
                // Backup cleanup is best effort once the new destination is installed.
                let _ = paths.run("remove backup after replacement", || {
                    fs::remove_file(&paths.backup)
                });
            } else {
                paths.run("rename temporary -> destination", || {
                    install(&paths.temporary, path)
                })?;
            }
            Ok(())
        })();
        if let Err(mut error) = result {
            if let Err(cleanup_error) = paths.run("remove temporary after failure", || {
                fs::remove_file(&paths.temporary)
            }) {
                error.push_str(&format!("; {cleanup_error}"));
            }
            return Err(error);
        }
        Ok(())
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("toml.{}.tmp", std::process::id()))
}

struct PersistencePaths<'a> {
    destination: &'a Path,
    temporary: PathBuf,
    backup: PathBuf,
}

impl<'a> PersistencePaths<'a> {
    fn new(destination: &'a Path) -> Self {
        Self {
            destination,
            temporary: temporary_path(destination),
            backup: destination.with_extension("toml.bak"),
        }
    }

    fn observe<T>(&self, operation: &str, action: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        #[cfg(test)]
        eprintln!("Saved Workspace before {operation}: {}", self.diagnostics());
        let result = action();
        #[cfg(test)]
        if let Err(error) = &result {
            eprintln!(
                "Saved Workspace {operation} failed: {error} (raw_os_error={:?}); after: {}",
                error.raw_os_error(),
                self.diagnostics()
            );
        }
        #[cfg(not(test))]
        let _ = operation;
        result
    }

    fn run<T>(&self, operation: &str, action: impl FnOnce() -> io::Result<T>) -> Result<T, String> {
        self.observe(operation, action)
            .map_err(|error| self.error(operation, error))
    }

    fn error(&self, operation: &str, error: io::Error) -> String {
        format!(
            "{operation}: {error} (raw_os_error={:?}, destination={:?}, temporary={:?}, backup={:?})",
            error.raw_os_error(),
            self.destination,
            self.temporary,
            self.backup
        )
    }

    #[cfg(test)]
    fn diagnostics(&self) -> String {
        let cwd = std::env::current_dir();
        let mut details = format!(
            "pid={}, cwd={cwd:?}, temp_dir={:?}",
            std::process::id(),
            std::env::temp_dir()
        );
        for (name, path) in [
            ("destination", self.destination),
            ("temporary", self.temporary.as_path()),
            ("backup", self.backup.as_path()),
        ] {
            let full = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.as_ref()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| path.to_path_buf())
            };
            let parent = full.parent();
            details.push_str(&format!("; {name}={full:?}, exists={:?}, parent={parent:?}, parent_exists={:?}, canonical_parent={:?}",
                path.try_exists(), parent.map(Path::try_exists), parent.map(fs::canonicalize)));
        }
        for name in [
            "TMP",
            "TEMP",
            "TMPDIR",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "SystemRoot",
            "WINDIR",
            "HOME",
            "XDG_CONFIG_HOME",
            "TERMINAL_CONFIG",
        ] {
            details.push_str(&format!("; {name}={:?}", std::env::var_os(name)));
        }
        details
    }
}

#[cfg(test)]
pub(crate) struct TestWorkspaceState {
    root: PathBuf,
}

#[cfg(test)]
impl TestWorkspaceState {
    pub(crate) fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "terminal saved workspace Unicode-工作区-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "create test directory {root:?}: {error}; {}",
                    PersistencePaths::new(&root.join("workspaces.toml")).diagnostics()
                ),
            }
        }
    }

    pub(crate) fn path(&self) -> PathBuf {
        self.root.join("app state").join("workspaces.toml")
    }
}

#[cfg(test)]
impl Drop for TestWorkspaceState {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("remove test directory {:?}: {error}", self.root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty_and_interrupted_replace_recovers_backup() {
        let state = TestWorkspaceState::new();
        let path = state.path();
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

    fn store(name: &str) -> SavedWorkspaces {
        let mut store = SavedWorkspaces::default();
        store
            .save(name.into(), WorkspaceDefinition::default())
            .unwrap();
        store
    }

    #[test]
    fn creates_missing_parent_round_trips_and_replaces_existing_file() {
        for extension in [Some("toml"), None, Some("custom")] {
            let state = TestWorkspaceState::new();
            let path = match extension {
                Some(extension) => state.path().with_extension(extension),
                None => state.path().with_extension(""),
            };
            let paths = PersistencePaths::new(&path);
            assert_eq!(paths.temporary.parent(), path.parent());
            assert_eq!(paths.backup.parent(), path.parent());
            assert!(!path.parent().unwrap().exists());
            assert!(SavedWorkspaces::load(&path).unwrap().workspaces.is_empty());
            store("Before").write(&path).unwrap();
            assert!(SavedWorkspaces::load(&path).unwrap().contains("Before"));
            store("After").write(&path).unwrap();
            let loaded = SavedWorkspaces::load(&path).unwrap();
            assert!(loaded.contains("After"));
            assert!(!loaded.contains("Before"));
            assert!(!paths.temporary.exists());
            assert!(!paths.backup.exists());
        }
    }

    #[test]
    fn failed_install_restores_previous_file_and_cleans_temporary() {
        let state = TestWorkspaceState::new();
        let path = state.path();
        let paths = PersistencePaths::new(&path);
        store("Before").write(&path).unwrap();
        let before = fs::read(&path).unwrap();
        let error = store("After")
            .write_with_install(&path, |temporary, destination| {
                assert!(temporary.is_file());
                assert!(!destination.exists());
                assert_eq!(fs::read(&paths.backup).unwrap(), before);
                // A real failing rename, without relying on permissions or timing.
                fs::rename(temporary.with_extension("missing"), destination)
            })
            .unwrap_err();
        assert!(error.contains("rename temporary -> destination"), "{error}");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(SavedWorkspaces::load(&path).unwrap().contains("Before"));
        assert!(!paths.temporary.exists());
        assert!(!paths.backup.exists());
    }

    #[test]
    fn concurrent_saves_own_separate_directories() {
        let states: Vec<_> = (0..8).map(|_| TestWorkspaceState::new()).collect();
        let paths: std::collections::HashSet<_> =
            states.iter().map(TestWorkspaceState::path).collect();
        assert_eq!(paths.len(), states.len());
        let barrier = std::sync::Barrier::new(states.len());
        std::thread::scope(|scope| {
            for (index, state) in states.iter().enumerate() {
                let barrier = &barrier;
                scope.spawn(move || {
                    let path = state.path();
                    let name = format!("Workspace {index}");
                    barrier.wait();
                    store(&name).write(&path).unwrap();
                    store(&name).write(&path).unwrap();
                    let loaded = SavedWorkspaces::load(&path).unwrap();
                    assert_eq!(loaded.workspaces.len(), 1);
                    assert!(loaded.contains(&name));
                });
            }
        });
    }
}
