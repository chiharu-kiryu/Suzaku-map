//! Optional authored family/home expressions, not personal data or a benchmark.
//! Every activation scenario gets a fresh process and an exclusively owned store.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self,
        packs::{self, OfflinePack, PackStore},
    },
};

const FAMILY_ID: &str = "org.suzaku.en.family";
// Fixed forms and examples do not derive expectations from the bundled JSON.
const FORMS: &[&str] = &[
    "grandparent",
    "grandparents",
    "grandmother",
    "grandmothers",
    "grandfather",
    "grandfathers",
    "grandchild",
    "grandchildren",
    "grandson",
    "grandsons",
    "granddaughter",
    "granddaughters",
    "sibling",
    "siblings",
    "cousin",
    "cousins",
    "niece",
    "nieces",
    "nephew",
    "nephews",
    "relative",
    "relatives",
    "visitor",
    "visitors",
    "babysit",
    "babysits",
    "babysat",
    "babysitting",
    "babysitter",
    "babysitters",
    "household",
    "households",
    "chore",
    "chores",
    "laundry",
    "detergent",
    "detergents",
    "fabric",
    "fabrics",
    "hanger",
    "hangers",
    "wardrobe",
    "wardrobes",
    "hamper",
    "hampers",
    "washcloth",
    "washcloths",
    "bedsheet",
    "bedsheets",
    "pillowcase",
    "pillowcases",
    "duvet",
    "duvets",
    "fold",
    "folded",
    "folding",
    "tidy",
    "tidies",
    "tidied",
    "tidying",
];

