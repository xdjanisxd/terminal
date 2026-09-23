//! Project-owned boundary for local pseudo-terminal sessions.
//!
//! This crate transports raw bytes only. It neither decodes output nor depends on
//! terminal parsing, screen state, rendering, input encoding, or a backend API.
//! `PortablePtyBackend` implements `PtyBackend` and `PtySession` with
//! `portable-pty` behind this project-owned boundary.

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use portable_pty::{CommandBuilder, MasterPty, PtyPair, PtySystem};

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

/// Mechanism-only request for a local child spawn.
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
    ReadFailed,
    WriteFailed,
    ResizeFailed,
    TerminateFailed,
    NotRunning,
}
impl fmt::Display for PtyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SpawnFailed => "PTY spawn failed",
            Self::ReadFailed => "PTY output read failed",
            Self::WriteFailed => "PTY input write failed",
            Self::ResizeFailed => "PTY resize failed",
            Self::TerminateFailed => "PTY termination failed",
            Self::NotRunning => "PTY session is not running",
        })
    }
}
impl Error for PtyError {}

/// Backend-independent live output-reader capability.
///
/// `read` returns arbitrary raw bytes. A return value of zero means raw output
/// EOF; it does not imply that child exit has or has not been observed.
pub trait PtyOutputReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, PtyError>;
}

/// Backend-independent live session capability.
///
/// The session owner calls `terminate` for deterministic shutdown; `Drop` is only
/// a backend safety net. Calls after exit or termination request return
/// `PtyError::NotRunning`; repeated termination succeeds without another effect.
pub trait PtySession {
    fn lifecycle(&self) -> PtyLifecycle;

    /// Transfers the single raw-output reader to the caller.
    ///
    /// The first successful call has exclusive ownership. Later calls return
    /// `PtyError::NotRunning`, without creating a competing reader. The reader
    /// remains usable to drain output independently of child-exit observation.
    fn take_output_reader(&mut self) -> Result<Box<dyn PtyOutputReader + Send>, PtyError>;

    fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError>;
    fn resize(&mut self, size: PtySize) -> Result<(), PtyError>;
    fn terminate(&mut self) -> Result<(), PtyError>;
}

/// A backend seam that keeps adapter types out of the rest of the project.
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

/// `portable-pty` session state kept entirely behind the project-owned traits.
pub struct PortablePtySession {
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    writer: Option<Box<dyn Write + Send>>,
    reader: Option<Box<dyn Read + Send>>,
    lifecycle: Mutex<PtyLifecycle>,
}

struct PortableOutputReader(Box<dyn Read + Send>);
impl PtyOutputReader for PortableOutputReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, PtyError> {
        self.0.read(bytes).map_err(|_| PtyError::ReadFailed)
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
        let reader = master
            .try_clone_reader()
            .map_err(|_| PtyError::SpawnFailed)?;
        let writer = master.take_writer().map_err(|_| PtyError::SpawnFailed)?;
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
        Ok(PortablePtySession {
            master: Mutex::new(Some(master)),
            child: Mutex::new(child),
            writer: Some(writer),
            reader: Some(reader),
            lifecycle: Mutex::new(PtyLifecycle::Running),
        })
    }
}

impl PortablePtySession {
    fn refresh_lifecycle(&self) -> PtyLifecycle {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("PTY lifecycle mutex is not poisoned");
        if matches!(*lifecycle, PtyLifecycle::Exited(_)) {
            return *lifecycle;
        }
        let mut child = self.child.lock().expect("PTY child mutex is not poisoned");
        if let Ok(Some(status)) = child.try_wait() {
            *lifecycle = PtyLifecycle::Exited(
                i32::try_from(status.exit_code())
                    .map(PtyExitStatus::code)
                    .unwrap_or_else(|_| PtyExitStatus::unknown()),
            );
            self.master
                .lock()
                .expect("PTY master mutex is not poisoned")
                .take();
        }
        *lifecycle
    }
}

impl PtySession for PortablePtySession {
    fn lifecycle(&self) -> PtyLifecycle {
        self.refresh_lifecycle()
    }

