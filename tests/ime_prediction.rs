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
