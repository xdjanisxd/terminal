//! Opt-in timing from main entry; presentation means submission to the window system.
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static STARTUP: OnceLock<Option<StartupDiagnostics>> = OnceLock::new();

struct StartupDiagnostics {
    started: Instant,
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
        Some(StartupDiagnostics { started, file })
    });
    startup_milestone("application-started");
}

/// Records a milestone without clock reads or output on the normal startup path.
pub fn startup_milestone(stage: &str) {
    let Some(Some(diagnostics)) = STARTUP.get() else {
        return;
    };
    let elapsed = diagnostics.started.elapsed().as_micros();
    diagnostics.write(format_args!(
        "terminal-startup stage={stage} elapsed_us={elapsed}"
    ));
}

impl StartupDiagnostics {
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
