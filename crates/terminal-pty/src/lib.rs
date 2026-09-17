//! Project-owned boundary for local pseudo-terminal sessions.
//!
//! This crate transports raw bytes only. It neither decodes output nor depends on
//! terminal parsing, screen state, rendering, input encoding, or a backend API.
//! A future `portable-pty` adapter implements `PtyBackend` and `PtySession`.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

/// A validated character-cell size for a PTY session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PtySize {
    rows: u16,
    columns: u16,
}

impl PtySize {
    pub fn new(rows: u16, columns: u16) -> Result<Self, PtySizeError> {
        if rows == 0 {
            return Err(PtySizeError::ZeroRows);
        }
        if columns == 0 {
            return Err(PtySizeError::ZeroColumns);
        }
        Ok(Self { rows, columns })
    }
    pub const fn rows(self) -> u16 {
        self.rows
    }
    pub const fn columns(self) -> u16 {
        self.columns
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PtySizeError {
    ZeroRows,
    ZeroColumns,
}
impl fmt::Display for PtySizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroRows => "PTY rows must be nonzero",
            Self::ZeroColumns => "PTY columns must be nonzero",
        })
    }
}
impl Error for PtySizeError {}

/// Mechanism-only request for a future local child spawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PtySpawnConfig {
    program: PathBuf,
    arguments: Vec<OsString>,
    working_directory: Option<PathBuf>,
    environment: Vec<(OsString, OsString)>,
    initial_size: PtySize,
}
impl PtySpawnConfig {
    pub fn new(program: PathBuf, initial_size: PtySize) -> Self {
        Self {
            program,
            arguments: Vec::new(),
            working_directory: None,
            environment: Vec::new(),
            initial_size,
        }
    }
    pub fn with_arguments(mut self, arguments: impl IntoIterator<Item = OsString>) -> Self {
        self.arguments = arguments.into_iter().collect();
        self
    }
    pub fn with_working_directory(mut self, directory: PathBuf) -> Self {
        self.working_directory = Some(directory);
        self
    }
    pub fn with_environment(
        mut self,
        environment: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Self {
        self.environment = environment.into_iter().collect();
        self
    }
    pub fn program(&self) -> &Path {
        &self.program
    }
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    pub fn working_directory(&self) -> Option<&Path> {
        self.working_directory.as_deref()
    }
    pub fn environment(&self) -> &[(OsString, OsString)] {
        &self.environment
    }
    pub const fn initial_size(&self) -> PtySize {
        self.initial_size
    }
}

/// Project-owned child exit information; absent code means backend unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PtyExitStatus {
    code: Option<i32>,
}
impl PtyExitStatus {
    pub const fn code(code: i32) -> Self {
        Self { code: Some(code) }
    }
    pub const fn unknown() -> Self {
        Self { code: None }
    }
    pub const fn code_value(self) -> Option<i32> {
        self.code
    }
}

/// A future worker's raw transport event. Bytes are never decoded or normalized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyOutput {
    Bytes(Vec<u8>),
    Eof,
    Exited(PtyExitStatus),
}
impl PtyOutput {
    /// Empty chunks are not events, avoiding meaningless transport wakeups.
    pub fn bytes(bytes: Vec<u8>) -> Option<Self> {
        (!bytes.is_empty()).then_some(Self::Bytes(bytes))
    }
}

/// EOF is an output event, not a lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PtyLifecycle {
    Running,
    TerminationRequested,
    Exited(PtyExitStatus),
}
impl PtyLifecycle {
    pub const fn allows_operations(self) -> bool {
        matches!(self, Self::Running)
    }
    pub const fn termination_is_idempotent(self) -> bool {
        matches!(self, Self::TerminationRequested | Self::Exited(_))
    }
}

/// Caller-relevant project-owned PTY failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyError {
    SpawnFailed,
    WriteFailed,
    ResizeFailed,
    TerminateFailed,
    NotRunning,
}
impl fmt::Display for PtyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SpawnFailed => "PTY spawn failed",
            Self::WriteFailed => "PTY input write failed",
            Self::ResizeFailed => "PTY resize failed",
            Self::TerminateFailed => "PTY termination failed",
            Self::NotRunning => "PTY session is not running",
        })
    }
}
impl Error for PtyError {}

/// Backend-independent live session capability.
///
/// The session owner calls `terminate` for deterministic shutdown; `Drop` is only
/// a backend safety net. Calls after exit or termination request return
/// `PtyError::NotRunning`; repeated termination succeeds without another effect.
pub trait PtySession {
    fn lifecycle(&self) -> PtyLifecycle;
    fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError>;
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError>;
    fn terminate(&mut self) -> Result<(), PtyError>;
}

/// A future backend seam that keeps adapter types out of the rest of the project.
pub trait PtyBackend {
    type Session: PtySession;
    fn spawn(&self, configuration: PtySpawnConfig) -> Result<Self::Session, PtyError>;
}
