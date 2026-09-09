//! Bounded Linux commands, shared by status probes and explicit tray operations.
//! No reader threads or wait-with-output: inherited stdout shares the deadline.
use std::{
    io::{self, ErrorKind, Read},
    os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

const MAX_OUTPUT_BYTES: usize = 65_536;

struct ProbeChild {
    child: Child,
    complete: bool,
}

impl Drop for ProbeChild {
    fn drop(&mut self) {
        // Each probe owns its process group, so cancellation also stops a wrapper's
        // children. This never targets the desktop's IBus daemon or process group.
        if !self.complete {
            // SAFETY: the unreaped direct child reserves this owned group ID.
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn child_exited(child: &Child) -> io::Result<bool> {
    // Unlike Child::try_wait, WNOWAIT leaves the parent unreaped until stdout
    // closes or cancellation kills the group. This prevents PID/group-ID reuse
    // while a wrapper's descendants still hold its stdout open.
    // SAFETY: siginfo is initialized and writable; the PID is our owned child.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error)
        };
    }
    // SAFETY: successful waitid initializes the SIGCHLD fields (or keeps PID 0).
    Ok(unsafe { info.si_pid() } != 0)
}

pub(super) fn stdout(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let output = run(Command::new(program).args(args), timeout).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

/// Captures at most 64 KiB of stdout. Stdin/stderr are discarded; a timeout stops
/// this command's process group and reaps its direct child, without retrying it.
pub fn run(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            ErrorKind::TimedOut,
            "Command deadline exceeded",
        ));
    }
    let deadline = Instant::now() + timeout;
    // A socketpair exposes nonblocking reads through stable std APIs. Only the
    // parent's end is nonblocking; subprocess stdout retains ordinary semantics.
    let (mut reader, writer) = UnixStream::pair()?;
    reader.set_nonblocking(true)?;
    let spawned = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    // Command retains its configured fd after spawn. Close that parent copy so
    // EOF depends only on the child/descendants, even if the caller reuses Command.
    command.stdout(Stdio::null());
    let mut child = ProbeChild {
        child: spawned?,
        complete: false,
    };
    let mut bytes = Vec::new();
    let mut exited = false;
    let mut eof = false;
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                ErrorKind::TimedOut,
                "Command deadline exceeded",
            ));
        }
        for _ in 0..4 {
            let mut chunk = [0; 4096];
            match reader.read(&mut chunk) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(count) => {
                    if bytes.len() + count > MAX_OUTPUT_BYTES {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "Command output too large",
                        ));
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if !exited {
            exited = child_exited(&child.child)?;
        }
        if eof && exited {
            child.complete = true;
            let status = child.child.wait()?;
            return Ok(Output {
                status,
                stdout: bytes,
                stderr: Vec::new(),
            });
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(5)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_probe_captures_unicode_and_rejects_failed_commands() {
        assert_eq!(
            stdout("/usr/bin/printf", &["rime 日本語"], Duration::from_secs(1)).as_deref(),
            Some("rime 日本語")
        );
        assert!(stdout("/usr/bin/false", &[], Duration::from_secs(1)).is_none());
        assert!(stdout("/suzaku-missing-probe-command", &[], Duration::from_secs(1)).is_none());
    }

    #[test]
    fn command_probe_times_out_and_reaps_a_stalled_process() {
        let started = Instant::now();
        assert!(stdout("/bin/sleep", &["2"], Duration::from_millis(45)).is_none());
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn command_probe_does_not_wait_on_inherited_stdout() {
        let started = Instant::now();
        // Exactly one bounded child, not a recursive/self-invoking fixture.
        assert!(
            stdout(
                "/bin/sh",
                &["-c", "/bin/sleep 2 & exit 0"],
                Duration::from_millis(45)
            )
            .is_none()
        );
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn command_probe_bounds_output_memory() {
        assert!(
            stdout(
                "/usr/bin/head",
                &["-c", "131072", "/dev/zero"],
                Duration::from_secs(1)
            )
            .is_none()
        );
    }
}
