//! Fixed synthetic protocol scenarios; no model, network or desktop input is used.
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};
use suzaku_map::prediction::{
    PROTOCOL_VERSION, PredictionCancellation, PredictionCandidate, PredictionError,
    PredictionInput, PredictionKind, PredictionOutput, PredictionProvider, PredictionRequest,
    PredictionResponse,
};

fn input(language: &str, seed: &str, conversion: &str, context: &str) -> PredictionInput {
    PredictionInput {
        language_id: language.into(),
        seed_text: seed.into(),
        normalized_phrase: conversion.into(),
        context_before_cursor: context.into(),
        confidence: 0.85,
        degraded: false,
    }
}

fn candidate(text: &str, kind: Option<PredictionKind>) -> PredictionCandidate {
    PredictionCandidate {
        text: text.into(),
        score_bias: 1.0,
        kind,
    }
}

#[test]
fn wire_roundtrips_exact_multilingual_drafts_without_provider_or_ui_fields() {
    for (language, seed, conversion, context) in [
        ("en", "  please  sen ", "  please  sen ", "First draft. "),
        ("zh-Hans", "中文，zhong w\u{3000}", "中文，中文", "你好。"),
        ("ja", "日本語\u{a0}nyu ", "日本語\u{a0}入力", "こんにちは。"),
    ] {
        let request = PredictionRequest::new(42, input(language, seed, conversion, context));
        assert_eq!(request.version, PROTOCOL_VERSION);
        assert_eq!(request.output, PredictionOutput::CompleteDraftReplacement);
        request.validate().unwrap();
        let wire = serde_json::to_value(&request).unwrap();
        let names: BTreeSet<_> = wire
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names,
            BTreeSet::from(["version", "request_id", "input", "output", "limits"])
        );
        let input_names: BTreeSet<_> = wire["input"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            input_names,
            BTreeSet::from([
                "language_id",
                "seed_text",
                "normalized_phrase",
                "context_before_cursor",
                "confidence",
                "degraded",
            ])
        );
        let decoded: PredictionRequest = serde_json::from_value(wire).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(decoded.input.seed_text, seed);
        assert_eq!(decoded.input.normalized_phrase, conversion);
        assert_eq!(decoded.input.context_before_cursor, context);

        let response = PredictionResponse::new(
            &request,
            vec![
                candidate(&format!("{seed}Word"), Some(PredictionKind::Word)),
                candidate(&format!("{conversion}。"), Some(PredictionKind::Sentence)),
                candidate(&format!("{seed}😀"), None),
            ],
        );
        response.validate_for(&request).unwrap();
        let response_wire = serde_json::to_value(&response).unwrap();
        let decoded: PredictionResponse = serde_json::from_value(response_wire).unwrap();
        assert_eq!(decoded, response);
        decoded.validate_for(&request).unwrap();
    }
}

