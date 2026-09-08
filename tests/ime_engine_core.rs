use std::sync::Arc;

use suzaku_map::ime::{
    Candidate, CommitOptions, CommitReason, EngineConfig, InputSource, LanguagePlugin, Mode,
    SignalState, Warning, XRTabletImeEngine,
};
use suzaku_map::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmLanguagePlugin,
};

#[test]
fn builds_draft_candidates_from_seed_input() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");

    assert_eq!(snapshot.mode, Mode::Composing);
    assert_eq!(snapshot.active_language, "en");
    assert_eq!(snapshot.seed_text, "ni hao");
    assert!(!snapshot.candidate_labels.is_empty());
    assert_eq!(snapshot.draft_text, snapshot.candidate_labels[0]);
}

#[test]
fn engine_defaults_to_english_language_plugin() {
    let engine = XRTabletImeEngine::new(EngineConfig::default());

    assert_eq!(engine.available_languages(), vec!["en", "ja", "zh-Hans"]);
}

#[test]
fn english_offline_completion_uses_only_current_session_context() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("good");
    assert!(engine.commit(CommitOptions { force: true }).ok);
    engine.seed("m");
    assert_eq!(engine.candidates()[0].text, "m");
    assert_eq!(engine.candidates()[1].text, "morning");
    engine.clear_session_context();
    engine.seed("m");
    assert_ne!(engine.candidates()[1].text, "morning");
    engine.seed("hello  ");
    assert_eq!(engine.candidates()[0].text, "hello  ");
    assert_eq!(engine.candidates()[1].text, "hello  world");
}

#[test]
fn engine_can_switch_to_a_registered_language_plugin() {
    struct EchoLanguagePlugin;

    impl LanguagePlugin for EchoLanguagePlugin {
        fn id(&self) -> &'static str {
            "echo"
        }

        fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
            vec![token.to_string()]
        }

        fn build_candidates(
            &self,
            parts: &[String],
            seed_text: &str,
            confidence: f32,
        ) -> Vec<Candidate> {
            vec![Candidate {
                text: format!("echo {} :: {}", seed_text, parts.join("-")),
                label: format!("echo {} :: {}", seed_text, parts.join("-")),
                score: confidence + 1.0,
            }]
        }
    }

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.register_language_plugin(EchoLanguagePlugin);
    engine.set_language("echo");

    let snapshot = engine.seed("apple seed");

    assert_eq!(snapshot.active_language, "echo");
    assert_eq!(
        snapshot.candidate_labels[0],
        "echo apple seed :: apple-seed"
    );
}

#[test]
fn llm_language_plugin_can_drive_sentence_candidates() {
    struct MockProvider;

    impl LlmCompletionProvider for MockProvider {
        fn provider_id(&self) -> &str {
            "mock"
        }

        fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
            vec![LlmCompletion {
                text: format!("{} becomes a full llm sentence", request.seed_text),
                score_bias: 0.6,
            }]
        }
    }

    let plugin = LlmLanguagePlugin::new("llm-en", "LLM English", Arc::new(MockProvider), |_| {
        vec!["fallback sentence".to_string()]
    });
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.register_language_plugin(plugin);
    engine.set_language("llm-en");

    let snapshot = engine.seed("apple");

    assert_eq!(snapshot.active_language, "llm-en");
    assert_eq!(
        snapshot.candidate_labels[0],
        "apple becomes a full llm sentence"
    );
}

#[test]
fn enters_degraded_mode_under_weak_signal_and_limits_candidates() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.4,
        source_confidence: 0.4,
    });

    let snapshot = engine.seed("ni hao xr");

    assert!(snapshot.degraded);
    assert!(snapshot.candidate_labels.len() <= 3);
    assert!(snapshot.candidate_labels[0].contains("[stable]"));
    assert!(snapshot.warnings.contains(&Warning::DegradedMode));
}

#[test]
fn requires_confirmation_when_confidence_is_weak() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.2,
        source_confidence: 0.2,
    });
    engine.seed("tablet ime");

    let result = engine.commit(CommitOptions::default());

    assert!(!result.ok);
    assert_eq!(result.reason, CommitReason::ConfirmationRequired);
}

#[test]
fn supports_forced_commit_and_one_step_undo() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("xr tablet");

    let commit = engine.commit(CommitOptions { force: true });
    assert!(commit.ok);
    assert!(commit.text.unwrap().to_lowercase().contains("xr"));

    let undo = engine.undo().expect("undo snapshot");
    assert_eq!(undo.committed_text, "");
}

#[test]
fn keeps_source_and_selection_flow_visible_for_xr_hosts() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.set_source(InputSource::HandTracking);
    engine.seed("hello");
    let snapshot = engine.select_candidate(1);

    assert_eq!(snapshot.active_source, InputSource::HandTracking);
    assert_eq!(snapshot.selected_index, 1);
}

#[test]
fn starts_from_base_candidate_and_offers_sentence_continuations() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("thank you");

    assert!(
        snapshot
            .candidate_labels
            .iter()
            .any(|candidate| candidate.split_whitespace().count() >= 4)
    );
    assert!(
        snapshot
            .candidate_labels
            .iter()
            .any(|candidate| candidate != "thank you")
    );
}

#[test]
fn selecting_a_continuation_can_commit_a_full_sentence_without_more_typing() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("thank you");
    let sentence_index = snapshot
        .candidate_labels
        .iter()
        .position(|candidate| candidate.split_whitespace().count() >= 4)
        .expect("sentence candidate");

    engine.select_candidate(sentence_index);
    let commit = engine.commit(CommitOptions { force: true });

    assert!(commit.ok);
    assert!(
        commit
            .text
            .as_deref()
            .is_some_and(|text| text.split_whitespace().count() >= 4)
    );
}

#[test]
fn degraded_mode_preserves_literal_input_and_bounds_local_candidates() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.update_signal(SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.4,
        source_confidence: 0.4,
    });

    let snapshot = engine.seed("xr");

    assert!(snapshot.degraded);
    assert!(snapshot.candidate_labels.len() <= 3);
    assert!(snapshot.candidate_labels[0].starts_with("xr"));
    assert_eq!(engine.candidates()[0].text, "xr");
}
