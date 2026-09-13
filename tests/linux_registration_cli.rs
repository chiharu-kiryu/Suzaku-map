#![cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzaku-register-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let home = root.join("home 朱雀 'quoted' \"double\" \\path $USER %test");
        fs::create_dir(&home).unwrap();
        fs::create_dir(root.join("commands")).unwrap();
        for name in ["systemctl", "ibus", "gsettings", "pgrep", "gdbus"] {
            let path = root.join("commands").join(name);
            fs::write(
                &path,
                include_bytes!("../scripts/fixtures/linux-registration-command.py"),
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let fixture = Self { root, home };
        fs::write(fixture.source(), "synthetic host v1").unwrap();
        fs::set_permissions(fixture.source(), fs::Permissions::from_mode(0o755)).unwrap();
        fixture.write_desktop(serde_json::json!({
            "engine":"xkb:us::eng", "sources":"[('xkb', 'us'), ('ibus', 'rime')]",
            "active":false, "enabled":false, "calls":[]
        }));
        fixture
    }
    fn source(&self) -> PathBuf {
        self.root.join("source-host")
    }
    fn host(&self) -> PathBuf {
        self.home.join(".local/libexec/suzaku/linux_ime_host")
    }
    fn marker(&self) -> PathBuf {
        self.root
            .join("data/ibus/component/dev.suzaku.linux.ime.xml")
    }
    fn unit(&self) -> PathBuf {
        self.root.join("config/systemd/user/suzaku-ibus.service")
    }
    fn desktop(&self) -> serde_json::Value {
        serde_json::from_slice(&fs::read(self.root.join("desktop.json")).unwrap()).unwrap()
    }
    fn write_desktop(&self, state: serde_json::Value) {
        fs::write(self.root.join("desktop.json"), state.to_string()).unwrap();
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
        command
            .current_dir(&self.root)
            // Only this child gets a disposable home and fake desktop commands.
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_RUNTIME_DIR", self.root.join("runtime"))
            .env("PATH", self.root.join("commands"))
            .env("SUZAKU_LINUX_IME_FRAMEWORK", "ibus")
            .env("SUZAKU_LINUX_IME_HOST_BIN", self.source())
            .env("SUZAKU_REGISTRATION_QA", &self.root)
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("IBUS_ADDRESS")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY");
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn run_bounded(&self, args: &[&str]) -> Output {
        use std::os::unix::process::CommandExt;
        use std::time::{Duration, Instant};
        let stdout = self.root.join("stdout.log");
        let stderr = self.root.join("stderr.log");
        let child = self
            .command()
            .args(args)
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .process_group(0)
            .spawn()
            .unwrap();
        let mut guard = PrivateCommand {
            child,
            reaped: false,
        };
        let start = Instant::now();
        let status = loop {
            if let Some(status) = guard.child.try_wait().unwrap() {
                guard.reaped = true;
                break status;
            }
            assert!(
                start.elapsed() < Duration::from_secs(4),
                "CLI exceeded the independent test deadline: {args:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        }
    }
    fn untouched(&self) {
        assert!(!self.host().exists());
        assert!(!self.unit().exists());
        assert!(!self.marker().exists());
    }
}
struct PrivateCommand {
    child: std::process::Child,
    reaped: bool,
}
impl Drop for PrivateCommand {
    fn drop(&mut self) {
        if !self.reaped {
            let group = i32::try_from(self.child.id()).unwrap();
            // SAFETY: the unreaped owned child reserves this private group ID.
            unsafe { libc::kill(-group, libc::SIGKILL) };
            let _ = self.child.wait();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn invalid_registration_arguments_and_help_never_modify_the_desktop() {
    let f = Fixture::new();
    for args in [
        vec!["linux-register", "install", "--unexpected"],
        vec!["linux-register", "uninstall", "--dry-run"],
        vec!["linux-register", "diag", "extra"],
    ] {
        assert!(!f.run(&args).status.success(), "{args:?}");
        f.untouched();
        assert!(f.desktop()["calls"].as_array().unwrap().is_empty());
    }
    for args in [vec!["linux-register"], vec!["linux-register", "--help"]] {
        success(f.run(&args));
        f.untouched();
        assert!(f.desktop()["calls"].as_array().unwrap().is_empty());
    }
}

#[test]
fn install_upgrade_verify_and_uninstall_preserve_user_data_and_input_sources() {
    let f = Fixture::new();
    let config = f.root.join("config/suzaku-ime/settings.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, r#"{"language":"ja","llm_enabled":false}"#).unwrap();
    let original = fs::read(&config).unwrap();
    success(f.run(&["linux-register", "install"]));
    assert_eq!(fs::read_to_string(f.host()).unwrap(), "synthetic host v1");
    assert_eq!(f.desktop()["engine"], "xkb:us::eng");
    assert_eq!(f.desktop()["enabled"], true);
    assert!(success(f.run(&["linux-register", "verify"])).contains("Verdict: PASS"));
    // Existing disabled autostart is a user preference, not an upgrade default.
    let mut desktop = f.desktop();
    desktop["enabled"] = false.into();
    f.write_desktop(desktop);
    fs::write(f.source(), "synthetic host v2").unwrap();
    success(f.run(&["linux-register", "install"]));
    assert_eq!(fs::read_to_string(f.host()).unwrap(), "synthetic host v2");
    assert_eq!(f.desktop()["enabled"], false);
    assert_eq!(f.desktop()["engine"], "xkb:us::eng");
    success(f.run(&["linux-register", "uninstall"]));
    f.untouched();
    assert_eq!(fs::read(&config).unwrap(), original);
    assert_eq!(f.desktop()["sources"], "[('xkb', 'us'), ('ibus', 'rime')]");
    assert_eq!(f.desktop()["engine"], "xkb:us::eng");
}

#[test]
fn component_and_service_quote_special_install_paths() {
    let f = Fixture::new();
    success(f.run(&["linux-register", "install"]));
    let output = Command::new("/usr/bin/python3")
        .args(["-c", "import sys,xml.etree.ElementTree as ET; from gi.repository import GLib; assert GLib.shell_parse_argv(ET.parse(sys.argv[1]).findtext('exec'))[1] == [sys.argv[2], '--ibus']"])
        .arg(f.marker()).arg(f.host()).output().unwrap();
    success(output);
    assert!(
        fs::read_to_string(f.unit())
            .unwrap()
            .contains("$$USER %%test")
    );
    assert!(
        fs::read_to_string(f.unit())
            .unwrap()
            .contains("ExecStart=/usr/bin/env -- \"")
    );
    success(
        Command::new("/usr/bin/systemd-analyze")
            .env("HOME", &f.home)
            .env("XDG_CONFIG_HOME", f.root.join("config"))
            .env("XDG_DATA_HOME", f.root.join("data"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .args(["--user", "--man=no", "--generators=no", "verify"])
            .arg(f.unit())
            .output()
            .unwrap(),
    );
    assert!(success(f.run(&["linux-register", "verify"])).contains("Verdict: PASS"));
}

#[test]
fn missing_host_or_service_manager_is_rejected_before_writing_registration() {
    let f = Fixture::new();
    assert!(
        !f.command()
            .env("SUZAKU_LINUX_IME_HOST_BIN", f.root.join("missing"))
            .args(["linux-register", "install"])
            .output()
            .unwrap()
            .status
            .success()
    );
    f.untouched();
    fs::remove_file(f.root.join("commands/systemctl")).unwrap();
    assert!(!f.run(&["linux-register", "install"]).status.success());
    f.untouched();
}

#[test]
fn a_missing_user_session_fails_before_installing_any_files() {
    let f = Fixture::new();
    let mut desktop = f.desktop();
    desktop["manager_unavailable"] = true.into();
    f.write_desktop(desktop);
    let output = f.run(&["linux-register", "install"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No systemd user manager"));
    f.untouched();
    assert_eq!(f.desktop()["sources"], "[('xkb', 'us'), ('ibus', 'rime')]");
    assert_eq!(f.desktop()["engine"], "xkb:us::eng");
}

#[test]
fn active_input_method_must_be_released_before_upgrade_or_uninstall() {
    for action in ["install", "uninstall"] {
        let f = Fixture::new();
        success(f.run(&["linux-register", "install"]));
        let original_unit = fs::read(f.unit()).unwrap();
        let mut desktop = f.desktop();
        desktop["engine"] = "dev.suzaku.linux.ime".into();
        desktop["calls"] = serde_json::json!([]);
        f.write_desktop(desktop.clone());
        assert!(
            !f.run(&["linux-register", action]).status.success(),
            "{action}"
        );
        assert_eq!(fs::read(f.unit()).unwrap(), original_unit);
        assert!(f.host().exists() && f.marker().exists());
        assert_eq!(f.desktop()["sources"], desktop["sources"]);
        assert_eq!(f.desktop()["engine"], desktop["engine"]);
    }
}

#[test]
fn empty_or_relative_home_never_becomes_an_install_prefix() {
    let f = Fixture::new();
    for home in ["", ".", "relative"] {
        assert!(
            !f.command()
                .env("HOME", home)
                .args(["linux-register", "install"])
                .output()
                .unwrap()
                .status
                .success()
        );
        f.untouched();
        assert!(!f.root.join(".local").exists());
    }
}

#[test]
fn upgrading_a_masked_user_service_never_unmasks_or_replaces_it() {
    let f = Fixture::new();
    success(f.run(&["linux-register", "install"]));
    let original_marker = fs::read(f.marker()).unwrap();
    fs::remove_file(f.unit()).unwrap();
    std::os::unix::fs::symlink("/dev/null", f.unit()).unwrap();
    fs::write(f.source(), "new host must not be installed").unwrap();
    let before = f.desktop();
    assert!(!f.run(&["linux-register", "install"]).status.success());
    assert_eq!(fs::read_link(f.unit()).unwrap(), PathBuf::from("/dev/null"));
    assert_eq!(fs::read_to_string(f.host()).unwrap(), "synthetic host v1");
    assert_eq!(fs::read(f.marker()).unwrap(), original_marker);
    assert_eq!(f.desktop(), before);
}

#[test]
fn failed_first_install_retry_preserves_initial_autostart_intent() {
    let mut failures = Vec::new();
    for (stage, partial) in [
        ("daemon-reload", false),
        ("enable", false),
        ("enable", true),
    ] {
        let f = Fixture::new();
        let mut state = f.desktop();
        let failure_key = if partial {
            "fail_after_effect"
        } else {
            "fail_once"
        };
        state[failure_key] = if stage == "enable" {
            serde_json::json!(["systemctl", "--user", stage, "suzaku-ibus.service"])
        } else {
            serde_json::json!(["systemctl", "--user", stage])
        };
        f.write_desktop(state);
        assert!(!f.run(&["linux-register", "install"]).status.success());
        assert_eq!(f.desktop()["enabled"], partial);
        assert_eq!(f.desktop()["engine"], "xkb:us::eng");
        let residual_unit = f.unit().exists();
        assert!(
            fs::read_to_string(f.unit())
                .unwrap()
                .contains("initial autostart pending")
        );
        success(f.run(&["linux-register", "install"]));
        assert!(
            !fs::read_to_string(f.unit())
                .unwrap()
                .contains("initial autostart pending")
        );
        let after = f.desktop();
        assert_eq!(after["active"], true);
        assert_eq!(after["engine"], "xkb:us::eng");
        eprintln!(
            "N11 stage={stage}: failed_install_left_unit={residual_unit}, retry_success=true, active={}, enabled={}",
            after["active"], after["enabled"]
        );
        if after["enabled"] != true {
            failures.push(stage);
        }
    }
    assert!(
        failures.is_empty(),
        "N11: successful first-install retries did not enable autostart: {failures:?}"
    );
}

#[test]
fn restart_failure_after_enabling_does_not_erase_a_later_disabled_preference() {
    let f = Fixture::new();
    let mut state = f.desktop();
    state["fail_once"] =
        serde_json::json!(["systemctl", "--user", "restart", "suzaku-ibus.service"]);
    f.write_desktop(state);
    assert!(!f.run(&["linux-register", "install"]).status.success());
    assert_eq!(f.desktop()["enabled"], true);
    assert!(
        !fs::read_to_string(f.unit())
            .unwrap()
            .contains("initial autostart pending")
    );
    let mut state = f.desktop();
    state["enabled"] = false.into();
    state["calls"] = serde_json::json!([]);
    f.write_desktop(state);
    success(f.run(&["linux-register", "install"]));
    assert_eq!(f.desktop()["enabled"], false);
    assert!(
        !f.desktop()["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call[0] == "systemctl" && call[2] == "enable")
    );
}

#[test]
fn registered_and_active_diagnostics_are_read_only_and_match_verify() {
    for engine in ["xkb:us::eng", "dev.suzaku.linux.ime"] {
        let f = Fixture::new();
        success(f.run(&["linux-register", "install"]));
        let originals = [f.host(), f.unit(), f.marker()].map(|path| fs::read(path).unwrap());
        let mut state = f.desktop();
        state["enabled"] = false.into();
        state["engine"] = engine.into();
        f.write_desktop(state);
        fs::write(f.source(), "must not install this diagnostic probe").unwrap();
        for entry in ["linux-register", "linux-register-ime"] {
            let verify = f.run(&[entry, "verify"]);
            let mut before = f.desktop();
            before["calls"] = serde_json::json!([]);
            f.write_desktop(before.clone());
            let diag = f.run(&[entry, "diag"]);
            assert_eq!(diag.status.code(), verify.status.code());
            assert_eq!(diag.stdout, verify.stdout);
            let mut after = f.desktop();
            assert!(!after["calls"].as_array().unwrap().iter().any(|call| {
                (call[0] == "systemctl" && call[2] != "is-active")
                    || call[0] == "gsettings"
                    || (call[0] == "ibus"
                        && call[1] == "engine"
                        && call.as_array().unwrap().len() > 2)
            }));
            after["calls"] = serde_json::json!([]);
            assert_eq!(after, before);
            assert_eq!(
                [f.host(), f.unit(), f.marker()].map(|path| fs::read(path).unwrap()),
                originals
            );
        }
    }
}

#[test]
fn dynamic_registry_fallback_queries_are_bounded() {
    for blocked in [
        serde_json::json!(["ibus", "address"]),
        serde_json::json!([
            "gdbus",
            "call",
            "--address",
            "unix:path=/synthetic/no-desktop-access",
            "--dest",
            "org.freedesktop.IBus",
            "--object-path",
            "/org/freedesktop/IBus",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.IBus",
            "ActiveEngines"
        ]),
    ] {
        let f = Fixture::new();
        let mut state = f.desktop();
        state["listed_engine"] = "".into();
        state["block_command"] = blocked;
        f.write_desktop(state);
        let result = f.run_bounded(&["linux-register", "verify"]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("deadline exceeded"));
        let pid = f.desktop()["blocked_pid"].as_u64().unwrap();
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        f.untouched();
    }
}

#[test]
fn engine_restoration_has_one_deadline_for_queries_switches_and_retries() {
    for scenario in ["query", "switch", "slow-unconfirmed"] {
        let f = Fixture::new();
        let mut state = f.desktop();
        match scenario {
            "query" => {
                state["block_command"] = serde_json::json!(["ibus", "engine"]);
                state["block_after_restart"] = true.into();
            }
            "switch" => {
                state["block_command"] = serde_json::json!(["ibus", "engine", "xkb:us::eng"])
            }
            _ => {
                state["ignore_engine_switch"] = true.into();
                state["slow_restore"] = true.into();
            }
        }
        f.write_desktop(state);
        let result = f.run_bounded(&["linux-register", "install"]);
        assert!(!result.status.success(), "{scenario}");
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("previous engine could not be restored"),
            "{scenario}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let after = f.desktop();
        assert_eq!(after["sources"], "[('xkb', 'us'), ('ibus', 'rime')]");
        if let Some(pid) = after["blocked_pid"].as_u64() {
            assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        }
    }
}

#[test]
fn an_unknown_previous_engine_is_rejected_before_registration_changes() {
    for engine in ["", "   ", "dummy"] {
        let f = Fixture::new();
        let mut state = f.desktop();
        state["engine"] = engine.into();
        f.write_desktop(state);
        for action in ["install", "uninstall"] {
            let result = f.run(&["linux-register", action]);
            assert!(!result.status.success());
            assert!(String::from_utf8_lossy(&result.stderr).contains("safe current IBus engine"));
            f.untouched();
        }
    }
}

#[test]
fn diag_does_not_change_registration() {
    let f = Fixture::new();
    let before = f.desktop();
    let result = f.run(&["linux-register", "diag"]);
    let after = f.desktop();
    let mutating_calls: Vec<_> = after["calls"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|call| {
            matches!(
                (call[0].as_str(), call[1].as_str(), call[2].as_str()),
                (
                    Some("systemctl"),
                    Some("--user"),
                    Some("daemon-reload" | "enable" | "restart" | "disable")
                ) | (Some("gsettings"), Some("set"), _)
            ) || (call[0] == "ibus" && call[1] == "engine" && call.as_array().unwrap().len() == 3)
        })
        .collect();
    eprintln!(
        "N12: diag_success={}, host_created={}, unit_created={}, marker_created={}, enabled={}, mutating_calls={mutating_calls:?}",
        result.status.success(),
        f.host().exists(),
        f.unit().exists(),
        f.marker().exists(),
        after["enabled"]
    );
    assert!(
        !f.host().exists()
            && !f.unit().exists()
            && !f.marker().exists()
            && after["sources"] == before["sources"]
            && after["enabled"] == before["enabled"]
            && mutating_calls.is_empty(),
        "N12: diagnostic entry modified registration or the desktop"
    );
}

#[test]
fn unresponsive_desktop_commands_are_bounded() {
    let get_sources = serde_json::json!([
        "gsettings",
        "get",
        "org.gnome.desktop.input-sources",
        "sources"
    ]);
    let cases = [
        ("status", serde_json::json!(["ibus", "list-engine"])),
        ("verify", serde_json::json!(["ibus", "list-engine"])),
        ("install", get_sources.clone()),
        ("uninstall", get_sources),
        ("status", serde_json::json!(["ibus", "engine"])),
        (
            "status",
            serde_json::json!(["pgrep", "-x", "linux_ime_host"]),
        ),
        (
            "verify",
            serde_json::json!([
                "systemctl",
                "--user",
                "is-active",
                "--quiet",
                "suzaku-ibus.service"
            ]),
        ),
        (
            "install",
            serde_json::json!([
                "gsettings",
                "set",
                "org.gnome.desktop.input-sources",
                "sources",
                "[('xkb', 'us'), ('ibus', 'rime'), ('ibus', 'dev.suzaku.linux.ime')]"
            ]),
        ),
        (
            "uninstall",
            serde_json::json!([
                "gsettings",
                "set",
                "org.gnome.desktop.input-sources",
                "sources",
                "[('xkb', 'us'), ('ibus', 'rime')]"
            ]),
        ),
        (
            "install",
            serde_json::json!([
                "systemctl",
                "--user",
                "show",
                "--property=Version",
                "--value"
            ]),
        ),
    ];
    for (action, blocked) in cases {
        let f = Fixture::new();
        if action == "uninstall" {
            success(f.run(&["linux-register", "install"]));
        }
        let mut state = f.desktop();
        state["block_command"] = blocked.clone();
        f.write_desktop(state);
        let result = f.run_bounded(&["linux-register", action]);
        let after = f.desktop();
        let blocked_call_reached = after["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call == &after["block_command"]);
        eprintln!(
            "N13 {action} {blocked}: command_reached={blocked_call_reached}, bounded_exit={}",
            result.status
        );
        assert!(blocked_call_reached);
        assert!(
            !result.status.success(),
            "a failed query must not report success"
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("deadline exceeded"));
        let pid = after["blocked_pid"].as_u64().unwrap();
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "timed-out child was not reaped"
        );
    }
}
