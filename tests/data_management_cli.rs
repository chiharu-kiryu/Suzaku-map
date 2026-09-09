#![cfg(target_os = "linux")]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
use suzaku_map::data::{files::DataLease, paths::DataPaths};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzaku-data-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn paths(&self) -> DataPaths {
        DataPaths {
            ime: self.0.join("config/suzaku-ime/settings.json"),
            panel: self.0.join("config/suzaku-panel/panel-settings.toml"),
            backups: self.0.join("data/suzaku/backups"),
        }
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
        cmd.current_dir(&self.0)
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env("XDG_RUNTIME_DIR", self.0.join("runtime"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY");
        cmd
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn cli_status_backup_preview_apply_and_undo_do_not_touch_unrelated_files() {
    let f = Fixture::new();
    assert!(success(f.run(&["data", "status"])).contains("使用默认值"));
    assert!(!f.paths().ime.exists());
    success(f.run(&["data", "backup", "defaults.json"]));
    success(f.run(&["data", "validate", "defaults.json"]));
    assert!(!f.run(&["data", "backup", "defaults.json"]).status.success());
    fs::write(&f.paths().ime, r#"{"language":"ja"}"#).unwrap();
    fs::write(f.0.join("unrelated.txt"), "keep").unwrap();
    assert!(success(f.run(&["data", "restore", "defaults.json"])).contains("仅预览"));
    assert!(f.paths().ime.exists());
    success(f.run(&["data", "restore", "defaults.json", "--apply"]));
    assert!(!f.paths().ime.exists());
    let backup = fs::read_dir(&f.paths().backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    success(f.run(&["data", "restore", backup.to_str().unwrap(), "--apply"]));
    let restored: serde_json::Value =
        serde_json::from_slice(&fs::read(f.paths().ime).unwrap()).unwrap();
    assert_eq!(restored["language"], "ja");
    assert_eq!(
        fs::read_to_string(f.0.join("unrelated.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn running_process_lock_and_legacy_endpoints_reject_cli_restore() {
    let f = Fixture::new();
    success(f.run(&["data", "backup", "settings.json"]));
    let lease = DataLease::acquire(&f.paths().lock_path().unwrap(), false).unwrap();
    assert!(
        !f.run(&["data", "restore", "settings.json", "--apply"])
            .status
            .success()
    );
    drop(lease);
    let endpoint = f.0.join("runtime/suzaku-ime/host.sock");
    fs::create_dir_all(endpoint.parent().unwrap()).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&endpoint).unwrap();
    assert!(
        !f.run(&["data", "restore", "settings.json", "--apply"])
            .status
            .success()
    );
    assert!(endpoint.exists());
}

#[test]
fn empty_and_relative_xdg_variables_fall_back_inside_the_isolated_home() {
    let f = Fixture::new();
    for invalid in ["", "relative"] {
        let output = f
            .command()
            .env("XDG_CONFIG_HOME", invalid)
            .env("XDG_DATA_HOME", invalid)
            .args(["data", "status"])
            .output()
            .unwrap();
        let status = success(output);
        assert!(
            status.contains(
                f.0.join(".config/suzaku-ime/settings.json")
                    .to_str()
                    .unwrap()
            )
        );
        assert!(status.contains(f.0.join(".local/share/suzaku/backups").to_str().unwrap()));
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn invalid_arguments_and_corrupt_import_never_apply() {
    let f = Fixture::new();
    fs::write(
        f.0.join("bad.json"),
        r#"{"files":{"../../settings.json":"bad"}}"#,
    )
    .unwrap();
    for args in [
        vec!["data", "restore", "bad.json", "--apply"],
        vec!["data", "validate", "bad.json"],
        vec!["data", "restore", "bad.json", "--force"],
        vec!["data", "status", "unexpected"],
        vec!["data", "open", "../unrelated"],
    ] {
        assert!(!f.run(&args).status.success(), "{args:?}");
    }
    assert!(!f.paths().ime.exists());
    assert!(!f.paths().backups.exists());
    assert!(success(f.run(&["--version"])).starts_with("suzaku-map "));
}

#[test]
fn an_explicit_ime_config_can_be_saved_without_home_or_data_directories() {
    let f = Fixture::new();
    let config = f.0.join("standalone.json");
    let output = f
        .command()
        .env_remove("HOME")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .env("SUZAKU_IME_CONFIG", &config)
        .args(["llama", "configure", "--model", "llama3.2:1b"])
        .output()
        .unwrap();
    success(output);
    let settings: serde_json::Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    assert_eq!(settings["llm_model"], "llama3.2:1b");
}
