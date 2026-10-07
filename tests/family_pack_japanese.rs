//! Fixed optional Japanese family-pack workflows, independent of pack JSON.
//! Each case owns a fresh process and private registry. No user installation,
//! desktop key injection, live provider or general morphology is exercised.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
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

const PACK_ID: &str = "org.suzaku.ja.family";
const ACTIVE_CASE: &str = "family_pack_active_startup_workflows";
const WORDS: &[(&str, &str, &str)] = &[
    ("kazoku", "かぞく", "家族"),
    ("chichi", "ちち", "父"),
    ("haha", "はは", "母"),
    ("ani", "あに", "兄"),
    ("ane", "あね", "姉"),
    ("otouto", "おとうと", "弟"),
    ("imouto", "いもうと", "妹"),
    ("sofu", "そふ", "祖父"),
    ("sobo", "そぼ", "祖母"),
    ("shinseki", "しんせき", "親戚"),
    ("houmon", "ほうもん", "訪問"),
    ("jikka", "じっか", "実家"),
    ("genkan", "げんかん", "玄関"),
    ("daidokoro", "だいどころ", "台所"),
    ("ima", "いま", "居間"),
    ("shokutaku", "しょくたく", "食卓"),
    ("souji", "そうじ", "掃除"),
    ("sentaku", "せんたく", "洗濯"),
    ("seiri", "せいり", "整理"),
    ("gomibako", "ごみばこ", "ごみ箱"),
    ("mado", "まど", "窓"),
    ("tobira", "とびら", "扉"),
    ("futon", "ふとん", "布団"),
    ("makura", "まくら", "枕"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "家族",
        [
            "家族で一緒に出かけませんか。",
            "家族の予定を確認しましょう。",
        ],
    ),
    ("父", ["父に後で電話します。", "父と週末に会う予定です。"]),
    (
        "母",
        [
            "母によろしく伝えてください。",
            "母と一緒に買い物に行きます。",
        ],
    ),
    (
        "祖父",
        ["祖父に会いに行きたいです。", "祖父とゆっくり話したいです。"],
    ),
    (
        "祖母",
        ["祖母に後で連絡します。", "祖母と一緒に写真を見たいです。"],
    ),
    (
        "親戚",
        ["親戚に会う予定です。", "親戚が来るので、部屋を片付けます。"],
    ),
    (
        "訪問",
        ["訪問する時間を教えてください。", "訪問する前に連絡します。"],
    ),
    (
        "実家",
        [
            "実家に週末帰る予定です。",
            "実家で家族とゆっくり過ごします。",
        ],
    ),
    (
        "玄関",
        ["玄関で待っています。", "玄関の靴をそろえてください。"],
    ),
    (
        "台所",
        [
            "台所で一緒に準備しましょう。",
            "台所を使ったら片付けましょう。",
        ],
    ),
    (
        "居間",
        [
            "居間で一緒に話しましょう。",
            "居間の窓を開けてもいいですか。",
        ],
    ),
    (
        "掃除",
        ["掃除を一緒にしませんか。", "掃除が終わったら休みましょう。"],
    ),
];

