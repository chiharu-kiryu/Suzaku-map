use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};

struct ControlledProvider {
    requests: mpsc::Sender<LlmCompletionRequest>,
    replies: Mutex<mpsc::Receiver<Vec<LlmCompletion>>>,
}
impl LlmCompletionProvider for ControlledProvider {
    fn provider_id(&self) -> &str {
        "controlled-test"
    }
    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        self.requests.send(request.clone()).unwrap();
        self.replies
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_default()
    }
}
fn controlled() -> (
    XRTabletImeEngine,
    mpsc::Receiver<LlmCompletionRequest>,
    mpsc::Sender<Vec<LlmCompletion>>,
) {
    controlled_with_config(EngineConfig::default())
}

fn controlled_with_config(
    config: EngineConfig,
) -> (
    XRTabletImeEngine,
    mpsc::Receiver<LlmCompletionRequest>,
    mpsc::Sender<Vec<LlmCompletion>>,
) {
    let (requests_tx, requests) = mpsc::channel();
    let (replies, replies_rx) = mpsc::channel();
    let mut engine = XRTabletImeEngine::new(config);
    engine.configure_prediction(Some(Arc::new(ControlledProvider {
        requests: requests_tx,
        replies: Mutex::new(replies_rx),
    })));
    (engine, requests, replies)
}
fn answer(text: &str) -> Vec<LlmCompletion> {
    vec![LlmCompletion {
        text: text.into(),
        score_bias: 1.0,
        kind: None,
    }]
}
fn settle(engine: &mut XRTabletImeEngine) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.prediction_pending() && Instant::now() < deadline {
        engine.poll_prediction();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!engine.prediction_pending(), "prediction did not settle");
}

#[test]
fn relative_navigation_at_the_first_row_keeps_prediction_and_spelling_unselected() {
    for mixed in [false, true] {
        for (language, seed, reply) in [
            ("en", "hel", "hello from a boundary test"),
            ("zh-Hans", "nihao", "你好，今天过得怎么样？"),
        ] {
            let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
                default_language: language.into(),
                ..Default::default()
            });
            if mixed {
                engine.enable_ibus_candidate_mix();
            }
            let before = engine.seed(seed);
            requests.recv_timeout(Duration::from_secs(2)).unwrap();
            let candidates = engine.candidates().to_vec();
            for delta in [-1, -2, isize::MIN, 0] {
                assert_eq!(engine.move_selection(delta), before);
                assert_eq!(engine.candidates(), candidates);
                assert_eq!(engine.selected_completion_text(true), None);
                assert_eq!(engine.prediction_status(), PredictionStatus::Pending);
            }
            replies.send(answer(reply)).unwrap();
            settle(&mut engine);
            assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|candidate| candidate.text == reply)
            );
            assert!(requests.try_recv().is_err());
            assert_eq!(engine.snapshot().seed_text, seed);
            assert!(engine.snapshot().committed_text.is_empty());
        }
    }
}

#[test]
fn relative_navigation_handles_empty_single_and_last_rows_without_creating_a_choice() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.enable_ibus_candidate_mix();
    for seed in [String::new(), "字".repeat(257)] {
        let before = engine.seed(&seed);
        assert!(engine.candidates().len() <= 1);
        for delta in [isize::MIN, -1, 0, 1, isize::MAX] {
            assert_eq!(engine.move_selection(delta), before);
            assert_eq!(engine.selected_completion_text(true), None);
        }
    }
    engine.seed("hel");
    let last = engine.candidates().len() - 1;
    assert!(last > 1);
    let before = engine.move_selection(isize::MAX);
    assert_eq!(before.selected_index, last);
    let text = engine.selected_completion_text(true).unwrap().to_owned();
    for delta in [0, 1, 2, isize::MAX] {
        assert_eq!(engine.move_selection(delta), before);
        assert_eq!(engine.selected_completion_text(true), Some(text.as_str()));
    }
    assert_eq!(engine.move_selection(isize::MIN).selected_index, 0);
}

#[test]
fn explicitly_selecting_the_current_row_still_locks_a_real_choice() {
    let (mut engine, requests, replies) = controlled();
    engine.enable_ibus_candidate_mix();
    engine.seed("hel");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    let candidates = engine.candidates().to_vec();
    engine.select_candidate(0);
    assert_eq!(engine.selected_completion_text(true), Some("hel"));
    assert!(!engine.prediction_pending());
    replies.send(answer("hello from a late model")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates(), candidates);
}

