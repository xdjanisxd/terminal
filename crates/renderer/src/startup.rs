//! Opt-in timing from main entry; presentation means submission to the window system.
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static STARTUP: OnceLock<Option<StartupDiagnostics>> = OnceLock::new();

struct StartupDiagnostics {
    started: Instant,
    stages_open: AtomicBool,
    file: Option<Mutex<File>>,
}

/// Initializes startup timing once. Disabled unless TERMINAL_STARTUP_DIAGNOSTICS=1.
/// TERMINAL_STARTUP_LOG optionally redirects timing to a file for GUI launches.
pub fn initialize_startup_diagnostics(started: Instant) {
    STARTUP.get_or_init(|| {
        if !std::env::var_os("TERMINAL_STARTUP_DIAGNOSTICS").is_some_and(|v| v == "1") {
            return None;
        }
        let file =
            std::env::var_os("TERMINAL_STARTUP_LOG").and_then(|path| match File::create(path) {
                Ok(file) => Some(Mutex::new(file)),
                Err(error) => {
                    eprintln!("could not open startup diagnostic log: {error}");
                    None
                }
            });
        Some(StartupDiagnostics {
            started,
            file,
            stages_open: AtomicBool::new(true),
        })
    });
    startup_milestone("application-started");
}

/// Whether startup timing was explicitly enabled at application entry.
pub fn startup_diagnostics_enabled() -> bool {
    matches!(STARTUP.get(), Some(Some(_)))
}

/// GPU request metadata without extra adapter queries or output when disabled.
pub(crate) fn gpu_detail(message: fmt::Arguments<'_>) {
    if let Some(Some(diagnostics)) = STARTUP.get() {
        diagnostics.write(format_args!("terminal-startup gpu {message}"));
    }
}

/// Records a milestone without clock reads or output on the normal startup path.
pub fn startup_milestone(stage: &str) {
    let Some(Some(diagnostics)) = STARTUP.get() else {
        return;
    };
    if stage == "window-shown" {
        diagnostics.stages_open.store(false, Ordering::Relaxed);
    }
    let elapsed = diagnostics.started.elapsed().as_micros();
    diagnostics.write(format_args!(
        "terminal-startup stage={stage} elapsed_us={elapsed}"
    ));
}

/// A monotonic CPU-side startup scope. No clock reads when disabled or after show.
/// GPU timings measure API calls, not GPU completion or compositor scanout.
pub struct StartupStage {
    name: &'static str,
    started: Option<Instant>,
}

/// Measures a dependency stage through native window show; existing milestones
/// continue to observe shell startup afterward. Nested scopes are inclusive.
pub fn startup_stage(name: &'static str) -> StartupStage {
    StartupStage {
        name,
        started: STARTUP
            .get()
            .and_then(|d| d.as_ref())
            .and_then(StartupDiagnostics::stage_started),
    }
}

impl Drop for StartupStage {
    fn drop(&mut self) {
        if let (Some(started), Some(Some(diagnostics))) = (self.started, STARTUP.get()) {
            diagnostics.write(format_args!(
                "terminal-startup cost={} duration_us={}",
                self.name,
                started.elapsed().as_micros()
            ));
        }
    }
}

impl StartupDiagnostics {
    fn stage_started(&self) -> Option<Instant> {
        self.stages_open.load(Ordering::Relaxed).then(Instant::now)
    }
    fn write(&self, message: fmt::Arguments<'_>) {
        if let Some(file) = &self.file {
            if let Ok(mut file) = file.lock() {
                let _ = writeln!(file, "{message}");
            }
        } else {
            eprintln!("{message}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_scope_has_no_clock_or_output() {
        // No diagnostics are initialized in this test process.
        assert!(startup_stage("unused").started.is_none());
    }

    #[test]
    fn scopes_stop_when_measurement_window_closes() {
        let diagnostics = StartupDiagnostics {
            started: Instant::now(),
            file: None,
            stages_open: AtomicBool::new(true),
        };
        assert!(diagnostics.stage_started().is_some());
        diagnostics.stages_open.store(false, Ordering::Relaxed);
        assert!(diagnostics.stage_started().is_none());
    }
}
