use super::PanelUserEvent;
use winit::event_loop::EventLoopProxy;

#[cfg(target_os = "linux")]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use std::env;
    use std::fs::{self, File, OpenOptions, TryLockError};
    use std::io::{self, ErrorKind, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixDatagram;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    const CONTROL_SHOW: &[u8] = b"show";
    const CONTROL_WAKE: &[u8] = b"wake";
    const CONTROL_RETRY_COUNT: usize = 12;
    const CONTROL_RETRY_DELAY: Duration = Duration::from_millis(25);
    const LISTENER_POLL_INTERVAL: Duration = Duration::from_millis(200);

    pub(crate) enum InstanceLaunch {
        Primary(SingleInstanceGuard),
        ExistingSignaled,
    }

    pub(crate) struct SingleInstanceGuard {
        listener: Option<JoinHandle<()>>,
        stop: Arc<AtomicBool>,
        socket_path: PathBuf,
        // Never unlink this file: all launches must contend on the same inode.
        // The OS releases the lock even if the process crashes.
        lock_file: File,
    }

    impl SingleInstanceGuard {
        pub(crate) fn shutdown(self) {
            drop(self);
        }

        fn stop_and_cleanup(&mut self) {
            self.stop.store(true, Ordering::Release);
            let _ = send_control_message(&self.socket_path, CONTROL_WAKE);
            if let Some(listener) = self.listener.take() {
                let _ = listener.join();
            }
            remove_file_if_present(&self.socket_path);
            // Clear legacy PID metadata while still holding the exclusive lock.
            let _ = self.lock_file.set_len(0);
        }
    }

    impl Drop for SingleInstanceGuard {
        fn drop(&mut self) {
            self.stop_and_cleanup();
        }
    }

    pub(crate) fn claim_single_instance(
        proxy: EventLoopProxy<PanelUserEvent>,
    ) -> io::Result<InstanceLaunch> {
        claim_instance_at(panel_runtime_dir()?, move || {
            let _ = proxy.send_event(PanelUserEvent::ShowPanel);
        })
    }

    fn claim_instance_at(
        runtime_dir: PathBuf,
        show_panel: impl Fn() + Send + 'static,
    ) -> io::Result<InstanceLaunch> {
        fs::create_dir_all(&runtime_dir)?;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))?;

        let lock_path = runtime_dir.join("panel.lock");
        let socket_path = runtime_dir.join("control.sock");
        let identity = current_process_identity()?;

        let mut lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock_path)?;
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                signal_existing_instance(&socket_path)?;
                return Ok(InstanceLaunch::ExistingSignaled);
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        // Respect an already-running older panel which only used PID metadata.
        if lock_owner_is_alive(&lock_path) {
            signal_existing_instance(&socket_path)?;
            return Ok(InstanceLaunch::ExistingSignaled);
        }
        lock_file.set_permissions(fs::Permissions::from_mode(0o600))?;
        lock_file.set_len(0)?;
        writeln!(lock_file, "{identity}")?;
        create_primary_instance(show_panel, socket_path, lock_file)
    }

    fn create_primary_instance(
        show_panel: impl Fn() + Send + 'static,
        socket_path: PathBuf,
        lock_file: File,
    ) -> io::Result<InstanceLaunch> {
        remove_file_if_present(&socket_path);
        let socket = match UnixDatagram::bind(&socket_path) {
            Ok(socket) => socket,
            Err(error) => {
                let _ = lock_file.set_len(0);
                return Err(error);
            }
        };
        if let Err(error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
            .and_then(|()| socket.set_read_timeout(Some(LISTENER_POLL_INTERVAL)))
        {
            remove_file_if_present(&socket_path);
            let _ = lock_file.set_len(0);
            return Err(error);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let listener_stop = Arc::clone(&stop);
        let listener = match thread::Builder::new()
            .name("suzaku-instance-control".into())
            .spawn(move || listen_for_instance_commands(socket, show_panel, listener_stop))
        {
            Ok(listener) => listener,
            Err(error) => {
                remove_file_if_present(&socket_path);
                let _ = lock_file.set_len(0);
                return Err(error);
            }
        };

        Ok(InstanceLaunch::Primary(SingleInstanceGuard {
            listener: Some(listener),
            stop,
            socket_path,
            lock_file,
        }))
    }

    fn listen_for_instance_commands(
        socket: UnixDatagram,
        show_panel: impl Fn(),
        stop: Arc<AtomicBool>,
    ) {
        let mut message = [0_u8; 32];
        while !stop.load(Ordering::Acquire) {
            match socket.recv(&mut message) {
                Ok(length) if message.get(..length) == Some(CONTROL_SHOW) => {
                    show_panel();
                }
                Ok(_) => {}
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => break,
            }
        }
    }

    fn signal_existing_instance(socket_path: &Path) -> io::Result<()> {
        let mut last_error = None;
        for attempt in 0..CONTROL_RETRY_COUNT {
            match send_control_message(socket_path, CONTROL_SHOW) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            if attempt + 1 < CONTROL_RETRY_COUNT {
                thread::sleep(CONTROL_RETRY_DELAY);
            }
        }
        Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                ErrorKind::NotFound,
                "Suzaku panel control socket is missing",
            )
        }))
    }

    fn send_control_message(socket_path: &Path, message: &[u8]) -> io::Result<()> {
        let socket = UnixDatagram::unbound()?;
        socket.set_write_timeout(Some(CONTROL_RETRY_DELAY))?;
        socket.send_to(message, socket_path)?;
        Ok(())
    }

    fn panel_runtime_dir() -> io::Result<PathBuf> {
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "XDG_RUNTIME_DIR is not set"))?;
        Ok(runtime_dir.join("suzaku-panel"))
    }

    fn current_process_identity() -> io::Result<String> {
        let pid = std::process::id();
        let start_ticks = process_start_ticks(pid).ok_or_else(|| {
            io::Error::new(
                ErrorKind::NotFound,
                "could not read the current process start time",
            )
        })?;
        Ok(format!("{pid} {start_ticks}"))
    }

    fn lock_owner_is_alive(lock_path: &Path) -> bool {
        let Ok(owner) = fs::read_to_string(lock_path) else {
            return false;
        };
        let mut fields = owner.split_whitespace();
        let (Some(pid), Some(expected_start_ticks), None) =
            (fields.next(), fields.next(), fields.next())
        else {
            return false;
        };
        let (Ok(pid), Ok(expected_start_ticks)) =
            (pid.parse::<u32>(), expected_start_ticks.parse::<u64>())
        else {
            return false;
        };
        process_start_ticks(pid) == Some(expected_start_ticks)
    }

    fn process_start_ticks(pid: u32) -> Option<u64> {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let after_process_name = stat.rsplit_once(')')?.1;
        after_process_name
            .split_whitespace()
            .nth(19)
            .and_then(|value| value.parse().ok())
    }

    fn remove_file_if_present(path: &Path) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => eprintln!("Could not remove {}: {error}", path.display()),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            CONTROL_SHOW, current_process_identity, lock_owner_is_alive, process_start_ticks,
            send_control_message,
        };
        use std::fs;
        use std::os::unix::net::UnixDatagram;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};

        fn test_path(label: &str) -> PathBuf {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            std::env::temp_dir().join(format!(
                "suzaku-panel-{label}-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ))
        }

        #[test]
        fn current_linux_process_identity_is_live() {
            let lock_path = test_path("identity.lock");
            let identity = current_process_identity().expect("current process identity");
            fs::write(&lock_path, format!("{identity}\n")).expect("write test lock");

            assert!(lock_owner_is_alive(&lock_path));
            assert!(process_start_ticks(std::process::id()).is_some());

            fs::remove_file(lock_path).expect("remove test lock");
        }

        #[test]
        fn stale_or_malformed_instance_locks_are_rejected() {
            let lock_path = test_path("stale.lock");
            fs::write(&lock_path, "not a process identity\n").expect("write malformed lock");
            assert!(!lock_owner_is_alive(&lock_path));

            fs::write(&lock_path, "4294967295 1\n").expect("write stale lock");
            assert!(!lock_owner_is_alive(&lock_path));

            fs::remove_file(lock_path).expect("remove test lock");
        }

        #[test]
        fn show_command_round_trips_over_the_runtime_socket() {
            let socket_path = test_path("control.sock");
            let receiver = UnixDatagram::bind(&socket_path).expect("bind test control socket");
            send_control_message(&socket_path, CONTROL_SHOW).expect("send show command");

            let mut message = [0_u8; 16];
            let length = receiver.recv(&mut message).expect("receive show command");
            assert_eq!(&message[..length], CONTROL_SHOW);

            drop(receiver);
            fs::remove_file(socket_path).expect("remove test socket");
        }

        #[test]
        fn concurrent_launches_keep_exactly_one_primary_and_can_relaunch() {
            use super::{InstanceLaunch, claim_instance_at};
            use std::sync::{Arc, Barrier};
            use std::thread;
            for _ in 0..4 {
                let directory = test_path("concurrent");
                let barrier = Arc::new(Barrier::new(24));
                let workers: Vec<_> = (0..24)
                    .map(|_| {
                        let directory = directory.clone();
                        let barrier = barrier.clone();
                        thread::spawn(move || {
                            barrier.wait();
                            claim_instance_at(directory, || {})
                        })
                    })
                    .collect();
                let outcomes: Vec<_> = workers
                    .into_iter()
                    .map(|worker| worker.join().unwrap())
                    .collect();
                let primary = outcomes
                    .iter()
                    .filter(|result| matches!(result, Ok(InstanceLaunch::Primary(_))))
                    .count();
                let failures: Vec<_> = outcomes
                    .iter()
                    .filter_map(|result| result.as_ref().err())
                    .collect();
                assert_eq!(primary, 1, "multiple live primary guards");
                assert!(failures.is_empty(), "secondary launch failed: {failures:?}");
                drop(outcomes);
                assert!(!directory.join("control.sock").exists());
                let relaunched = claim_instance_at(directory.clone(), || {}).unwrap();
                assert!(matches!(relaunched, InstanceLaunch::Primary(_)));
                drop(relaunched);
                fs::remove_dir_all(directory).unwrap();
            }
        }

        #[test]
        fn a_running_legacy_panel_keeps_its_socket_and_identity() {
            use super::{InstanceLaunch, claim_instance_at};
            let directory = test_path("legacy");
            fs::create_dir(&directory).unwrap();
            let lock_path = directory.join("panel.lock");
            let identity = current_process_identity().unwrap();
            fs::write(&lock_path, &identity).unwrap();
            let socket_path = directory.join("control.sock");
            let receiver = UnixDatagram::bind(&socket_path).unwrap();
            receiver
                .set_read_timeout(Some(std::time::Duration::from_secs(1)))
                .unwrap();
            assert!(matches!(
                claim_instance_at(directory.clone(), || {}).unwrap(),
                InstanceLaunch::ExistingSignaled
            ));
            let mut bytes = [0; 16];
            let size = receiver.recv(&mut bytes).unwrap();
            assert_eq!(&bytes[..size], CONTROL_SHOW);
            assert_eq!(fs::read_to_string(lock_path).unwrap(), identity);
            drop(receiver);
            fs::remove_dir_all(directory).unwrap();
        }

        #[test]
        fn instance_process_fixture() {
            use std::io::Read;
            let Some(directory) = std::env::var_os("SUZAKU_INSTANCE_TEST_DIR") else {
                return;
            };
            let directory = PathBuf::from(directory);
            assert!(directory.starts_with(std::env::temp_dir()));
            assert!(
                directory
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("suzaku-panel-crash-")
            );
            let guard = super::claim_instance_at(directory.clone(), || {}).unwrap();
            assert!(matches!(guard, super::InstanceLaunch::Primary(_)));
            fs::write(directory.join("ready"), b"ready").unwrap();
            let _ = std::io::stdin().read(&mut [0]);
            drop(guard);
        }

        #[test]
        fn crashed_process_releases_the_lock_and_stale_socket_can_be_replaced() {
            use std::process::{Child, Command, Stdio};
            use std::time::{Duration, Instant};
            struct TestChild(Child);
            impl Drop for TestChild {
                fn drop(&mut self) {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }
            let directory = test_path("crash");
            let mut child = TestChild(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "instance::platform::tests::instance_process_fixture",
                        "--nocapture",
                    ])
                    .env("SUZAKU_INSTANCE_TEST_DIR", &directory)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while !directory.join("ready").exists() {
                assert!(Instant::now() < deadline, "child did not acquire the lock");
                assert!(child.0.try_wait().unwrap().is_none(), "child exited early");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(matches!(
                super::claim_instance_at(directory.clone(), || {}).unwrap(),
                super::InstanceLaunch::ExistingSignaled
            ));
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            let guard = super::claim_instance_at(directory.clone(), || {}).unwrap();
            assert!(matches!(guard, super::InstanceLaunch::Primary(_)));
            drop(guard);
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use std::io;

    pub(crate) enum InstanceLaunch {
        Primary(SingleInstanceGuard),
        ExistingSignaled,
    }

    pub(crate) struct SingleInstanceGuard;

    impl SingleInstanceGuard {
        pub(crate) fn shutdown(self) {}
    }

    pub(crate) fn claim_single_instance(
        _proxy: EventLoopProxy<PanelUserEvent>,
    ) -> io::Result<InstanceLaunch> {
        Ok(InstanceLaunch::Primary(SingleInstanceGuard))
    }
}

pub(super) use platform::{InstanceLaunch, SingleInstanceGuard, claim_single_instance};
