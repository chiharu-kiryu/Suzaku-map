//! Accepted offline pack fields must remain usable at decoder boundaries.
//! Each fixture starts a separate CLI, so no process-global catalog leaks across
//! tests and no personal pack registry, model service or desktop is touched.
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use suzaku_map::lexicon::packs::{OfflinePack, PackStore};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzaku-english-edge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn install(&self, layer: serde_json::Value) {
        self.install_lexicon(serde_json::json!({
            "format_version": 1,
            "language": "en",
            "word_layers": [layer]
        }));
    }

    fn install_lexicon(&self, lexicon: serde_json::Value) {
        let pack = serde_json::json!({
            "format_version": 1,
            "manifest": {
                "id": "org.suzaku.test.english-edge",
                "version": "1.0.0",
                "name": "English boundary fixture",
                "description": "Authored deterministic decoder-boundary cases.",
                "language": "en",
                "topics": ["testing"],
                "license": "MIT",
                "authors": ["Suzaku tests"]
            },
            "lexicon": lexicon
        });
        // These are already accepted decoder forms, not malformed-pack tests.
        let pack = OfflinePack::from_json(&pack.to_string()).unwrap();
        PackStore::new(self.0.join("packs"))
            .unwrap()
            .install(pack, false)
            .unwrap();
    }

    fn preview(&self, seed: &str) -> serde_json::Value {
        let output = Command::new(env!("CARGO_BIN_EXE_suzaku_tool"))
            .args(["pack", "preview", "en", seed])
            .env("SUZAKU_LEXICON_DIR", self.0.join("packs"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let preview: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            preview["report"]["loaded"],
            serde_json::json!(["org.suzaku.test.english-edge"])
        );
        assert_eq!(preview["report"]["errors"], serde_json::json!([]));
        let candidates = preview["candidates"].as_array().unwrap();
        assert_eq!(candidates[0]["text"], seed);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate["text"].as_str().unwrap().starts_with(seed))
        );
        preview
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn assert_first_page(preview: &serde_json::Value, text: &str, kind: &str) {
    assert!(
        preview["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .take(suzaku_map::ime::candidate_mix::PAGE_SIZE)
            .any(|candidate| candidate["text"] == text && candidate["kind"] == kind),
        "missing {kind} {text:?}: {preview}"
    );
}

#[test]
fn accepted_pack_contexts_match_authored_case_and_smart_apostrophes() {
    let fixture = Fixture::new();
    fixture.install(serde_json::json!({
        "id": "contexts",
        "next_words": [
            ["Schedule an observatory", ["visit", "tour"]],
            ["we’re picnicking", ["outside", "together"]]
        ]
    }));
    for (seed, choices) in [
        ("schedule an observatory ", ["visit", "tour"]),
        ("Schedule an observatory ", ["visit", "tour"]),
        ("  Schedule  an  observatory  ", ["visit", "tour"]),
        ("we're picnicking ", ["outside", "together"]),
        ("we’re picnicking ", ["outside", "together"]),
    ] {
        let preview = fixture.preview(seed);
        for choice in choices {
            assert_first_page(&preview, &format!("{seed}{choice}"), "Word");
        }
    }
}

#[test]
fn authored_smart_contractions_have_words_as_well_as_sentences() {
    let fixture = Fixture::new();
    fixture.install(serde_json::json!({
        "id": "contractions",
        "sentences": ["please check whether y’all can join us."]
    }));
    let preview = fixture.preview("please check whether y");
    assert_first_page(
        &preview,
        "please check whether y’all can join us.",
        "Sentence",
    );
    // Before any apostrophe is typed, either normalized or authored typography
    // is acceptable. Splitting y’all into the unrelated bare y / all is not.
    assert!(
        preview["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .take(suzaku_map::ime::candidate_mix::PAGE_SIZE)
            .any(|candidate| {
                candidate["kind"] == "Word"
                    && matches!(
                        candidate["text"].as_str(),
                        Some("please check whether y'all" | "please check whether y’all")
                    )
            }),
        "missing complete contraction word: {preview}"
    );
    for (seed, word, sentence) in [
        (
            "please check whether y'",
            "please check whether y'all",
            "please check whether y'all can join us.",
        ),
        (
            "Please  check  whether  y’",
            "Please  check  whether  y’all",
            "Please  check  whether  y’all can join us.",
        ),
        (
            "please check whether Y’AL",
            "please check whether Y’ALL",
            "please check whether Y’ALL can join us.",
        ),
        (
            "please check whether ",
            "please check whether y’all",
            "please check whether y’all can join us.",
        ),
    ] {
        let preview = fixture.preview(seed);
        assert_first_page(&preview, word, "Word");
        assert_first_page(&preview, sentence, "Sentence");
    }
    let prefix = "Earlier note. ".repeat(40);
    let preview = fixture.preview(&format!("{prefix}please check whether y"));
    assert_first_page(
        &preview,
        &format!("{prefix}please check whether y’all"),
        "Word",
    );
    assert_first_page(
        &preview,
        &format!("{prefix}please check whether y’all can join us."),
        "Sentence",
    );
}

#[test]
fn sentence_only_smart_contractions_are_indexed_as_whole_words() {
    let fixture = Fixture::new();
    fixture.install(serde_json::json!({
        "id": "contraction-index",
        "sentences": ["please check whether y’all can join us."]
    }));
    for (seed, expected) in [("y'", "y'all"), ("y’", "y’all"), ("Y’AL", "Y’ALL")] {
        assert_first_page(&fixture.preview(seed), expected, "Word");
    }
    for seed in ["y'all", "y’all"] {
        // A whole indexed contraction must also be recognized as a known word,
        // not merely accidentally projected by a matching sentence context.
        let preview = fixture.preview(seed);
        assert_eq!(preview["candidates"][0]["kind"], "Word");
    }
}

#[test]
fn accepted_pack_phrase_endings_use_the_same_context_normalization() {
    let fixture = Fixture::new();
    fixture.install_lexicon(serde_json::json!({
        "format_version": 1,
        "language": "en",
        "phrase_endings": [
            ["Observe the SKY", ["after sunset"]],
            ["we’re hiking", ["before sunrise"]]
        ]
    }));
    for (seed, expected) in [
        ("observe the sky", "observe the sky after sunset"),
        ("Observe the sky", "Observe the sky after sunset"),
        ("  Observe  the  sky", "  Observe  the  sky after sunset"),
        ("we're hiking", "we're hiking before sunrise"),
        ("We’re hiking", "We’re hiking before sunrise"),
    ] {
        let preview = fixture.preview(seed);
        assert_first_page(&preview, expected, "Sentence");
    }
    let preview = fixture.preview("Observe\nthe sky");
    assert!(
        preview["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|candidate| !candidate["text"].as_str().unwrap().contains("after sunset"))
    );
}

#[test]
fn normalized_pack_context_collisions_cannot_replace_builtin_anchors() {
    let fixture = Fixture::new();
    fixture.install_lexicon(serde_json::json!({
        "format_version": 1,
        "language": "en",
        "word_layers": [{
            "id": "collision",
            "next_words": [["THANK YOU", ["unicorns"]]]
        }],
        "phrase_endings": [["THANK YOU", ["for magical unicorns"]]]
    }));
    let preview = fixture.preview("thank you ");
    assert_eq!(preview["candidates"][1]["text"], "thank you for");
    assert_first_page(&preview, "thank you very", "Word");
    assert_first_page(&preview, "thank you for your help.", "Sentence");
    for seed in ["thank you ", "thank you", "Thank You"] {
        let preview = fixture.preview(seed);
        assert!(
            preview["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .all(|candidate| !candidate["text"].as_str().unwrap().contains("unicorns")),
            "a later equivalent context displaced an earlier one: {preview}"
        );
    }
}