#[test]
fn learned_model_choices_do_not_replay_text_or_accumulate_bonuses_on_refresh() {
    use suzaku_map::ime::candidate_mix::CandidateKind;
    let (mut engine, requests, replies) = controlled();
    engine.enable_ibus_candidate_mix();
    let completion = || {
        vec![LlmCompletion {
            text: "helioseismology".into(),
            kind: Some(CandidateKind::Word),
            score_bias: 1.0,
        }]
    };
    engine.seed("hel");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    replies.send(completion()).unwrap();
    settle(&mut engine);
    let index = engine
        .candidates()
        .iter()
        .position(|c| c.text == "helioseismology")
        .unwrap();
    engine.select_candidate(index);
    assert!(engine.commit(CommitOptions { force: true }).ok);
    assert_eq!(engine.preference_count(), 1);
    for _ in 0..3 {
        engine.clear_session_context();
        engine.seed("hel");
        assert!(
            !engine
                .candidates()
                .iter()
                .any(|c| c.text == "helioseismology"),
            "cache must not become a text-generation source"
        );
        requests.recv_timeout(Duration::from_secs(2)).unwrap();
        replies.send(completion()).unwrap();
        settle(&mut engine);
        let word = engine
            .candidates()
            .iter()
            .find(|c| c.text == "helioseismology")
            .unwrap();
        assert!(
            (word.score - 99.0).abs() < 0.01,
            "bonus was stacked: {}",
            word.score
        );
        assert_eq!(engine.candidates()[0].text, "hel");
        assert_eq!(
            engine.candidates()[1].text,
            "hello",
            "late model must preserve typing anchor"
        );
        assert_eq!(engine.preference_count(), 1);
    }
}

#[test]
fn learned_chinese_anchor_is_the_model_prefix_and_explicit_selection_stays_frozen() {
    let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    engine.configure_prediction(None);
    engine.enable_ibus_candidate_mix();
    for _ in 0..8 {
        engine.seed("shijian");
        let index = engine
            .candidates()
            .iter()
            .position(|c| c.text == "实践")
            .unwrap();
        engine.select_candidate(index);
        assert!(engine.commit(CommitOptions { force: true }).ok);
    }
    // Reconfigure with a fresh provider; preference keys have no provider ID.
    drop(requests);
    drop(replies);
    let (tx, requests) = mpsc::channel();
    let (replies, rx) = mpsc::channel();
    engine.configure_prediction(Some(Arc::new(ControlledProvider {
        requests: tx,
        replies: Mutex::new(rx),
    })));
    engine.seed("shijian");
    let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(request.normalized_phrase, "实践");
    replies.send(answer("实践可以积累经验。")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates()[0].text, "实践");
    engine.seed("shijian");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    engine.move_selection(1);
    let frozen = engine.candidates().to_vec();
    replies.send(answer("实践让知识更牢固。")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates(), frozen);
}

fn assert_learned_sentence_survives_model_batch(
    language: &str,
    seed: &str,
    target: &str,
    completions: &[&str],
) {
    use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    for _ in 0..8 {
        engine.seed(seed);
        let index = engine
            .candidates()
            .iter()
            .position(|c| c.text == target && c.kind == CandidateKind::Sentence)
            .unwrap();
        engine.select_candidate(index);
        assert!(engine.commit(CommitOptions { force: true }).ok);
        engine.clear_session_context();
    }
    assert_eq!(engine.preference_count(), 1);
    let (requests_tx, requests) = mpsc::channel();
    let (replies, replies_rx) = mpsc::channel();
    engine.configure_prediction(Some(Arc::new(ControlledProvider {
        requests: requests_tx,
        replies: Mutex::new(replies_rx),
    })));
    engine.seed(seed);
    let anchors: Vec<_> = engine.candidates()[..if language == "en" { 2 } else { 1 }]
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        engine.candidates()[..PAGE_SIZE]
            .iter()
            .any(|c| c.text == target)
    );
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(completions.len(), 6);
    replies
        .send(
            completions
                .iter()
                .map(|text| LlmCompletion {
                    text: (*text).into(),
                    kind: Some(CandidateKind::Sentence),
                    score_bias: 1.0,
                })
                .collect(),
        )
        .unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert!(
        engine
            .candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.text == target),
        "learned {language} sentence vanished from page one after a full model batch: {:?}",
        engine.candidates()
    );
    assert_eq!(
        engine
            .candidates()
            .iter()
            .find(|c| c.kind == CandidateKind::Sentence)
            .unwrap()
            .text,
        target
    );
    for (index, anchor) in anchors.iter().enumerate() {
        assert_eq!(&engine.candidates()[index].text, anchor);
    }
    assert!(engine.candidates().iter().any(|c| c.text == seed));
    assert!(engine.candidates().len() <= 12);
    assert!(
        engine
            .candidates()
            .iter()
            .all(|c| (0.0..=100.0).contains(&c.score))
    );
    let page = &engine.candidates()[..PAGE_SIZE];
    for kind in [CandidateKind::Word, CandidateKind::Sentence] {
        assert!(page.iter().filter(|c| c.kind == kind).count() >= 2);
    }
    assert!(page.iter().any(|c| c.source == CandidateSource::Model));
    assert_eq!(
        engine.preference_count(),
        1,
        "displaying results must not learn"
    );
}

