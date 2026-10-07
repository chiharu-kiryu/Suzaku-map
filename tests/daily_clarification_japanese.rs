//! Fixed, authored Japanese clarification and explanation fallback examples.
//! Expectations are independent of production JSON; no live model, user data,
//! desktop key injection or general morphological analysis is exercised.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{EntryKind, builtin},
};

const WORDS: &[(&str, &str, &str)] = &[
    ("setsumei", "せつめい", "説明"),
    ("shitsumon", "しつもん", "質問"),
    ("imi", "いみ", "意味"),
    ("youten", "ようてん", "要点"),
    ("gutairei", "ぐたいれい", "具体例"),
    ("reibun", "れいぶん", "例文"),
    ("naiyou", "ないよう", "内容"),
    ("haikei", "はいけい", "背景"),
    ("hosoku", "ほそく", "補足"),
    ("rikai", "りかい", "理解"),
    ("gokai", "ごかい", "誤解"),
    ("saikakunin", "さいかくにん", "再確認"),
    ("hosokusetsumei", "ほそくせつめい", "補足説明"),
    ("youshi", "ようし", "要旨"),
    ("youyaku", "ようやく", "要約"),
    ("jouhou", "じょうほう", "情報"),
    ("jijitsu", "じじつ", "事実"),
    ("junjo", "じゅんじょ", "順序"),
    ("kankei", "かんけい", "関係"),
    ("shousai", "しょうさい", "詳細"),
    ("taishou", "たいしょう", "対象"),
    ("yougo", "ようご", "用語"),
    ("tejun", "てじゅん", "手順"),
    ("ketsuron", "けつろん", "結論"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "説明",
        [
            "説明をもう一度お願いします。",
            "説明を聞いてから確認します。",
        ],
    ),
    (
        "質問",
        ["質問してもいいですか。", "質問があれば教えてください。"],
    ),
    (
        "意味",
        [
            "意味を教えてもらえますか。",
            "意味が分かったら、もう一度試します。",
        ],
    ),
    (
        "要点",
        [
            "要点を短く教えてください。",
            "要点を確認してから進めましょう。",
        ],
    ),
    (
        "具体例",
        [
            "具体例を一つ教えてください。",
            "具体例があると分かりやすいです。",
        ],
    ),
    (
        "例文",
        [
            "例文を見せてもらえますか。",
            "例文を使って説明してください。",
        ],
    ),
    (
        "内容",
        [
            "内容をもう一度確認しましょう。",
            "内容が違っていたら教えてください。",
        ],
    ),
    (
        "背景",
        ["背景を少し教えてください。", "背景を聞いてから考えます。"],
    ),
    (
        "補足",
        [
            "補足があれば教えてください。",
            "補足してもらえると助かります。",
        ],
    ),
    (
        "理解",
        [
            "理解できたか確認したいです。",
            "理解するまで一緒に考えましょう。",
        ],
    ),
    (
        "誤解",
        [
            "誤解があったら教えてください。",
            "誤解しないように確認します。",
        ],
    ),
    (
        "再確認",
        ["再確認してから連絡します。", "再確認をお願いできますか。"],
    ),
];

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
fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
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
            "not page one {text:?}, seed={:?}: {:?}",
            ime.snapshot().seed_text,
            ime.candidates()
        );
    }
    index
}
fn commit_once_and_undo(
    ime: &mut XRTabletImeEngine,
    text: &str,
    kind: CandidateKind,
    first_page: bool,
) {
    let selected = index(ime, text, kind, first_page);
    ime.select_candidate(selected);
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
fn clarification_preserves_all_previous_readings_contexts_and_sentence_order() {
    // Captured from the full pre-expansion fallback, never regenerated from
    // new data to disguise changes in old wording, kinds or homophone order.
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 178);
    let mut readings = 0xcbf29ce484222325_u64;
    for entry in ja.readings().iter().take(178) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            readings = (readings ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(readings, 0x91c5ef8f578fa5fe);
    assert!(ja.continuations().count() >= 90);
    let mut contexts = 0xcbf29ce484222325_u64;
    for (context, sentences) in ja.continuations().take(90) {
        for text in std::iter::once(context).chain(sentences.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                contexts = (contexts ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        contexts = (contexts ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(contexts, 0x3a63c247ce55752b);
}

#[test]
fn clarification_words_support_complete_romaji_case_and_kana() {
    assert_eq!(WORDS.len(), 24);
    for &(romaji, kana, word) in WORDS {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let mut ime = engine(&seed);
            check_draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word, true);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal, false);
        }
    }
}

#[test]
fn complete_clarification_readings_offer_word_and_both_sentence_choices() {
    assert_eq!(SENTENCES.len(), 12);
    for &(word, sentences) in SENTENCES {
        let &(romaji, kana, _) = WORDS.iter().find(|(_, _, text)| *text == word).unwrap();
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
        ] {
            let ime = engine(&seed);
            check_draft(&ime, &seed);
            index(&ime, word, CandidateKind::Word, true);
            for sentence in sentences {
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence, true);
            }
        }
    }
}