fn fresh(name: &str, body: fn()) {
    if std::env::var("SUZAKU_FAMILY_JA_CASE").ok().as_deref() == Some(name) {
        body();
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("SUZAKU_FAMILY_JA_CASE", name)
        .env_remove("SUZAKU_LEXICON_DIR")
        .env_remove("SUZAKU_IME_CONFIG")
        .env_remove("SUZAKU_LINUX_IME_SOCKET")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fresh {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn family() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("the optional family collection is discoverable by its stable ID")
}

fn active_workflow(body: fn()) {
    assert_eq!(std::env::var("SUZAKU_FAMILY_JA_CASE").unwrap(), ACTIVE_CASE);
    body();
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("suzaku-ja-family-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
    fn activate() -> Self {
        let fixture = Self::new();
        let store = fixture.store();
        store.install(family(), false).unwrap();
        let rail = packs::recommended()
            .into_iter()
            .find(|pack| pack.manifest.id == "org.suzaku.ja.rail")
            .unwrap();
        store.install(rail, false).unwrap();
        let report = packs::initialize(&store).unwrap();
        assert_eq!(report.loaded.len(), 2);
        assert!(report.loaded.iter().any(|id| id == PACK_ID));
        assert!(report.errors.is_empty(), "{report:?}");
        let builtin = lexicon::builtin("ja").unwrap();
        let active = lexicon::active("ja").unwrap();
        assert_eq!(
            builtin.readings(),
            &active.readings()[..builtin.readings().len()]
        );
        assert_eq!(
            builtin.continuations().collect::<Vec<_>>(),
            active
                .continuations()
                .take(builtin.continuations().count())
                .collect::<Vec<_>>()
        );
        fixture
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn blank_engine() -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = blank_engine();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}
fn check(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed && candidate.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind, first_page: bool) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if first_page {
        assert!(
            index < PAGE_SIZE,
            "not first page {text:?}, seed={:?}: {:?}",
            ime.snapshot().seed_text,
            ime.candidates()
        );
    }
    index
}
fn commit(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind, first_page: bool) {
    let chosen = index(ime, text, kind, first_page);
    ime.select_candidate(chosen);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok);
    assert!(duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn family_catalog_is_optional_and_does_not_change_uninstalled_builtin() {
    fresh(
        "family_catalog_is_optional_and_does_not_change_uninstalled_builtin",
        || {
            let fixture = Fixture::new();
            let pack = family();
            assert_eq!(pack.manifest.version, "1.0.0");
            assert_eq!(pack.manifest.language, "ja");
            assert_eq!(pack.manifest.topics, ["family", "home"]);
            assert_eq!(pack.manifest.license, "MIT");
            assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
            assert_eq!(pack.lexicon.readings().len(), 24);
            assert_eq!(pack.lexicon.continuations().count(), 12);
            assert_eq!(
                pack.lexicon
                    .continuations()
                    .map(|(_, values)| values.len())
                    .sum::<usize>(),
                24
            );
            let builtin = lexicon::builtin("ja").unwrap();
            assert_eq!(builtin.readings().len(), 202);
            assert_eq!(builtin.continuations().count(), 102);
            assert!(
                WORDS
                    .iter()
                    .all(|(_, _, word)| builtin.readings().iter().all(|entry| entry.text != *word))
            );
            let store = fixture.store();
            assert!(!store.path().exists());
            let report = packs::initialize(&store).unwrap();
            assert!(report.loaded.is_empty() && report.errors.is_empty());
            assert!(std::ptr::eq(lexicon::active("ja").unwrap(), builtin));
            let mut ime = engine("kazoku");
            check(&ime, "kazoku");
            assert!(
                ime.candidates()
                    .iter()
                    .all(|candidate| candidate.text != "家族")
            );
            commit(&mut ime, "kazoku", CandidateKind::Literal, false);
            assert!(!store.path().exists());
        },
    );
}

#[test]
fn family_pack_active_startup_workflows() {
    fresh(ACTIVE_CASE, || {
        let _fixture = Fixture::activate();
        family_words_support_complete_romaji_case_and_kana();
        family_readings_offer_words_and_both_authored_sentences();
        family_specific_partial_readings_keep_authored_choices();
        family_homophones_and_broad_prefixes_preserve_old_priority_and_paging();
        family_adoption_space_restore_and_particles_remain_editable();
        family_separators_and_unknown_line_boundaries_preserve_raw_text();
        family_pack_remains_usable_during_synthetic_provider_failures();
    });
}

fn family_words_support_complete_romaji_case_and_kana() {
    active_workflow(|| {
        assert_eq!(WORDS.len(), 24);
        for &(romaji, kana, word) in WORDS {
            for seed in [
                romaji.to_owned(),
                romaji.to_ascii_uppercase(),
                kana.to_owned(),
            ] {
                let mut ime = engine(&seed);
                check(&ime, &seed);
                commit(&mut ime, word, CandidateKind::Word, true);
                commit(&mut engine(&seed), &seed, CandidateKind::Literal, false);
            }
        }
    });
}

fn family_readings_offer_words_and_both_authored_sentences() {
    active_workflow(|| {
        assert_eq!(SENTENCES.len(), 12);
        for &(word, sentences) in SENTENCES {
            let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
            for seed in [
                romaji.to_owned(),
                romaji.to_ascii_uppercase(),
                kana.to_owned(),
            ] {
                let ime = engine(&seed);
                check(&ime, &seed);
                index(&ime, word, CandidateKind::Word, true);
                for sentence in sentences {
                    commit(&mut engine(&seed), sentence, CandidateKind::Sentence, true);
                }
            }
        }
    });
}

fn family_specific_partial_readings_keep_authored_choices() {
    active_workflow(|| {
        for (seed, word) in [
            ("kazok", "家族"),
            ("chich", "父"),
            ("hah", "母"),
            ("sof", "祖父"),
            ("sob", "祖母"),
            ("shinsek", "親戚"),
            ("houmo", "訪問"),
            ("jikk", "実家"),
            ("genka", "玄関"),
            ("daidokor", "台所"),
            ("souj", "掃除"),
        ] {
            let ime = engine(seed);
            check(&ime, seed);
            index(&ime, word, CandidateKind::Word, true);
            let &(_, sentences) = SENTENCES.iter().find(|(text, _)| *text == word).unwrap();
            for sentence in sentences {
                commit(&mut engine(seed), sentence, CandidateKind::Sentence, true);
            }
            commit(&mut engine(seed), seed, CandidateKind::Literal, false);
        }
    });
}

fn family_homophones_and_broad_prefixes_preserve_old_priority_and_paging() {
    active_workflow(|| {
        for seed in ["sentaku", "SENTAKU", "せんたく"] {
            let sentaku = engine(seed);
            check(&sentaku, seed);
            assert_eq!(sentaku.candidates()[0].text, "選択");
            assert!(
                index(&sentaku, "選択", CandidateKind::Word, true)
                    < index(&sentaku, "洗濯", CandidateKind::Word, false)
            );
            index(
                &sentaku,
                "選択に迷ったら相談してください。",
                CandidateKind::Sentence,
                true,
            );
            index(
                &sentaku,
                "選択する前に比べてみましょう。",
                CandidateKind::Sentence,
                true,
            );
            commit(&mut engine(seed), "洗濯", CandidateKind::Word, false);
        }
        let ma = engine("ま");
        check(&ma, "ま");
        let wait = index(&ma, "待ってください", CandidateKind::Word, true);
        let tomorrow = index(&ma, "また明日", CandidateKind::Word, true);
        let unsure = index(&ma, "迷う", CandidateKind::Word, false);
        let window = index(&ma, "窓", CandidateKind::Word, false);
        assert!(wait < tomorrow && tomorrow < unsure && unsure < window);
        assert!((PAGE_SIZE..12).contains(&window));
        commit(&mut engine("ま"), "窓", CandidateKind::Word, false);
        commit(&mut engine("ま"), "ま", CandidateKind::Literal, false);
        let rail = engine("kaisatsu");
        index(&rail, "改札", CandidateKind::Word, true);
        commit(
            &mut engine("kaisatsu"),
            "改札はどこですか。",
            CandidateKind::Sentence,
            true,
        );
    });
}

fn family_adoption_space_restore_and_particles_remain_editable() {
    active_workflow(|| {
        for (reading, word, particle, sentence) in [
            ("kazoku", "家族", "de", "家族で一緒に出かけませんか。"),
            ("chichi", "父", "ni", "父に後で電話します。"),
            ("haha", "母", "NI", "母によろしく伝えてください。"),
            ("sofu", "祖父", "ni", "祖父に会いに行きたいです。"),
            ("sobo", "祖母", "ni", "祖母に後で連絡します。"),
            ("shinseki", "親戚", "に", "親戚に会う予定です。"),
            ("houmon", "訪問", "suru", "訪問する時間を教えてください。"),
            ("jikka", "実家", "ni", "実家に週末帰る予定です。"),
            ("genkan", "玄関", "de", "玄関で待っています。"),
            ("daidokoro", "台所", "de", "台所で一緒に準備しましょう。"),
            ("ima", "居間", "de", "居間で一緒に話しましょう。"),
            ("souji", "掃除", "wo", "掃除を一緒にしませんか。"),
        ] {
            let mut ime = engine(reading);
            let chosen = index(&ime, word, CandidateKind::Word, true);
            ime.select_candidate(chosen);
            assert_eq!(ime.selected_completion_text(true), Some(word));
            assert!(ime.snapshot().committed_text.is_empty());
            // Engine-level editable host updates: Space is a preserved reading
            // separator, not an implicit commit. No physical IBus event claim.
            let spaced = format!("{word} ");
            ime.seed(&spaced);
            check(&ime, &spaced);
            index(&ime, sentence, CandidateKind::Sentence, true);
            ime.seed(reading);
            check(&ime, reading);
            index(&ime, word, CandidateKind::Word, true);
            let continued = format!("{word} {particle}");
            ime.seed(&continued);
            check(&ime, &continued);
            commit(&mut ime, sentence, CandidateKind::Sentence, true);
            commit(
                &mut engine(&continued),
                &continued,
                CandidateKind::Literal,
                false,
            );
        }
    });
}

fn family_separators_and_unknown_line_boundaries_preserve_raw_text() {
    active_workflow(|| {
        for (seed, word, sentence) in [
            ("  ka zoku  ", "家族", "家族で一緒に出かけませんか。"),
            ("ち ち", "父", "父に後で電話します。"),
            (
                "\tDAI\tDOKORO\u{3000}",
                "台所",
                "台所で一緒に準備しましょう。",
            ),
            ("so\u{3000}fu", "祖父", "祖父に会いに行きたいです。"),
            ("shin'seki", "親戚", "親戚に会う予定です。"),
            ("SHIN’SEKI", "親戚", "親戚が来るので、部屋を片付けます。"),
            ("gen kan", "玄関", "玄関で待っています。"),
        ] {
            let ime = engine(seed);
            check(&ime, seed);
            index(&ime, word, CandidateKind::Word, true);
            commit(&mut engine(seed), sentence, CandidateKind::Sentence, true);
            commit(&mut engine(seed), seed, CandidateKind::Literal, false);
        }
        let mut seeds = vec![
            "xyzkazoku".to_owned(),
            "🙂家族で".into(),
            "https://souji".into(),
            "父nixyz".into(),
            "台所をで".into(),
        ];
        for boundary in [
            "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            for (head, tail) in [("ka", "zoku"), ("家族", "de"), ("dai", "dokoro")] {
                seeds.push(format!("{head}{boundary}{tail}"));
            }
        }
        for seed in seeds {
            let mut ime = engine(&seed);
            check(&ime, &seed);
            for candidate in ime.candidates() {
                for &(_, sentences) in SENTENCES {
                    for sentence in sentences {
                        assert!(
                            !candidate.text.ends_with(sentence),
                            "manufactured {sentence:?} from {seed:?}: {:?}",
                            ime.candidates()
                        );
                    }
                }
                for boundary in [
                    "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
                ] {
                    if seed.contains(boundary) {
                        assert!(candidate.text.contains(boundary));
                    }
                }
            }
            commit(&mut ime, &seed, CandidateKind::Literal, false);
        }
    });
}

struct Unavailable(LlmProviderError);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "family-ja-synthetic-failure"
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
fn family_pack_remains_usable_during_synthetic_provider_failures() {
    active_workflow(|| {
        for error in [
            LlmProviderError::NoLocalModel,
            LlmProviderError::Unavailable,
            LlmProviderError::Timeout,
            LlmProviderError::HttpStatus(503),
        ] {
            for (seed, word, sentence) in [
                ("kazoku", "家族", "家族で一緒に出かけませんか。"),
                ("daidokoro", "台所", "台所で一緒に準備しましょう。"),
                ("souji", "掃除", "掃除を一緒にしませんか。"),
            ] {
                let mut ime = blank_engine();
                ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
                ime.seed(seed);
                check(&ime, seed);
                index(&ime, word, CandidateKind::Word, true);
                index(&ime, sentence, CandidateKind::Sentence, true);
                let local = ime.candidates().to_vec();
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(ime.prediction_error(), Some(&error));
                assert_eq!(ime.candidates(), local);
                check(&ime, seed);
                commit(&mut ime, sentence, CandidateKind::Sentence, true);
            }
        }
    });
}