#[test]
fn learned_english_sentence_survives_full_model_batch() {
    assert_learned_sentence_survives_model_batch(
        "en",
        "hel",
        "hello, nice to meet you.",
        &[
            "hello from the new model.",
            "help with the new task.",
            "helmet straps should be secure.",
            "held a meeting yesterday.",
            "helium is lighter than air.",
            "helicopter flights need careful planning.",
        ],
    );
}

#[test]
fn learned_chinese_sentence_survives_full_model_batch() {
    assert_learned_sentence_survives_model_batch(
        "zh-Hans",
        "nihao",
        "你好，请问有什么可以帮忙？",
        &[
            "你好，今天有什么安排？",
            "你好，我们明天再联系。",
            "你好，资料已经收到了。",
            "你好，请确认一下时间。",
            "你好，欢迎参加这次会议。",
            "你好，祝你今天一切顺利。",
        ],
    );
}

#[test]
fn unlearned_model_batches_keep_the_existing_bounded_candidate_policy() {
    use suzaku_map::ime::candidate_mix::{CandidateKind, merge_model};
    for (language, seed, prefix) in [
        ("en", "hel", "hello"),
        ("en", "qzx", "qzx"),
        ("zh-Hans", "nihao", "你好"),
        ("zh-Hans", "shijian", "时间"),
        ("ja", "nihongo", "日本語"),
    ] {
        let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
            default_language: language.into(),
            ..Default::default()
        });
        engine.enable_ibus_candidate_mix();
        engine.seed(seed);
        let batch: Vec<_> = (0..6)
            .map(|index| LlmCompletion {
                text: format!("{prefix} model sentence {index}."),
                kind: Some(CandidateKind::Sentence),
                score_bias: index as f32 / 5.0,
            })
            .collect();
        let (expected, accepted) = merge_model(
            language,
            seed,
            engine.candidates().to_vec(),
            batch.clone(),
            12,
        );
        assert!(accepted);
        requests.recv_timeout(Duration::from_secs(2)).unwrap();
        replies.send(batch).unwrap();
        settle(&mut engine);
        assert_eq!(engine.candidates(), expected, "{language} {seed}");
        assert_eq!(engine.preference_count(), 0);
    }
}

#[test]
fn mixed_prose_model_sentences_project_words_without_losing_typed_prefixes() {
    use suzaku_map::ime::Candidate;
    use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, classify, merge_model};

    for (seed, word, sentence) in [
        (
            "hello—wor",
            "hello—worldwide",
            "hello—worldwide surveys help.",
        ),
        (
            "你好，hel",
            "你好，heliosphere",
            "你好，heliosphere studies continue.",
        ),
        (
            "前文。please sen",
            "前文。please send",
            "前文。please send reports today.",
        ),
    ] {
        assert_eq!(classify("en", seed, word), CandidateKind::Word, "{seed}");
        let local = vec![Candidate {
            text: seed.into(),
            label: seed.into(),
            kind: CandidateKind::Literal,
            ..Default::default()
        }];
        let (candidates, accepted) = merge_model(
            "en",
            seed,
            local,
            vec![LlmCompletion {
                text: sentence.into(),
                kind: Some(CandidateKind::Sentence),
                score_bias: 1.0,
            }],
            12,
        );
        assert!(accepted);
        assert_eq!(candidates[0].text, seed);
        assert_eq!(candidates[0].source, CandidateSource::Local);
        for (text, kind) in [
            (word, CandidateKind::Word),
            (sentence, CandidateKind::Sentence),
        ] {
            let candidate = candidates
                .iter()
                .find(|candidate| candidate.text == text)
                .unwrap();
            assert_eq!(candidate.kind, kind, "{seed} -> {text}");
            assert_eq!(candidate.source, CandidateSource::Model);
            assert!(candidate.text.starts_with(seed));
        }
    }
}

