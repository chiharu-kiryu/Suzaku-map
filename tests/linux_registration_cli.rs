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
    fn untouched(&self) {
        assert!(!self.host().exists());
        assert!(!self.unit().exists());
        assert!(!self.marker().exists());
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
