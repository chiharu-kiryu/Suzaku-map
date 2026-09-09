//! Dedicated push connection and revision-checked actions, separate from settings.
use super::linux_ipc::Deadline;
pub use crate::ime::companion::NativeOperation;
use crate::ime::companion::{MAX_FRAME_BYTES, MAX_TEXT_BYTES, NativeComposition};
use std::{
    io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

pub fn socket_path() -> Option<PathBuf> {
    std::env::var_os("SUZAKU_LINUX_IME_SOCKET")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|p| PathBuf::from(p).join("suzaku-ime/host.sock"))
        })
}

pub struct Subscription {
    stream: UnixStream,
    buffered: Vec<u8>,
}

impl Subscription {
    pub fn connect(path: &Path) -> io::Result<Self> {
        Self::connect_until(path, &Deadline::new(Duration::from_millis(200)))
    }
    pub fn connect_cancellable(path: &Path, stop: &AtomicBool) -> io::Result<Self> {
        Self::connect_until(
            path,
            &Deadline::cancellable(Duration::from_millis(200), stop),
        )
    }
    fn connect_until(path: &Path, deadline: &Deadline<'_>) -> io::Result<Self> {
        let mut stream = deadline.connect(path)?;
        deadline.send(&mut stream, b"W")?;
        Ok(Self {
            stream,
            buffered: Vec::new(),
        })
    }
    pub fn next_frame(&mut self) -> io::Result<Option<NativeComposition>> {
        self.next_frame_until(&Deadline::new(Duration::from_millis(200)))
    }
    pub fn next_frame_cancellable(
        &mut self,
        stop: &AtomicBool,
    ) -> io::Result<Option<NativeComposition>> {
        self.next_frame_until(&Deadline::cancellable(Duration::from_millis(200), stop))
    }
    fn next_frame_until(
        &mut self,
        deadline: &Deadline<'_>,
    ) -> io::Result<Option<NativeComposition>> {
        // An idle subscription is long-lived, but each call yields within its
        // budget. Partial frames survive a timeout without recursive stack growth.
        loop {
            if let Some(index) = self.buffered.iter().position(|b| *b == b'\n') {
                let line = self.buffered.drain(..=index).collect::<Vec<_>>();
                return NativeComposition::parse(&line)
                    .map(Some)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
            }
            if self.buffered.len() >= MAX_FRAME_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Native frame too large",
                ));
            }
            let mut chunk = [0; 4096];
            match deadline.read(&mut self.stream, &mut chunk) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Native host disconnected",
                    ));
                }
                Ok(n) => {
                    self.buffered.extend_from_slice(&chunk[..n]);
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn action_command(
    frame: &NativeComposition,
    operation: &NativeOperation,
) -> Result<String, String> {
    if !frame.focused || frame.private {
        return Err("Native input is not public/active".into());
    }
    let action = match operation {
        NativeOperation::Commit(index) | NativeOperation::Select(index) => {
            if frame.seed.is_empty() || *index >= frame.candidates.len() {
                return Err("Candidate is no longer available".into());
            }
            format!(
                "{}{index}",
                if matches!(operation, NativeOperation::Commit(_)) {
                    'K'
                } else {
                    'N'
                }
            )
        }
        NativeOperation::Replace(text) => {
            if text.len() > MAX_TEXT_BYTES || text.chars().any(char::is_control) {
                return Err("Invalid preedit".into());
            }
            format!("T{text}")
        }
        NativeOperation::Clear => "X".into(),
    };
    Ok(format!("A{} {} {}", frame.host, frame.revision, action))
}

/// Never retry an uncertain commit or fall back to arbitrary target text output.
pub fn send_action(path: &Path, command: &str) -> Result<bool, String> {
    send_action_until(path, command, &Deadline::new(Duration::from_millis(350)))
}

pub fn send_action_cancellable(
    path: &Path,
    command: &str,
    stop: &AtomicBool,
) -> Result<bool, String> {
    send_action_until(
        path,
        command,
        &Deadline::cancellable(Duration::from_millis(350), stop),
    )
}

fn send_action_until(path: &Path, command: &str, deadline: &Deadline<'_>) -> Result<bool, String> {
    let request = || -> io::Result<bool> {
        let mut stream = deadline.connect(path)?;
        deadline.send(&mut stream, command.as_bytes())?;
        let mut response = [0];
        deadline.read_exact(&mut stream, &mut response)?;
        Ok(response == [b'1'])
    };
    request().map_err(|_| "Native host did not acknowledge; action was not retried".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ime::companion::NativeCandidate;
    use crate::platform::linux_ipc::tests::Endpoint;
    use std::io::Write;

    #[test]
    fn subscription_and_action_connects_obey_their_budgets_and_never_replay() {
        let endpoint = Endpoint::new();
        let _queued = endpoint.fill_backlog();
        let started = std::time::Instant::now();
        assert_eq!(
            Subscription::connect(&endpoint.path).err().unwrap().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        let started = std::time::Instant::now();
        assert!(
            send_action(&endpoint.path, "Q")
                .unwrap_err()
                .contains("not retried")
        );
        assert!(started.elapsed() < Duration::from_millis(600));
        drop(endpoint.accept());
        assert_eq!(
            endpoint.listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn pending_subscription_frames_yield_and_preserve_bytes_until_cancelled() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let raw = frame().to_json().to_string() + "\n";
        let first = raw.as_bytes()[..2].to_vec();
        let remainder = raw.as_bytes()[2..].to_vec();
        writer.write_all(&first).unwrap();
        let mut subscription = Subscription {
            stream: reader,
            buffered: Vec::new(),
        };
        assert!(
            subscription
                .next_frame_until(&Deadline::new(Duration::from_millis(20)))
                .unwrap()
                .is_none()
        );
        assert_eq!(subscription.buffered, first);
        writer.write_all(&remainder).unwrap();
        assert_eq!(subscription.next_frame().unwrap(), Some(frame()));
        let stop = AtomicBool::new(true);
        assert_eq!(
            subscription
                .next_frame_cancellable(&stop)
                .unwrap_err()
                .kind(),
            io::ErrorKind::ConnectionAborted
        );
    }

    fn frame() -> NativeComposition {
        NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 1,
            revision: 4,
            focused: true,
            private: false,
            language: "ja".into(),
            seed: "nihongo".into(),
            selected: 0,
            candidates: vec![NativeCandidate {
                text: "日本語".into(),
                label: "日本語 · AI".into(),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn subscription_preserves_partial_utf8_frames_across_timeouts_and_disconnects() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let mut subscription = Subscription {
            stream: reader,
            buffered: Vec::new(),
        };
        let first = frame();
        let mut next = first.clone();
        next.revision += 1;
        let raw = first.to_json().to_string();
        let split = raw.find('日').unwrap() + 1;
        writer.write_all(&raw.as_bytes()[..split]).unwrap();
        assert!(subscription.next_frame().unwrap().is_none());
        writer.write_all(&raw.as_bytes()[split..]).unwrap();
        writer.write_all(b"\n").unwrap();
        writer
            .write_all((next.to_json().to_string() + "\n").as_bytes())
            .unwrap();
        assert_eq!(subscription.next_frame().unwrap(), Some(first));
        assert_eq!(subscription.next_frame().unwrap(), Some(next));
        drop(writer);
        assert_eq!(
            subscription.next_frame().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn subscription_rejects_oversized_and_legacy_streams() {
        for bytes in [vec![b'x'; MAX_FRAME_BYTES], b"0\n".to_vec()] {
            let (reader, mut writer) = UnixStream::pair().unwrap();
            writer.write_all(&bytes).unwrap();
            drop(writer);
            let mut stream = Subscription {
                stream: reader,
                buffered: Vec::new(),
            };
            assert_eq!(
                stream.next_frame().unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn actions_are_revision_bound_and_reject_private_or_invalid_candidates() {
        let mut frame = frame();
        assert_eq!(
            action_command(&frame, &NativeOperation::Commit(0)).unwrap(),
            format!("A{} 4 K0", frame.host)
        );
        assert!(action_command(&frame, &NativeOperation::Commit(1)).is_err());
        assert!(action_command(&frame, &NativeOperation::Replace("bad\ntext".into())).is_err());
        frame.seed.clear();
        frame.candidates.clear();
        assert!(
            action_command(&frame, &NativeOperation::Replace("hello".into()))
                .unwrap()
                .ends_with(" Thello")
        );
        assert!(action_command(&frame, &NativeOperation::Commit(0)).is_err());
        frame.private = true;
        assert!(action_command(&frame, &NativeOperation::Replace("hello".into())).is_err());
    }
}
