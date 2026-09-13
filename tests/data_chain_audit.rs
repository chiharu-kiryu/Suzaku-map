//! F45-F49 regressions. Private files, D-Bus and IBus only; no desktop activation.
#![cfg(target_os = "linux")]

use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::Shutdown,
    os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use suzaku_map::{
    data::{backup::Backup, paths::DataPaths},
    ime::settings::ImeSettings,
};

struct Process(Child);

impl Process {
    fn start(command: &mut Command) -> Self {
        Self(
            command
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        assert_eq!(std::env::var("SUZAKU_DATA_CHAIN_AUDIT").as_deref(), Ok("1"));
        assert!(std::env::var_os("DISPLAY").is_none());
        assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
        assert!(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some());
        let base = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
        assert!(base.starts_with("/tmp"));
        assert!(
            base.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("suzaku-data-audit.")
        );
        let root = base.join(label);
        fs::create_dir(&root).unwrap();
        let fixture = Self { root };
        for directory in [
            "config/suzaku-ime",
            "host-runtime",
            "cli-runtime",
            "overrides",
        ] {
            fs::create_dir_all(fixture.root.join(directory)).unwrap();
        }
        fs::write(
            fixture.ime(),
            r#"{"language":"en","llm_model":"synthetic-before","llm_enabled":false}"#,
        )
        .unwrap();
        let desired = ImeSettings::from_json(
            r#"{"language":"ja","llm_model":"synthetic-restored","llm_enabled":false}"#,
        )
        .unwrap();
        Backup {
            ime: Some(desired.to_json()),
            panel: None,
        }
        .write_new(&fixture.root.join("restore.json"))
        .unwrap();
        fixture
    }

    fn ime(&self) -> PathBuf {
        self.root.join("config/suzaku-ime/settings.json")
    }

    fn paths(&self) -> DataPaths {
        DataPaths {
            ime: self.ime(),
            panel: self.root.join("config/suzaku-panel/panel-settings.toml"),
            backups: self.root.join("data/suzaku/backups"),
        }
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>, runtime: &str) -> Command {
        let mut command = Command::new(executable);
        command
            .current_dir(&self.root)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("XDG_RUNTIME_DIR", self.root.join(runtime))
            .env("GSETTINGS_BACKEND", "memory")
            .env("GIO_USE_VFS", "local")
            .env(
                "IBUS_ADDRESS",
                format!(
                    "unix:path={}",
                    self.root.join("private-ibus.sock").display()
                ),
            )
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("SUZAKU_LINUX_IME_ACTIVE");
        command
    }

    fn tool(&self, runtime: &str, arguments: &[&str]) -> Output {
        self.tool_at(runtime, arguments, None)
    }

    fn tool_at(&self, runtime: &str, arguments: &[&str], config: Option<&Path>) -> Output {
        let mut command = self.command(env!("CARGO_BIN_EXE_suzaku_tool"), runtime);
        if let Some(config) = config {
            command.env("SUZAKU_IME_CONFIG", config);
        }
        let mut child = command
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("bounded data CLI timed out: {output:?}");
            }
            thread::sleep(Duration::from_millis(5));
        }
        child.wait_with_output().unwrap()
    }

    fn start_host(&self, config: &Path) -> (Process, Process) {
        let daemon = Process::start(
            self.command("ibus-daemon", "host-runtime")
                .args(["--single", "--cache=none"])
                .arg(format!(
                    "--address=unix:path={}",
                    self.root.join("private-ibus.sock").display()
                )),
        );
        wait_for(
            || self.root.join("private-ibus.sock").exists(),
            "private IBus address",
        );
        let mut host = Process::start(
            self.command(env!("CARGO_BIN_EXE_linux_ime_host"), "host-runtime")
                .env("SUZAKU_IME_CONFIG", config),
        );
        wait_for(
            || {
                assert!(
                    host.0.try_wait().unwrap().is_none(),
                    "private host exited before readiness"
                );
                self.root.join("host-runtime/suzaku-ime/host.sock").exists()
            },
            "private host endpoint",
        );
        assert_eq!(self.status()["settings"]["llm_model"], "synthetic-before");
        assert_eq!(self.status()["settings"]["llm_enabled"], false);
        (daemon, host)
    }

    fn status(&self) -> Value {
        self.control("S")
    }

    fn control(&self, command: &str) -> Value {
        let mut stream =
            UnixStream::connect(self.root.join("host-runtime/suzaku-ime/host.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(command.as_bytes()).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut reply = String::new();
        stream.take(65_536).read_to_string(&mut reply).unwrap();
        serde_json::from_str(&reply).unwrap()
    }
}

fn wait_for(mut ready: impl FnMut() -> bool, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "private D-Bus/IBus fixture; run by test-data-chain-audit.sh in Linux CI"]
fn audit_restore_must_respect_a_live_host_using_a_settings_file_alias() {
    for label in [
        "direct",
        "parent-alias",
        "file-alias",
        "relative-alias",
        "chained-alias",
    ] {
        let fixture = Fixture::new(label);
        let host_config = match label {
            "direct" => fixture.ime(),
            "parent-alias" => {
                let directory = fixture.root.join("linked-config");
                symlink(fixture.ime().parent().unwrap(), &directory).unwrap();
                directory.join("settings.json")
            }
            "relative-alias" => {
                symlink(
                    "../config/suzaku-ime/settings.json",
                    fixture.root.join("overrides/ime.json"),
                )
                .unwrap();
                PathBuf::from("overrides/ime.json")
            }
            "chained-alias" => {
                fs::create_dir(fixture.root.join("middle")).unwrap();
                symlink(
                    "../config/suzaku-ime/settings.json",
                    fixture.root.join("middle/ime.json"),
                )
                .unwrap();
                let alias = fixture.root.join("overrides/ime.json");
                symlink("../middle/ime.json", &alias).unwrap();
                alias
            }
            _ => {
                let alias = fixture.root.join("overrides/ime.json");
                symlink(fixture.ime(), &alias).unwrap();
                alias
            }
        };
        let (_daemon, mut host) = fixture.start_host(&host_config);
        let before = fs::read(fixture.ime()).unwrap();
        let preview = fixture.tool("cli-runtime", &["data", "restore", "restore.json"]);
        assert!(preview.status.success(), "{preview:?}");
        assert_eq!(fs::read(fixture.ime()).unwrap(), before);
        assert!(!fixture.root.join("data/suzaku/backups").exists());
        let same_runtime = fixture.tool(
            "host-runtime",
            &["data", "restore", "restore.json", "--apply"],
        );
        assert!(
            !same_runtime.status.success(),
            "same-runtime restore must be blocked"
        );
        assert_eq!(fs::read(fixture.ime()).unwrap(), before);
        let independent_runtime = fixture.tool(
            "cli-runtime",
            &["data", "restore", "restore.json", "--apply"],
        );
        assert!(
            host.0.try_wait().unwrap().is_none(),
            "the audited host must still be running"
        );
        let active_model = fixture.status()["settings"]["llm_model"].clone();
        assert_eq!(active_model, "synthetic-before");
        let after = fs::read(fixture.ime()).unwrap();
        assert!(
            !independent_runtime.status.success(),
            "N10 {label}: restore bypassed the live host: {independent_runtime:?}"
        );
        assert_eq!(
            after, before,
            "N10 {label}: running configuration was replaced"
        );
        assert!(String::from_utf8_lossy(&independent_runtime.stderr).contains("正在运行"));
        println!(
            "PASS: {label}, preview is read-only; same and independent CLI runtimes both block restore"
        );

        // A writer reached through a different alias must block the host's narrow
        // save promptly, without changing either the file or active controls.
        let writer = suzaku_map::data::files::DataLease::settings_writer(&fixture.ime()).unwrap();
        let refused = fixture.control("U{\"llm_temperature_tenths\":2}");
        assert_eq!(refused["ok"], false);
        assert!(refused["error"].as_str().unwrap().contains("正在保存"));
        assert_eq!(fs::read(fixture.ime()).unwrap(), before);
        assert_eq!(fixture.status()["settings"]["llm_temperature_tenths"], 4);
        drop(writer);

        if label == "chained-alias" {
            let middle = fixture.root.join("middle/ime.json");
            let configured = fixture.tool_at(
                "cli-runtime",
                &["model", "configure", "--model", "synthetic-before"],
                Some(&middle),
            );
            assert!(configured.status.success(), "{configured:?}");
            assert!(
                !middle.is_symlink(),
                "existing atomic-save behavior replaces only the named link"
            );
            let middle_bytes = fs::read(&middle).unwrap();
            let refused = fixture.tool_at(
                "cli-runtime",
                &["data", "restore", "restore.json", "--apply"],
                Some(&middle),
            );
            assert!(
                !refused.status.success(),
                "the intermediate alias must keep its lifetime gate"
            );
            assert_eq!(fs::read(&middle).unwrap(), middle_bytes);
        }

        // Save through the actual host, then prove both its original read target
        // and its now-regular named configuration remain protected until exit.
        assert_eq!(
            fixture.control("U{\"llm_temperature_tenths\":2}")["ok"],
            true
        );
        let named = fixture.root.join(&host_config);
        assert!(!named.is_symlink());
        let named_bytes = fs::read(&named).unwrap();
        let target_bytes = fs::read(fixture.ime()).unwrap();
        for config in [&named, &fixture.ime()] {
            let refused = fixture.tool_at(
                "cli-runtime",
                &["data", "restore", "restore.json", "--apply"],
                Some(config),
            );
            assert!(
                !refused.status.success(),
                "{label}: the lifetime gate disappeared after saving"
            );
        }
        assert_eq!(fs::read(&named).unwrap(), named_bytes);
        assert_eq!(fs::read(fixture.ime()).unwrap(), target_bytes);
        assert_eq!(fixture.status()["settings"]["llm_temperature_tenths"], 2);
        println!(
            "PASS: {label}, writer collision is rejected; a native save keeps all lifetime gates"
        );
        drop(host);
        let stopped = fixture.tool(
            "cli-runtime",
            &["data", "restore", "restore.json", "--apply"],
        );
        assert!(
            stopped.status.success(),
            "restore should work after the private host stops: {stopped:?}"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(fixture.ime()).unwrap()).unwrap()["llm_model"],
            "synthetic-restored"
        );
        println!("PASS: {label}, after stopping the private host, restoration succeeds");
    }
}

