//! Manage only Suzaku's registered user service, never the desktop IBus daemon.
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use suzaku_map::platform::{linux_command, linux_ipc::Deadline};

const UNIT: &str = "suzaku-ibus.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const PROBE_TIMEOUT: Duration = Duration::from_millis(350);
const START_TIMEOUT: Duration = Duration::from_secs(4);
const STOP_TIMEOUT: Duration = Duration::from_secs(7);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitState {
    Missing,
    Stopped,
    Running,
}

pub(super) trait Transport {
    fn can_manage(&self) -> bool;
    fn probe(&mut self, timeout: Duration) -> Result<(), String>;
    fn state(&mut self, timeout: Duration) -> Result<UnitState, String>;
    fn command(&mut self, operation: &str, timeout: Duration) -> Result<(), String>;
}

pub(super) struct HostService<T = SystemdTransport> {
    transport: T,
    managed: bool,
}

impl Default for HostService {
    fn default() -> Self {
        Self {
            transport: SystemdTransport::default(),
            managed: false,
        }
    }
}

impl<T: Transport> HostService<T> {
    pub fn ensure_ready(&mut self) -> Result<(), String> {
        self.ensure_with_timeout(START_TIMEOUT)
    }

    fn ensure_with_timeout(&mut self, timeout: Duration) -> Result<(), String> {
        let end = Instant::now() + timeout;
        let ready = self.transport.probe(remaining(end)?.min(PROBE_TIMEOUT));
        if !self.transport.can_manage() {
            // Custom/private endpoints belong to their launcher, not this desktop service.
            return ready;
        }
        let state = match self.transport.state(remaining(end)?.min(COMMAND_TIMEOUT)) {
            Ok(state) => state,
            Err(_) if ready.is_ok() => return Ok(()),
            Err(error) => return Err(error),
        };
        if state == UnitState::Running {
            self.managed = true;
        }
        if ready.is_ok() {
            return Ok(());
        }
        if state == UnitState::Missing {
            return Err(
                "Suzaku 输入法服务尚未注册，请先运行 suzaku-tool linux-register install".into(),
            );
        }
        // Even a command timeout may have queued the start; retain cleanup ownership.
        self.managed = true;
        self.transport
            .command("start", remaining(end)?.min(COMMAND_TIMEOUT))?;
        loop {
            if self
                .transport
                .probe(remaining(end)?.min(PROBE_TIMEOUT))
                .is_ok()
            {
                return Ok(());
            }
            pause(end)?;
        }
    }

    pub fn needs_stop(&mut self) -> Result<bool, String> {
        if !self.managed {
            return Ok(false);
        }
        if self.transport.state(COMMAND_TIMEOUT)? != UnitState::Running {
            self.managed = false;
        }
        Ok(self.managed)
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if !self.managed {
            return Ok(());
        }
        let end = Instant::now() + STOP_TIMEOUT;
        self.transport
            .command("stop", remaining(end)?.min(COMMAND_TIMEOUT))?;
        loop {
            if self.transport.state(remaining(end)?.min(COMMAND_TIMEOUT))? != UnitState::Running {
                self.managed = false;
                return Ok(());
            }
            pause(end)?;
        }
    }
}

fn remaining(end: Instant) -> Result<Duration, String> {
    let left = end.saturating_duration_since(Instant::now());
    if left.is_zero() {
        Err("等待 Suzaku 输入法服务就绪或停止超时，请重试并检查服务日志".into())
    } else {
        Ok(left)
    }
}

fn pause(end: Instant) -> Result<(), String> {
    std::thread::sleep(remaining(end)?.min(Duration::from_millis(40)));
    Ok(())
}

pub(super) struct SystemdTransport {
    socket: Option<PathBuf>,
    can_manage: bool,
}

fn is_desktop_endpoint(
    runtime: Option<&Path>,
    socket: Option<&Path>,
    bus: Option<&str>,
    uid: u32,
) -> bool {
    let expected = PathBuf::from(format!("/run/user/{uid}"));
    runtime == Some(expected.as_path())
        && socket == Some(expected.join("suzaku-ime/host.sock").as_path())
        && bus.is_none_or(|bus| bus == format!("unix:path={}/bus", expected.display()))
}