#[test]
fn mixed_prose_model_projection_does_not_split_generated_technical_or_cjk_suffixes() {
    use suzaku_map::ime::Candidate;
    use suzaku_map::ime::candidate_mix::{CandidateKind, merge_model};

    let seed = "你好，hel";
    // Prose boundaries in the already typed prefix do not broaden the separate
    // model-word suffix policy: only complete, unambiguous English words split.
    for sentence in [
        "你好，hello.com site",
        "你好，hello-world event",
        "你好，hello@example.com",
        "你好，hello，世界",
        "你好，hello—world",
    ] {
        let local = vec![Candidate {
            text: seed.into(),
            label: seed.into(),
            kind: CandidateKind::Literal,
            ..Default::default()
        }];
        let (candidates, accepted) = merge_model("en", seed, local, answer(sentence), 12);
        assert!(accepted);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.text == sentence)
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.kind == CandidateKind::Word),
            "must not invent a standalone word from {sentence:?}"
        );
    }
}

#[test]
fn slow_model_does_not_block_typing_and_never_replaces_primary_candidate() {
    let (mut engine, requests, replies) = controlled();
    let start = Instant::now();
    let snapshot = engine.seed("hel");
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(snapshot.candidate_labels[0], "hel");
    assert_eq!(
        requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .language_id,
        "en"
    );
    replies.send(answer("hello, how are you?")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert_eq!(engine.candidates()[0].text, "hel");
    assert_eq!(engine.candidates()[1].text, "hello");
    assert_eq!(engine.candidates()[2].label, "hello, how are you? · AI");
    engine.select_candidate(2);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.unwrap(),
        "hello, how are you?"
    );
}

#[test]
fn ibus_mix_accepts_typed_predictions_but_late_results_do_not_reorder_a_selection() {
    use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
    let (mut engine, requests, replies) = controlled();
    engine.enable_ibus_candidate_mix();
    engine.seed("hel");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    replies
        .send(vec![LlmCompletion {
            text: "hello from the model".into(),
            score_bias: 1.0,
            kind: Some(CandidateKind::Sentence),
        }])
        .unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates()[0].text, "hel");
    assert_eq!(engine.candidates()[1].text, "hello");
    let candidate = engine
        .candidates()
        .iter()
        .find(|c| c.text == "hello from the model")
        .unwrap();
    assert_eq!(candidate.kind, CandidateKind::Sentence);
    assert_eq!(candidate.source, CandidateSource::Model);
    engine.seed("hel");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    engine.move_selection(3);
    let displayed = engine.candidates().to_vec();
    let selected = engine.snapshot().draft_text;
    replies.send(answer("hello from a late model")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates(), displayed);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.unwrap(),
        selected
    );
}

#[test]
fn rapid_input_is_coalesced_and_inflight_stale_results_are_discarded() {
    let (mut engine, requests, replies) = controlled();
    engine.seed("old");
    assert_eq!(
        requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .seed_text,
        "old"
    );
    for seed in ["n", "ne", "new", "newest"] {
        engine.seed(seed);
    }
    replies.send(answer("obsolete result")).unwrap();
    assert_eq!(
        requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .seed_text,
        "newest"
    );
    engine.poll_prediction();
    assert!(
        !engine
            .candidates()
            .iter()
            .any(|candidate| candidate.text == "obsolete result")
    );
    replies.send(answer("newest useful result")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.candidates()[1].text, "newest useful result");
    assert!(requests.try_recv().is_err());
}

