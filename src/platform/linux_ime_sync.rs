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
        NativeOperation::Commit(index)
        | NativeOperation::Select(index)
        | NativeOperation::Adopt(index) => {
            if frame.seed.is_empty() || *index >= frame.candidates.len() {
                return Err("Candidate is no longer available".into());
            }
            format!(
                "{}{index}",
                if matches!(operation, NativeOperation::Commit(_)) {
                    'K'
                } else if matches!(operation, NativeOperation::Adopt(_)) {
                    'D'
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
        NativeOperation::Backspace => "B".into(),
        NativeOperation::Continue => "S".into(),
    };
    let prefix = if matches!(
        operation,
        NativeOperation::Backspace | NativeOperation::Continue
    ) {
        'E'
    } else {
        'A'
    };
    Ok(format!(
        "{prefix}{} {} {}",
        frame.host, frame.revision, action
    ))
}

/// Semantic keyboard edits cannot predict their resulting text locally: an
/// adoption undo or unfinished Compose sequence belongs to the host. Require
/// its complete, bounded result on the same connection as the acknowledgement.
/// Older hosts reject this distinct request instead of applying a guessed edit.
pub fn send_keyboard_action_cancellable(
    path: &Path,
    command: &str,
    stop: &AtomicBool,
) -> Result<NativeComposition, String> {
    let deadline = Deadline::cancellable(Duration::from_millis(350), stop);
    send_keyboard_action_until(path, command, &deadline)
}

fn keyboard_target(command: &str) -> Result<(&str, u64), String> {
    let mut parts = command
        .strip_prefix('E')
        .ok_or("Invalid native keyboard command")?
        .split(' ');
    let host = parts.next().ok_or("Missing native keyboard host")?;
    let revision = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or("Invalid native keyboard revision")?;
    if host.len() != 36
        || !host.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        || !matches!(parts.next(), Some("B" | "S"))
        || parts.next().is_some()
    {
        return Err("Invalid native keyboard command".into());
    }
    Ok((host, revision))
}

fn read_keyboard_result(
    stream: &mut UnixStream,
    command: &str,
    deadline: &Deadline<'_>,
) -> Result<NativeComposition, String> {
    let (host, revision) = keyboard_target(command)?;
    let failure = || "Native keyboard edit was not confirmed; action was not retried".to_string();
    let mut acknowledged = [0];
    deadline
        .read_exact(stream, &mut acknowledged)
        .map_err(|_| failure())?;
    if acknowledged != [b'1'] {
        return Err(failure());
    }
    let raw = deadline
        .read_to_end(stream, MAX_FRAME_BYTES)
        .map_err(|_| failure())?;
    // Exactly one newline-delimited snapshot, never a success byte on its own
    // or a second frame whose later state could silently replace this result.
    let body = raw
        .strip_suffix(b"\n")
        .filter(|body| !body.contains(&b'\n'))
        .ok_or_else(failure)?;
    let frame = NativeComposition::parse(body).map_err(|_| failure())?;
    if frame.host != host || frame.revision <= revision || !frame.focused || frame.private {
        return Err(failure());
    }
    Ok(frame)
}

fn send_keyboard_action_until(
    path: &Path,
    command: &str,
    deadline: &Deadline<'_>,
) -> Result<NativeComposition, String> {
    keyboard_target(command)?;
    let mut stream = deadline
        .connect(path)
        .map_err(|_| "Native keyboard host unavailable; action was not retried")?;
    deadline
        .send(&mut stream, command.as_bytes())
        .map_err(|_| "Native keyboard write unconfirmed; action was not retried")?;
    read_keyboard_result(&mut stream, command, deadline)
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
            cursor: None,
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
        assert_eq!(
            action_command(&frame, &NativeOperation::Adopt(0)).unwrap(),
            format!("A{} 4 D0", frame.host)
        );
        assert!(action_command(&frame, &NativeOperation::Adopt(1)).is_err());
        assert!(action_command(&frame, &NativeOperation::Replace("bad\ntext".into())).is_err());
        frame.seed.clear();
        frame.candidates.clear();
        assert!(
            action_command(&frame, &NativeOperation::Replace("hello".into()))
                .unwrap()
                .ends_with(" Thello")
        );
        assert!(action_command(&frame, &NativeOperation::Commit(0)).is_err());
        assert!(action_command(&frame, &NativeOperation::Adopt(0)).is_err());
        assert_eq!(
            action_command(&frame, &NativeOperation::Backspace).unwrap(),
            format!("E{} 4 B", frame.host)
        );
        assert_eq!(
            action_command(&frame, &NativeOperation::Continue).unwrap(),
            format!("E{} 4 S", frame.host)
        );
        frame.private = true;
        assert!(action_command(&frame, &NativeOperation::Replace("hello".into())).is_err());
        assert!(action_command(&frame, &NativeOperation::Backspace).is_err());
        assert!(action_command(&frame, &NativeOperation::Continue).is_err());
        frame.private = false;
        frame.focused = false;
        assert!(action_command(&frame, &NativeOperation::Continue).is_err());
    }

    #[test]
    fn keyboard_actions_require_a_complete_exact_authoritative_result() {
        let mut next = frame();
        let command = action_command(&next, &NativeOperation::Backspace).unwrap();
        next.revision += 1;
        next.seed = "原拼写😀é".into();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer
            .write_all(format!("1{}\n", next.to_json()).as_bytes())
            .unwrap();
        drop(writer);
        assert_eq!(
            read_keyboard_result(
                &mut reader,
                &command,
                &Deadline::new(Duration::from_millis(100))
            )
            .unwrap(),
            next
        );

        let mut responses = vec![
            b"0".to_vec(),
            b"1".to_vec(),
            b"1{}\n".to_vec(),
            format!("1{}", next.to_json()).into_bytes(),
            format!("1{}\n{}\n", next.to_json(), next.to_json()).into_bytes(),
            [b"1".as_slice(), &vec![b'x'; MAX_FRAME_BYTES + 1]].concat(),
        ];
        for case in ["host", "revision", "private", "focus"] {
            let mut invalid = next.clone();
            match case {
                "host" => invalid.host = "00000000-0000-0000-0000-000000000002".into(),
                "revision" => invalid.revision = 4,
                "private" => {
                    invalid.private = true;
                    invalid.seed.clear();
                    invalid.candidates.clear();
                }
                _ => {
                    invalid.focused = false;
                    invalid.seed.clear();
                    invalid.candidates.clear();
                }
            }
            responses.push(format!("1{}\n", invalid.to_json()).into_bytes());
        }
        for response in responses {
            let (mut reader, mut writer) = UnixStream::pair().unwrap();
            writer.write_all(&response).unwrap();
            drop(writer);
            let error = read_keyboard_result(
                &mut reader,
                &command,
                &Deadline::new(Duration::from_millis(100)),
            )
            .unwrap_err();
            assert!(error.contains("not retried"));
        }
    }

    #[test]
    fn keyboard_actions_send_once_and_accept_fragmented_unicode_results() {
        use std::io::Read;
        let endpoint = Endpoint::new();
        let mut next = frame();
        let command = action_command(&next, &NativeOperation::Continue).unwrap();
        next.revision += 1;
        next.seed = "你好 café😀 ".into();
        let response = format!("1{}\n", next.to_json());
        std::thread::scope(|scope| {
            let server = scope.spawn(|| {
                let mut peer = endpoint.accept();
                let mut request = String::new();
                peer.read_to_string(&mut request).unwrap();
                assert_eq!(request, command);
                for chunk in response.as_bytes().chunks(7) {
                    peer.write_all(chunk).unwrap();
                }
            });
            let stop = AtomicBool::new(false);
            assert_eq!(
                send_keyboard_action_cancellable(&endpoint.path, &command, &stop).unwrap(),
                next
            );
            server.join().unwrap();
        });
        assert_eq!(
            endpoint.listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn keyboard_success_byte_alone_obeys_the_original_deadline_and_cancellation() {
        let command = action_command(&frame(), &NativeOperation::Backspace).unwrap();
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"1").unwrap();
        let started = std::time::Instant::now();
        assert!(
            read_keyboard_result(
                &mut reader,
                &command,
                &Deadline::new(Duration::from_millis(20))
            )
            .unwrap_err()
            .contains("not retried")
        );
        assert!(started.elapsed() < Duration::from_millis(200));
        let stop = AtomicBool::new(true);
        assert!(
            read_keyboard_result(
                &mut reader,
                &command,
                &Deadline::cancellable(Duration::from_secs(1), &stop)
            )
            .unwrap_err()
            .contains("not retried")
        );
        assert!(started.elapsed() < Duration::from_millis(200));
    }

    #[test]
    fn keyboard_commands_fail_before_connecting_on_invalid_targets_or_cancellation() {
        let endpoint = Endpoint::new();
        let valid = action_command(&frame(), &NativeOperation::Backspace).unwrap();
        let stop = AtomicBool::new(false);
        for command in [
            valid.replace(" B", " X"),
            valid.clone() + " junk",
            valid.replacen('E', "A", 1),
        ] {
            assert!(send_keyboard_action_cancellable(&endpoint.path, &command, &stop).is_err());
        }
        let stop = AtomicBool::new(true);
        assert!(send_keyboard_action_cancellable(&endpoint.path, &valid, &stop).is_err());
        assert_eq!(
            endpoint.listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}
