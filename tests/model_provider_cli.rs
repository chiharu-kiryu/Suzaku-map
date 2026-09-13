//! Configuration tests use private files and never contact a real model service.
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-model-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
            .args(args)
            .env("SUZAKU_IME_CONFIG", self.0.join("settings.json"))
            .env("XDG_CONFIG_HOME", &self.0)
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // A regression in configuration loading must fail, not hang the test suite.
        // These configuration-only commands produce less than one pipe's capacity.
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("configuration command blocked: {args:?}; {output:?}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        child.wait_with_output().unwrap()
    }
    fn saved(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.0.join("settings.json")).unwrap()).unwrap()
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
fn cloud_configuration_never_grants_consent_implicitly_or_contacts_a_service_for_status() {
    let fixture = Fixture::new();
    success(fixture.run(&[
        "model",
        "configure",
        "--scope",
        "cloud",
        "--endpoint",
        "https://unreachable.invalid/v1/chat/completions",
        "--model",
        "any-family",
        "--api-key-env",
        "SUZAKU_TEST_MODEL_KEY",
    ]));
    let configured = fixture.saved();
    assert_eq!(configured["llm_cloud_consent"], false);
    assert_eq!(configured["llm_enabled"], false);
    assert_eq!(configured["llm_api_key_env"], "SUZAKU_TEST_MODEL_KEY");
    assert!(success(fixture.run(&["model", "status"])).contains("no remote request"));
    let rejected = fixture.run(&["model", "probe", "en"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stdout).contains("云端联想尚未授权"));
    assert_eq!(fixture.saved(), configured);
    success(fixture.run(&["model", "configure", "--cloud-consent", "true"]));
    assert_eq!(fixture.saved()["llm_cloud_consent"], true);
    success(fixture.run(&["model", "configure", "--model", "another-family"]));
    assert_eq!(fixture.saved()["llm_cloud_consent"], false);
    success(fixture.run(&["llama", "configure"]));
    assert_eq!(fixture.saved()["llm_scope"], "local");
    assert_eq!(fixture.saved()["llm_model"], "auto");
    assert_eq!(fixture.saved()["llm_api_key_env"], Value::Null);
}

#[test]
fn invalid_or_literal_credentials_never_replace_existing_settings() {
    let fixture = Fixture::new();
    success(fixture.run(&["llama", "configure", "--model", "my-local-model"]));
    let before = fs::read(fixture.0.join("settings.json")).unwrap();
    for args in [
        vec![
            "model",
            "configure",
            "--endpoint",
            "http://remote.invalid/v1/chat/completions",
        ],
        vec![
            "model",
            "configure",
            "--api-key",
            "synthetic-not-a-real-key",
        ],
        vec!["model", "configure", "--cloud-consent", "yes"],
    ] {
        assert!(!fixture.run(&args).status.success());
        assert_eq!(fs::read(fixture.0.join("settings.json")).unwrap(), before);
    }
}

#[test]
fn concurrent_configuration_writers_fail_without_replacing_settings() {
    let fixture = Fixture::new();
    success(fixture.run(&["model", "configure", "--model", "before"]));
    let path = fixture.0.join("settings.json");
    let before = fs::read(&path).unwrap();
    let _running = suzaku_map::data::files::DataLease::acquire(
        &suzaku_map::data::paths::lock_for_settings(&path).unwrap(),
        false,
    )
    .unwrap();
    let writer = suzaku_map::data::files::DataLease::settings_writer(&path).unwrap();
    let rejected = fixture.run(&["model", "configure", "--model", "not-written"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("正在保存"));
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(writer);
    success(fixture.run(&["model", "configure", "--model", "after"]));
    assert_eq!(fixture.saved()["llm_model"], "after");
}

#[test]
fn settings_read_limits_and_invalid_contents_never_overwrite_the_source() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let mut exact = br#"{"language":"ja","llm_enabled":false}"#.to_vec();
    exact.resize(65_536, b' ');
    fs::write(&path, &exact).unwrap();
    success(fixture.run(&["model", "configure", "--model", "synthetic-model"]));
    assert_eq!(fixture.saved()["language"], "ja");
    exact.push(b' ');
    for raw in [exact, vec![0xff], b"{".to_vec(), b"[]".to_vec()] {
        fs::write(&path, &raw).unwrap();
        let result = fixture.run(&["model", "configure", "--model", "not-applied"]);
        assert!(!result.status.success());
        assert_eq!(fs::read(&path).unwrap(), raw);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn special_configuration_files_are_rejected_without_blocking_or_replacement() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.json");
    let rejected = || {
        let result = fixture.run(&["model", "configure", "--model", "not-applied"]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("无法读取输入法设置"));
    };

    fs::create_dir(&path).unwrap();
    rejected();
    assert!(path.is_dir());
    fs::remove_dir(&path).unwrap();

    let socket = UnixListener::bind(&path).unwrap();
    rejected();
    drop(socket);
    fs::remove_file(&path).unwrap();

    let fifo = fixture.0.join("pipe");
    let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    for linked in [false, true] {
        if linked {
            symlink(&fifo, &path).unwrap();
        } else {
            fs::rename(&fifo, &path).unwrap();
        }
        let before = fs::symlink_metadata(&path).unwrap().file_type();
        rejected();
        assert_eq!(fs::symlink_metadata(&path).unwrap().file_type(), before);
        if linked {
            fs::remove_file(&path).unwrap();
        } else {
            fs::rename(&path, &fifo).unwrap();
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinks_to_regular_configuration_files_remain_readable() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.0.join("target.json");
    let raw = br#"{"language":"ja","llm_scope":"cloud","llm_model":"synthetic-model","llm_endpoint":"https://unreachable.invalid/v1/chat/completions"}"#;
    fs::write(&target, raw).unwrap();
    let path = fixture.0.join("settings.json");
    symlink(&target, &path).unwrap();
    let output = success(fixture.run(&["model", "status"]));
    assert!(output.contains("input-language: ja"));
    assert!(output.contains("no remote request"));
    assert!(path.is_symlink());
    assert_eq!(fs::read(&target).unwrap(), raw);
}