const FAMILY: &[(&str, [(&str, &str); 2])] = &[
    (
        "our grandparents are coming",
        [
            ("over", "our grandparents are coming over this afternoon."),
            (
                "tomorrow",
                "our grandparents are coming tomorrow for a family visit.",
            ),
        ],
    ),
    (
        "i'll call my",
        [
            ("grandmother", "i'll call my grandmother after dinner."),
            ("grandfather", "i'll call my grandfather this evening."),
        ],
    ),
    (
        "my cousin is",
        [
            ("visiting", "my cousin is visiting us this weekend."),
            ("staying", "my cousin is staying with us tonight."),
        ],
    ),
    (
        "could you help your",
        [
            ("sister", "could you help your sister fold the laundry?"),
            ("brother", "could you help your brother tidy the room?"),
        ],
    ),
    (
        "let's share the household",
        [
            ("chores", "let's share the household chores this week."),
            ("tasks", "let's share the household tasks fairly."),
        ],
    ),
    (
        "can you take care of the",
        [
            (
                "laundry",
                "can you take care of the laundry while i tidy up?",
            ),
            ("bins", "can you take care of the bins this evening?"),
        ],
    ),
    (
        "i'll tidy the",
        [
            (
                "bedroom",
                "i'll tidy the bedroom before our visitors arrive.",
            ),
            ("living", "i'll tidy the living room after lunch."),
        ],
    ),
    (
        "could you fold the",
        [
            (
                "towels",
                "could you fold the towels while i hang the shirts?",
            ),
            (
                "bedsheets",
                "could you fold the bedsheets before putting them away?",
            ),
        ],
    ),
    (
        "please put the clean clothes",
        [
            ("away", "please put the clean clothes away in the wardrobe."),
            ("here", "please put the clean clothes here on the bed."),
        ],
    ),
    (
        "let's sort the laundry",
        [
            ("first", "let's sort the laundry first by color."),
            (
                "together",
                "let's sort the laundry together before washing.",
            ),
        ],
    ),
    (
        "where should i put the spare",
        [
            ("pillowcases", "where should i put the spare pillowcases?"),
            ("hangers", "where should i put the spare hangers?"),
        ],
    ),
    (
        "we need to change the",
        [
            ("bedsheets", "we need to change the bedsheets today."),
            (
                "pillowcases",
                "we need to change the pillowcases before our visitors arrive.",
            ),
        ],
    ),
    (
        "please hang these",
        [
            ("shirts", "please hang these shirts on the hangers."),
            ("jackets", "please hang these jackets in the wardrobe."),
        ],
    ),
    (
        "could you move the laundry",
        [
            (
                "hamper",
                "could you move the laundry hamper out of the doorway?",
            ),
            (
                "basket",
                "could you move the laundry basket next to the washing machine?",
            ),
        ],
    ),
    (
        "we're expecting family",
        [
            ("today", "we're expecting family today, so let's tidy up."),
            ("tomorrow", "we're expecting family tomorrow afternoon."),
        ],
    ),
    (
        "let's make room for our",
        [
            (
                "visitors",
                "let's make room for our visitors before they arrive.",
            ),
            (
                "relatives",
                "let's make room for our relatives this weekend.",
            ),
        ],
    ),
];

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-family-en-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn child(&self, mode: &str) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "family_pack_snapshot_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SUZAKU_FAMILY_EN_QA_MODE", mode)
            .env("SUZAKU_FAMILY_EN_QA_STORE", self.0.join("packs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "family child {mode}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn family_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == FAMILY_ID)
        .expect("the optional English family pack must be offered by the catalog")
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && candidate.text.starts_with(seed)
    }));
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn candidate_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn assert_page(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_local_draft(&ime, seed);
    candidate_index(&ime, word, CandidateKind::Word);
    candidate_index(&ime, sentence, CandidateKind::Sentence);
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = candidate_index(ime, text, kind);
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert!(second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

struct UnavailableProvider(LlmProviderError);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "family-pack-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

fn quality_workflows() {
    for word in FORMS {
        assert!(
            suzaku_map::languages::english::is_known_english_word(word),
            "unindexed authored form {word}"
        );
    }
    for (seed, word) in [
        ("granddaug", "granddaughter"),
        ("granddaug", "granddaughters"),
        ("grandchi", "grandchild"),
        ("grandchi", "grandchildren"),
        ("househol", "households"),
        ("hange", "hangers"),
        ("hamp", "hamper"),
    ] {
        let ime = engine(seed);
        assert_local_draft(&ime, seed);
        candidate_index(&ime, word, CandidateKind::Word);
    }

    for (context, branches) in FAMILY {
        let draft = format!("{context} ");
        let ime = engine(&draft);
        assert_local_draft(&ime, &draft);
        assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            let seed = format!("{context} {}", &next[..2]);
            assert_page(&seed, &word, sentence);
            commit_once_and_undo(&mut engine(&seed), &word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }

    for (seed, word, sentence) in [
        (
            "Our grandparents are coming ov",
            "Our grandparents are coming over",
            "Our grandparents are coming over this afternoon.",
        ),
        (
            "  could  you  fold  the  be",
            "  could  you  fold  the  bedsheets",
            "  could  you  fold  the  bedsheets before putting them away?",
        ),
        (
            "please hang these JA",
            "please hang these JACKETS",
            "please hang these JACKETS in the wardrobe.",
        ),
        (
            "I’ll tidy the be",
            "I’ll tidy the bedroom",
            "I’ll tidy the bedroom before our visitors arrive.",
        ),
        (
            "Let’s sort the laundry to",
            "Let’s sort the laundry together",
            "Let’s sort the laundry together before washing.",
        ),
        (
            "We’re expecting family to",
            "We’re expecting family today",
            "We’re expecting family today, so let's tidy up.",
        ),
    ] {
        assert_page(seed, word, sentence);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
    }

    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}could you move the laundry ha");
            let word = format!("{prefix}{separator}could you move the laundry hamper");
            let sentence =
                format!("{prefix}{separator}could you move the laundry hamper out of the doorway?");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            commit_once_and_undo(&mut engine(&seed), &sentence, CandidateKind::Sentence);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }

    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}please put the clean clothes aw");
    let word = format!("{prefix}please put the clean clothes away");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // Host draft replacement tests preservation, not physical adoption/undo.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}please put the clean clothes away  in"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}please put the clean clothes away  in the wardrobe."),
        CandidateKind::Sentence,
    );
    let continued = format!("{adopted}  in the wa");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    candidate_index(
        &ime,
        &format!("{prefix}please put the clean clothes away  in the wardrobe"),
        CandidateKind::Word,
    );
    let completed = format!("{prefix}please put the clean clothes away  in the wardrobe.");
    commit_once_and_undo(&mut ime, &completed, CandidateKind::Sentence);

    // Typed synthetic errors are not actual network/model quality evidence.
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (context, branches) in FAMILY.iter().step_by(4) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..2]);
            let word = format!("{context} {next}");
            let mut ime = engine(&seed);
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(&seed);
            assert_local_draft(&ime, &seed);
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), local);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn family_catalog_is_optional_authored_and_has_fixed_examples() {
    let pack = family_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "en");
    assert_eq!(pack.manifest.topics, ["family", "home"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    let layers = pack.lexicon.word_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].words, FORMS);
    assert_eq!(FORMS.len(), 60);
    assert_eq!(layers[0].next_words.len(), 16);
    assert_eq!(layers[0].sentences.len(), 32);
    assert!(pack.units() <= packs::MAX_PACK_UNITS);
    for (context, branches) in FAMILY {
        let values = layers[0]
            .next_words
            .iter()
            .find(|(key, _)| key == context)
            .unwrap();
        assert_eq!(values.1, branches.map(|(word, _)| word));
        for (_, sentence) in branches {
            assert!(layers[0].sentences.iter().any(|value| value == sentence));
        }
    }
    // Offering the package never mutates a registry or built-in resource.
    let fixture = Fixture::new();
    assert!(!fixture.store().path().exists());
    assert!(fixture.store().list().unwrap().is_empty());
    assert!(!fixture.store().path().exists());
}

