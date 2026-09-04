use super::PanelUserEvent;
use winit::event_loop::EventLoopProxy;

#[cfg(target_os = "linux")]
mod platform {
    use super::{EventLoopProxy, PanelUserEvent};
    use std::env;
    use std::fs::{self, OpenOptions};
    use std::io::{self, ErrorKind, Write};
    use std::os::unix::fs::PermissionsExt;
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
        lock_path: PathBuf,
        identity: String,
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
            if fs::read_to_string(&self.lock_path).is_ok_and(|owner| owner.trim() == self.identity)
            {
                remove_file_if_present(&self.lock_path);
            }
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
        let runtime_dir = panel_runtime_dir()?;
        fs::create_dir_all(&runtime_dir)?;
        fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))?;

        let lock_path = runtime_dir.join("panel.lock");
        let socket_path = runtime_dir.join("control.sock");
        let identity = current_process_identity()?;

        for _ in 0..3 {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut lock_file) => {
                    if let Err(error) = lock_file
                        .set_permissions(fs::Permissions::from_mode(0o600))
                        .and_then(|()| writeln!(lock_file, "{identity}"))
                    {
                        drop(lock_file);
                        remove_file_if_present(&lock_path);
                        return Err(error);
                    }
                    return create_primary_instance(proxy, socket_path, lock_path, identity);
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if lock_owner_is_alive(&lock_path) {
                        signal_existing_instance(&socket_path)?;
                        return Ok(InstanceLaunch::ExistingSignaled);
                    }
                    match fs::remove_file(&lock_path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            }
        }

        Err(io::Error::new(
            ErrorKind::AlreadyExists,
            "could not claim the Suzaku panel instance lock",
        ))
    }

    fn create_primary_instance(
        proxy: EventLoopProxy<PanelUserEvent>,
        socket_path: PathBuf,
        lock_path: PathBuf,
        identity: String,
    ) -> io::Result<InstanceLaunch> {
        remove_file_if_present(&socket_path);
        let socket = match UnixDatagram::bind(&socket_path) {
            Ok(socket) => socket,
            Err(error) => {
                remove_owned_lock(&lock_path, &identity);
                return Err(error);
            }
        };
        if let Err(error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
            .and_then(|()| socket.set_read_timeout(Some(LISTENER_POLL_INTERVAL)))
        {
            remove_file_if_present(&socket_path);
            remove_owned_lock(&lock_path, &identity);
            return Err(error);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let listener_stop = Arc::clone(&stop);
        let listener = match thread::Builder::new()
            .name("suzaku-instance-control".into())
            .spawn(move || listen_for_instance_commands(socket, proxy, listener_stop))
        {
            Ok(listener) => listener,
            Err(error) => {
                remove_file_if_present(&socket_path);
                remove_owned_lock(&lock_path, &identity);
                return Err(error);
            }
        };

        Ok(InstanceLaunch::Primary(SingleInstanceGuard {
            listener: Some(listener),
            stop,
            socket_path,
            lock_path,
            identity,
        }))
    }

    fn listen_for_instance_commands(
        socket: UnixDatagram,
        proxy: EventLoopProxy<PanelUserEvent>,
        stop: Arc<AtomicBool>,
    ) {
        let mut message = [0_u8; 32];
        while !stop.load(Ordering::Acquire) {
            match socket.recv(&mut message) {
                Ok(length) if message.get(..length) == Some(CONTROL_SHOW) => {
                    let _ = proxy.send_event(PanelUserEvent::ShowPanel);
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
        UnixDatagram::unbound()?.send_to(message, socket_path)?;
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

    fn remove_owned_lock(lock_path: &Path, identity: &str) {
        if fs::read_to_string(lock_path).is_ok_and(|owner| owner.trim() == identity) {
            remove_file_if_present(lock_path);
        }
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
