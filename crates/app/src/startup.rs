//! App-owned opt-in session diagnostics. These observations never gate startup.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use terminal_core::TerminalState;
use terminal_pty::PtyStartupObserver;
use terminal_renderer::{startup_diagnostics_enabled, startup_milestone};
use terminal_workspace::PaneId;

static SESSIONS: OnceLock<Mutex<HashMap<PaneId, Progress>>> = OnceLock::new();

#[derive(Default)]
struct Progress {
    seen: HashSet<&'static str>,
    marker: Vec<u8>,
}

impl Progress {
    fn first(&mut self, stage: &'static str) -> bool {
        self.seen.insert(stage)
    }

    fn presented(&mut self) -> bool {
        self.seen.contains("first-shell-text-processed")
            && self.first("first-shell-output-rendered")
    }

    fn shell_text(&mut self, terminal: &TerminalState) -> bool {
        if self.seen.contains("first-shell-text-processed") {
            return false;
        }
        let grid = terminal.screen();
        let size = grid.dimensions();
        let has_text = (0..size.rows()).any(|row| {
            (0..size.columns()).any(|column| {
                grid.cell(row, column)
                    .is_some_and(|cell| !cell.character().is_whitespace())
            })
        });
        has_text && self.first("first-shell-text-processed")
    }

    // A bounded observer for diagnostic-only OSCs, independent of terminal parsing.
    // It handles markers split across arbitrary raw PTY chunks without retaining output.
    fn shell_markers(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut found = Vec::new();
        for &byte in bytes {
            if byte == 27 {
                self.marker.clear();
                self.marker.push(byte);
            } else if !self.marker.is_empty() {
                if byte == 7 {
                    if let Some(payload) = self.marker.strip_prefix(b"\x1b]9;terminal-startup;") {
                        let text = String::from_utf8_lossy(payload);
                        if text == "shell-integration-begin"
                            || text.starts_with("shell-integration-end shell_duration_us=")
                        {
                            found.push(text.into_owned());
                        }
                    }
                    self.marker.clear();
                } else if self.marker.len() < 256 {
                    self.marker.push(byte);
                } else {
                    self.marker.clear();
                }
            }
        }
        found
    }
}

fn with_progress(pane: PaneId, f: impl FnOnce(&mut Progress)) {
    if !startup_diagnostics_enabled() {
        return;
    }
    if let Ok(mut sessions) = SESSIONS.get_or_init(Default::default).lock() {
        f(sessions.entry(pane).or_default());
    }
}

pub fn milestone(pane: PaneId, stage: &'static str) {
    with_progress(pane, |progress| {
        if progress.first(stage) {
            startup_milestone(&format!("{stage} session={pane:?}"));
        }
    });
}

pub fn observer(pane: PaneId) -> Option<PtyStartupObserver> {
    startup_diagnostics_enabled().then(|| {
        with_progress(pane, |progress| *progress = Progress::default());
        Arc::new(move |stage| milestone(pane, stage)) as PtyStartupObserver
    })
}

pub fn output(pane: PaneId, bytes: &[u8], terminal: &TerminalState) {
    with_progress(pane, |progress| {
        if progress.shell_text(terminal) {
            startup_milestone(&format!("first-shell-text-processed session={pane:?}"));
        }
        for marker in progress.shell_markers(bytes) {
            startup_milestone(&format!("{marker} session={pane:?}"));
        }
    });
    milestone(pane, "first-output-processed");
    if terminal.shell_prompt_version() != 0 {
        milestone(pane, "prompt-ready");
        milestone(pane, "shell-ready");
    }
}

pub fn presented(pane: PaneId) {
    with_progress(pane, |progress| {
        if progress.presented() {
            startup_milestone(&format!("first-shell-output-rendered session={pane:?}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::Progress;

    #[test]
    fn presentation_requires_processed_output_and_is_recorded_once() {
        let mut progress = Progress::default();
        assert!(!progress.presented());
        assert!(progress.first("first-shell-text-processed"));
        assert!(progress.presented());
        assert!(!progress.presented());
        assert!(!progress.first("first-shell-text-processed"));
    }

    #[test]
    fn control_only_output_does_not_count_as_shell_text() {
        let mut progress = Progress::default();
        let mut terminal = terminal_core::TerminalState::new(
            terminal_core::TerminalDimensions::new(80, 24).unwrap(),
        );
        let mut parser = terminal_core::TerminalParser::new();
        parser
            .advance(&mut terminal, b"\x1b[6n\x1b]133;A\x07")
            .unwrap();
        assert!(!progress.shell_text(&terminal));
        assert!(!progress.presented());
        parser.advance(&mut terminal, b"PS>").unwrap();
        assert!(progress.shell_text(&terminal));
        assert!(progress.presented());
        assert!(!progress.shell_text(&terminal));
    }

    #[test]
    fn diagnostic_markers_are_bounded_and_survive_chunk_boundaries() {
        let mut progress = Progress::default();
        assert!(
            progress
                .shell_markers(b"banner\x1b]9;terminal-start")
                .is_empty()
        );
        assert_eq!(
            progress.shell_markers(b"up;shell-integration-begin\x07"),
            ["shell-integration-begin"]
        );
        assert_eq!(
            progress.shell_markers(
                b"\x1b]9;terminal-startup;shell-integration-end shell_duration_us=42\x07"
            ),
            ["shell-integration-end shell_duration_us=42"]
        );
        assert!(progress.shell_markers(b"\x1b]7;file:///C:/\x07").is_empty());
        progress.shell_markers(b"\x1b]");
        assert!(progress.shell_markers(&[b'x'; 1024]).is_empty());
        assert!(progress.marker.is_empty());
    }
}