#[test]
fn request_limits_count_unicode_scalars_and_reject_unbounded_inputs() {
    let request = PredictionRequest::new(
        7,
        input(
            "zh-Hans",
            &"字".repeat(256),
            &"字".repeat(416),
            &"😀".repeat(160),
        ),
    );
    request.validate().unwrap();
    for field in 0..3 {
        let mut invalid = request.clone();
        match field {
            0 => invalid.input.seed_text.push('字'),
            1 => invalid.input.normalized_phrase.push('字'),
            _ => invalid.input.context_before_cursor.push('😀'),
        }
        assert!(invalid.validate().is_err(), "oversized field {field}");
    }
    for max_candidates in [0, 7, usize::MAX] {
        let mut invalid = request.clone();
        invalid.limits.max_candidates = max_candidates;
        assert!(invalid.validate().is_err());
    }
    for max_generated_chars in [0, 161, usize::MAX] {
        let mut invalid = request.clone();
        invalid.limits.max_generated_chars = max_generated_chars;
        assert!(invalid.validate().is_err());
    }
    for confidence in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut invalid = request.clone();
        invalid.input.confidence = confidence;
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn response_identity_and_version_are_checked_before_candidates_are_accepted() {
    let request = PredictionRequest::new(100, input("en", "hel", "hel", ""));
    let response = PredictionResponse::new(
        &request,
        vec![candidate("hello", Some(PredictionKind::Word))],
    );
    response.validate_for(&request).unwrap();

    let mut wrong_request = request.clone();
    wrong_request.version = PROTOCOL_VERSION + 1;
    assert!(wrong_request.validate().is_err());
    assert!(response.validate_for(&wrong_request).is_err());

    let mut wrong_response = response.clone();
    wrong_response.version = PROTOCOL_VERSION + 1;
    assert!(wrong_response.validate_for(&request).is_err());
    wrong_response = response.clone();
    wrong_response.request_id += 1;
    assert!(wrong_response.validate_for(&request).is_err());
}

#[test]
fn wire_rejects_unknown_output_actions_and_nonprediction_candidate_kinds() {
    let request = PredictionRequest::new(3, input("en", "hel", "hel", ""));
    let request_wire = serde_json::to_value(&request).unwrap();
    for action in ["append_token", "commit_text", "unknown"] {
        let mut wire = request_wire.clone();
        wire["output"] = serde_json::json!(action);
        assert!(
            serde_json::from_value::<PredictionRequest>(wire).is_err(),
            "unsupported output action {action} must not become a replacement"
        );
    }
    let response = PredictionResponse::new(&request, vec![candidate("hello", None)]);
    let response_wire = serde_json::to_value(&response).unwrap();
    serde_json::from_value::<PredictionResponse>(response_wire.clone())
        .unwrap()
        .validate_for(&request)
        .unwrap();
    for kind in ["literal", "unknown", "tool_call"] {
        let mut wire = response_wire.clone();
        wire["candidates"][0]["kind"] = serde_json::json!(kind);
        assert!(
            serde_json::from_value::<PredictionResponse>(wire).is_err(),
            "unsupported candidate kind {kind} must not cross the protocol boundary"
        );
    }
}

#[test]
fn response_rejects_unsafe_nonfinite_and_oversized_candidate_batches() {
    let request = PredictionRequest::new(1, input("en", "hel", "hel", ""));
    for text in ["", "   ", "hel\0lo", "hello\nworld", "hello\u{7f}"] {
        let response = PredictionResponse::new(&request, vec![candidate(text, None)]);
        assert!(response.validate_for(&request).is_err(), "{text:?}");
    }
    for score_bias in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut item = candidate("hello", None);
        item.score_bias = score_bias;
        assert!(
            PredictionResponse::new(&request, vec![item])
                .validate_for(&request)
                .is_err()
        );
    }
    let too_many = vec![candidate("hello", None); request.limits.max_candidates + 1];
    assert!(
        PredictionResponse::new(&request, too_many)
            .validate_for(&request)
            .is_err()
    );
    assert!(
        PredictionResponse::new(&request, vec![candidate(&"新".repeat(417), None)])
            .validate_for(&request)
            .is_err()
    );
}

#[test]
fn complete_replacements_charge_the_generated_suffix_not_the_typed_prefix() {
    let prefix = "字".repeat(256);
    let request = PredictionRequest::new(19, input("zh-Hans", &prefix, &prefix, ""));
    let full = format!("{prefix}{}", "新".repeat(160));
    PredictionResponse::new(
        &request,
        vec![candidate(&full, Some(PredictionKind::Sentence))],
    )
    .validate_for(&request)
    .unwrap();
    assert!(
        PredictionResponse::new(&request, vec![candidate(&format!("{full}新"), None)])
            .validate_for(&request)
            .is_err()
    );
    let mut small = PredictionRequest::new(20, input("en", "hel", "hel", ""));
    small.limits.max_candidates = 1;
    small.limits.max_generated_chars = 2;
    PredictionResponse::new(&small, vec![candidate("hello", Some(PredictionKind::Word))])
        .validate_for(&small)
        .unwrap();
    assert!(
        PredictionResponse::new(&small, vec![candidate("hellos", None)])
            .validate_for(&small)
            .is_err()
    );
}

#[test]
fn cancellation_is_shared_by_clones_and_isolated_between_requests() {
    let cancellation = PredictionCancellation::default();
    let observer = cancellation.clone();
    let other = PredictionCancellation::default();
    cancellation.check().unwrap();
    assert!(!observer.is_cancelled());
    cancellation.cancel();
    assert!(observer.is_cancelled());
    assert_eq!(observer.check(), Err(PredictionError::Cancelled));
    other.check().unwrap();
}

type Reply = Result<PredictionResponse, PredictionError>;
type ObservedRequest = (PredictionRequest, PredictionCancellation);

struct ControlledProvider {
    requests: mpsc::Sender<ObservedRequest>,
    replies: Mutex<mpsc::Receiver<Reply>>,
}

impl PredictionProvider for ControlledProvider {
    fn predict(&self, request: &PredictionRequest, cancellation: &PredictionCancellation) -> Reply {
        self.requests
            .send((request.clone(), cancellation.clone()))
            .unwrap();
        // Deliberately allow a late response after cancellation: the engine must
        // defend its own revision even if an external provider ignores the flag.
        self.replies
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or(Err(PredictionError::Timeout))
    }
}