#[test]
fn moving_selection_freezes_the_displayed_list_until_the_next_edit() {
    let (mut engine, requests, replies) = controlled();
    engine.seed("hel");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    engine.move_selection(1);
    let selected = engine.snapshot().draft_text;
    replies.send(answer("a late suggestion")).unwrap();
    settle(&mut engine);
    assert_eq!(engine.snapshot().draft_text, selected);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.unwrap(),
        selected
    );
}

#[test]
fn cancelling_or_changing_language_invalidates_previous_predictions() {
    let (mut engine, requests, replies) = controlled();
    engine.seed("hello");
    requests.recv_timeout(Duration::from_secs(2)).unwrap();
    engine.set_language("ja-JP");
    engine.seed("nihongo");
    replies.send(answer("stale English sentence")).unwrap();
    let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(request.language_id, "ja");
    assert_eq!(request.normalized_phrase, "日本語");
    engine.seed("");
    replies.send(answer("日本語を勉強しています")).unwrap();
    assert!(!engine.poll_prediction());
    assert!(engine.candidates().is_empty());
}

#[test]
fn failure_and_unsafe_model_text_keep_lossless_local_candidates() {
    for reply in [Vec::new(), answer("bad\0text"), answer(&"字".repeat(161))] {
        let (mut engine, requests, replies) = controlled();
        engine.seed("hello");
        let original = engine.candidates().to_vec();
        requests.recv_timeout(Duration::from_secs(2)).unwrap();
        replies.send(reply).unwrap();
        settle(&mut engine);
        assert_eq!(engine.prediction_status(), PredictionStatus::Unavailable);
        assert_eq!(engine.candidates(), original);
    }
}

#[test]
fn model_candidates_preserve_typed_indentation_through_merge_and_commit() {
    for mixed in [false, true] {
        let (mut engine, requests, replies) = controlled();
        if mixed {
            engine.enable_ibus_candidate_mix();
        }
        engine.seed("  hel");
        assert_eq!(
            requests
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .normalized_phrase,
            "  hel"
        );
        replies.send(answer("  hello from the model")).unwrap();
        settle(&mut engine);
        let index = engine
            .candidates()
            .iter()
            .position(|candidate| candidate.text == "  hello from the model")
            .expect("candidate merging must not remove typed spaces");
        engine.select_candidate(index);
        assert_eq!(
            engine.commit(CommitOptions { force: true }).text.as_deref(),
            Some("  hello from the model")
        );
    }
}

#[test]
fn chinese_model_continuations_preserve_the_exact_local_prefix() {
    for mixed in [false, true] {
        for (seed, local) in [
            ("  nihao".to_owned(), "  你好".to_owned()),
            ("  你好  ".to_owned(), "  你好  ".to_owned()),
            (
                "\u{3000}你好\u{a0}".to_owned(),
                "\u{3000}你好\u{a0}".to_owned(),
            ),
            ("1、 你好 ".to_owned(), "1、 你好 ".to_owned()),
            (
                format!("  {}", "你".repeat(160)),
                format!("  {}", "你".repeat(160)),
            ),
            (
                format!("{}nihao", "你".repeat(160)),
                format!("{}你好", "你".repeat(160)),
            ),
        ] {
            let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
                default_language: "zh-Hans".into(),
                ..Default::default()
            });
            if mixed {
                engine.enable_ibus_candidate_mix();
            }
            engine.seed(&seed);
            let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(request.seed_text, seed);
            assert_eq!(request.normalized_phrase, local);
            let expected = format!("{local}新的朋友。");
            replies.send(answer(&format!("{expected}  "))).unwrap();
            settle(&mut engine);
            assert_eq!(engine.candidates()[0].text, local);
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|candidate| candidate.text == seed)
            );
            let index = engine
                .candidates()
                .iter()
                .position(|candidate| candidate.text == expected)
                .unwrap_or_else(|| panic!("mixed={mixed}, {seed:?}: {:?}", engine.candidates()));
            engine.select_candidate(index);
            assert_eq!(
                engine.selected_completion_text(true),
                Some(expected.as_str())
            );
            assert_eq!(
                engine.commit(CommitOptions { force: true }).text.as_deref(),
                Some(expected.as_str())
            );
        }
    }
}