#[test]
fn specific_partial_clarification_readings_keep_words_and_sentence_choices() {
    // Fixed multi-kana prefixes, not a promise that all one-key or ambiguous
    // prefixes displace older completions. Broad じゅん is tested separately.
    for (seed, word) in [
        ("setsume", "説明"),
        ("shitsumo", "質問"),
        ("youte", "要点"),
        ("gutaire", "具体例"),
        ("reibu", "例文"),
        ("naiyo", "内容"),
        ("haike", "背景"),
        ("hosok", "補足"),
        ("rika", "理解"),
        ("goka", "誤解"),
        ("saikakuni", "再確認"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        let &(_, sentences) = SENTENCES.iter().find(|(text, _)| *text == word).unwrap();
        for sentence in sentences {
            commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        }
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn broad_jun_prefix_preserves_old_priority_and_selectable_new_word() {
    let mut ime = engine("じゅん");
    check_draft(&ime, "じゅん");
    assert_eq!(ime.candidates()[0].text, "じゅん");
    let preparation = index(&ime, "準備", CandidateKind::Word, true);
    let preparing = index(&ime, "準備中", CandidateKind::Word, true);
    let order = index(&ime, "順序", CandidateKind::Word, false);
    assert!(preparation < preparing && preparing < order);
    // The pre-expansion snapshot already fills page one with the two old
    // words and their continuations. Preserve their priority while requiring
    // bounded new-word reachability; do not guess an unobserved page index.
    assert!(order < 12);
    index(
        &ime,
        "準備ができたら連絡します。",
        CandidateKind::Sentence,
        true,
    );
    index(
        &ime,
        "準備を手伝ってもらえますか。",
        CandidateKind::Sentence,
        true,
    );
    ime.select_candidate(order);
    assert_eq!(ime.selected_completion_text(true), Some("順序"));
    assert!(ime.snapshot().committed_text.is_empty());
    ime.seed("順序");
    check_draft(&ime, "順序");
    ime.seed("じゅん");
    check_draft(&ime, "じゅん");
    commit_once_and_undo(&mut ime, "順序", CandidateKind::Word, false);
    commit_once_and_undo(
        &mut engine("じゅん"),
        "じゅん",
        CandidateKind::Literal,
        false,
    );
}

#[test]
fn horizontal_separators_preserve_raw_clarification_spelling() {
    for (seed, word, sentence) in [
        ("setsu mei", "説明", "説明をもう一度お願いします。"),
        ("SHITSU\u{3000}MON", "質問", "質問してもいいですか。"),
        ("しつ もん", "質問", "質問があれば教えてください。"),
        ("  ho soku\t", "補足", "補足があれば教えてください。"),
        (
            "\tnai\tyou\u{3000}",
            "内容",
            "内容をもう一度確認しましょう。",
        ),
        ("shitsumon'", "質問", "質問してもいいですか。"),
        ("SHITSUMON’", "質問", "質問があれば教えてください。"),
        ("you ten", "要点", "要点を短く教えてください。"),
        ("れい ぶん", "例文", "例文を見せてもらえますか。"),
        ("sai kaku nin", "再確認", "再確認してから連絡します。"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word, true);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal, false);
    }
}

#[test]
fn clarification_adoption_space_restore_and_particle_progress_stay_editable() {
    for (reading, word, particle, sentence) in [
        ("setsumei", "説明", "wo", "説明をもう一度お願いします。"),
        ("shitsumon", "質問", "が", "質問があれば教えてください。"),
        ("imi", "意味", "WO", "意味を教えてもらえますか。"),
        ("youten", "要点", "wo", "要点を短く教えてください。"),
        ("gutairei", "具体例", "wo", "具体例を一つ教えてください。"),
        ("reibun", "例文", "wo", "例文を見せてもらえますか。"),
        ("naiyou", "内容", "WO", "内容をもう一度確認しましょう。"),
        ("haikei", "背景", "wo", "背景を少し教えてください。"),
        ("hosoku", "補足", "ga", "補足があれば教えてください。"),
        ("rikai", "理解", "deki", "理解できたか確認したいです。"),
        ("gokai", "誤解", "GA", "誤解があったら教えてください。"),
        (
            "saikakunin",
            "再確認",
            "shite",
            "再確認してから連絡します。",
        ),
    ] {
        let mut ime = engine(reading);
        let selected = index(&ime, word, CandidateKind::Word, true);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level host draft replacement: Space remains a reading
        // separator, not a physical key event or implicit host commit.
        let spaced = format!("{word} ");
        ime.seed(&spaced);
        check_draft(&ime, &spaced);
        index(&ime, sentence, CandidateKind::Sentence, true);
        ime.seed(reading);
        check_draft(&ime, reading);
        index(&ime, word, CandidateKind::Word, true);
        let continued = format!("{word} {particle}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence, true);
        commit_once_and_undo(
            &mut engine(&continued),
            &continued,
            CandidateKind::Literal,
            false,
        );
    }
}

#[test]
fn clarification_never_discards_unknown_text_or_joins_line_boundaries() {
    let mut seeds = vec![
        "xyzsetsumei".to_owned(),
        "🙂説明を".into(),
        "https://shitsumon".into(),
        "質問gaxyz".into(),
        "意味をが".into(),
        "再確認して🙂".into(),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [("setsu", "mei"), ("説明", "wo"), ("sai", "kakunin")] {
            seeds.push(format!("{head}{boundary}{tail}"));
        }
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        check_draft(&ime, &seed);
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
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal, false);
    }
}

struct Unavailable(LlmProviderError);
impl LlmCompletionProvider for Unavailable {
    fn provider_id(&self) -> &str {
        "ja-clarification-synthetic-error"
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
fn clarification_candidates_survive_typed_synthetic_provider_failures() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (seed, word, sentence) in [
            ("setsumei", "説明", "説明をもう一度お願いします。"),
            ("shitsumon", "質問", "質問してもいいですか。"),
            ("saikakunin", "再確認", "再確認してから連絡します。"),
        ] {
            let mut ime = blank_engine();
            ime.configure_prediction(Some(Arc::new(Unavailable(error.clone()))));
            ime.seed(seed);
            check_draft(&ime, seed);
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
            check_draft(&ime, seed);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence, true);
        }
    }
}
