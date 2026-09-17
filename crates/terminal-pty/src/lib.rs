//! Project-owned boundary for local pseudo-terminal sessions.
//!
//! This crate transports raw bytes only. It neither decodes output nor depends on
//! terminal parsing, screen state, rendering, input encoding, or a backend API.
//! A future `portable-pty` adapter implements `PtyBackend` and `PtySession`.

use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{CommandBuilder, MasterPty, PtyPair, PtySystem};

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
pub trait PtyOutputReader {
    /// Reads raw PTY bytes without decoding, parsing, or normalization.
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, PtyError>;
}

pub trait PtySession {
    fn lifecycle(&self) -> PtyLifecycle;
    fn take_output_reader(&mut self) -> Result<Box<dyn PtyOutputReader + Send>, PtyError>;
    fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError>;
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError>;
    fn terminate(&mut self) -> Result<(), PtyError>;
}

/// A future backend seam that keeps adapter types out of the rest of the project.
pub trait PtyBackend {
    type Session: PtySession;
    fn spawn(&self, configuration: PtySpawnConfig) -> Result<Self::Session, PtyError>;
}

/// Concrete `portable-pty` adapter hidden behind project-owned types.
pub struct PortablePtyBackend {
    system: Box<dyn PtySystem + Send>,
}

impl PortablePtyBackend {
    pub fn new() -> Self {
        Self {
            system: portable_pty::native_pty_system(),
        }
    }
}

impl Default for PortablePtyBackend {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PortablePtySession {
    master: Box<dyn MasterPty + Send>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    writer: Option<Box<dyn Write + Send>>,
    lifecycle: PtyLifecycle,
}

struct PortableOutputReader(Box<dyn Read + Send>);
impl PtyOutputReader for PortableOutputReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, PtyError> {
        self.0.read(bytes).map_err(|_| PtyError::WriteFailed)
    }
}

impl PtyBackend for PortablePtyBackend {
    type Session = PortablePtySession;

    fn spawn(&self, configuration: PtySpawnConfig) -> Result<Self::Session, PtyError> {
        let size = portable_pty::PtySize {
            rows: configuration.initial_size.rows(),
            cols: configuration.initial_size.columns(),
            pixel_width: 0,
            pixel_height: 0,
        };
        let PtyPair { master, slave } = self
            .system
            .openpty(size)
            .map_err(|_| PtyError::SpawnFailed)?;
        let mut command = CommandBuilder::new(configuration.program());
        command.args(configuration.arguments());
        if let Some(directory) = configuration.working_directory() {
            command.cwd(directory);
        }
        for (key, value) in configuration.environment() {
            command.env(key, value);
        }
        let child = slave
            .spawn_command(command)
            .map_err(|_| PtyError::SpawnFailed)?;
        let writer = master.take_writer().map_err(|_| PtyError::SpawnFailed)?;
        Ok(PortablePtySession {
            master,
            child: Mutex::new(child),
            writer: Some(writer),
            lifecycle: PtyLifecycle::Running,
        })
    }
}

impl PortablePtySession {
    fn refresh_lifecycle(&self) -> PtyLifecycle {
        let mut child = self.child.lock().expect("PTY child mutex is not poisoned");
        match child.try_wait() {
            Ok(Some(status)) => PtyLifecycle::Exited(
                i32::try_from(status.exit_code())
                    .map(PtyExitStatus::code)
                    .unwrap_or_else(|_| PtyExitStatus::unknown()),
            ),
            Ok(None) | Err(_) => PtyLifecycle::Running,
        }
    }
}

impl PtySession for PortablePtySession {
    fn lifecycle(&self) -> PtyLifecycle {
        match self.lifecycle {
            PtyLifecycle::Running => self.refresh_lifecycle(),
            state => state,
        }
    }

    fn take_output_reader(&mut self) -> Result<Box<dyn PtyOutputReader + Send>, PtyError> {
        if !self.lifecycle().allows_operations() {
            return Err(PtyError::NotRunning);
        }
        self.master
            .try_clone_reader()
            .map(|reader| Box::new(PortableOutputReader(reader)) as Box<dyn PtyOutputReader + Send>)
            .map_err(|_| PtyError::SpawnFailed)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        if !self.lifecycle().allows_operations() {
            return Err(PtyError::NotRunning);
        }
        self.writer
            .as_mut()
            .ok_or(PtyError::NotRunning)?
            .write_all(bytes)
            .map_err(|_| PtyError::WriteFailed)
    }

    fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        if !self.lifecycle().allows_operations() {
            return Err(PtyError::NotRunning);
        }
        self.master
            .resize(portable_pty::PtySize {
                rows: size.rows(),
                cols: size.columns(),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| PtyError::ResizeFailed)
    }

    fn terminate(&mut self) -> Result<(), PtyError> {
        if !self.lifecycle().allows_operations() {
            return Ok(());
        }
        self.lifecycle = PtyLifecycle::TerminationRequested;
        self.writer.take();
        self.child
            .lock()
            .expect("PTY child mutex is not poisoned")
            .kill()
            .map_err(|_| PtyError::TerminateFailed)
    }
}
