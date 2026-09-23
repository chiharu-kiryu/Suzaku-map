#![cfg(all(target_os = "linux", feature = "gpu"))]

use std::{
    fs::{self, File},
    os::unix::{fs::PermissionsExt, net::UnixDatagram},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzaku-instance-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["runtime/suzaku-panel", "config", "data"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        // A regressed extra panel must not discover or contact a real model.
        // Invalid configuration leaves its confirmed provider unavailable.
        fs::write(root.join("ime.json"), b"{").unwrap();
        Self(root)
    }

    fn launch(&self) -> std::io::Result<Output> {
        suzaku_map::platform::linux_command::run(
            Command::new(env!("CARGO_BIN_EXE_panel"))
                .env_remove("WAYLAND_DISPLAY")
                .env("XDG_RUNTIME_DIR", self.0.join("runtime"))
                .env("XDG_CONFIG_HOME", self.0.join("config"))
                .env("XDG_DATA_HOME", self.0.join("data"))
                .env("SUZAKU_IME_CONFIG", self.0.join("ime.json"))
                .env("SUZAKU_LINUX_IME_SOCKET", self.0.join("host.sock"))
                .env(
                    "IBUS_ADDRESS",
                    format!("unix:path={}/absent-ibus.sock", self.0.display()),
                )
                .env("GSETTINGS_BACKEND", "memory")
                .env("GIO_USE_VFS", "local"),
            Duration::from_secs(5),
        )
    }

    fn assert_rejected(&self, case: &str) {
        let output = self.launch().unwrap_or_else(|error| {
            panic!("N21: {case} did not reject the extra panel within the deadline: {error}")
        });
        assert!(
            !output.status.success(),
            "N21: {case} reported a successful launch"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
#[ignore = "requires private Xvfb/D-Bus/XDG; run by scripts/test-linux-ci.sh ui"]
fn single_instance_failures_never_launch_an_unguarded_panel() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    assert!(std::env::var_os("DISPLAY").is_some());
    assert!(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some());
    for key in ["XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
        assert!(Path::new(&std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir()));
    }

    let fixture = Fixture::new();
    let runtime = fixture.0.join("runtime/suzaku-panel");
    let lock_path = runtime.join("panel.lock");
    let lock = File::create(&lock_path).unwrap();
    lock.try_lock().unwrap();
    let socket_path = runtime.join("control.sock");
    let receiver = UnixDatagram::bind(&socket_path).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    // Prove the real binary reaches the instance gate with this display/config,
    // and that a healthy repeated launch sends Show instead of opening a window.
    assert!(fixture.launch().unwrap().status.success());
    let mut message = [0; 16];
    let length = receiver.recv(&mut message).unwrap();
    assert_eq!(&message[..length], b"show");
    drop(receiver);
    fs::remove_file(&socket_path).unwrap();

    fixture.assert_rejected("held lock with missing Show socket");
    assert!(!socket_path.exists());
    fs::write(&socket_path, b"synthetic unrelated file").unwrap();
    fixture.assert_rejected("held lock with unusable Show socket");
    assert_eq!(fs::read(&socket_path).unwrap(), b"synthetic unrelated file");
    drop(lock);

    fs::remove_file(&lock_path).unwrap();
    fs::create_dir(&lock_path).unwrap();
    fixture.assert_rejected("unavailable instance lock");
    assert!(lock_path.is_dir());
    println!(
        "PASS: healthy Show handoff; missing/unusable control and lock errors reject extra panels"
    );
}
