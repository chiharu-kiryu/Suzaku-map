//! Regressions from the 2026-09-13 functional-network audit.
//! These run in normal CI without a desktop, real models, or user configuration.
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{
    CommitOptions, EngineConfig, XRTabletImeEngine, companion::NativeComposition,
};
use suzaku_map::languages::BuiltinLanguage;
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};

#[path = "../src/bin/panel/native_typing.rs"]
mod native_typing;

struct RecordingProvider(mpsc::Sender<LlmCompletionRequest>);

impl LlmCompletionProvider for RecordingProvider {
    fn provider_id(&self) -> &str {
        "in-memory-network-audit"
    }

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        let _ = self.0.send(request.clone());
        [
            (
                format!("{}ioseismology", request.seed_text),
                CandidateKind::Word,
            ),
            (
                format!("{}ioseismology is interesting.", request.seed_text),
                CandidateKind::Sentence,
            ),
            (
                format!("{}{}", request.seed_text, "x".repeat(161)),
                CandidateKind::Word,
            ),
        ]
        .into_iter()
        .map(|(text, kind)| LlmCompletion {
            text,
            score_bias: 1.0,
            kind: Some(kind),
        })
        .collect()
    }
}

#[test]
fn audit_model_request_must_allow_a_valid_completion() {
    for mixed in [false, true] {
        for length in [159, 160, 256] {
            let seed = format!(
                "{}{}hel",
                "note ".repeat((length - 3) / 5),
                " ".repeat((length - 3) % 5)
            );
            assert_eq!(seed.chars().count(), length);
            let (sender, requests) = mpsc::channel();
            let mut engine = XRTabletImeEngine::new(EngineConfig::default());
            if mixed {
                engine.enable_ibus_candidate_mix();
            }
            engine.configure_prediction(Some(Arc::new(RecordingProvider(sender))));
            engine.seed(&seed);
            let request = requests
                .recv_timeout(Duration::from_secs(2))
                .expect("bounded long drafts must still receive model completions");
            assert_eq!(request.seed_text, seed);
            assert_eq!(request.normalized_phrase, seed);
            let deadline = Instant::now() + Duration::from_secs(2);
            while engine.prediction_pending() {
                assert!(Instant::now() < deadline);
                engine.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(
                engine
                    .candidates()
                    .iter()
                    .any(|c| c.text == format!("{seed}ioseismology")
                        && c.source == CandidateSource::Model),
                "N02: a valid {length}-character prefix extension was rejected (mixed={mixed}); status={:?}",
                engine.prediction_status()
            );
            assert!(engine.candidates().iter().any(|c| c.text == seed));
            assert!(
                !engine
                    .candidates()
                    .iter()
                    .any(|c| c.text == format!("{}{}", seed, "x".repeat(161)))
            );
            if mixed {
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .any(|c| c.text == format!("{seed}ioseismology is interesting.")
                            && c.kind == CandidateKind::Sentence)
                );
            }
        }
    }
}

#[test]
fn audit_whitespace_native_draft_must_be_committable() {
    for language in [
        BuiltinLanguage::English,
        BuiltinLanguage::ChineseSimplified,
        BuiltinLanguage::Japanese,
    ] {
        for mixed in [false, true] {
            for draft in [" ", "   ", "\u{a0}", " \u{3000} "] {
                let mut engine = XRTabletImeEngine::new(EngineConfig::default());
                engine.set_language(language.id());
                if mixed {
                    engine.enable_ibus_candidate_mix();
                }
                engine.seed(draft);
                assert_eq!(engine.snapshot().seed_text, draft);
                let result = engine.commit(CommitOptions { force: true });
                assert!(
                    result.ok && result.text.as_deref() == Some(draft),
                    "N03: an accepted nonempty whitespace draft has no candidate, so Enter cannot submit it: {:?}",
                    result.reason
                );
                assert!(engine.snapshot().seed_text.is_empty());
                assert!(engine.candidates().is_empty());
                assert!(
                    !engine.commit(CommitOptions { force: true }).ok,
                    "never commit whitespace twice"
                );
            }
        }
    }
}

#[test]
fn empty_whitespace_and_oversized_drafts_do_not_start_model_requests() {
    for draft in ["".into(), " \u{3000} ".into(), "a".repeat(257)] {
        let (sender, requests) = mpsc::channel();
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.enable_ibus_candidate_mix();
        engine.configure_prediction(Some(Arc::new(RecordingProvider(sender))));
        engine.seed(&draft);
        assert!(!engine.prediction_pending());
        assert!(requests.try_recv().is_err());
        if draft.is_empty() {
            assert!(engine.candidates().is_empty());
        } else {
            assert!(engine.candidates().iter().any(|c| c.text == draft));
        }
    }
}

#[test]
fn audit_coalesced_frame_must_not_leave_keyboard_waiting_forever() {
    let frame = NativeComposition {
        host: "11111111-1111-1111-1111-111111111111".into(),
        context: 1,
        revision: 10,
        focused: true,
        private: false,
        language: "en".into(),
        seed: "hel".into(),
        selected: 0,
        candidates: vec![],
    };
    // Host applies "hell" at revision 11, then physical Backspace restores
    // "hel" at 12. The latest-only mailbox legitimately skips frame 11.
    let reverted = NativeComposition {
        revision: 12,
        ..frame.clone()
    };
    let mut failures = Vec::new();
    for frame_first in [true, false] {
        let mut typing = native_typing::NativeTyping::new(&frame);
        assert!(typing.edit(Some("l")));
        typing.sent(10);
        assert!(typing.edit(Some("o")));
        if frame_first {
            typing.observe(&reverted);
        }
        assert!(typing.acknowledge(10, true));
        typing.observe(&reverted);
        assert_eq!(typing.draft, "hello", "retain user recovery text");
        assert!(
            !typing.ready(&reverted),
            "never overwrite the physical edit"
        );
        assert!(!typing.settled());
        if !typing.blocked {
            failures.push(if frame_first {
                "frame before ACK"
            } else {
                "ACK before frame"
            });
        }
    }
    assert!(
        failures.is_empty(),
        "N04: successful ACK plus a newer reverted frame leaves an in-flight edit forever, without marking recovery required: {failures:?}"
    );
}