struct ReadOnlyDirectory {
    path: PathBuf,
    original: fs::Permissions,
}

impl ReadOnlyDirectory {
    fn new(path: &Path) -> Self {
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "root bypasses the permission fixture"
        );
        let original = fs::metadata(path).unwrap().permissions();
        fs::set_permissions(path, fs::Permissions::from_mode(0o500)).unwrap();
        Self {
            path: path.into(),
            original,
        }
    }
}

impl Drop for ReadOnlyDirectory {
    fn drop(&mut self) {
        fs::set_permissions(&self.path, self.original.clone()).unwrap();
    }
}

#[test]
#[ignore = "private paths and non-root permissions; run by test-data-chain-audit.sh in Linux CI"]
fn audit_data_failures_preserve_sources_and_recovery_copies() {
    let fixture = Fixture::new("file-boundaries");
    let paths = fixture.paths();
    fs::create_dir_all(paths.panel.parent().unwrap()).unwrap();
    fs::write(
        &paths.panel,
        "theme_preset=suzaku\nui_language=ja\nwindow_scale=1.2\n",
    )
    .unwrap();
    let original = Backup::collect(&paths).unwrap();
    let ime_bytes = fs::read(&paths.ime).unwrap();
    let panel_bytes = fs::read(&paths.panel).unwrap();
    let source_unchanged = || {
        assert_eq!(fs::read(&paths.ime).unwrap(), ime_bytes);
        assert_eq!(fs::read(&paths.panel).unwrap(), panel_bytes);
    };
    let export = fixture.tool("cli-runtime", &["data", "backup", "export.json"]);
    assert!(export.status.success(), "{export:?}");
    let archive = fs::read(fixture.root.join("export.json")).unwrap();
    assert_eq!(
        fs::metadata(fixture.root.join("export.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let parsed = Backup::read(&fixture.root.join("export.json")).unwrap();
    assert_eq!(parsed.ime, original.ime);
    assert_eq!(parsed.panel, original.panel);
    assert!(
        !fixture
            .tool("cli-runtime", &["data", "backup", "export.json"])
            .status
            .success()
    );
    assert_eq!(fs::read(fixture.root.join("export.json")).unwrap(), archive);
    source_unchanged();
    println!(
        "PASS: CLI export round-trips both configurations, is private and never overwrites an existing backup"
    );

    fs::write(fixture.root.join("broken.json"), b"{").unwrap();
    fs::write(fixture.root.join("oversized.json"), vec![b' '; 262_145]).unwrap();
    symlink(
        fixture.root.join("export.json"),
        fixture.root.join("linked.json"),
    )
    .unwrap();
    let fifo = std::ffi::CString::new(
        fixture
            .root
            .join("fifo.json")
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    for name in ["broken.json", "oversized.json", "linked.json", "fifo.json"] {
        for arguments in [
            vec!["data", "validate", name],
            vec!["data", "restore", name, "--apply"],
        ] {
            let output = fixture.tool("cli-runtime", &arguments);
            assert!(!output.status.success(), "{arguments:?}: {output:?}");
            source_unchanged();
        }
        println!("PASS: {name} rejected without blocking or replacing configuration");
    }

    fs::write(&paths.ime, b"{").unwrap();
    let corrupt_source = fixture.tool(
        "cli-runtime",
        &["data", "restore", "restore.json", "--apply"],
    );
    assert!(!corrupt_source.status.success());
    assert_eq!(fs::read(&paths.ime).unwrap(), b"{");
    assert_eq!(fs::read(&paths.panel).unwrap(), panel_bytes);
    assert!(!paths.backups.exists());
    fs::write(&paths.ime, &ime_bytes).unwrap();
    println!("PASS: corrupt current configuration is preserved and blocks restore");

    let read_only = ReadOnlyDirectory::new(paths.panel.parent().unwrap());
    let denied = fixture.tool(
        "cli-runtime",
        &["data", "restore", "restore.json", "--apply"],
    );
    assert!(
        !denied.status.success(),
        "permission fixture must reject staging: {denied:?}"
    );
    source_unchanged();
    assert!(!paths.backups.exists());
    drop(read_only);
    assert!(
        !fs::read_dir(paths.ime.parent().unwrap())
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
    );
    println!(
        "PASS: unwritable second configuration directory prevents partial restore and cleans staged files"
    );

    fs::create_dir_all(&paths.backups).unwrap();
    let read_only = ReadOnlyDirectory::new(&paths.backups);
    let denied = fixture.tool(
        "cli-runtime",
        &["data", "restore", "restore.json", "--apply"],
    );
    assert!(
        !denied.status.success(),
        "permission fixture must reject safety backup: {denied:?}"
    );
    source_unchanged();
    assert_eq!(fs::read_dir(&paths.backups).unwrap().count(), 0);
    drop(read_only);
    println!("PASS: failure to create the before-restore backup leaves both configurations intact");

    let unrelated = paths.panel.parent().unwrap().join("notes.txt");
    fs::write(&unrelated, "synthetic-unrelated-data").unwrap();
    let restored = fixture.tool(
        "cli-runtime",
        &["data", "restore", "restore.json", "--apply"],
    );
    assert!(restored.status.success(), "{restored:?}");
    assert!(
        !paths.panel.exists(),
        "null restores defaults only for the named file"
    );
    assert_eq!(
        fs::read_to_string(&unrelated).unwrap(),
        "synthetic-unrelated-data"
    );
    let recovery = fs::read_dir(&paths.backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = Backup::read(&recovery).unwrap();
    assert_eq!(saved.ime, original.ime);
    assert_eq!(saved.panel, original.panel);
    let undo = fixture.tool(
        "cli-runtime",
        &["data", "restore", recovery.to_str().unwrap(), "--apply"],
    );
    assert!(undo.status.success(), "{undo:?}");
    let recovered = Backup::collect(&paths).unwrap();
    assert_eq!(recovered.ime, original.ime);
    assert_eq!(recovered.panel, original.panel);
    println!(
        "PASS: successful restore retains a readable recovery backup, preserves unrelated data and is reversible"
    );

    let unavailable = fixture.tool("cli-runtime", &["data", "open", "backups"]);
    assert!(
        !unavailable.status.success(),
        "no desktop/session bus is available to this child"
    );
    assert!(String::from_utf8_lossy(&unavailable.stderr).contains("手动打开"));
    println!(
        "PASS: unavailable file manager returns an actionable error instead of reporting success"
    );
}
