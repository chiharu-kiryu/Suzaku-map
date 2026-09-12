//! Tray-owned IBus activation. All host I/O runs on the tray worker, never winit.

use std::process::{Command, Output};
use std::time::Duration;

#[path = "input_method_service.rs"]
mod service;

pub(super) const SUZAKU_ENGINE: &str = "dev.suzaku.linux.ime";
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
    fn host_needs_stop(&mut self) -> Result<bool, String>;
    fn stop_host(&mut self) -> Result<(), String>;
}

pub(super) struct InputMethodController<B> {
    backend: B,
    state: InputMethodState,
    last_non_suzaku: Option<String>,
}

impl<B: InputMethodBackend> InputMethodController<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            state: InputMethodState::default(),
            last_non_suzaku: None,
        }
    }

    pub fn state(&self) -> &InputMethodState {
        &self.state
    }

    pub fn refresh(&mut self) -> Result<(), String> {
        match self.backend.current_engine().and_then(validate_engine) {
            Ok(engine) => {
                if engine != SUZAKU_ENGINE {
                    self.last_non_suzaku = Some(engine.clone());
                }
                self.state.current_engine = Some(engine);
                Ok(())
            }
            Err(error) => {
                self.state.current_engine = None;
                Err(error)
            }
        }
    }

    /// Prepare the service without activating Suzaku or stealing the current engine.
    pub fn start(&mut self) -> Result<(), String> {
        let _ = self.refresh();
        let previous = self.state.current_engine.clone();
        self.backend.ensure_suzaku_available()?;
        match self.refresh() {
            Ok(()) => Ok(()),
            Err(_) if previous.is_some() => self.switch_and_verify(previous.as_deref().unwrap()),
            Err(error) => Err(error),
        }
    }

    pub fn activate(&mut self) -> Result<(), String> {
        // A disconnected active engine can temporarily leave IBus without a
        // global engine. Start its host before requiring a valid switch target.
        let _ = self.refresh();
        self.backend.ensure_suzaku_available()?;
        self.refresh()?;
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

    pub fn shutdown(&mut self) -> Result<(), String> {
        self.release()?;
        if !self.backend.host_needs_stop()? {
            return Ok(());
        }
        self.refresh()?;
        if self.state.active() {
            // A system menu can activate Suzaku after panel startup. Use only an
            // actually observed previous engine; never guess another user's default.
            let previous = self
                .last_non_suzaku
                .clone()
                .ok_or("无法确认可恢复的原输入法；请先从系统菜单切换其他输入法，再退出 Suzaku")?;
            self.state.restore_engine = Some(previous);
            self.release()?;
        }
        let previous = self
            .state
            .current_engine
            .clone()
            .ok_or("无法确认安全回退，已保留输入法服务")?;
        self.backend.stop_host()?;
        // Removing an IBus component can leave no global engine. Repair only that
        // missing state; a valid engine selected manually must always win.
        if self.refresh().is_err() {
            self.switch_and_verify(&previous)?;
        }
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

#[derive(Default)]
pub(super) struct IBusBackend {
    host: service::HostService,
}

impl InputMethodBackend for IBusBackend {
    fn current_engine(&mut self) -> Result<String, String> {
        run_ibus(&[]).and_then(validate_engine)
    }

    fn ensure_suzaku_available(&mut self) -> Result<(), String> {
        self.host.ensure_ready()
    }

    fn switch_engine(&mut self, engine: &str) -> Result<(), String> {
        validate_engine(engine.to_string())?;
        run_ibus(&[engine]).map(|_| ())
    }

    fn host_needs_stop(&mut self) -> Result<bool, String> {
        self.host.needs_stop()
    }

    fn stop_host(&mut self) -> Result<(), String> {
        self.host.stop()
    }
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
    suzaku_map::platform::linux_command::run(command, timeout).map_err(|error| match error.kind() {
        std::io::ErrorKind::TimedOut => "输入法切换超时，请重试；原输入法已保留".into(),
        std::io::ErrorKind::NotFound => "无法启动 ibus 命令，请确认 IBus 已安装".into(),
        std::io::ErrorKind::InvalidData => "IBus 返回数据过大，已停止请求".into(),
        _ => "无法读取 IBus 状态，请重试".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    struct FakeBackend {
        current: String,
        switches: Vec<String>,
        available: bool,
        read_fails: bool,
        switch_fails: bool,
        change_on_failure: bool,
        ignore_switch: bool,
        managed: bool,
        stop_fails: bool,
        engine_after_stop: Option<String>,
        lifecycle: Vec<String>,
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
                managed: false,
                stop_fails: false,
                engine_after_stop: None,
                lifecycle: Vec::new(),
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
            self.lifecycle.push("ready".into());
            if self.available {
                Ok(())
            } else {
                Err("host unavailable".into())
            }
        }

        fn switch_engine(&mut self, engine: &str) -> Result<(), String> {
            self.lifecycle.push(format!("switch:{engine}"));
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

        fn host_needs_stop(&mut self) -> Result<bool, String> {
            Ok(self.managed)
        }

        fn stop_host(&mut self) -> Result<(), String> {
            self.lifecycle.push("stop".into());
            if self.stop_fails {
                return Err("stop failed".into());
            }
            self.managed = false;
            if let Some(current) = self.engine_after_stop.take() {
                self.current = current;
            }
            Ok(())
        }
    }

    #[test]
    fn startup_keeps_current_engine_and_quit_restores_before_stopping() {
        let mut control = InputMethodController::new(FakeBackend {
            managed: true,
            ..Default::default()
        });
        control.start().unwrap();
        assert_eq!(control.backend.current, "rime");
        assert_eq!(control.state.restore_engine, None);
        control.activate().unwrap();
        control.shutdown().unwrap();
        control.shutdown().unwrap();
        assert_eq!(
            control.backend.lifecycle,
            [
                "ready",
                "ready",
                "switch:dev.suzaku.linux.ime",
                "switch:rime",
                "stop"
            ]
        );
    }

    #[test]
    fn failed_restore_keeps_service_and_failed_stop_is_retryable() {
        let mut control = InputMethodController::new(FakeBackend {
            managed: true,
            ..Default::default()
        });
        control.activate().unwrap();
        control.backend.switch_fails = true;
        assert!(control.shutdown().is_err());
        assert!(control.backend.managed);
        assert!(!control.backend.lifecycle.contains(&"stop".into()));
        control.backend.switch_fails = false;
        control.backend.stop_fails = true;
        assert!(control.shutdown().is_err());
        assert_eq!(control.backend.current, "rime");
        assert!(control.backend.managed);
        control.backend.stop_fails = false;
        control.shutdown().unwrap();
        assert!(!control.backend.managed);
    }

    #[test]
    fn system_activated_session_uses_observed_fallback_or_refuses_unsafe_stop() {
        let mut control = InputMethodController::new(FakeBackend {
            managed: true,
            ..Default::default()
        });
        control.start().unwrap();
        control.backend.current = SUZAKU_ENGINE.into();
        control.shutdown().unwrap();
        assert_eq!(control.backend.current, "rime");
        let mut unknown = InputMethodController::new(FakeBackend {
            current: SUZAKU_ENGINE.into(),
            managed: true,
            ..Default::default()
        });
        unknown.start().unwrap();
        assert!(unknown.shutdown().unwrap_err().contains("系统菜单"));
        assert!(unknown.backend.managed);
        assert!(unknown.backend.switches.is_empty());
    }

    #[test]
    fn shutdown_preserves_manual_switches_and_repairs_missing_global_engine() {
        for after in [None, Some(""), Some("mozc-jp")] {
            let mut control = InputMethodController::new(FakeBackend {
                managed: true,
                engine_after_stop: after.map(str::to_string),
                ..Default::default()
            });
            control.activate().unwrap();
            control.backend.current = "xkb:us::eng".into();
            control.shutdown().unwrap();
            assert_eq!(
                control.backend.current,
                if after == Some("mozc-jp") {
                    "mozc-jp"
                } else {
                    "xkb:us::eng"
                }
            );
            assert!(!control.backend.switches.contains(&"rime".into()));
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
    fn tray_command_deadline_covers_inherited_stdout() {
        let started = Instant::now();
        // One finite child and no real ibus; never invoke the test executable.
        let result = run_bounded(
            Command::new("/bin/sh").args(["-c", "/bin/sleep 2 & printf rime; exit 0"]),
            Duration::from_millis(45),
        );
        assert!(result.unwrap_err().contains("超时"));
        assert!(started.elapsed() < Duration::from_millis(400));
    }

    #[test]
    fn tray_command_drains_bounded_output_and_can_reuse_the_command() {
        let mut command = Command::new("/usr/bin/head");
        command.args(["-c", "65536", "/dev/zero"]);
        for _ in 0..2 {
            assert_eq!(
                run_bounded(&mut command, COMMAND_TIMEOUT)
                    .unwrap()
                    .stdout
                    .len(),
                65536
            );
        }
        assert!(
            run_bounded(
                Command::new("/usr/bin/head").args(["-c", "131072", "/dev/zero"]),
                COMMAND_TIMEOUT
            )
            .unwrap_err()
            .contains("过大")
        );
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
    #[ignore = "requires the private D-Bus/IBus fixture in scripts/test-linux-ci.sh ibus"]
    fn native_activation_input_and_release_roundtrip() {
        // Check all isolation markers before invoking any backend operation.
        assert_eq!(
            std::env::var("SUZAKU_NATIVE_SYNC_QA").as_deref(),
            Ok("1"),
            "private IBus fixture required"
        );
        let runtime = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
        assert_eq!(runtime.parent(), Some(std::path::Path::new("/tmp")));
        assert!(
            runtime
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("suzaku-sync-qa.")
        );
        assert!(!runtime.is_symlink());
        assert!(std::env::var_os("DISPLAY").is_none());
        assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
        assert_eq!(
            std::env::var("IBUS_ADDRESS").unwrap(),
            format!("unix:path={}/ibus.sock", runtime.display())
        );
        for (name, path) in [
            ("SUZAKU_IME_CONFIG", runtime.join("ime.json")),
            (
                "SUZAKU_LINUX_IME_SOCKET",
                runtime.join("suzaku-ime/host.sock"),
            ),
            ("XDG_CONFIG_HOME", runtime.join("config")),
            ("XDG_DATA_HOME", runtime.join("data")),
        ] {
            assert_eq!(
                std::env::var_os(name).map(std::path::PathBuf::from),
                Some(path)
            );
        }
        use suzaku_map::{languages::BuiltinLanguage, platform::linux_ime_control};
        let original = linux_ime_control::status().unwrap().settings;
        assert!(
            !original.llm_enabled,
            "activation QA must not request a model"
        );
        struct RestoreOnDrop(InputMethodController<IBusBackend>);
        impl Drop for RestoreOnDrop {
            fn drop(&mut self) {
                if let Err(error) = self.0.release() {
                    eprintln!("Native activation probe could not restore input: {error}");
                }
            }
        }
        let previous = IBusBackend::default()
            .current_engine()
            .expect("the fixture's restorable IBus engine");
        assert_ne!(
            previous, SUZAKU_ENGINE,
            "the fixture must select its initial engine before running this probe"
        );
        {
            let mut guard = RestoreOnDrop(InputMethodController::new(IBusBackend::default()));
            let control = &mut guard.0;
            control.start().unwrap();
            assert_eq!(
                control.state.current_engine.as_deref(),
                Some(previous.as_str())
            );
            for (language, seed, expected) in [
                (BuiltinLanguage::English, "hel", "hel"),
                (BuiltinLanguage::ChineseSimplified, "nihao", "你好"),
                (BuiltinLanguage::Japanese, "nihongo", "日本語"),
            ] {
                linux_ime_control::set_language(language).unwrap();
                control.activate().unwrap();
                control.activate().unwrap();
                assert_eq!(
                    control.state.restore_engine.as_deref(),
                    Some(previous.as_str())
                );
                let report =
                    suzaku_map::platform::linux_ibus_host::probe_roundtrip_report(seed).unwrap();
                assert_eq!(report.preedit, seed);
                assert_eq!(report.page_size, 6);
                assert!(report.candidate_count > 0);
                assert_eq!(report.committed, expected);
                control.release().unwrap();
                control.release().unwrap();
                assert_eq!(IBusBackend::default().current_engine().unwrap(), previous);
                assert!(control.state.restore_engine.is_none());
            }
            // This fixture's custom/private endpoint must not be stopped through
            // the real desktop's service manager during full panel shutdown.
            control.shutdown().unwrap();
            IBusBackend::default().ensure_suzaku_available().unwrap();
            // Exercise the same release-on-cleanup path used after a failed assertion.
            control.activate().unwrap();
        }
        assert_eq!(IBusBackend::default().current_engine().unwrap(), previous);
        linux_ime_control::set_language(original.language).unwrap();
        println!(
            "PASS: private IBus activation, six-row English/Chinese/Japanese input, repeated release and cleanup restore"
        );
    }
}
