//! Independent, authored examples for the optional Chinese family/home pack.
//! Every case initializes one private registry in a fresh subprocess. Merely
//! exposing a pack in the catalog must not activate it in the built-in fallback.
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

const PACK_ID: &str = "org.suzaku.zh-hans.family";
const CHILD_CASE: &str = "SUZAKU_FAMILY_CHINESE_CHILD_CASE";

const WORDS: &[(&str, &str)] = &[
    ("fu'mu", "父母"),
    ("ba'ba", "爸爸"),
    ("ma'ma", "妈妈"),
    ("ge'ge", "哥哥"),
    ("jie'jie", "姐姐"),
    ("di'di", "弟弟"),
    ("mei'mei", "妹妹"),
    ("ye'ye", "爷爷"),
    ("nai'nai", "奶奶"),
    ("wai'gong", "外公"),
    ("wai'po", "外婆"),
    ("tang'xiong'di", "堂兄弟"),
    ("biao'jie'mei", "表姐妹"),
    ("jia'ren'tuan'ju", "家人团聚"),
    ("jia'ting'he'zhao", "家庭合照"),
    ("bai'fang'qin'you", "拜访亲友"),
    ("qu'kan'wang'jia'ren", "去看望家人"),
    ("hui'qu'pei'pei'fu'mu", "回去陪陪父母"),
    ("yue'ge'tan'fang'shi'jian", "约个探访时间"),
    ("qin'you'lai'fang", "亲友来访"),
    ("jia'wu'lun'ban", "家务轮班"),
    ("jia'wu'fen'gong", "家务分工"),
    ("lun'liu'da'sao", "轮流打扫"),
    ("yi'qi'zheng'li'fang'jian", "一起整理房间"),
    ("zheng'li'yi'gui", "整理衣柜"),
    ("huan'xi'yi'wu", "换洗衣物"),
    ("zang'yi'lan", "脏衣篮"),
    ("xi'yi'dai", "洗衣袋"),
    ("die'hao'yi'fu", "叠好衣服"),
    ("fen'kai'fang'yi'fu", "分开放衣服"),
    ("shou'hao'liang'gan'de'yi'fu", "收好晾干的衣服"),
    ("chuang'dan'shou'na", "床单收纳"),
    ("mao'jin'shou'na", "毛巾收纳"),
    ("zheng'li'chu'wu'gui", "整理储物柜"),
    ("huan'ji'shou'na", "换季收纳"),
    ("qing'dian'jia'yong'wu'pin", "清点家用物品"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "父母",
        [
            "父母最近有空吗，我想回去看看。",
            "父母来访前，我们一起收拾一下房间。",
        ],
    ),
    (
        "家人团聚",
        [
            "家人团聚的时间先问问大家。",
            "家人团聚时我们一起拍张照片吧。",
        ],
    ),
    (
        "家庭合照",
        ["家庭合照留一份给大家吧。", "家庭合照拍好后，我再发给你。"],
    ),
    (
        "拜访亲友",
        ["拜访亲友之前先确认时间。", "拜访亲友回来后再跟你联系。"],
    ),
    (
        "去看望家人",
        [
            "去看望家人，我们先商量一下时间。",
            "去看望家人时，顺便带上那份照片。",
        ],
    ),
    (
        "回去陪陪父母",
        [
            "回去陪陪父母，晚一点再联系你。",
            "回去陪陪父母，时间确定后告诉你。",
        ],
    ),
    (
        "亲友来访",
        [
            "亲友来访的时间已经确认了吗？",
            "亲友来访前，我们先整理客厅。",
        ],
    ),
    (
        "家务分工",
        [
            "家务分工先一起商量，不用一个人全做。",
            "家务分工说好了，我们就分别开始。",
        ],
    ),
    (
        "轮流打扫",
        ["轮流打扫会轻松一些。", "轮流打扫，今天我先整理房间。"],
    ),
    (
        "一起整理房间",
        [
            "一起整理房间，把东西放回原处。",
            "一起整理房间，做完就可以休息了。",
        ],
    ),
    (
        "整理衣柜",
        [
            "整理衣柜时先留出常用的位置。",
            "整理衣柜，我们把换季的衣服单独放。",
        ],
    ),
    (
        "换洗衣物",
        [
            "换洗衣物放进篮子里了吗？",
            "换洗衣物先分开放，等会儿一起整理。",
        ],
    ),
    (
        "叠好衣服",
        [
            "叠好衣服再放进抽屉里。",
            "叠好衣服，留一套在容易拿到的位置。",
        ],
    ),
    (
        "收好晾干的衣服",
        [
            "收好晾干的衣服，别落在阳台上。",
            "收好晾干的衣服后，我们再整理衣柜。",
        ],
    ),
    (
        "换季收纳",
        [
            "换季收纳的时候记得贴好标签。",
            "换季收纳先分好类，再放进储物柜。",
        ],
    ),
    (
        "清点家用物品",
        [
            "清点家用物品，缺什么先记下来。",
            "清点家用物品之前，先看看储物柜。",
        ],
    ),
];