#[test]
fn language_profiles_convert_input_and_use_script_appropriate_commit_spacing() {
    for (language, first, second, expected) in [
        ("zh-CN", "nihao", "shijie", "你好世界"),
        ("ja-JP", "watashi", "nihongo", "私日本語"),
        ("en-US", "hello", "world", "hello world"),
    ] {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: language.into(),
            ..Default::default()
        });
        engine.seed(first);
        engine.commit(CommitOptions { force: true });
        engine.seed(second);
        assert_eq!(
            engine.commit(CommitOptions { force: true }).text.as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn context_contains_only_recent_session_commits_and_is_cleared_at_focus_boundaries() {
    let (mut engine, requests, replies) = controlled();
    engine.seed("hello");
    engine.commit(CommitOptions { force: true });
    engine.seed("world");
    assert_eq!(
        requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .context_before_cursor,
        "hello"
    );
    engine.clear_session_context();
    replies.send(answer("stale context")).unwrap();
    engine.seed("new field");
    assert!(
        requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .context_before_cursor
            .is_empty()
    );
    replies.send(Vec::new()).unwrap();
    settle(&mut engine);
}

#[test]
fn unsupported_languages_are_not_silently_treated_as_simplified_chinese() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    assert_eq!(engine.available_languages(), ["en", "ja", "zh-Hans"]);
    engine.set_language("ja_JP");
    assert_eq!(engine.set_language("zh-Hant").active_language, "ja");
}

#[test]
fn literal_input_survives_local_and_ai_candidate_limits() {
    for (language, seed) in [("zh-Hans", "nihao"), ("ja", "nihongo")] {
        for limit in [2, 3, 6] {
            let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
                default_language: language.into(),
                max_candidates: limit,
                ..Default::default()
            });
            engine.seed(seed);
            let primary = engine.candidates()[0].text.clone();
            assert!(engine.candidates().iter().any(|item| item.text == seed));
            requests.recv_timeout(Duration::from_secs(2)).unwrap();
            replies
                .send(
                    ["first suggestion", "second suggestion", "third suggestion"]
                        .into_iter()
                        .flat_map(answer)
                        .collect(),
                )
                .unwrap();
            settle(&mut engine);
            assert_eq!(engine.candidates()[0].text, primary);
            assert!(engine.candidates().len() <= limit);
            assert!(engine.candidates().iter().any(|item| item.text == seed));
        }
    }
}

#[test]
fn large_pasted_input_avoids_token_recursion_and_is_not_sent_to_a_model() {
    let (mut engine, requests, _replies) = controlled();
    let seed = "word ".repeat(8_000);
    engine.seed(&seed);
    assert_eq!(engine.candidates()[0].text, seed);
    assert!(!engine.prediction_pending());
    assert!(requests.try_recv().is_err());
}

#[test]
fn long_local_tail_candidates_do_not_expand_the_model_request_budget() {
    let chinese_tail = "ni".repeat(128);
    let chinese_completed = "你".repeat(128);
    let english_tail = format!("please{}sen", " ".repeat(247));
    let english_completed = format!("{english_tail}d");
    for (language, prefix, tail, completed) in [
        ("en", "note ".repeat(60), "hel", "hello"),
        ("zh-Hans", "你".repeat(260), "nihao", "你好"),
        (
            "zh-Hans",
            "你".repeat(260),
            "你好 ",
            "你好 ，很高兴认识你。",
        ),
        (
            "zh-Hans",
            "前文。".repeat(100),
            chinese_tail.as_str(),
            chinese_completed.as_str(),
        ),
        (
            "en",
            "Earlier note. ".repeat(30),
            english_tail.as_str(),
            english_completed.as_str(),
        ),
    ] {
        for mixed in [false, true] {
            let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
                default_language: language.into(),
                ..Default::default()
            });
            if mixed {
                engine.enable_ibus_candidate_mix();
            }
            engine.seed(format!("{prefix}{tail}"));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|c| c.text == format!("{prefix}{completed}"))
            );
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.source == suzaku_map::ime::candidate_mix::CandidateSource::Local)
            );
            assert_eq!(engine.prediction_status(), PredictionStatus::Idle);
            assert!(requests.recv_timeout(Duration::from_millis(180)).is_err());
            engine.seed(tail);
            let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(request.seed_text, tail);
            assert!(request.context_before_cursor.is_empty());
            replies.send(answer(completed)).unwrap();
            settle(&mut engine);
        }
    }
}