fn controlled(
    language: &str,
) -> (
    XRTabletImeEngine,
    mpsc::Receiver<ObservedRequest>,
    mpsc::Sender<Reply>,
) {
    controlled_with_mix(language, true)
}

fn controlled_with_mix(
    language: &str,
    ibus_mix: bool,
) -> (
    XRTabletImeEngine,
    mpsc::Receiver<ObservedRequest>,
    mpsc::Sender<Reply>,
) {
    let (requests_tx, requests) = mpsc::channel();
    let (replies, replies_rx) = mpsc::channel();
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    if ibus_mix {
        engine.enable_ibus_candidate_mix();
    }
    engine.configure_prediction_provider(Some(Arc::new(ControlledProvider {
        requests: requests_tx,
        replies: Mutex::new(replies_rx),
    })));
    (engine, requests, replies)
}

fn receive(requests: &mpsc::Receiver<ObservedRequest>) -> ObservedRequest {
    requests.recv_timeout(Duration::from_secs(2)).unwrap()
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
fn protocol_provider_adds_typed_words_and_sentences_without_implicit_commit() {
    let (mut engine, requests, replies) = controlled("en");
    engine.seed("hel");
    let anchors = engine.candidates()[..2].to_vec();
    let (request, _) = receive(&requests);
    request.validate().unwrap();
    assert_eq!(request.input.seed_text, "hel");
    let sentence = "hello from the model-neutral protocol.";
    replies
        .send(Ok(PredictionResponse::new(
            &request,
            vec![
                candidate("heliography", Some(PredictionKind::Word)),
                candidate(sentence, Some(PredictionKind::Sentence)),
            ],
        )))
        .unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    for (item, anchor) in engine.candidates()[..2].iter().zip(&anchors) {
        assert_eq!(item.text, anchor.text);
        assert_eq!(item.kind, anchor.kind);
        assert_eq!(item.source, CandidateSource::Local);
    }
    for (text, kind) in [
        ("heliography", CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        assert!(engine.candidates().iter().any(|item| {
            item.text == text && item.kind == kind && item.source == CandidateSource::Model
        }));
    }
    assert!(engine.candidates().iter().any(|item| item.text == "hel"));
    assert!(engine.snapshot().committed_text.is_empty());
    assert_eq!(engine.snapshot().seed_text, "hel");
    let index = engine
        .candidates()
        .iter()
        .position(|item| item.text == sentence)
        .unwrap();
    engine.select_candidate(index);
    assert!(engine.snapshot().committed_text.is_empty());
    let committed = engine.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(sentence));
    assert!(engine.snapshot().seed_text.is_empty());
    assert!(engine.candidates().is_empty());
}

#[test]
fn sentence_only_protocol_replies_project_words_with_exact_extra_spacing_and_commit() {
    for (seed, gap) in [
        ("we need ", " "),
        ("we need\u{a0}", "\u{3000}"),
        ("  hello", "\u{a0}"),
    ] {
        let (mut engine, requests, replies) = controlled("en");
        engine.seed(seed);
        let (request, _) = receive(&requests);
        let word = format!("{seed}{gap}reliable");
        let sentence = format!("{word} backups.");
        replies
            .send(Ok(PredictionResponse::new(
                &request,
                vec![candidate(&sentence, Some(PredictionKind::Sentence))],
            )))
            .unwrap();
        settle(&mut engine);
        assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
        assert_eq!(engine.snapshot().seed_text, seed);
        assert!(engine.snapshot().committed_text.is_empty());
        assert!(
            engine
                .candidates()
                .iter()
                .any(|item| item.text == sentence && item.kind == CandidateKind::Sentence)
        );
        let index = engine
            .candidates()
            .iter()
            .position(|item| {
                item.text == word
                    && item.kind == CandidateKind::Word
                    && item.source == CandidateSource::Model
            })
            .expect("separate exact word projection");
        engine.select_candidate(index);
        assert_eq!(engine.selected_completion_text(true), Some(word.as_str()));
        assert!(engine.snapshot().committed_text.is_empty());
        let committed = engine.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(word.as_str()));
        assert!(engine.snapshot().seed_text.is_empty());
        assert!(engine.undo().unwrap().committed_text.is_empty());
        assert!(engine.undo().is_none());
    }
}