    fn take_output_reader(&mut self) -> Result<Box<dyn PtyOutputReader + Send>, PtyError> {
        self.reader
            .take()
            .map(|reader| Box::new(PortableOutputReader(reader)) as Box<dyn PtyOutputReader + Send>)
            .ok_or(PtyError::NotRunning)
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
            .lock()
            .expect("PTY master mutex is not poisoned")
            .as_ref()
            .ok_or(PtyError::NotRunning)?
            .resize(portable_pty::PtySize {
                rows: size.rows(),
                cols: size.columns(),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|_| PtyError::ResizeFailed)
    }

    fn terminate(&mut self) -> Result<(), PtyError> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("PTY lifecycle mutex is not poisoned");
        if !lifecycle.allows_operations() {
            return Ok(());
        }
        *lifecycle = PtyLifecycle::TerminationRequested;
        self.writer.take();
        self.child
            .lock()
            .expect("PTY child mutex is not poisoned")
            .kill()
            .map_err(|_| PtyError::TerminateFailed)
    }
}

/// Maximum queued commands. Eight commands bounds controller memory while allowing
/// short write/resize bursts; a full queue rejects the newest command explicitly.
pub const PTY_COMMAND_CAPACITY: usize = 8;
/// Maximum queued events. At most eight 1 KiB raw chunks (plus terminal events)
/// await the consumer; a full queue blocks the producer instead of dropping data.
pub const PTY_EVENT_CAPACITY: usize = 8;
/// Largest raw output payload placed in one event. Reads remain arbitrary chunks.
pub const PTY_READ_CHUNK_SIZE: usize = 1024;
const LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Owned operations accepted by a PTY worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyCommand {
    Write(Vec<u8>),
    Resize(PtySize),
    Terminate,
}

/// Bounded worker event stream. Output remains raw and exit remains distinct from EOF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyWorkerEvent {
    Output(PtyOutput),
    Error(PtyWorkerError),
}

/// Project-owned worker failures; channel and backend implementation errors stay hidden.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PtyWorkerError {
    CommandQueueFull,
    WorkerUnavailable,
    Pty(PtyError),
    JoinFailed,
}
impl fmt::Display for PtyWorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandQueueFull => f.write_str("PTY worker command queue is full"),
            Self::WorkerUnavailable => f.write_str("PTY worker is unavailable"),
            Self::Pty(error) => write!(f, "PTY worker operation failed: {error}"),
            Self::JoinFailed => f.write_str("PTY worker thread panicked"),
        }
    }
}
impl Error for PtyWorkerError {}

/// Project-owned bounded worker handle. The controller owns this sender, receiver,
/// and join handle; the worker owns the live session and its exclusive reader.
pub struct PtyWorker {
    commands: Option<SyncSender<PtyCommand>>,
    events: Option<Receiver<PtyWorkerEvent>>,
    join: Option<JoinHandle<()>>,
}

impl PtyWorker {
    /// Starts one session thread and one blocking-reader thread.
    pub fn start<S>(session: S) -> Result<Self, PtyWorkerError>
    where
        S: PtySession + Send + 'static,
    {
        Self::start_with_notifier(session, || {})
    }