impl Default for SystemdTransport {
    fn default() -> Self {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let socket = suzaku_map::platform::linux_ime_sync::socket_path();
        let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        // SAFETY: getuid has no pointer arguments or state-changing effects.
        let uid = unsafe { libc::getuid() };
        Self {
            can_manage: is_desktop_endpoint(
                runtime.as_deref(),
                socket.as_deref(),
                bus.as_deref(),
                uid,
            ),
            socket,
        }
    }
}

impl SystemdTransport {
    fn run(&self, arguments: &[&str], timeout: Duration) -> Result<String, String> {
        let output = linux_command::run(
            Command::new("systemctl").arg("--user").args(arguments),
            timeout,
        )
        .map_err(|error| format!("无法控制 Suzaku 用户服务：{error}"))?;
        if !output.status.success() {
            return Err(
                "Suzaku 用户服务操作失败，请检查 systemctl --user status suzaku-ibus.service"
                    .into(),
            );
        }
        String::from_utf8(output.stdout).map_err(|_| "用户服务返回了无效状态".into())
    }
}

impl Transport for SystemdTransport {
    fn can_manage(&self) -> bool {
        self.can_manage
    }

    fn probe(&mut self, timeout: Duration) -> Result<(), String> {
        let path = self
            .socket
            .as_ref()
            .ok_or("无法定位本机输入法服务（缺少 XDG_RUNTIME_DIR）")?;
        let deadline = Deadline::new(timeout);
        let exchange = || -> std::io::Result<bool> {
            let mut stream = deadline.connect(path)?;
            deadline.send(&mut stream, b"Q")?;
            let mut response = [0];
            deadline.read_exact(&mut stream, &mut response)?;
            Ok(response == [b'1'])
        };
        match exchange() {
            Ok(true) => Ok(()),
            Ok(false) => Err("Suzaku 输入法服务尚未就绪，请重试".into()),
            Err(error) => Err(format!("Suzaku 输入法服务未运行或连接已断开：{error}")),
        }
    }

    fn state(&mut self, timeout: Duration) -> Result<UnitState, String> {
        parse_state(&self.run(&["show", "--property=LoadState,ActiveState", UNIT], timeout)?)
    }

    fn command(&mut self, operation: &str, timeout: Duration) -> Result<(), String> {
        self.run(&["--no-block", operation, UNIT], timeout)
            .map(|_| ())
    }
}

