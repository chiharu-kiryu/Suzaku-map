use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::{
    self,
    packs::{self, OfflinePack, PackStore},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-packs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"));
        cmd.env("SUZAKU_LEXICON_DIR", self.0.join("packs"))
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS");
        cmd.arg("pack");
        cmd
    }
    fn cli(&self, args: &[&str]) -> String {
        success(self.command().args(args).output().unwrap())
    }
    fn preview(&self, lang: &str, text: &str) -> serde_json::Value {
        serde_json::from_str(&self.cli(&["preview", lang, text])).unwrap()
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
fn pack() -> OfflinePack {
    packs::recommended().remove(0)
}
fn contains(preview: &serde_json::Value, expected: &str, kind: &str) -> bool {
    preview["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["text"] == expected && c["kind"] == kind)
}

#[test]
fn schema_rejects_unsupported_unsafe_and_ignored_fields() {
    let good = serde_json::to_value(pack()).unwrap();
    for (pointer, value) in [
        ("/format_version", serde_json::json!(2)),
        ("/manifest/id", serde_json::json!("../../outside")),
        ("/manifest/version", serde_json::json!("01.0.0")),
        ("/manifest/language", serde_json::json!("fr")),
        ("/lexicon/language", serde_json::json!("ja")),
        ("/manifest/topics", serde_json::json!([])),
        ("/manifest/authors", serde_json::json!([])),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(
            OfflinePack::from_json(&bad.to_string()).is_err(),
            "{pointer}"
        );
    }
    let mut bad = good.clone();
    bad["script"] = serde_json::json!("run me");
    assert!(OfflinePack::from_json(&bad.to_string()).is_err());
    let mut bad = good;
    bad["lexicon"]["readings"] = serde_json::json!([{"reading":"foo","text":"bar","kind":"word"}]);
    assert!(OfflinePack::from_json(&bad.to_string()).is_err());
    assert!(OfflinePack::from_json(&" ".repeat(packs::MAX_PACK_BYTES + 1)).is_err());
    let mut bad = serde_json::to_value(packs::recommended().remove(1)).unwrap();
    for invalid in ["", "'abc", "A", "123", "xing kong", "xing''kong", "星空"] {
        bad["lexicon"]["readings"][0]["reading"] = serde_json::json!(invalid);
        assert!(
            OfflinePack::from_json(&bad.to_string()).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn catalog_filter_export_validate_and_no_clobber_are_read_only() {
    let f = Fixture::new();
    let filtered: serde_json::Value =
        serde_json::from_str(&f.cli(&["catalog", "--language", "en", "--topic", "astronomy"]))
            .unwrap();
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    f.cli(&["list"]);
    assert!(!f.store().path().parent().unwrap().exists());
    let destination = f.0.join("export.json");
    let name = destination.to_str().unwrap();
    f.cli(&["export", "org.suzaku.en.outdoors", name]);
    f.cli(&["validate", name]);
    let before = fs::read(&destination).unwrap();
    assert!(
        !f.command()
            .args(["export", "org.suzaku.en.outdoors", name])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(fs::read(destination).unwrap(), before);
    assert!(!f.store().path().exists());
}

#[test]
fn bundled_collections_match_sources_and_topic_filters_without_installing() {
    let f = Fixture::new();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/offline-packs");
    let files: Vec<_> = fs::read_dir(&source)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    let catalog = packs::recommended();
    assert_eq!(catalog.len(), 11);
    assert_eq!(files.len(), catalog.len());
    let mut ids = std::collections::HashSet::new();
    for path in files {
        let from_file = OfflinePack::read(&path).unwrap();
        assert!(ids.insert(from_file.manifest.id.clone()));
        let built_in = catalog
            .iter()
            .find(|pack| pack.manifest.id == from_file.manifest.id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(built_in).unwrap(),
            serde_json::to_value(&from_file).unwrap()
        );
        let export = f.0.join(path.file_name().unwrap());
        f.cli(&["export", &from_file.manifest.id, export.to_str().unwrap()]);
        assert_eq!(
            serde_json::to_value(OfflinePack::read(&export).unwrap()).unwrap(),
            serde_json::to_value(from_file).unwrap()
        );
    }
    for topic in ["study", "cooking", "travel", "work"] {
        for language in ["en", "zh-Hans"] {
            let filtered: serde_json::Value = serde_json::from_str(&f.cli(&[
                "catalog",
                "--topic",
                topic,
                "--language",
                language,
            ]))
            .unwrap();
            assert_eq!(filtered.as_array().unwrap().len(), 1);
            assert_eq!(
                filtered[0]["id"],
                format!("org.suzaku.{}.{topic}", language.to_lowercase())
            );
        }
    }
    assert!(!f.store().path().parent().unwrap().exists());
}

#[test]
fn all_collections_merge_without_changing_builtin_prefixes() {
    let f = Fixture::new();
    for pack in packs::recommended() {
        f.store().install(pack, false).unwrap();
    }
    let catalog = f.store().load();
    assert!(
        catalog.report.errors.is_empty(),
        "{:?}",
        catalog.report.errors
    );
    assert_eq!(catalog.report.loaded.len(), 11);
    for language in ["en", "zh-Hans", "ja"] {
        let base = lexicon::builtin(language).unwrap();
        let active = catalog.get(language).unwrap();
        assert_eq!(
            serde_json::to_value(base.word_layers()).unwrap(),
            serde_json::to_value(&active.word_layers()[..base.word_layers().len()]).unwrap()
        );
        assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
        assert_eq!(
            base.continuations().collect::<Vec<_>>(),
            active
                .continuations()
                .take(base.continuations().count())
                .collect::<Vec<_>>()
        );
    }
    // Explicit stores never change the process-global catalog implicitly.
    assert!(std::ptr::eq(
        lexicon::active("en").unwrap(),
        lexicon::builtin("en").unwrap()
    ));
}

#[test]
fn cli_install_toggle_replace_remove_changes_fresh_process_candidates() {
    let f = Fixture::new();
    let path = f.0.join("pack.json");
    fs::write(&path, serde_json::to_vec(&pack()).unwrap()).unwrap();
    let args = ["install", path.to_str().unwrap()];
    assert!(!contains(&f.preview("en", "stargaz"), "stargazing", "Word"));
    assert!(f.cli(&args).contains("Restart"));
    assert!(contains(&f.preview("en", "stargaz"), "stargazing", "Word"));
    let result = f.preview("en", "please bring your bino");
    assert!(
        contains(&result, "please bring your binoculars", "Word"),
        "{result}"
    );
    assert!(
        contains(&result, "please bring your binoculars.", "Sentence"),
        "{result}"
    );
    let before = fs::read(f.store().path()).unwrap();
    assert!(!f.command().args(args).output().unwrap().status.success());
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
    f.cli(&["disable", "org.suzaku.en.outdoors"]);
    assert!(!contains(&f.preview("en", "stargaz"), "stargazing", "Word"));
    f.cli(&["install", path.to_str().unwrap(), "--replace"]);
    assert!(!f.store().list().unwrap()[0].enabled);
    f.cli(&["enable", "org.suzaku.en.outdoors"]);
    assert!(contains(&f.preview("en", "stargaz"), "stargazing", "Word"));
    f.cli(&["remove", "org.suzaku.en.outdoors"]);
    assert!(f.store().list().unwrap().is_empty());
    assert!(!contains(&f.preview("en", "stargaz"), "stargazing", "Word"));
}

#[test]
fn chinese_and_japanese_packs_produce_words_and_sentences() {
    let f = Fixture::new();
    for pack in packs::recommended().into_iter().skip(1) {
        f.store().install(pack, false).unwrap();
    }
    for (language, seed, word, sentence) in [
        (
            "zh-Hans",
            "xing kong guan ce",
            "星空观测",
            "星空观测安排在周末晚上。",
        ),
        (
            "zh-Hans",
            "xing'kong'guan'ce",
            "星空观测",
            "星空观测安排在周末晚上。",
        ),
        (
            "zh-Hans",
            "xingkongguance",
            "星空观测",
            "星空观测安排在周末晚上。",
        ),
        ("ja", "kaisatsu", "改札", "改札はどこですか。"),
    ] {
        let result = f.preview(language, seed);
        assert!(contains(&result, word, "Word"), "{result}");
        assert!(contains(&result, sentence, "Sentence"), "{result}");
    }
}

#[test]
fn travel_and_work_collections_are_opt_in_and_can_be_disabled_independently() {
    let f = Fixture::new();
    let cases = [
        (
            "org.suzaku.en.travel",
            "en",
            "could you store our lugg",
            "could you store our luggage until this afternoon?",
        ),
        (
            "org.suzaku.zh-hans.travel",
            "zh-Hans",
            "xing li ji cun",
            "行李寄存可以到下午吗？",
        ),
        (
            "org.suzaku.en.work",
            "en",
            "please update the project road",
            "please update the project roadmap before our next meeting.",
        ),
        (
            "org.suzaku.zh-hans.work",
            "zh-Hans",
            "xiang mu lu xian tu",
            "项目路线图请在下次会议前更新。",
        ),
    ];
    for (_, language, seed, sentence) in cases {
        assert!(!contains(&f.preview(language, seed), sentence, "Sentence"));
    }
    assert!(
        !f.store().path().exists(),
        "preview must not install any recommended pack"
    );
    for pack in packs::recommended() {
        f.store().install(pack, false).unwrap();
    }
    for (id, language, seed, sentence) in cases {
        assert!(contains(&f.preview(language, seed), sentence, "Sentence"));
        f.cli(&["disable", id]);
        assert!(!contains(&f.preview(language, seed), sentence, "Sentence"));
        for (other_id, other_language, other_seed, other_sentence) in cases {
            if other_id != id {
                assert!(
                    contains(
                        &f.preview(other_language, other_seed),
                        other_sentence,
                        "Sentence"
                    ),
                    "disabling {id} affected {other_id}"
                );
            }
        }
        f.cli(&["enable", id]);
        assert!(contains(&f.preview(language, seed), sentence, "Sentence"));
    }
    assert_eq!(f.store().list().unwrap().len(), 11);
    assert!(f.store().load().report.errors.is_empty());
}

#[test]
fn conflicts_and_failed_transactions_preserve_registry_bytes() {
    let f = Fixture::new();
    f.store().install(pack(), false).unwrap();
    let before = fs::read(f.store().path()).unwrap();
    let mut bad = serde_json::to_value(pack()).unwrap();
    bad["manifest"]["id"] = serde_json::json!("example.conflict");
    bad["lexicon"]["word_layers"][0]["next_words"][0][1] = serde_json::json!(["tomorrow"]);
    let error = f
        .store()
        .install(OfflinePack::from_json(&bad.to_string()).unwrap(), false)
        .unwrap_err();
    assert!(error.contains("context already exists"), "{error}");
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
    assert!(f.store().set_enabled("missing", false).is_err());
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
    let _lease =
        suzaku_map::data::files::DataLease::acquire(&f.0.join("packs/.registry.lock"), true)
            .unwrap();
    assert!(f.store().remove("org.suzaku.en.outdoors").is_err());
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
}

#[test]
fn corrupt_record_is_skipped_but_never_erased_by_a_write() {
    let f = Fixture::new();
    f.store().install(pack(), false).unwrap();
    let mut raw: serde_json::Value =
        serde_json::from_slice(&fs::read(f.store().path()).unwrap()).unwrap();
    raw["packages"]
        .as_array_mut()
        .unwrap()
        .insert(0, serde_json::json!({"garbage":true}));
    fs::write(f.store().path(), raw.to_string()).unwrap();
    let catalog = f.store().load();
    assert_eq!(catalog.report.loaded, ["org.suzaku.en.outdoors"]);
    assert_eq!(catalog.report.errors.len(), 1);
    assert!(f.store().remove("org.suzaku.en.outdoors").is_err());
    assert_eq!(
        fs::read_to_string(f.store().path()).unwrap(),
        raw.to_string()
    );
    fs::write(f.store().path(), "invalid").unwrap();
    let catalog = f.store().load();
    assert_eq!(catalog.report.errors.len(), 1);
    assert!(std::ptr::eq(
        catalog.get("en").unwrap(),
        lexicon::builtin("en").unwrap()
    ));
}

#[test]
fn identical_entries_are_deduplicated_but_flags_cannot_override_builtins() {
    let f = Fixture::new();
    let base = lexicon::builtin("zh-Hans").unwrap();
    let mut raw = serde_json::to_value(packs::recommended().remove(1)).unwrap();
    raw["lexicon"]["readings"] = serde_json::to_value(&base.readings()[..2]).unwrap();
    raw["lexicon"]["continuations"] =
        serde_json::to_value(base.continuations().take(1).collect::<Vec<_>>()).unwrap();
    f.store()
        .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
        .unwrap();
    assert_eq!(
        base.readings(),
        f.store().load().get("zh-Hans").unwrap().readings()
    );
    let before = fs::read(f.store().path()).unwrap();
    raw["manifest"]["id"] = serde_json::json!("example.conflicting-flags");
    raw["lexicon"]["readings"][0]["require_separators"] =
        serde_json::json!(!base.readings()[0].require_separators);
    assert!(
        f.store()
            .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
            .unwrap_err()
            .contains("flags/type")
    );
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
}

#[test]
fn cli_errors_never_create_a_personal_store_or_guess_a_language() {
    let f = Fixture::new();
    for args in [
        vec!["install"],
        vec!["preview", "fr", "hello"],
        vec!["catalog", "--language", "fr"],
        vec!["catalog", "--topic"],
        vec!["export", "missing", "output.json"],
    ] {
        assert!(!f.command().args(args).output().unwrap().status.success());
    }
    for invalid in ["", "relative"] {
        let output = f
            .command()
            .env("SUZAKU_LEXICON_DIR", invalid)
            .arg("list")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("absolute"));
    }
    assert!(!f.store().path().parent().unwrap().exists());
}

#[test]
fn sdk_invalid_source_and_missing_validator_never_publish_a_package() {
    let f = Fixture::new();
    let source = f.0.join("source.json");
    let destination = f.0.join("result.json");
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sdk/offline-packs/suzaku_pack.py");
    for invalid in [
        "{}",
        "{\"format_version\":2,\"format_version\":1}",
        "invalid",
    ] {
        fs::write(&source, invalid).unwrap();
        let output = Command::new("python3")
            .arg(&sdk)
            .arg(&source)
            .arg(&destination)
            .args(["--tool", env!("CARGO_BIN_EXE_suzaku_tool")])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!destination.exists());
    }
    fs::write(&source, serde_json::to_vec(&pack()).unwrap()).unwrap();
    let output = Command::new("python3")
        .arg(&sdk)
        .arg(&source)
        .arg(&destination)
        .arg("--tool")
        .arg(f.0.join("missing-validator"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!destination.exists());
    assert!(!fs::read_dir(&f.0).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".suzaku-pack-")
    }));
}

#[test]
fn package_count_and_active_work_budgets_are_enforced_transactionally() {
    let f = Fixture::new();
    let mut raw = serde_json::to_value(pack()).unwrap();
    raw["lexicon"]["word_layers"] =
        serde_json::json!([{"id":"large", "words":vec!["stargazing"; 1500]}]);
    for id in ["example.first", "example.second"] {
        raw["manifest"]["id"] = serde_json::json!(id);
        f.store()
            .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
            .unwrap();
    }
    let before = fs::read(f.store().path()).unwrap();
    raw["manifest"]["id"] = serde_json::json!("example.third");
    assert!(
        f.store()
            .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
            .unwrap_err()
            .contains("4096")
    );
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
    raw["lexicon"]["word_layers"][0]["words"] = serde_json::json!(vec!["stargazing"; 2049]);
    assert!(OfflinePack::from_json(&raw.to_string()).is_err());
    f.store().set_enabled("example.first", false).unwrap();
    raw["lexicon"]["word_layers"][0]["words"] = serde_json::json!(["stargazing"]);
    for n in 0..14 {
        raw["manifest"]["id"] = serde_json::json!(format!("example.small-{n}"));
        f.store()
            .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
            .unwrap();
    }
    let before = fs::read(f.store().path()).unwrap();
    raw["manifest"]["id"] = serde_json::json!("example.excess");
    assert!(
        f.store()
            .install(OfflinePack::from_json(&raw.to_string()).unwrap(), false)
            .is_err()
    );
    assert_eq!(fs::read(f.store().path()).unwrap(), before);
}

#[test]
fn sdk_builds_validates_and_preserves_existing_output() {
    let f = Fixture::new();
    let path = f.0.join("garden.json");
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sdk/offline-packs/example.py");
    let invoke = || {
        Command::new("python3")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .arg(&example)
            .arg(&path)
            .arg(env!("CARGO_BIN_EXE_suzaku_tool"))
            .output()
            .unwrap()
    };
    success(invoke());
    let before = fs::read(&path).unwrap();
    assert!(!invoke().status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    f.store()
        .install(OfflinePack::read(&path).unwrap(), false)
        .unwrap();
    let result = f.preview("zh-Hans", "yu miao pan");
    assert!(contains(&result, "育苗盘", "Word"), "{result}");
    assert!(
        contains(&result, "育苗盘放在窗边了。", "Sentence"),
        "{result}"
    );
    assert!(!fs::read_dir(&f.0).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".suzaku-pack-")
    }));
}

