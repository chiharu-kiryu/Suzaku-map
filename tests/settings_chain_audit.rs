//! Isolated host/CLI configuration handoff regressions. No IME is activated.
use serde_json::Value;
use std::{
    ffi::{CStr, CString},
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::settings::ImeSettings,
    ime_host::{host_bridge_snapshot, suzaku_host_ime_control_utf8, suzaku_host_ime_free_utf8},
};

fn control(command: &str) -> Value {
    let command = CString::new(command).unwrap();
    let raw = suzaku_host_ime_control_utf8(command.as_ptr());
    assert!(!raw.is_null());
    // This is the owned, NUL-terminated response allocated by the matching host API.
    let response = unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { suzaku_host_ime_free_utf8(raw) };
    serde_json::from_str(&response).unwrap()
}

fn configure(args: &[&str]) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
        .args(["model", "configure"])
        .args(args)
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("configuration CLI blocked: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
#[ignore = "requires private configuration; run by test-settings-chain-audit.sh in Linux CI"]
fn audit_native_patch_must_not_clobber_a_newer_saved_configuration() {
    assert_eq!(
        std::env::var("SUZAKU_SETTINGS_CHAIN_AUDIT").as_deref(),
        Ok("1")
    );
    for key in [
        "SUZAKU_IME_CONFIG",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
    ] {
        assert!(PathBuf::from(std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir()));
    }
    let path = suzaku_map::ime::settings::settings_path().unwrap();
    let cloud = ImeSettings::from_json(r#"{"llm_enabled":false,"llm_scope":"cloud","llm_protocol":"openai-compatible","llm_endpoint":"https://old.synthetic.invalid/v1/chat/completions","llm_model":"synthetic-old","llm_cloud_consent":true}"#).unwrap();
    let cases = [
        (
            "local model replacement",
            ImeSettings::default(),
            vec!["--model", "synthetic-new", "--timeout-ms", "1234"],
        ),
        (
            "cloud consent revoked",
            cloud.clone(),
            vec!["--cloud-consent", "false"],
        ),
        (
            "cloud target changed",
            cloud,
            vec![
                "--endpoint",
                "https://new.synthetic.invalid/v1/chat/completions",
                "--model",
                "synthetic-new",
            ],
        ),
    ];
    let mut findings = Vec::new();
    for (case, base, arguments) in cases {
        for command in ["U{\"llm_temperature_tenths\":7}", "P0", "Lja"] {
            assert!(!base.llm_enabled);
            base.save().unwrap();
            assert_eq!(control("R")["ok"], true);
            assert_eq!(control("S")["settings"], base.to_json());
            configure(&arguments);
            let updated = ImeSettings::load().unwrap();
            let bytes = fs::read(&path).unwrap();
            assert_ne!(updated, base);
            assert!(!updated.llm_enabled);
            assert_eq!(control("S")["settings"], base.to_json());
            assert_eq!(
                fs::read(&path).unwrap(),
                bytes,
                "read-only status does not overwrite the edit"
            );
            let mut expected = updated.clone();
            match command {
                "P0" => expected.llm_enabled = false,
                "Lja" => expected.language = suzaku_map::languages::BuiltinLanguage::Japanese,
                _ => expected.provider.temperature_tenths = 7,
            }
            let response = control(command);
            let actual = ImeSettings::load().unwrap();
            assert_eq!(
                response["ok"], false,
                "do not implicitly reload a new provider or consent"
            );
            assert!(response["error"].as_str().unwrap().contains("重新加载"));
            assert_eq!(
                response["settings"],
                base.to_json(),
                "conflict leaves active settings unchanged"
            );
            // Either merge only the requested fields or reject a conflict without
            // replacing the new file. Both preserve the user's saved intent.
            if (response["ok"] == true && actual != expected)
                || (response["ok"] != true && fs::read(&path).unwrap() != bytes)
            {
                findings.push(format!("N08 {case}, command={command}: expected model={}, consent={}, timeout={}; saved model={}, consent={}, timeout={}",
                    expected.provider.model, expected.provider.cloud_consent, expected.provider.timeout_ms,
                    actual.provider.model, actual.provider.cloud_consent, actual.provider.timeout_ms));
            }
            // Explicitly reloading first is the currently working sequence.
            updated.save().unwrap();
            assert_eq!(control("R")["ok"], true);
            assert_eq!(control(command)["ok"], true);
            assert_eq!(ImeSettings::load().unwrap(), expected);
            let snapshot = host_bridge_snapshot();
            assert!(!snapshot.active);
            assert!(snapshot.marked_text.is_empty());
            assert!(snapshot.committed_text.is_empty());
            println!(
                "PASS: {case}, {command}: status preserves disk; explicit reload then patch preserves the new configuration"
            );
        }
    }
    let baseline = ImeSettings::load().unwrap();
    let bytes = fs::read(&path).unwrap();
    let writer = suzaku_map::data::files::DataLease::settings_writer(&path).unwrap();
    let started = Instant::now();
    let busy = control("U{\"llm_temperature_tenths\":2}");
    assert_eq!(busy["ok"], false);
    assert!(busy["error"].as_str().unwrap().contains("正在保存"));
    assert!(
        started.elapsed() < Duration::from_millis(250),
        "never wait for a writer on the input loop"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(writer);

    let mut stale_edit = baseline.clone();
    stale_edit.provider.timeout_ms = 2345;
    let mut newer = baseline.clone();
    newer.provider.model = "newer-synthetic-model".into();
    newer.save().unwrap();
    assert!(
        stale_edit
            .save_if_unchanged(&baseline)
            .unwrap_err()
            .contains("重新加载")
    );
    assert_eq!(ImeSettings::load().unwrap(), newer);
    baseline.save().unwrap();
    fs::write(&path, b"{").unwrap();
    assert_eq!(control("P0")["ok"], false);
    assert_eq!(
        fs::read(&path).unwrap(),
        b"{",
        "a narrow control must not erase an invalid external edit"
    );
    fs::write(&path, &bytes).unwrap();
    assert_eq!(control("R")["ok"], true);
    println!(
        "PASS: held writer returns promptly; compare-and-save rejects stale callers; invalid external files remain intact"
    );
    for finding in &findings {
        eprintln!("AUDIT: {finding}");
    }
    assert!(
        findings.is_empty(),
        "{} stale-host configuration overwrites",
        findings.len()
    );
}
