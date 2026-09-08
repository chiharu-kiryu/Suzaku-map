//! Tray-owned IBus activation. All host I/O runs on the tray worker, never winit.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const SUZAKU_ENGINE: &str = "dev.suzaku.linux.ime";
const HOST_TIMEOUT: Duration = Duration::from_millis(350);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct InputMethodState {
    pub current_engine: Option<String>,
    pub restore_engine: Option<String>,
}

impl InputMethodState {
    pub fn active(&self) -> bool {
        self.current_engine.as_deref() == Some(SUZAKU_ENGINE)
    }
}

pub(super) trait InputMethodBackend {
    fn current_engine(&mut self) -> Result<String, String>;
    fn ensure_suzaku_available(&mut self) -> Result<(), String>;
    fn switch_engine(&mut self, engine: &str) -> Result<(), String>;
}

pub(super) struct InputMethodController<B> {
    backend: B,
    state: InputMethodState,
}

impl<B: InputMethodBackend> InputMethodController<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            state: InputMethodState::default(),
        }
    }

    pub fn state(&self) -> &InputMethodState {
        &self.state
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        match self.backend.current_engine().and_then(validate_engine) {
            Ok(engine) => {
                self.state.current_engine = Some(engine);
                Ok(())
            }
            Err(error) => {
                self.state.current_engine = None;
                Err(error)
            }
        }
    }

    pub fn activate(&mut self) -> Result<(), String> {
        self.refresh()?;
        self.backend.ensure_suzaku_available()?;
        if self.state.active() {
            // Repeated activation must not replace the restore target with Suzaku.
            // A system-selected Suzaku session is not owned by this tray.
            return Ok(());
        }
        // Remember before switching: even a timeout may have changed the engine.
        self.state.restore_engine = self.state.current_engine.clone();
        self.switch_and_verify(SUZAKU_ENGINE)
    }

    pub fn release(&mut self) -> Result<(), String> {
        let Some(previous) = self.state.restore_engine.clone() else {
            return Ok(());
        };
        self.refresh()?;
        if self.state.active() {
            self.switch_and_verify(&previous)?;
        }
        // A manual switch away from Suzaku takes precedence over our old target.
        self.state.restore_engine = None;
        Ok(())
    }

    fn switch_and_verify(&mut self, target: &str) -> Result<(), String> {
        let switched = self.backend.switch_engine(target);
        let refreshed = self.refresh();
        if refreshed.is_ok() && self.state.current_engine.as_deref() == Some(target) {
            return Ok(());
        }
        // Retain restore_engine on every uncertain/failed switch, so release can retry.
        switched?;
        refreshed?;
        Err("系统未完成输入法切换，请重试；原输入法已保留".into())
    }
}

fn validate_engine(engine: String) -> Result<String, String> {
    if engine.is_empty()
        || engine == "dummy"
        || engine.len() > 256
        || engine.starts_with('-')
        || engine.chars().any(char::is_whitespace)
        || engine.chars().any(char::is_control)
    {
        Err("无法确认当前输入法，未改动系统输入设置".into())
    } else {
        Ok(engine)
    }
}

pub(super) struct IBusBackend;

impl InputMethodBackend for IBusBackend {
    fn current_engine(&mut self) -> Result<String, String> {
        run_ibus(&[]).and_then(validate_engine)
    }

    fn ensure_suzaku_available(&mut self) -> Result<(), String> {
        let socket = std::env::var_os("SUZAKU_LINUX_IME_SOCKET")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_RUNTIME_DIR")
                    .map(std::path::PathBuf::from)
                    .map(|directory| directory.join("suzaku-ime/host.sock"))
            })
            .ok_or("无法定位本机输入法服务（缺少 XDG_RUNTIME_DIR）")?;
        let mut stream = UnixStream::connect(socket).map_err(|_| {
            "Suzaku 输入法服务未运行，请先运行 suzaku_tool linux-register install".to_string()
        })?;
        stream
            .set_read_timeout(Some(HOST_TIMEOUT))
            .map_err(host_error)?;
        stream
            .set_write_timeout(Some(HOST_TIMEOUT))
            .map_err(host_error)?;
        stream.write_all(b"Q").map_err(host_error)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(host_error)?;
        let mut response = [0];
        stream.read_exact(&mut response).map_err(host_error)?;
        if response == [b'1'] {
            Ok(())
        } else {
            Err("Suzaku 输入法服务尚未就绪，请重启输入法服务后重试".into())
        }
    }

    fn switch_engine(&mut self, engine: &str) -> Result<(), String> {
        validate_engine(engine.to_string())?;
        run_ibus(&[engine]).map(|_| ())
    }
}

