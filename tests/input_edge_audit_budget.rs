//! Large but valid offline packages must not hide the entire native candidate
//! frame. Each child owns a frozen catalog; no personal IME or packs are used.
use std::{fs, path::PathBuf, process::Command};
use suzaku_map::ime::companion::{
    MAX_FRAME_BYTES, MAX_TEXT_BYTES, NativeCandidate, NativeComposition,
};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};
use suzaku_map::lexicon::packs::{self, OfflinePack, PackStore};

#[test]
fn short_chinese_keeps_primary_conversion_with_a_single_candidate_limit() {
    for limit in [0, 1, 2, 3, 6, 12] {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: "zh-Hans".into(),
            ..Default::default()
        });
        engine.seed("nihao");
        // The IBus convenience switch deliberately sets the engine limit to 12;
        // exercise the ranking function's explicit limit directly instead.
        let choices = suzaku_map::ime::candidate_mix::offline(
            "zh-Hans",
            "nihao",
            "",
            engine.candidates().to_vec(),
            limit,
        );
        assert_eq!(choices[0].text, "你好");
        assert!(choices.len() <= limit.max(1));
        if limit > 1 {
            assert!(choices.iter().any(|c| c.text == "nihao"));
        }
    }
}

#[test]
fn valid_pack_candidates_fit_native_transport_and_keep_commit_indices() {
    const CHILD: &str = "SUZAKU_CANDIDATE_BUDGET_AUDIT";
    if let Some(path) = std::env::var_os(CHILD) {
        let store = PackStore::new(PathBuf::from(path)).unwrap();
        let report = packs::initialize(&store).unwrap();
        assert_eq!(report.loaded, ["org.suzaku.test.candidate-budget"]);
        assert!(report.errors.is_empty());
        for seed in ["ce'shi", "shi'yan", "jian'yan"] {
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: "zh-Hans".into(),
                ..Default::default()
            });
            engine.enable_ibus_candidate_mix();
            engine.seed(seed);
            let choices = engine.candidates().to_vec();
            assert!(choices.len() > 1);
            assert!(choices.iter().any(|c| c.text == seed));
            assert!(
                choices
                    .iter()
                    .all(|c| { c.text.len() <= MAX_TEXT_BYTES && c.label.len() <= MAX_TEXT_BYTES }),
                "oversized individual candidate for {seed}"
            );
            let frame = NativeComposition {
                host: "00000000-0000-0000-0000-000000000001".into(),
                context: 1,
                revision: 1,
                focused: true,
                private: false,
                cursor: None,
                language: "zh-Hans".into(),
                seed: seed.into(),
                selected: 0,
                candidates: choices
                    .iter()
                    .map(|c| NativeCandidate {
                        text: c.text.clone(),
                        label: c.label.clone(),
                        kind: c.kind,
                        source: c.source,
                        weight: c.score.clamp(0.0, 100.0) as u8,
                    })
                    .collect(),
            };
            let raw = frame.to_json().to_string();
            assert!(
                raw.len() < MAX_FRAME_BYTES,
                "oversized {seed} frame: {}",
                raw.len()
            );
            assert_eq!(NativeComposition::parse(raw.as_bytes()).unwrap(), frame);
            // Every rendered index is the actual full commit payload, including
            // choices on later pages. Never truncate text only in the mirror.
            for (index, expected) in choices.iter().enumerate() {
                engine.seed(seed);
                engine.select_candidate(index);
                assert_eq!(
                    engine.commit(CommitOptions { force: true }).text.as_deref(),
                    Some(expected.text.as_str())
                );
                assert!(!engine.commit(CommitOptions { force: true }).ok);
                assert!(engine.undo().unwrap().committed_text.is_empty());
            }
            if seed == "ce'shi" {
                assert!(choices.iter().any(|c| c.text == "测试"));
            } else {
                assert!(
                    choices.iter().any(|c| c.text.len() > 3000),
                    "valid full-length alternatives disappeared"
                );
            }
        }
        return;
    }

    let root = std::env::temp_dir().join(format!("suzaku-candidate-budget-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let _cleanup = Cleanup(root.clone());
    let mut readings = Vec::new();
    for (reading, body) in [
        ("ce'shi", "字".repeat(3000)),
        ("shi'yan", "文".repeat(2000)),
        ("jian'yan", "\"\\".repeat(1800)),
    ] {
        for suffix in ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛'] {
            readings.push(serde_json::json!({"reading":reading, "text":format!("{body}{suffix}"), "kind":"word"}));
        }
    }
    let raw = serde_json::json!({
        "format_version":1,
        "manifest": {
            "id":"org.suzaku.test.candidate-budget", "version":"1.0.0",
            "name":"Candidate budget fixture", "description":"Valid long local candidates.",
            "language":"zh-Hans", "topics":["testing"], "license":"MIT", "authors":["Suzaku tests"]
        },
        "lexicon":{"format_version":1,"language":"zh-Hans","readings":readings}
    });
    let store = PackStore::new(root.join("packs")).unwrap();
    store
        .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
        .unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "valid_pack_candidates_fit_native_transport_and_keep_commit_indices",
            "--nocapture",
        ])
        .env(CHILD, store.path().parent().unwrap())
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env_remove("SUZAKU_IME_CONFIG")
        .env_remove("SUZAKU_LINUX_IME_SOCKET")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
