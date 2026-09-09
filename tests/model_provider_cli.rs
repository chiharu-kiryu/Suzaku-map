//! Configuration tests use private files and never contact a real model service.
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
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
        Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
            .args(args)
            .env("SUZAKU_IME_CONFIG", self.0.join("settings.json"))
            .env("XDG_CONFIG_HOME", &self.0)
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap()
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