fn host_error(error: std::io::Error) -> String {
    format!("Suzaku 输入法服务未响应：{error}")
}

fn run_ibus(args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("ibus");
    command.arg("engine").args(args);
    let output = run_bounded(&mut command, COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err("IBus 操作失败，请确认桌面输入法服务正在运行".into());
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_string())
        .map_err(|_| "IBus 返回了无法识别的输入法名称".into())
}

fn run_bounded(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法启动 ibus 命令，请确认 IBus 已安装".to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|_| "无法读取 IBus 状态".into());
            }
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(15));
            }
            status => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if status.is_err() {
                    "无法检查 IBus 状态，请重试".into()
                } else {
                    "输入法切换超时，请重试；原输入法已保留".into()
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend {
        current: String,
        switches: Vec<String>,
        available: bool,
        read_fails: bool,
        switch_fails: bool,
        change_on_failure: bool,
        ignore_switch: bool,
    }

    impl Default for FakeBackend {
        fn default() -> Self {
            Self {
                current: "rime".into(),
                switches: Vec::new(),
                available: true,
                read_fails: false,
                switch_fails: false,
                change_on_failure: false,
                ignore_switch: false,
            }
        }
    }

    impl InputMethodBackend for FakeBackend {
        fn current_engine(&mut self) -> Result<String, String> {
            if self.read_fails {
                Err("read failed".into())
            } else {
                Ok(self.current.clone())
            }
        }

        fn ensure_suzaku_available(&mut self) -> Result<(), String> {
            if self.available {
                Ok(())
            } else {
                Err("host unavailable".into())
            }
        }

        fn switch_engine(&mut self, engine: &str) -> Result<(), String> {
            self.switches.push(engine.into());
            if !self.ignore_switch && (!self.switch_fails || self.change_on_failure) {
                self.current = engine.into();
            }
            if self.switch_fails {
                Err("switch failed".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn activation_restores_the_actual_previous_engine() {
        for previous in ["rime", "mozc-jp", "xkb:us::eng"] {
            let mut control = InputMethodController::new(FakeBackend {
                current: previous.into(),
                ..Default::default()
            });
            control.activate().unwrap();
            assert!(control.state.active());
            assert_eq!(control.state.restore_engine.as_deref(), Some(previous));
            control.release().unwrap();
            assert_eq!(control.backend.current, previous);
            assert_eq!(control.state.restore_engine, None);
        }
    }

    #[test]
    fn repeated_activation_and_release_are_idempotent() {
        let mut control = InputMethodController::new(FakeBackend::default());
        control.activate().unwrap();
        control.activate().unwrap();
        assert_eq!(control.state.restore_engine.as_deref(), Some("rime"));
        control.release().unwrap();
        control.release().unwrap();
        assert_eq!(control.backend.switches, [SUZAKU_ENGINE, "rime"]);
    }

    #[test]
    fn release_preserves_a_manual_switch_to_another_engine() {
        let mut control = InputMethodController::new(FakeBackend::default());
        control.activate().unwrap();
        control.backend.current = "mozc-jp".into();
        control.release().unwrap();
        assert_eq!(control.backend.current, "mozc-jp");
        assert_eq!(control.backend.switches, [SUZAKU_ENGINE]);
        assert_eq!(control.state.restore_engine, None);
    }

    #[test]
    fn reactivation_after_manual_switch_uses_the_new_restore_target() {
        let mut control = InputMethodController::new(FakeBackend::default());
        control.activate().unwrap();
        control.backend.current = "mozc-jp".into();
        control.activate().unwrap();
        control.release().unwrap();
        assert_eq!(control.backend.current, "mozc-jp");
    }

    #[test]
    fn external_suzaku_activation_is_not_owned_by_the_tray() {
        let mut control = InputMethodController::new(FakeBackend {
            current: SUZAKU_ENGINE.into(),
            ..Default::default()
        });
        control.activate().unwrap();
        control.release().unwrap();
        assert!(control.backend.switches.is_empty());
        assert!(control.state.active());
        assert_eq!(control.state.restore_engine, None);
    }

    #[test]
    fn missing_host_or_unrestorable_engine_never_changes_system_input() {
        let mut control = InputMethodController::new(FakeBackend {
            available: false,
            ..Default::default()
        });
        assert!(control.activate().is_err());
        assert!(control.backend.switches.is_empty());
        assert_eq!(control.state.restore_engine, None);
        for current in ["", "dummy", "--help", "rime\nmozc-jp", "bad\0name"] {
            let mut control = InputMethodController::new(FakeBackend {
                current: current.into(),
                ..Default::default()
            });
            assert!(control.activate().is_err());
            assert!(control.backend.switches.is_empty());
        }
    }

    #[test]
    fn failed_restore_remains_retryable() {
        let mut control = InputMethodController::new(FakeBackend::default());
        control.activate().unwrap();
        control.backend.switch_fails = true;
        assert!(control.release().is_err());
        assert_eq!(control.state.restore_engine.as_deref(), Some("rime"));
        control.backend.switch_fails = false;
        control.release().unwrap();
        assert_eq!(control.backend.current, "rime");
    }

    #[test]
    fn unavailable_status_keeps_restore_target_and_never_switches_blindly() {
        let mut control = InputMethodController::new(FakeBackend::default());
        control.activate().unwrap();
        control.backend.read_fails = true;
        assert!(control.release().is_err());
        assert_eq!(control.state.restore_engine.as_deref(), Some("rime"));
        assert_eq!(control.state.current_engine, None);
        assert_eq!(control.backend.switches, [SUZAKU_ENGINE]);
    }

    #[test]
    fn successful_command_without_an_actual_switch_is_an_error() {
        let mut control = InputMethodController::new(FakeBackend {
            ignore_switch: true,
            ..Default::default()
        });
        assert!(control.activate().is_err());
        assert!(!control.state.active());
        assert_eq!(control.state.restore_engine.as_deref(), Some("rime"));
    }

    #[test]
    fn command_error_after_successful_switch_is_verified_and_restorable() {
        let mut control = InputMethodController::new(FakeBackend {
            switch_fails: true,
            change_on_failure: true,
            ..Default::default()
        });
        control.activate().unwrap();
        assert!(control.state.active());
        control.release().unwrap();
        assert_eq!(control.backend.current, "rime");
    }

    #[test]
    fn stalled_host_command_is_killed_and_reaped() {
        let start = Instant::now();
        let result = run_bounded(Command::new("sleep").arg("10"), Duration::from_millis(45));
        assert!(result.unwrap_err().contains("超时"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_command_captures_status_output() {
        let output = run_bounded(Command::new("printf").arg("rime"), COMMAND_TIMEOUT).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"rime");
    }

    #[cfg(feature = "linux-ibus")]
    #[test]
    #[ignore = "temporarily switches the live desktop input method; requires the native Suzaku host"]
    fn native_activation_input_and_release_roundtrip() {
        struct RestoreOnDrop(InputMethodController<IBusBackend>);
        impl Drop for RestoreOnDrop {
            fn drop(&mut self) {
                if let Err(error) = self.0.release() {
                    eprintln!("Native activation probe could not restore input: {error}");
                }
            }
        }
        let previous = IBusBackend
            .current_engine()
            .expect("a live, restorable IBus engine");
        assert_ne!(
            previous, SUZAKU_ENGINE,
            "select another input method before running this probe"
        );
        let mut guard = RestoreOnDrop(InputMethodController::new(IBusBackend));
        let control = &mut guard.0;
        control.activate().unwrap();
        control.activate().unwrap();
        assert_eq!(
            control.state.restore_engine.as_deref(),
            Some(previous.as_str())
        );
        let report = suzaku_map::platform::linux_ibus_host::probe_roundtrip_report("ni").unwrap();
        assert_eq!(report.preedit, "ni");
        assert_eq!(report.page_size, 3);
        assert!(report.candidate_count > 0);
        assert!(!report.committed.is_empty());
        control.release().unwrap();
        assert_eq!(IBusBackend.current_engine().unwrap(), previous);
        assert!(control.state.restore_engine.is_none());
    }
}