#[test]
fn runtime_snapshot_does_not_reload_during_composition() {
    let f = Fixture::new();
    f.store().install(pack(), false).unwrap();
    success(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "runtime_snapshot_child", "--nocapture"])
            .env("SUZAKU_PACK_RUNTIME_CHILD", f.0.join("packs"))
            .output()
            .unwrap(),
    );
}

#[test]
fn runtime_snapshot_child() {
    let Some(path) = std::env::var_os("SUZAKU_PACK_RUNTIME_CHILD") else {
        return;
    };
    let store = PackStore::new(path.into()).unwrap();
    assert_eq!(packs::initialize(&store).unwrap().loaded.len(), 1);
    let before = lexicon::active("en").unwrap();
    assert!(
        before
            .word_layers()
            .iter()
            .any(|l| l.words.iter().any(|w| w == "stargazing"))
    );
    store.set_enabled("org.suzaku.en.outdoors", false).unwrap();
    assert!(packs::initialize(&store).is_err());
    assert!(std::ptr::eq(before, lexicon::active("en").unwrap()));
    assert!(!suzaku_map::languages::english::is_known_english_word(
        "zqxunknown"
    ));
    assert!(suzaku_map::languages::english::is_known_english_word(
        "stargazing"
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn symlinks_and_fifos_are_rejected_without_touching_targets() {
    let f = Fixture::new();
    let target = f.0.join("target.json");
    fs::write(&target, serde_json::to_vec(&pack()).unwrap()).unwrap();
    let link = f.0.join("linked.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(OfflinePack::read(&link).is_err());
    let fifo = f.0.join("fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(OfflinePack::read(&fifo).is_err());
    fs::create_dir(f.0.join("packs")).unwrap();
    std::os::unix::fs::symlink(&target, f.store().path()).unwrap();
    assert!(f.store().install(pack(), false).is_err());
    assert!(OfflinePack::read(&target).is_ok());
}
