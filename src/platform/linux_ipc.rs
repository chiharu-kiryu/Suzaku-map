//! Deadline-bound Unix IPC. Connect, partial writes and partial reads share one
//! monotonic budget. Cancellation never retries a request or changes its target.
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{ffi::OsStrExt, net::UnixStream},
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct Deadline<'a> {
    until: Instant,
    cancelled: Option<&'a AtomicBool>,
}

impl<'a> Deadline<'a> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            until: Instant::now() + timeout,
            cancelled: None,
        }
    }

    pub fn cancellable(timeout: Duration, cancelled: &'a AtomicBool) -> Self {
        Self {
            until: Instant::now() + timeout,
            cancelled: Some(cancelled),
        }
    }

    fn remaining(&self) -> io::Result<Duration> {
        if self
            .cancelled
            .is_some_and(|stop| stop.load(Ordering::Acquire))
        {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "IPC request cancelled",
            ));
        }
        self.until
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "IPC deadline exceeded"))
    }

    fn wait(&self, fd: i32, events: i16) -> io::Result<()> {
        loop {
            let remaining = self.remaining()?;
            let slice = if self.cancelled.is_some() {
                remaining.min(Duration::from_millis(25))
            } else {
                remaining
            };
            let millis = slice.as_millis().saturating_add(1).min(i32::MAX as u128) as i32;
            let mut pollfd = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            // SAFETY: pollfd is live for this call and its descriptor is borrowed.
            let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
            self.remaining()?;
            if result > 0 {
                if pollfd.revents & libc::POLLNVAL != 0 {
                    return Err(io::Error::from_raw_os_error(libc::EBADF));
                }
                return Ok(()); // Let read/write report EOF or the socket error.
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
    }

    pub fn connect(&self, path: &Path) -> io::Result<UnixStream> {
        // SAFETY: zero is valid for sockaddr_un; sun_path stays NUL terminated.
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let bytes = path.as_os_str().as_bytes();
        if bytes.is_empty() || bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid Unix socket path",
            ));
        }
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (dest, source) in address.sun_path.iter_mut().zip(bytes) {
            *dest = *source as _;
        }
        let length = (std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1)
            as libc::socklen_t;
        loop {
            self.remaining()?;
            // SAFETY: no pointers are passed; the returned fd is immediately owned.
            let raw = unsafe {
                libc::socket(
                    libc::AF_UNIX,
                    libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                    0,
                )
            };
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: socket returned a fresh valid descriptor, owned only here.
            let stream = UnixStream::from(unsafe { OwnedFd::from_raw_fd(raw) });
            // SAFETY: address/length describe a live, initialized sockaddr_un.
            let result = unsafe {
                libc::connect(raw, (&address as *const libc::sockaddr_un).cast(), length)
            };
            if result == 0 {
                self.remaining()?;
                return Ok(stream);
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EINPROGRESS) => {
                    self.wait(raw, libc::POLLOUT)?;
                    if let Some(error) = stream.take_error()? {
                        return Err(error);
                    }
                    stream.peer_addr()?;
                    return Ok(stream);
                }
                Some(libc::EAGAIN | libc::EINTR) => {
                    // Linux AF_UNIX EAGAIN means the accept queue is full, not an
                    // in-progress connection. No bytes have been sent: retry only
                    // connection establishment with a fresh descriptor and budget.
                    drop(stream);
                    std::thread::sleep(self.remaining()?.min(Duration::from_millis(5)));
                }
                _ => return Err(error),
            }
        }
    }

    pub fn send(&self, stream: &mut UnixStream, mut bytes: &[u8]) -> io::Result<()> {
        stream.set_nonblocking(true)?;
        while !bytes.is_empty() {
            self.remaining()?;
            match stream.write(bytes) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => bytes = &bytes[count..],
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(stream.as_raw_fd(), libc::POLLOUT)?
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        self.remaining()?;
        stream.shutdown(std::net::Shutdown::Write)
    }

    pub fn read(&self, stream: &mut UnixStream, bytes: &mut [u8]) -> io::Result<usize> {
        stream.set_nonblocking(true)?;
        loop {
            self.remaining()?;
            match stream.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.wait(stream.as_raw_fd(), libc::POLLIN)?
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }

    pub fn read_exact(&self, stream: &mut UnixStream, mut bytes: &mut [u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let count = self.read(stream, bytes)?;
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            bytes = &mut bytes[count..];
        }
        Ok(())
    }

    pub fn read_to_end(&self, stream: &mut UnixStream, limit: usize) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        loop {
            let count = self.read(stream, &mut chunk)?;
            if count == 0 {
                return Ok(bytes);
            }
            if count > limit.saturating_sub(bytes.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "IPC response too large",
                ));
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
    }
}

#[cfg(test)]
#[path = "linux_ipc_test.rs"]
pub(crate) mod tests;
