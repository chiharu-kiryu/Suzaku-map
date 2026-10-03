//! A short-lived presentation lease, not an input action. The UI only refreshes
//! a mailbox; a dedicated serial worker owns every socket and uncertain claim.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PresentationTarget {
    pub host: String,
    pub context: u64,
    pub revision: u64,
}

pub(super) struct NativePresentation {
    #[cfg(target_os = "linux")]
    worker: linux::Worker,
}

pub(super) fn start() -> Option<NativePresentation> {
    #[cfg(target_os = "linux")]
    {
        let path = suzaku_map::platform::linux_ime_sync::socket_path()?;
        Some(NativePresentation {
            worker: linux::Worker::start(path)?,
        })
    }
    #[cfg(not(target_os = "linux"))]
    None
}

impl NativePresentation {
    pub(super) fn update(&self, target: Option<PresentationTarget>) {
        #[cfg(target_os = "linux")]
        self.worker.update(target);
        #[cfg(not(target_os = "linux"))]
        let _ = target;
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::PresentationTarget;
    use std::{
        io,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread::{self, JoinHandle},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use suzaku_map::platform::linux_ipc::Deadline;

    const RENEW_INTERVAL: Duration = Duration::from_millis(200);
    const UI_FRESHNESS: Duration = Duration::from_millis(600);
    const IPC_TIMEOUT: Duration = Duration::from_millis(100);

    struct Desired {
        target: Option<PresentationTarget>,
        updated: Instant,
    }

    impl Desired {
        fn fresh(&self, now: Instant) -> Option<PresentationTarget> {
            (now.saturating_duration_since(self.updated) <= UI_FRESHNESS)
                .then(|| self.target.clone())
                .flatten()
        }
    }

    pub(super) struct Worker {
        desired: Arc<Mutex<Desired>>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    fn owner_id() -> String {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!(
            "{}-{nanos:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn valid_host(host: &str) -> bool {
        // Match the native composition identity contract; never interpolate a
        // whitespace/control character from an untrusted frame into protocol.
        host.len() == 36
            && host
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    }

    fn command(target: &PresentationTarget, owner: &str, claim: bool) -> Option<String> {
        if !valid_host(&target.host)
            || owner.is_empty()
            || owner.len() > 64
            || !owner
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return None;
        }
        Some(format!(
            "V{} {} {} {} {}",
            target.host,
            target.context,
            target.revision,
            owner,
            u8::from(claim)
        ))
    }

    fn exchange(
        path: &Path,
        target: &PresentationTarget,
        owner: &str,
        claim: bool,
        deadline: Deadline<'_>,
        still_current: impl FnOnce() -> bool,
    ) -> io::Result<bool> {
        let command = command(target, owner, claim)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        let mut stream = deadline.connect(path)?;
        // Connection establishment may have waited while the UI hid or moved
        // to a different context. Never send that obsolete claim afterward.
        if !still_current() {
            return Ok(false);
        }
        deadline.send(&mut stream, command.as_bytes())?;
        let mut response = [0];
        deadline.read_exact(&mut stream, &mut response)?;
        match response[0] {
            b'0' => Ok(false),
            b'1' => Ok(true),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    fn release(path: &Path, target: &PresentationTarget, owner: &str) {
        // Release is idempotent and owner-bound, and accepts an old revision.
        // It also runs after cancellation; its own 100 ms deadline is the bound.
        let _ = exchange(
            path,
            target,
            owner,
            false,
            Deadline::new(IPC_TIMEOUT),
            || true,
        );
    }

    impl Worker {
        pub(super) fn start(path: PathBuf) -> Option<Self> {
            let desired = Arc::new(Mutex::new(Desired {
                target: None,
                updated: Instant::now(),
            }));
            let stop = Arc::new(AtomicBool::new(false));
            let reader = desired.clone();
            let cancelled = stop.clone();
            let owner = owner_id();
            let thread = thread::Builder::new()
                .name("suzaku-native-presentation".into())
                .spawn(move || {
                    let mut held: Option<PresentationTarget> = None;
                    while !cancelled.load(Ordering::Acquire) {
                        let next = reader.lock().unwrap().fresh(Instant::now());
                        if held.as_ref().is_some_and(|old| {
                            next.as_ref().is_none_or(|new| {
                                old.host != new.host || old.context != new.context
                            })
                        }) {
                            release(&path, &held.take().unwrap(), &owner);
                            // Release can block and the desired target can change
                            // meanwhile. Re-read it instead of claiming `next`.
                            continue;
                        }
                        if let Some(target) = next {
                            // The host may accept a claim even when its reply is
                            // lost. Keep every attempt for later/final release.
                            held = Some(target.clone());
                            let _ = exchange(
                                &path,
                                &target,
                                &owner,
                                true,
                                Deadline::cancellable(IPC_TIMEOUT, &cancelled),
                                || {
                                    reader.lock().unwrap().fresh(Instant::now()).as_ref()
                                        == Some(&target)
                                },
                            );
                        }
                        if !cancelled.load(Ordering::Acquire) {
                            thread::park_timeout(RENEW_INTERVAL);
                        }
                    }
                    if let Some(target) = held {
                        release(&path, &target, &owner);
                    }
                })
                .ok()?;
            Some(Self {
                desired,
                stop,
                thread: Some(thread),
            })
        }

        pub(super) fn update(&self, target: Option<PresentationTarget>) {
            let target = target.filter(|target| valid_host(&target.host));
            let changed = {
                let mut desired = self.desired.lock().unwrap();
                let changed = desired.target != target;
                desired.target = target;
                desired.updated = Instant::now();
                changed
            };
            if changed && let Some(worker) = &self.thread {
                worker.thread().unpark();
            }
        }
    }

    impl Drop for Worker {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(worker) = self.thread.take() {
                worker.thread().unpark();
                let _ = worker.join();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{os::unix::net::UnixListener, sync::mpsc};

        fn target(context: u64, revision: u64) -> PresentationTarget {
            PresentationTarget {
                host: "11111111-1111-4111-8111-111111111111".into(),
                context,
                revision,
            }
        }

        #[test]
        fn presentation_commands_reject_untrusted_protocol_tokens() {
            assert_eq!(
                command(&target(2, 3), "test-owner_1", true).unwrap(),
                "V11111111-1111-4111-8111-111111111111 2 3 test-owner_1 1"
            );
            assert!(
                command(&target(2, 0), "test-owner_1", false)
                    .unwrap()
                    .ends_with(" 0")
            );
            for host in [
                "",
                "bad host",
                "a".repeat(35).as_str(),
                "11111111-1111-4111-8111-11111111111\n",
                "11111111-1111-4111-8111-11111111111界",
            ] {
                let mut invalid = target(2, 3);
                invalid.host = host.into();
                assert!(command(&invalid, "owner", true).is_none(), "{host:?}");
            }
            for owner in [
                "",
                "has space",
                "line\nbreak",
                "非ASCII",
                "x".repeat(65).as_str(),
            ] {
                assert!(command(&target(2, 3), owner, true).is_none(), "{owner:?}");
            }
            assert!(command(&target(u64::MAX, u64::MAX), &owner_id(), true).is_some());
        }

        #[test]
        fn presentation_heartbeat_expires_without_ui_progress() {
            let now = Instant::now();
            let mut desired = Desired {
                target: Some(target(1, 1)),
                updated: now,
            };
            assert_eq!(desired.fresh(now + UI_FRESHNESS), Some(target(1, 1)));
            assert_eq!(
                desired.fresh(now + UI_FRESHNESS + Duration::from_nanos(1)),
                None
            );
            desired.target = Some(target(1, 5));
            desired.updated = now + UI_FRESHNESS;
            assert_eq!(desired.fresh(now + UI_FRESHNESS), Some(target(1, 5)));
            desired.target = None;
            assert_eq!(desired.fresh(now + UI_FRESHNESS), None);
        }

        struct SocketFixture(PathBuf);
        impl Drop for SocketFixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(self.0.join("ipc"));
                let _ = std::fs::remove_dir(&self.0);
            }
        }

        #[test]
        fn unknown_claim_is_released_before_latest_context_and_on_shutdown() {
            let directory = std::env::temp_dir().join(format!("suzaku-lease-{}", owner_id()));
            std::fs::create_dir(&directory).unwrap();
            let fixture = SocketFixture(directory);
            let path = fixture.0.join("ipc");
            let listener = UnixListener::bind(&path).unwrap();
            listener.set_nonblocking(true).unwrap();
            let (published, received) = mpsc::channel();
            let server = thread::spawn(move || {
                let end = Instant::now() + Duration::from_secs(3);
                let mut unanswered = Vec::new();
                let mut count = 0;
                let mut owner = None;
                while count < 4 && Instant::now() < end {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let request = Deadline::new(Duration::from_millis(250))
                                .read_to_end(&mut stream, 256)
                                .unwrap();
                            // Cancellation can occur after connect but before
                            // send. An empty EOF is not a protocol request and
                            // must not consume the expected final release.
                            if request.is_empty() {
                                continue;
                            }
                            let request = String::from_utf8(request).unwrap();
                            let owner = owner.get_or_insert_with(|| {
                                request.split_whitespace().nth(3).unwrap().to_owned()
                            });
                            let latest_claim = command(&target(4, 40), owner, true).unwrap();
                            // A queued unpark token may cause another renewal
                            // before Drop cancels the worker. Only that exact
                            // owner/context/revision renewal is permitted;
                            // still require the explicit final release below.
                            if count == 3 && request == latest_claim {
                                let _ = Deadline::new(IPC_TIMEOUT).send(&mut stream, b"1");
                                continue;
                            }
                            let expected = match count {
                                0 => command(&target(1, 10), owner, true).unwrap(),
                                1 => command(&target(1, 10), owner, false).unwrap(),
                                2 => latest_claim,
                                3 => command(&target(4, 40), owner, false).unwrap(),
                                _ => unreachable!(),
                            };
                            assert_eq!(request, expected, "unexpected presentation request");
                            published.send(request).unwrap();
                            if count < 2 {
                                // First claim has an unknown ACK; its release is
                                // also delayed so a newer desired target wins.
                                unanswered.push(stream);
                            } else {
                                let _ = Deadline::new(IPC_TIMEOUT).send(&mut stream, b"1");
                            }
                            count += 1;
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("private presentation listener: {error}"),
                    }
                }
                drop(unanswered);
                assert_eq!(count, 4);
            });
            let worker = Worker::start(path).unwrap();
            worker.update(Some(target(1, 10)));
            let first = received.recv_timeout(Duration::from_secs(1)).unwrap();
            worker.update(Some(target(2, 20)));
            worker.update(Some(target(3, 30)));
            let release = received.recv_timeout(Duration::from_secs(1)).unwrap();
            worker.update(Some(target(4, 40)));
            let latest = received.recv_timeout(Duration::from_secs(1)).unwrap();
            let before = Instant::now();
            drop(worker);
            assert!(
                before.elapsed() < Duration::from_millis(500),
                "shutdown must be bounded"
            );
            let final_release = received.recv_timeout(Duration::from_secs(1)).unwrap();
            let owner = first.split_whitespace().nth(3).unwrap();
            assert_eq!(first, command(&target(1, 10), owner, true).unwrap());
            assert_eq!(release, command(&target(1, 10), owner, false).unwrap());
            assert_eq!(latest, command(&target(4, 40), owner, true).unwrap());
            assert_eq!(
                final_release,
                command(&target(4, 40), owner, false).unwrap()
            );
            server.join().unwrap();
        }
    }
}