    /// Starts one session thread and one blocking-reader thread, notifying the
    /// controller after each event is successfully queued.
    ///
    /// The notifier is intentionally transport-agnostic so the PTY crate does
    /// not acquire a dependency on any particular application event loop.
    pub fn start_with_notifier<S, N>(mut session: S, notifier: N) -> Result<Self, PtyWorkerError>
    where
        S: PtySession + Send + 'static,
        N: Fn() + Send + Sync + 'static,
    {
        let reader = session.take_output_reader().map_err(PtyWorkerError::Pty)?;
        let (command_sender, command_receiver) = mpsc::sync_channel(PTY_COMMAND_CAPACITY);
        let (event_sender, event_receiver) = mpsc::sync_channel(PTY_EVENT_CAPACITY);
        let (reader_done_sender, reader_done_receiver) = mpsc::sync_channel(1);
        let receiver_disconnected = Arc::new(AtomicBool::new(false));
        let reader_failed = Arc::new(AtomicBool::new(false));
        let notifier: Arc<dyn Fn() + Send + Sync> = Arc::new(notifier);
        let reader_events = event_sender.clone();
        let reader_disconnected = Arc::clone(&receiver_disconnected);
        let reader_failed_flag = Arc::clone(&reader_failed);
        let reader_notifier = Arc::clone(&notifier);

        let join = thread::spawn(move || {
            let reader_join = thread::spawn(move || {
                run_reader(
                    reader,
                    reader_events,
                    reader_done_sender,
                    reader_disconnected,
                    reader_failed_flag,
                    reader_notifier,
                );
            });
            run_session(
                &mut session,
                command_receiver,
                event_sender,
                reader_done_receiver,
                receiver_disconnected,
                reader_failed,
                notifier,
            );
            let _ = reader_join.join();
        });

        Ok(Self {
            commands: Some(command_sender),
            events: Some(event_receiver),
            join: Some(join),
        })
    }

    pub fn write(&self, bytes: Vec<u8>) -> Result<(), PtyWorkerError> {
        self.send(PtyCommand::Write(bytes))
    }

    pub fn resize(&self, size: PtySize) -> Result<(), PtyWorkerError> {
        self.send(PtyCommand::Resize(size))
    }

    pub fn terminate(&self) -> Result<(), PtyWorkerError> {
        self.send(PtyCommand::Terminate)
    }

    pub fn send(&self, command: PtyCommand) -> Result<(), PtyWorkerError> {
        let Some(sender) = &self.commands else {
            return Err(PtyWorkerError::WorkerUnavailable);
        };
        match sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(PtyWorkerError::CommandQueueFull),
            Err(TrySendError::Disconnected(_)) => Err(PtyWorkerError::WorkerUnavailable),
        }
    }

    /// Returns `None` for either a bounded timeout or a closed event stream.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<PtyWorkerEvent>, PtyWorkerError> {
        let Some(events) = &self.events else {
            return Err(PtyWorkerError::WorkerUnavailable);
        };
        match events.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    /// Joins after natural exit or an earlier explicit termination request.
    pub fn join(&mut self) -> Result<(), PtyWorkerError> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        join.join().map_err(|_| PtyWorkerError::JoinFailed)
    }

    /// Discards unread events, requests termination, and joins both worker threads.
    pub fn shutdown_and_join(&mut self) -> Result<(), PtyWorkerError> {
        self.events.take();
        if let Some(sender) = &self.commands {
            let _ = sender.send(PtyCommand::Terminate);
        }
        self.commands.take();
        self.join()
    }
}

impl Drop for PtyWorker {
    fn drop(&mut self) {
        let _ = self.shutdown_and_join();
    }
}

fn run_reader(
    mut reader: Box<dyn PtyOutputReader + Send>,
    events: SyncSender<PtyWorkerEvent>,
    done: SyncSender<()>,
    receiver_disconnected: Arc<AtomicBool>,
    reader_failed: Arc<AtomicBool>,
    notifier: Arc<dyn Fn() + Send + Sync>,
) {
    let mut bytes = [0_u8; PTY_READ_CHUNK_SIZE];
    loop {
        match reader.read(&mut bytes) {
            Ok(0) => {
                if !send_event(
                    &events,
                    PtyWorkerEvent::Output(PtyOutput::Eof),
                    &receiver_disconnected,
                    &notifier,
                ) {
                    break;
                }
                break;
            }
            Ok(read) => {
                let event = PtyOutput::bytes(bytes[..read].to_vec()).expect("nonzero PTY read");
                if !send_event(
                    &events,
                    PtyWorkerEvent::Output(event),
                    &receiver_disconnected,
                    &notifier,
                ) {
                    break;
                }
            }
            Err(error) => {
                reader_failed.store(true, Ordering::Release);
                let _ = send_event(
                    &events,
                    PtyWorkerEvent::Error(PtyWorkerError::Pty(error)),
                    &receiver_disconnected,
                    &notifier,
                );
                break;
            }
        }
    }
    let _ = done.send(());
}