fn parse_state(raw: &str) -> Result<UnitState, String> {
    let value = |name: &str| raw.lines().find_map(|line| line.strip_prefix(name));
    match (value("LoadState="), value("ActiveState=")) {
        (Some("not-found"), _) => Ok(UnitState::Missing),
        (Some("loaded"), Some("inactive" | "failed")) => Ok(UnitState::Stopped),
        (
            Some("loaded"),
            Some("active" | "activating" | "deactivating" | "reloading" | "refreshing"),
        ) => Ok(UnitState::Running),
        (Some("masked"), _) => Err("Suzaku 用户服务已被屏蔽，未擅自解除屏蔽".into()),
        _ => Err("无法确认 Suzaku 用户服务状态，未强制启停".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        state: UnitState,
        ready: bool,
        custom: bool,
        fail: bool,
        delayed: bool,
        calls: Vec<String>,
    }
    impl Transport for Fake {
        fn can_manage(&self) -> bool {
            !self.custom
        }
        fn probe(&mut self, _: Duration) -> Result<(), String> {
            if self.ready {
                Ok(())
            } else {
                Err("not ready".into())
            }
        }
        fn state(&mut self, _: Duration) -> Result<UnitState, String> {
            self.calls.push("state".into());
            Ok(self.state)
        }
        fn command(&mut self, operation: &str, _: Duration) -> Result<(), String> {
            self.calls.push(operation.into());
            if self.fail {
                return Err("command failed".into());
            }
            self.state = if operation == "start" {
                UnitState::Running
            } else {
                UnitState::Stopped
            };
            self.ready = operation == "start" && !self.delayed;
            Ok(())
        }
    }
    fn fixture(state: UnitState, ready: bool) -> HostService<Fake> {
        HostService {
            managed: false,
            transport: Fake {
                state,
                ready,
                custom: false,
                fail: false,
                delayed: false,
                calls: vec![],
            },
        }
    }

    #[test]
    fn cold_start_waits_for_readiness_and_repeated_start_stop_are_safe() {
        let mut host = fixture(UnitState::Stopped, false);
        host.ensure_ready().unwrap();
        host.ensure_ready().unwrap();
        assert!(host.needs_stop().unwrap());
        host.stop().unwrap();
        host.stop().unwrap();
        assert_eq!(
            host.transport
                .calls
                .iter()
                .filter(|call| *call == "start")
                .count(),
            1
        );
        assert_eq!(
            host.transport
                .calls
                .iter()
                .filter(|call| *call == "stop")
                .count(),
            1
        );
        assert!(!host.needs_stop().unwrap());
        host.ensure_ready().unwrap();
        assert!(host.needs_stop().unwrap());
    }

    #[test]
    fn existing_service_is_adopted_without_restart_but_external_hosts_are_not_stopped() {
        for (state, managed) in [
            (UnitState::Running, true),
            (UnitState::Stopped, false),
            (UnitState::Missing, false),
        ] {
            let mut host = fixture(state, true);
            host.ensure_ready().unwrap();
            assert_eq!(host.needs_stop().unwrap(), managed);
            host.stop().unwrap();
            assert!(!host.transport.calls.contains(&"start".into()));
            assert_eq!(host.transport.calls.contains(&"stop".into()), managed);
        }
    }

    #[test]
    fn private_endpoints_never_start_or_stop_desktop_services() {
        for ready in [false, true] {
            let mut host = fixture(UnitState::Running, ready);
            host.transport.custom = true;
            assert_eq!(host.ensure_ready().is_ok(), ready);
            host.stop().unwrap();
            assert!(host.transport.calls.is_empty());
        }
        let root = Path::new("/run/user/1000");
        let socket = root.join("suzaku-ime/host.sock");
        assert!(is_desktop_endpoint(
            Some(root),
            Some(&socket),
            Some("unix:path=/run/user/1000/bus"),
            1000
        ));
        assert!(!is_desktop_endpoint(
            Some(root),
            Some(&socket),
            Some("unix:path=/tmp/private-bus"),
            1000
        ));
        assert!(!is_desktop_endpoint(
            Some(Path::new("/tmp/private")),
            Some(&socket),
            None,
            1000
        ));
        assert!(!is_desktop_endpoint(
            Some(root),
            Some(Path::new("/tmp/custom.sock")),
            None,
            1000
        ));
    }

    #[test]
    fn missing_service_and_failed_or_unready_start_never_report_ready() {
        let mut host = fixture(UnitState::Missing, false);
        assert!(host.ensure_ready().unwrap_err().contains("注册"));
        assert!(!host.managed);
        let mut host = fixture(UnitState::Stopped, false);
        host.transport.fail = true;
        assert!(host.ensure_ready().is_err());
        host.transport.fail = false;
        host.transport.delayed = true;
        assert!(
            host.ensure_with_timeout(Duration::from_millis(25))
                .unwrap_err()
                .contains("超时")
        );
        assert!(host.needs_stop().unwrap());
        host.transport.fail = true;
        assert!(host.stop().is_err());
        assert!(host.managed, "a failed stop must remain retryable");
        host.transport.fail = false;
        host.stop().unwrap();
    }

    #[test]
    fn unit_state_requires_known_loaded_service_status() {
        assert_eq!(
            parse_state("LoadState=loaded\nActiveState=active\n").unwrap(),
            UnitState::Running
        );
        assert_eq!(
            parse_state("ActiveState=inactive\nLoadState=loaded\n").unwrap(),
            UnitState::Stopped
        );
        assert_eq!(
            parse_state("LoadState=not-found\nActiveState=inactive\n").unwrap(),
            UnitState::Missing
        );
        assert!(
            parse_state("LoadState=masked\nActiveState=inactive\n")
                .unwrap_err()
                .contains("屏蔽")
        );
        assert!(parse_state("unrecognized response").is_err());
    }
}
