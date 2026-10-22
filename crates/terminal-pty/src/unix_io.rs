//! Nonblocking PTY descriptors with event-driven cancellation of readiness waits.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use crate::{PtyError, PtyIoCancellation};

struct Cancellation(Mutex<Option<UnixStream>>);

impl PtyIoCancellation for Cancellation {
    fn cancel(&self) -> Result<(), PtyError> {
        // Closing the sole peer wakes both readiness waits, including waiters
        // entering after cancellation. Only shutdown takes this lock.
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        Ok(())
    }
}

struct PtyIo {
    file: File,
    cancelled: Arc<UnixStream>,
}

pub(super) type Handles = (
    Box<dyn Read + Send>,
    Box<dyn Write + Send>,
    Option<Box<dyn PtyIoCancellation>>,
);

pub(super) fn open(master: RawFd) -> io::Result<Handles> {
    // SAFETY: the caller retains the master throughout this function. Clones own
    // their descriptors and share its nonblocking file status with the master.
    let descriptor = unsafe { BorrowedFd::borrow_raw(master) };
    let reader = File::from(descriptor.try_clone_to_owned()?);
    let writer = File::from(descriptor.try_clone_to_owned()?);
    let (cancel, cancelled) = UnixStream::pair()?;
    let cancelled = Arc::new(cancelled);
    // SAFETY: fcntl operates on the live borrowed descriptor, with integer flags.
    let flags = unsafe { libc::fcntl(master, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok((
        Box::new(PtyIo {
            file: reader,
            cancelled: Arc::clone(&cancelled),
        }),
        Box::new(PtyIo {
            file: writer,
            cancelled,
        }),
        Some(Box::new(Cancellation(Mutex::new(Some(cancel))))),
    ))
}

impl PtyIo {
    fn wait(&self, readiness: libc::c_short) -> io::Result<()> {
        let mut descriptors = [
            libc::pollfd {
                fd: self.cancelled.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: readiness,
                revents: 0,
            },
        ];
        loop {
            // SAFETY: both descriptors stay owned throughout this infinite,
            // event-driven wait. No timeout or periodic polling is used.
            let result = unsafe { libc::poll(descriptors.as_mut_ptr(), 2, -1) };
            if result == -1 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if descriptors[0].revents != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "PTY I/O cancelled",
                ));
            }
            if descriptors[1].revents != 0 {
                return Ok(());
            }
        }
    }
}

impl Read for PtyIo {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.file.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(libc::POLLIN)?
                }
                // Match portable-pty's Unix EOF normalization.
                Err(error) if error.raw_os_error() == Some(libc::EIO) => return Ok(0),
                result => return result,
            }
        }
    }
}

impl Write for PtyIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            match self.file.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(libc::POLLOUT)?
                }
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn full_pty() -> (portable_pty::PtyPair, Handles) {
        let pair = portable_pty::native_pty_system()
            .openpty(portable_pty::PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let fd = pair.master.as_raw_fd().unwrap();
        // SAFETY: fd is a live PTY; tcgetattr initializes termios before use.
        unsafe {
            let mut attributes = std::mem::MaybeUninit::uninit();
            assert_eq!(libc::tcgetattr(fd, attributes.as_mut_ptr()), 0);
            let mut attributes = attributes.assume_init();
            libc::cfmakeraw(&mut attributes);
            assert_eq!(libc::tcsetattr(fd, libc::TCSANOW, &attributes), 0);
        }
        let handles = open(fd).unwrap();
        // Keep the slave open without a reader and fill the input queue before
        // starting the operation under test. No scheduler-dependent sleep.
        let mut file = File::from(
            unsafe { BorrowedFd::borrow_raw(fd) }
                .try_clone_to_owned()
                .unwrap(),
        );
        loop {
            match file.write(&[b'x'; 4096]) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("fill PTY: {error}"),
            }
        }
        (pair, handles)
    }

    #[test]
    fn cancellation_wakes_full_pty_write_and_empty_read_without_closing_slave() {
        let (_pair, (mut reader, mut writer, cancellation)) = full_pty();
        let cancellation = cancellation.unwrap();
        let (done, results) = mpsc::channel();
        let write_done = done.clone();
        let write = std::thread::spawn(move || {
            write_done
                .send(
                    writer
                        .write_all(&[b'x'; 4096])
                        .map_err(|error| error.kind()),
                )
                .unwrap();
        });
        let read = std::thread::spawn(move || {
            let result = reader
                .read(&mut [0; 4096])
                .map(|_| ())
                .map_err(|error| error.kind());
            done.send(result).unwrap();
        });
        assert!(
            results.try_recv().is_err(),
            "full PTY write unexpectedly completed"
        );
        cancellation.cancel().unwrap();
        for _ in 0..2 {
            assert_eq!(
                results.recv_timeout(Duration::from_secs(5)).unwrap(),
                Err(io::ErrorKind::BrokenPipe)
            );
        }
        write.join().unwrap();
        read.join().unwrap();
        cancellation.cancel().unwrap();
    }

    #[test]
    fn cancellation_before_wait_is_sticky_and_safe_after_io_owners_drop() {
        let (_pair, (_, mut writer, cancellation)) = full_pty();
        let cancellation = cancellation.unwrap();
        cancellation.cancel().unwrap();
        assert_eq!(
            writer.write_all(&[b'x'; 4096]).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        drop(writer);
        cancellation.cancel().unwrap();
    }
}