struct Fixture(PathBuf);

impl Fixture {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "suzaku-family-chinese-{}-{case}",
            std::process::id()
        ));
        // Cleanup ownership begins only after exclusive creation succeeds.
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> PackStore {
        PackStore::new(self.0.join("packs")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn in_fresh_process(name: &str, test: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        let fixture = Fixture::new(name);
        test(&fixture);
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fresh Chinese family case {name:?} failed ({:?}):\n{}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn family_pack() -> OfflinePack {
    packs::recommended()
        .into_iter()
        .find(|pack| pack.manifest.id == PACK_ID)
        .expect("Chinese family pack must be present in the optional catalog")
}

fn initialize_fixture(fixture: &Fixture, enabled: bool) {
    let base = lexicon::builtin("zh-Hans").unwrap();
    let original = serde_json::to_value(base).unwrap();
    assert_eq!(WORDS.len(), 36);
    assert_eq!(SENTENCES.len(), 16);
    for &(_, word) in WORDS {
        assert!(base.readings().iter().all(|entry| entry.text != word));
    }
    for &(context, _) in SENTENCES {
        assert!(base.continuations().all(|(old, _)| old != context));
    }

    let pack = family_pack();
    assert_eq!(pack.manifest.version, "1.0.0");
    assert_eq!(pack.manifest.language, "zh-Hans");
    assert_eq!(pack.manifest.topics, ["family", "home"]);
    assert_eq!(pack.manifest.license, "MIT");
    assert_eq!(pack.manifest.authors, ["Suzaku contributors"]);
    assert_eq!(pack.lexicon.readings().len(), 36);
    assert_eq!(pack.lexicon.continuations().count(), 16);
    assert_eq!(
        pack.lexicon
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        32
    );
    let store = fixture.store();
    assert!(store.list().unwrap().is_empty());
    assert!(!store.path().parent().unwrap().exists());
    // The data catalog alone neither installs nor modifies built-in resources.
    assert_eq!(serde_json::to_value(base).unwrap(), original);
    store.install(pack, false).unwrap();
    if !enabled {
        store.set_enabled(PACK_ID, false).unwrap();
    }
    assert_eq!(serde_json::to_value(base).unwrap(), original);
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    if enabled {
        assert_eq!(report.loaded, [PACK_ID]);
        assert!(report.disabled.is_empty());
    } else {
        assert!(report.loaded.is_empty());
        assert_eq!(report.disabled, [PACK_ID]);
    }
    let active = lexicon::active("zh-Hans").unwrap();
    assert_eq!(base.readings(), &active.readings()[..base.readings().len()]);
    assert_eq!(
        base.continuations().collect::<Vec<_>>(),
        active
            .continuations()
            .take(base.continuations().count())
            .collect::<Vec<_>>()
    );
    assert_eq!(serde_json::to_value(base).unwrap(), original);
    if !enabled {
        assert!(std::ptr::eq(base, active));
    }
}

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.to_owned(),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn reading_for(word: &str) -> &'static str {
    WORDS.iter().find(|(_, text)| *text == word).unwrap().0
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
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

fn first_page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = if kind == CandidateKind::Literal {
        ime.candidates()
            .iter()
            .position(|candidate| candidate.text == text && candidate.kind == kind)
            .unwrap_or_else(|| panic!("missing literal {text:?}: {:?}", ime.candidates()))
    } else {
        first_page_index(ime, text, kind)
    };
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
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn disabled_family_pack_does_not_change_builtin_candidates() {
    in_fresh_process(
        "disabled_family_pack_does_not_change_builtin_candidates",
        |fixture| {
            initialize_fixture(fixture, false);
            for &(reading, word) in WORDS {
                let seed = reading.replace('\'', " ");
                let mut ime = engine(&seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                assert_local_draft(&ime, &seed);
                assert!(ime.candidates().iter().all(|candidate| {
                    candidate.text != word || candidate.kind != CandidateKind::Word
                }));
                commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
            }
            for &(word, sentences) in SENTENCES {
                let ime = engine(word);
                for sentence in sentences {
                    assert!(
                        ime.candidates()
                            .iter()
                            .all(|candidate| candidate.text != sentence)
                    );
                }
            }
        },
    );
}

#[test]
fn family_words_are_on_page_one_in_all_four_spellings() {
    in_fresh_process(
        "family_words_are_on_page_one_in_all_four_spellings",
        |fixture| {
            initialize_fixture(fixture, true);
            for &(reading, word) in WORDS {
                for seed in spellings(reading) {
                    let mut ime = engine(&seed);
                    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                    assert_local_draft(&ime, &seed);
                    commit_once_and_undo(&mut ime, word, CandidateKind::Word);
                    commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        },
    );
}

#[test]
fn family_readings_offer_words_and_both_authored_sentences() {
    in_fresh_process(
        "family_readings_offer_words_and_both_authored_sentences",
        |fixture| {
            initialize_fixture(fixture, true);
            for &(word, sentences) in SENTENCES {
                for seed in spellings(reading_for(word)) {
                    let ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    first_page_index(&ime, word, CandidateKind::Word);
                    for sentence in sentences {
                        commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                    }
                }
            }
        },
    );
}

#[test]
fn unfinished_family_readings_keep_word_and_sentence_choices() {
    in_fresh_process(
        "unfinished_family_readings_keep_word_and_sentence_choices",
        |fixture| {
            initialize_fixture(fixture, true);
            for &(reading, word) in WORDS {
                for seed in spellings(&reading[..reading.len() - 1]) {
                    let ime = engine(&seed);
                    assert_local_draft(&ime, &seed);
                    first_page_index(&ime, word, CandidateKind::Word);
                }
            }
            for &(word, sentences) in SENTENCES {
                let reading = reading_for(word);
                for seed in spellings(&reading[..reading.len() - 1]) {
                    let ime = engine(&seed);
                    first_page_index(&ime, sentences[0], CandidateKind::Sentence);
                    for sentence in sentences {
                        assert!(
                            ime.candidates().iter().any(|candidate| {
                                candidate.text == sentence
                                    && candidate.kind == CandidateKind::Sentence
                            }),
                            "missing {sentence:?} for {seed:?}: {:?}",
                            ime.candidates()
                        );
                    }
                }
            }
        },
    );
}

#[test]
fn adopted_family_words_preserve_space_padding_without_committing() {
    in_fresh_process(
        "adopted_family_words_preserve_space_padding_without_committing",
        |fixture| {
            initialize_fixture(fixture, true);
            for &(word, sentences) in SENTENCES {
                for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                    for sentence in sentences {
                        let mut ime = engine(reading_for(word));
                        ime.select_candidate(first_page_index(&ime, word, CandidateKind::Word));
                        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
                        assert_eq!(adopted, word);
                        assert!(ime.snapshot().committed_text.is_empty());
                        // Host-style word adoption and Space, not a physical-key claim.
                        let seed = format!("{adopted}{padding}");
                        ime.seed(&seed);
                        assert_local_draft(&ime, &seed);
                        assert_eq!(ime.candidates()[0].text, seed);
                        let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                        commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                        commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                    }
                }
            }
        },
    );
}

#[test]
fn family_sentences_continue_after_adoption_and_more_pinyin() {
    in_fresh_process(
        "family_sentences_continue_after_adoption_and_more_pinyin",
        |fixture| {
            initialize_fixture(fixture, true);
            for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
                for padding in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
                    for (word, tail_reading, tail_word, sentence) in [
                        (
                            "家人团聚",
                            "de'shi'jian",
                            "的时间",
                            "家人团聚的时间先问问大家。",
                        ),
                        ("拜访亲友", "zhi'qian", "之前", "拜访亲友之前先确认时间。"),
                    ] {
                        let mut adopted_ime = engine(reading_for(word));
                        adopted_ime.select_candidate(first_page_index(
                            &adopted_ime,
                            word,
                            CandidateKind::Word,
                        ));
                        let adopted = adopted_ime
                            .selected_completion_text(true)
                            .unwrap()
                            .to_owned();
                        assert_eq!(adopted, word);
                        assert!(adopted_ime.snapshot().committed_text.is_empty());
                        for tail in
                            std::iter::once(tail_word.to_owned()).chain(spellings(tail_reading))
                        {
                            let seed = format!("{prefix}{adopted}{padding}{tail}");
                            let expected = format!(
                                "{prefix}{adopted}{padding}{}",
                                sentence.strip_prefix(word).unwrap()
                            );
                            let mut ime = engine(&seed);
                            assert_local_draft(&ime, &seed);
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                        for trailing in ["", " ", "  ", "\u{3000}"] {
                            let seed = format!("{prefix}{adopted}{padding}{tail_word}{trailing}");
                            let suffix = sentence
                                .strip_prefix(word)
                                .unwrap()
                                .strip_prefix(tail_word)
                                .unwrap();
                            let expected = format!("{seed}{suffix}");
                            let mut ime = engine(&seed);
                            assert_local_draft(&ime, &seed);
                            commit_once_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
                        }
                    }
                }
            }
        },
    );
}

struct UnavailableProvider(LlmProviderError);

impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "family-chinese-synthetic-error"
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

#[test]
fn family_candidates_survive_synthetic_provider_failures() {
    in_fresh_process(
        "family_candidates_survive_synthetic_provider_failures",
        |fixture| {
            initialize_fixture(fixture, true);
            // Typed synthetic failures, never a real model/network benchmark.
            for error in [
                LlmProviderError::NoLocalModel,
                LlmProviderError::Unavailable,
                LlmProviderError::Timeout,
                LlmProviderError::HttpStatus(503),
            ] {
                for (seed, word, sentence) in [
                    ("jia ren tuan ju", "家人团聚", "家人团聚的时间先问问大家。"),
                    (
                        "jia wu fen gon",
                        "家务分工",
                        "家务分工先一起商量，不用一个人全做。",
                    ),
                    (
                        "zheng li yi gui",
                        "整理衣柜",
                        "整理衣柜时先留出常用的位置。",
                    ),
                    ("huan ji shou n", "换季收纳", "换季收纳的时候记得贴好标签。"),
                ] {
                    let mut ime = engine("");
                    ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
                    ime.seed(seed);
                    assert_local_draft(&ime, seed);
                    first_page_index(&ime, word, CandidateKind::Word);
                    first_page_index(&ime, sentence, CandidateKind::Sentence);
                    let before = ime.candidates().to_vec();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    while ime.prediction_pending() && Instant::now() < deadline {
                        ime.poll_prediction();
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                    assert_eq!(ime.prediction_error(), Some(&error));
                    assert_eq!(ime.candidates(), before);
                    commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
        },
    );
}