fn run_session<S>(
    session: &mut S,
    commands: Receiver<PtyCommand>,
    events: SyncSender<PtyWorkerEvent>,
    reader_done: Receiver<()>,
    receiver_disconnected: Arc<AtomicBool>,
    reader_failed: Arc<AtomicBool>,
    notifier: Arc<dyn Fn() + Send + Sync>,
) where
    S: PtySession,
{
    let mut command_channel_closed = false;
    let mut termination_requested = false;
    let mut exited_emitted = false;
    let mut reader_finished = false;

    loop {
        if receiver_disconnected.load(Ordering::Acquire)
            || reader_failed.load(Ordering::Acquire)
            || command_channel_closed
        {
            request_termination(
                session,
                &events,
                &receiver_disconnected,
                &mut termination_requested,
                &notifier,
            );
        }

        match commands.recv_timeout(LIFECYCLE_POLL_INTERVAL) {
            Ok(command) => handle_command(
                session,
                command,
                &events,
                &receiver_disconnected,
                &mut termination_requested,
                &notifier,
            ),
            Err(RecvTimeoutError::Disconnected) => command_channel_closed = true,
            Err(RecvTimeoutError::Timeout) => {}
        }

        match reader_done.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => reader_finished = true,
            Err(TryRecvError::Empty) => {}
        }

        if let PtyLifecycle::Exited(status) = session.lifecycle() {
            if !exited_emitted {
                if !send_event(
                    &events,
                    PtyWorkerEvent::Output(PtyOutput::Exited(status)),
                    &receiver_disconnected,
                    &notifier,
                ) {
                    request_termination(
                        session,
                        &events,
                        &receiver_disconnected,
                        &mut termination_requested,
                        &notifier,
                    );
                }
                exited_emitted = true;
            }
            if reader_finished {
                break;
            }
        }
    }
}

fn handle_command<S>(
    session: &mut S,
    command: PtyCommand,
    events: &SyncSender<PtyWorkerEvent>,
    receiver_disconnected: &AtomicBool,
    termination_requested: &mut bool,
    notifier: &Arc<dyn Fn() + Send + Sync>,
) where
    S: PtySession,
{
    let result = if *termination_requested {
        Err(PtyError::NotRunning)
    } else {
        match command {
            PtyCommand::Write(bytes) => {
                let result = session.write(&bytes);
                if result.is_ok() && std::env::var_os("TERMINAL_RENDERER_DIAGNOSTICS").is_some() {
                    eprintln!("terminal-pty event=write-submitted bytes={bytes:?}");
                }
                result
            }
            PtyCommand::Resize(size) => session.resize(size),
            PtyCommand::Terminate => {
                *termination_requested = true;
                session.terminate()
            }
        }
    };
    if let Err(error) = result {
        let _ = send_event(
            events,
            PtyWorkerEvent::Error(PtyWorkerError::Pty(error)),
            receiver_disconnected,
            notifier,
        );
    }
}

fn request_termination<S>(
    session: &mut S,
    events: &SyncSender<PtyWorkerEvent>,
    receiver_disconnected: &AtomicBool,
    termination_requested: &mut bool,
    notifier: &Arc<dyn Fn() + Send + Sync>,
) where
    S: PtySession,
{
    if *termination_requested {
        return;
    }
    *termination_requested = true;
    if let Err(error) = session.terminate() {
        let _ = send_event(
            events,
            PtyWorkerEvent::Error(PtyWorkerError::Pty(error)),
            receiver_disconnected,
            notifier,
        );
    }
}

fn send_event(
    events: &SyncSender<PtyWorkerEvent>,
    event: PtyWorkerEvent,
    receiver_disconnected: &AtomicBool,
    notifier: &Arc<dyn Fn() + Send + Sync>,
) -> bool {
    if events.send(event).is_ok() {
        notifier();
        true
    } else {
        receiver_disconnected.store(true, Ordering::Release);
        false
    }
}
