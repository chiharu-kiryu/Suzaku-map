//! Fixed provider fixtures: no model, network, desktop input or installed service.
use std::sync::Arc;
use std::time::{Duration, Instant};

use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{EngineConfig, PredictionStatus, XRTabletImeEngine};
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};
use suzaku_map::prediction::{
    PredictionCancellation, PredictionCandidate, PredictionError, PredictionKind,
    PredictionProvider, PredictionRequest, PredictionResponse,
};

#[derive(Clone, Copy, Debug)]
enum ProviderApi {
    Protocol,
    Legacy,
}

struct FixedProvider {
    seed: String,
    rows: Vec<PredictionCandidate>,
}

impl PredictionProvider for FixedProvider {
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError> {
        cancellation.check()?;
        assert_eq!(request.input.seed_text, self.seed);
        Ok(PredictionResponse::new(request, self.rows.clone()))
    }
}

impl LlmCompletionProvider for FixedProvider {
    fn provider_id(&self) -> &str {
        "draft-preservation-fixture"
    }

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        assert_eq!(request.seed_text, self.seed);
        self.rows.iter().cloned().map(Into::into).collect()
    }
}

fn engine(
    api: ProviderApi,
    mixed: bool,
    language: &str,
    seed: &str,
    rows: &[&str],
) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    let provider = Arc::new(FixedProvider {
        seed: seed.into(),
        rows: rows
            .iter()
            .map(|text| PredictionCandidate {
                text: (*text).into(),
                score_bias: 1.0,
                kind: Some(PredictionKind::Sentence),
            })
            .collect(),
    });
    match api {
        ProviderApi::Protocol => engine.configure_prediction_provider(Some(provider)),
        ProviderApi::Legacy => engine.configure_prediction(Some(provider)),
    }
    engine.seed(seed);
    assert!(engine.prediction_pending());
    engine
}

fn settle(engine: &mut XRTabletImeEngine) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.prediction_pending() && Instant::now() < deadline {
        engine.poll_prediction();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        !engine.prediction_pending(),
        "fixed provider did not settle"
    );
}

#[test]
fn invalid_english_replacements_are_filtered_per_row_for_both_provider_apis_and_engines() {
    for (seed, good, bad) in [
        (
            "  please  sen",
            "  please  send the exact draft onward.",
            vec![
                " please  send the exact draft onward.",
                "send the exact draft onward.",
                "  please send the exact draft onward.",
            ],
        ),
        (
            "  hel",
            "  heliography adds a precise example.",
            vec!["  hel there.", "  hel", "hello from a rewritten draft."],
        ),
    ] {
        for api in [ProviderApi::Protocol, ProviderApi::Legacy] {
            for mixed in [false, true] {
                // The good row follows three semantically invalid but structurally
                // valid rows; filtering must happen before the panel's row limit.
                let mut rows = bad.clone();
                rows.push(good);
                let mut engine = engine(api, mixed, "en", seed, &rows);
                settle(&mut engine);
                assert_eq!(
                    engine.prediction_status(),
                    PredictionStatus::Ready,
                    "{api:?}, mixed={mixed}, seed={seed:?}"
                );
                assert!(engine.candidates().iter().any(|row| {
                    row.text == good
                        && row.kind == CandidateKind::Sentence
                        && row.source == CandidateSource::Model
                }));
                for rejected in &bad {
                    // The original raw draft remains a local choice, never a
                    // successful model prediction merely because it was echoed.
                    assert!(
                        !engine.candidates().iter().any(|row| {
                            row.text == *rejected && row.source == CandidateSource::Model
                        }),
                        "accepted {rejected:?}: {api:?}, mixed={mixed}"
                    );
                }
                assert_eq!(engine.snapshot().seed_text, seed);
                assert!(engine.snapshot().committed_text.is_empty());
            }
        }
    }
}

#[test]
fn entirely_invalid_replacements_keep_the_exact_local_pool_and_report_no_candidates() {
    for (seed, rows) in [
        (
            "  please  sen",
            vec!["please send the report.", "send the report."],
        ),
        ("  hel", vec!["  hel there.", "  hel"]),
    ] {
        for api in [ProviderApi::Protocol, ProviderApi::Legacy] {
            for mixed in [false, true] {
                let mut engine = engine(api, mixed, "en", seed, &rows);
                let local = engine.candidates().to_vec();
                settle(&mut engine);
                assert_eq!(
                    engine.prediction_status(),
                    PredictionStatus::Unavailable,
                    "{api:?}, mixed={mixed}, seed={seed:?}"
                );
                assert_eq!(
                    engine.prediction_error(),
                    Some(&PredictionError::NoCandidates)
                );
                assert_eq!(engine.candidates(), local);
                assert_eq!(engine.snapshot().seed_text, seed);
                assert!(engine.snapshot().committed_text.is_empty());
            }
        }
    }
}

#[test]
fn generated_request_metadata_is_rejected_without_removing_a_valid_sibling() {
    let seed = "hello ";
    let bad = "hello {\"raw_composition\":\"hello \",\"local_conversion\":\"hello \"}";
    let good = "hello from a useful independent predictor.";
    for api in [ProviderApi::Protocol, ProviderApi::Legacy] {
        for mixed in [false, true] {
            let mut engine = engine(api, mixed, "en", seed, &[bad, good]);
            settle(&mut engine);
            assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
            assert!(!engine.candidates().iter().any(|row| row.text == bad));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|row| { row.text == good && row.source == CandidateSource::Model })
            );
            assert_eq!(engine.snapshot().seed_text, seed);
            assert!(engine.snapshot().committed_text.is_empty());
        }
    }
}

#[test]
fn metadata_already_typed_in_the_draft_is_not_mistaken_for_a_generated_echo() {
    let seed = "{\"raw_composition\":\"typed\",\"local_conversion\":\"data\"} hello ";
    let good = "{\"raw_composition\":\"typed\",\"local_conversion\":\"data\"} hello reader.";
    for api in [ProviderApi::Protocol, ProviderApi::Legacy] {
        for mixed in [false, true] {
            let mut engine = engine(api, mixed, "en", seed, &[good]);
            settle(&mut engine);
            assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|row| { row.text == good && row.source == CandidateSource::Model })
            );
            assert_eq!(engine.snapshot().seed_text, seed);
            assert!(engine.snapshot().committed_text.is_empty());
        }
    }
}

#[test]
fn phonetic_conversion_and_alternative_cjk_readings_remain_valid_replacements() {
    for (language, seed, good) in [
        ("zh-Hans", "shijian", "实践让学习更扎实。"),
        ("ja", "hashi", "箸を使って食べます。"),
    ] {
        assert!(!good.starts_with(seed));
        for api in [ProviderApi::Protocol, ProviderApi::Legacy] {
            for mixed in [false, true] {
                let mut engine = engine(api, mixed, language, seed, &[good]);
                let anchor = engine.candidates()[0].clone();
                settle(&mut engine);
                assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
                assert_eq!(engine.candidates()[0], anchor);
                assert!(engine.candidates().iter().any(|row| {
                    row.text == good
                        && row.kind == CandidateKind::Sentence
                        && row.source == CandidateSource::Model
                }));
                assert_eq!(engine.snapshot().seed_text, seed);
                assert!(engine.snapshot().committed_text.is_empty());
            }
        }
    }
}