#[test]
fn non_ibus_engine_accepts_protocol_kinds_and_keeps_local_candidates_on_failure() {
    let (mut engine, requests, replies) = controlled_with_mix("en", false);
    engine.seed("hel");
    let anchors = engine.candidates()[..2].to_vec();
    let (request, _) = receive(&requests);
    let sentence = "hello from the platform-neutral engine.";
    replies
        .send(Ok(PredictionResponse::new(
            &request,
            vec![
                candidate("heliography", Some(PredictionKind::Word)),
                candidate(sentence, Some(PredictionKind::Sentence)),
            ],
        )))
        .unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert_eq!(&engine.candidates()[..2], anchors);
    for (text, kind) in [
        ("heliography", CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        assert!(engine.candidates().iter().any(|item| {
            item.text == text && item.kind == kind && item.source == CandidateSource::Model
        }));
    }
    assert!(engine.snapshot().committed_text.is_empty());
    assert_eq!(engine.snapshot().seed_text, "hel");

    engine.seed("hel");
    let local = engine.candidates().to_vec();
    receive(&requests);
    replies.send(Err(PredictionError::Unavailable)).unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Unavailable);
    assert_eq!(engine.candidates(), local);
    assert!(engine.candidates().iter().any(|item| item.text == "hel"));
    assert_eq!(engine.snapshot().seed_text, "hel");
    assert!(engine.snapshot().committed_text.is_empty());
}

#[test]
fn legacy_bridge_filters_invalid_rows_without_erasing_valid_predictions() {
    use suzaku_map::languages::llm::{
        LegacyPredictionAdapter, LlmCompletion, LlmCompletionProvider, LlmCompletionRequest,
    };

    struct LegacyBatch;
    impl LlmCompletionProvider for LegacyBatch {
        fn provider_id(&self) -> &str {
            "legacy-protocol-fixture"
        }

        fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
            assert_eq!(request.seed_text, "hel");
            assert_eq!(request.context_before_cursor, "only this session.");
            vec![
                LlmCompletion {
                    text: "heliography".into(),
                    score_bias: 0.7,
                    kind: Some(CandidateKind::Word),
                },
                LlmCompletion::default(),
                LlmCompletion {
                    text: "helium".into(),
                    score_bias: f32::NAN,
                    kind: None,
                },
                LlmCompletion {
                    text: "hello\0world".into(),
                    score_bias: 1.0,
                    kind: Some(CandidateKind::Sentence),
                },
                LlmCompletion {
                    text: "hello from the legacy provider.".into(),
                    score_bias: 0.6,
                    kind: Some(CandidateKind::Sentence),
                },
                LlmCompletion {
                    text: "hello with an untyped legacy continuation.".into(),
                    score_bias: 0.5,
                    kind: None,
                },
            ]
        }
    }

    let request = PredictionRequest::new(71, input("en", "hel", "hel", "only this session."));
    let response = LegacyPredictionAdapter(Arc::new(LegacyBatch))
        .predict(&request, &PredictionCancellation::default())
        .unwrap();
    response.validate_for(&request).unwrap();
    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.candidates.len(), 3);
    for (item, text, kind, score) in [
        (
            &response.candidates[0],
            "heliography",
            Some(PredictionKind::Word),
            0.7,
        ),
        (
            &response.candidates[1],
            "hello from the legacy provider.",
            Some(PredictionKind::Sentence),
            0.6,
        ),
        (
            &response.candidates[2],
            "hello with an untyped legacy continuation.",
            None,
            0.5,
        ),
    ] {
        assert_eq!(item.text, text);
        assert_eq!(item.kind, kind);
        assert_eq!(item.score_bias, score);
    }
}

#[test]
fn protocol_keeps_chinese_and_japanese_conversion_and_literal_anchors() {
    for (language, seed, conversion, sentence) in [
        ("zh-Hans", "nihao", "你好", "你好，这是一条独立协议的预测。"),
        (
            "ja",
            "nihongo",
            "日本語",
            "日本語を毎日少しずつ練習しています。",
        ),
    ] {
        let (mut engine, requests, replies) = controlled(language);
        engine.seed(seed);
        let anchor = engine.candidates()[0].clone();
        let (request, _) = receive(&requests);
        assert_eq!(request.input.language_id, language);
        assert_eq!(request.input.seed_text, seed);
        assert_eq!(request.input.normalized_phrase, conversion);
        replies
            .send(Ok(PredictionResponse::new(
                &request,
                vec![candidate(sentence, Some(PredictionKind::Sentence))],
            )))
            .unwrap();
        settle(&mut engine);
        assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
        assert_eq!(engine.candidates()[0], anchor);
        assert!(engine.candidates().iter().any(|item| item.text == seed));
        assert!(engine.candidates().iter().any(|item| {
            item.text == sentence
                && item.kind == CandidateKind::Sentence
                && item.source == CandidateSource::Model
        }));
        assert!(engine.snapshot().committed_text.is_empty());
    }
}