#[test]
fn family_enabled_disabled_and_absent_snapshots_use_fresh_processes() {
    let pack = family_pack();
    let fixture = Fixture::new();
    fixture.child("absent");
    fixture.store().install(pack, false).unwrap();
    fixture.store().set_enabled(FAMILY_ID, false).unwrap();
    fixture.child("disabled");
    fixture.store().set_enabled(FAMILY_ID, true).unwrap();
    fixture.child("enabled");
    fixture.store().set_enabled(FAMILY_ID, false).unwrap();
    fixture.child("disabled");
}

#[test]
fn family_words_sentences_and_failures_coexist_with_all_recommended_packs() {
    family_pack();
    let fixture = Fixture::new();
    for pack in packs::recommended() {
        fixture.store().install(pack, false).unwrap();
    }
    fixture.child("quality");
}

#[test]
#[ignore = "subprocess-only immutable pack snapshot; invoked by the owning parent tests"]
fn family_pack_snapshot_child() {
    let mode = std::env::var("SUZAKU_FAMILY_EN_QA_MODE").unwrap();
    let path = PathBuf::from(std::env::var_os("SUZAKU_FAMILY_EN_QA_STORE").unwrap());
    assert!(path.is_absolute());
    assert_ne!(path, Path::new("/"));
    let store = PackStore::new(path).unwrap();
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    match mode.as_str() {
        "absent" | "disabled" => {
            assert!(report.loaded.is_empty());
            assert_eq!(
                serde_json::to_value(lexicon::active("en").unwrap().word_layers()).unwrap(),
                serde_json::to_value(lexicon::builtin("en").unwrap().word_layers()).unwrap()
            );
            for _ in 0..2 {
                assert!(!suzaku_map::languages::english::is_known_english_word(
                    "granddaughters"
                ));
                let seed = "our grandparents are coming ov";
                let ime = engine(seed);
                assert_local_draft(&ime, seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert!(ime.candidates().iter().all(|candidate| {
                    candidate.text != "our grandparents are coming over this afternoon."
                }));
                if mode == "disabled" {
                    // Updating the private registry cannot hot-swap this already
                    // initialized process; only the next child sees the change.
                    store.set_enabled(FAMILY_ID, true).unwrap();
                }
            }
        }
        "enabled" => {
            assert_eq!(report.loaded, [FAMILY_ID]);
            assert!(suzaku_map::languages::english::is_known_english_word(
                "granddaughters"
            ));
            assert_page(
                "our grandparents are coming ov",
                "our grandparents are coming over",
                "our grandparents are coming over this afternoon.",
            );
        }
        "quality" => {
            assert_eq!(report.loaded.len(), packs::recommended().len());
            assert!(report.loaded.iter().any(|id| id == FAMILY_ID));
            quality_workflows();
        }
        _ => panic!("unknown family snapshot case {mode}"),
    }
}