#[test]
fn chinese_adoption_threshold_preserves_local_sentences_and_full_model_requests() {
    use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
    let (mut engine, requests, replies) = controlled_with_config(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    let prefix = "你".repeat(254);
    let raw = format!("{prefix}ni hao");
    let adopted = format!("{prefix}你好");
    let spaced = format!("{adopted} ");
    engine.seed(&raw);
    assert_eq!(engine.prediction_status(), PredictionStatus::Idle);
    assert!(requests.recv_timeout(Duration::from_millis(180)).is_err());

    engine.seed(&adopted);
    let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(request.seed_text, adopted);
    assert_eq!(request.normalized_phrase, adopted);
    assert!(request.context_before_cursor.is_empty());
    assert!(engine.candidates().iter().take(6).any(|c| {
        c.text == format!("{adopted}，很高兴认识你。")
            && c.kind == CandidateKind::Sentence
            && c.source == CandidateSource::Local
    }));

    // Space grows the full draft to 257 scalars: only local continuations remain.
    engine.seed(&spaced);
    assert_eq!(engine.prediction_status(), PredictionStatus::Idle);
    assert!(engine.candidates().iter().take(6).any(|c| {
        c.text == format!("{spaced}，很高兴认识你。")
            && c.kind == CandidateKind::Sentence
            && c.source == CandidateSource::Local
    }));
    assert!(requests.recv_timeout(Duration::from_millis(180)).is_err());
    let stale = format!("{adopted}，旧的模型结果。");
    replies.send(answer(&stale)).unwrap();

    // Deleting that Space requests the entire draft again, never a cropped tail.
    engine.seed(&adopted);
    let request = requests.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(request.seed_text, adopted);
    assert_eq!(request.normalized_phrase, adopted);
    assert!(request.context_before_cursor.is_empty());
    engine.poll_prediction();
    assert!(
        engine
            .candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    let fresh = format!("{adopted}，新的模型结果。");
    replies.send(answer(&fresh)).unwrap();
    settle(&mut engine);
    assert!(!engine.candidates().iter().any(|c| c.text == stale));
    assert!(
        engine
            .candidates()
            .iter()
            .any(|c| c.text == fresh && c.source == CandidateSource::Model)
    );
    assert_eq!(engine.candidates()[0].text, adopted);
    assert!(engine.snapshot().committed_text.is_empty());
}

#[test]
fn legacy_next_word_environment_cannot_trigger_model_io_from_previews() {
    const CHILD: &str = "SUZAKU_IME_TEST_PREVIEW_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.seed("hello");
        let previews = suzaku_map::panel_support::composition_candidate_previews(
            "hello",
            "en",
            engine.candidates(),
            6,
            4,
        );
        assert!(!previews.next_tokens.is_empty());
        return;
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "legacy_next_word_environment_cannot_trigger_model_io_from_previews",
        ])
        .env(CHILD, "1")
        .env(
            "IME_NEXT_TOKEN_MODEL_ENDPOINT",
            format!(
                "http://{}/v1/chat/completions",
                listener.local_addr().unwrap()
            ),
        )
        .env("IME_NEXT_TOKEN_MODEL_TIMEOUT_MS", "20")
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "preview rendering opened a model connection despite the disabled LLM switch"
    );
}

#[test]
fn provider_diagnostics_survive_fallback_and_clear_when_disabled() {
    use suzaku_map::languages::llm::LlmProviderError;
    struct MissingModel;
    impl LlmCompletionProvider for MissingModel {
        fn provider_id(&self) -> &str {
            "missing-model"
        }
        fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
            Vec::new()
        }
        fn generate_checked(
            &self,
            _: &LlmCompletionRequest,
        ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
            Err(LlmProviderError::HttpStatus(404))
        }
    }
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.configure_prediction(Some(Arc::new(MissingModel)));
    engine.seed("hello");
    let local = engine.candidates().to_vec();
    settle(&mut engine);
    assert_eq!(engine.candidates(), local);
    assert_eq!(
        engine.prediction_error(),
        Some(&LlmProviderError::HttpStatus(404))
    );
    engine.configure_prediction(None);
    assert_eq!(engine.prediction_status(), PredictionStatus::Disabled);
    assert!(engine.prediction_error().is_none());
}