#[test]
fn protocol_failures_keep_the_exact_local_pool_and_raw_draft() {
    for error in [
        PredictionError::Unavailable,
        PredictionError::Timeout,
        PredictionError::InvalidResponse,
        PredictionError::NoCandidates,
    ] {
        let (mut engine, requests, replies) = controlled("zh-Hans");
        engine.seed("zhong w ");
        let local = engine.candidates().to_vec();
        receive(&requests);
        replies.send(Err(error)).unwrap();
        settle(&mut engine);
        assert_eq!(engine.prediction_status(), PredictionStatus::Unavailable);
        assert_eq!(engine.candidates(), local);
        assert_eq!(engine.snapshot().seed_text, "zhong w ");
        assert!(engine.snapshot().committed_text.is_empty());
    }
}

#[test]
fn even_a_valid_candidate_with_the_wrong_response_id_is_discarded() {
    let (mut engine, requests, replies) = controlled("en");
    engine.seed("hel");
    let local = engine.candidates().to_vec();
    let (request, _) = receive(&requests);
    let mut response = PredictionResponse::new(
        &request,
        vec![candidate("heliography", Some(PredictionKind::Word))],
    );
    response.request_id = response.request_id.wrapping_add(1);
    replies.send(Ok(response)).unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Unavailable);
    assert_eq!(engine.candidates(), local);
}

#[test]
fn older_protocol_responses_cannot_replace_a_newer_draft() {
    let (mut engine, requests, replies) = controlled("en");
    engine.seed("hel");
    let (old, cancelled) = receive(&requests);
    engine.seed("newest");
    assert!(cancelled.is_cancelled());
    replies
        .send(Ok(PredictionResponse::new(
            &old,
            vec![candidate(
                "hello from the old draft.",
                Some(PredictionKind::Sentence),
            )],
        )))
        .unwrap();
    let (fresh, _) = receive(&requests);
    assert_ne!(old.request_id, fresh.request_id);
    assert_eq!(fresh.input.seed_text, "newest");
    engine.poll_prediction();
    assert!(
        !engine
            .candidates()
            .iter()
            .any(|item| item.text == "hello from the old draft.")
    );
    replies
        .send(Ok(PredictionResponse::new(
            &fresh,
            vec![candidate(
                "newest useful result.",
                Some(PredictionKind::Sentence),
            )],
        )))
        .unwrap();
    settle(&mut engine);
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert!(
        engine
            .candidates()
            .iter()
            .any(|item| item.text == "newest useful result.")
    );
    assert_eq!(engine.snapshot().seed_text, "newest");
    assert!(engine.snapshot().committed_text.is_empty());
}

#[test]
fn focus_boundaries_clear_protocol_context_and_cancel_inflight_results() {
    let (mut engine, requests, replies) = controlled("en");
    engine.seed("hello");
    receive(&requests);
    replies.send(Err(PredictionError::NoCandidates)).unwrap();
    settle(&mut engine);
    let literal = engine
        .candidates()
        .iter()
        .position(|item| item.text == "hello")
        .unwrap();
    engine.select_candidate(literal);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.as_deref(),
        Some("hello")
    );
    engine.seed("world");
    let (old, cancelled) = receive(&requests);
    assert_eq!(old.input.context_before_cursor, "hello");
    engine.clear_session_context();
    assert!(cancelled.is_cancelled());
    assert!(engine.snapshot().committed_text.is_empty());
    assert!(engine.snapshot().seed_text.is_empty());
    replies
        .send(Ok(PredictionResponse::new(
            &old,
            vec![candidate(
                "world from the old field.",
                Some(PredictionKind::Sentence),
            )],
        )))
        .unwrap();
    engine.seed("newest");
    let (fresh, _) = receive(&requests);
    assert_ne!(fresh.request_id, old.request_id);
    assert!(fresh.input.context_before_cursor.is_empty());
    replies.send(Err(PredictionError::NoCandidates)).unwrap();
    settle(&mut engine);
    assert!(
        !engine
            .candidates()
            .iter()
            .any(|item| item.text == "world from the old field.")
    );
    assert_eq!(engine.snapshot().seed_text, "newest");
    assert!(engine.snapshot().committed_text.is_empty());
}
