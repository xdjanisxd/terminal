use std::ffi::OsString;
use std::path::PathBuf;

use terminal_pty::{
    PortablePtyBackend, PtyExitStatus, PtyLifecycle, PtyOutput, PtySize, PtySizeError,
    PtySpawnConfig,
};

#[test]
fn size_rejects_zero_dimensions() {
    assert_eq!(PtySize::new(0, 80), Err(PtySizeError::ZeroRows));
    assert_eq!(PtySize::new(24, 0), Err(PtySizeError::ZeroColumns));
    assert_eq!(PtySize::new(24, 80).unwrap().rows(), 24);
    assert_eq!(PtySize::new(24, 80).unwrap().columns(), 80);
}

#[test]
fn spawn_config_owns_platform_arguments_environment_and_working_directory() {
    let program = PathBuf::from("C:/Program Files/example.exe");
    let directory = PathBuf::from("C:/work area");
    let argument = OsString::from("--raw=\u{fffd}");
    let environment = (OsString::from("TERM"), OsString::from("xterm-256color"));
    let config = PtySpawnConfig::new(program.clone(), PtySize::new(24, 80).unwrap())
        .with_arguments([argument.clone()])
        .with_working_directory(directory.clone())
        .with_environment([environment.clone()]);

    assert_eq!(config.program(), program.as_path());
    assert_eq!(config.arguments(), [argument]);
    assert_eq!(config.working_directory(), Some(directory.as_path()));
    assert_eq!(config.environment(), [environment]);
    assert_eq!(config.initial_size(), PtySize::new(24, 80).unwrap());
}

#[test]
fn output_preserves_arbitrary_bytes_and_distinguishes_eof_from_exit() {
    let bytes = vec![0xff, 0x1b, b'[', 0xe2, 0x82];
    assert_eq!(
        PtyOutput::bytes(bytes.clone()),
        Some(PtyOutput::Bytes(bytes))
    );
    assert_eq!(PtyOutput::bytes(Vec::new()), None);
    assert_ne!(PtyOutput::Eof, PtyOutput::Exited(PtyExitStatus::code(7)));
}

#[test]
fn portable_backend_is_project_owned_and_constructible() {
    let _backend = PortablePtyBackend::new();
}

#[test]
fn lifecycle_keeps_exit_and_termination_distinct() {
    assert!(PtyLifecycle::Running.allows_operations());
    assert!(!PtyLifecycle::TerminationRequested.allows_operations());
    assert!(!PtyLifecycle::Exited(PtyExitStatus::unknown()).allows_operations());
    assert!(PtyLifecycle::TerminationRequested.termination_is_idempotent());
}
