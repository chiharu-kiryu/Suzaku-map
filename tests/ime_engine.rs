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

    assert_eq!(engine.available_languages(), vec!["en".to_string()]);
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
    engine.seed("ni hao xr");
    let snapshot = engine.select_candidate(1);

    assert_eq!(snapshot.active_source, InputSource::HandTracking);
    assert_eq!(snapshot.selected_index, 1);
}

#[test]
fn starts_from_base_candidate_and_offers_sentence_continuations() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");

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
            .any(|candidate| candidate != "ni hao")
    );
}

#[test]
fn selecting_a_continuation_can_commit_a_full_sentence_without_more_typing() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("tablet ime");
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
fn degraded_mode_still_prefers_tap_selectable_sentence_candidates() {
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
    assert!(
        snapshot
            .candidate_labels
            .iter()
            .all(|candidate| candidate.split_whitespace().count() >= 2)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_builder_marks_selected_candidate() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("ni hao xr");
    let snapshot = engine.select_candidate(1);

    let renderer = WgpuCandidateRenderer::new(1024.0, 768.0);
    let scene = renderer.build_scene(&snapshot);

    assert_eq!(
        scene.hit_targets.len(),
        snapshot.candidate_labels.len().min(4)
    );
    assert_eq!(scene.labels[1], snapshot.candidate_labels[1]);
    assert_ne!(scene.quads[2].color, scene.quads[1].color);
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_builder_keeps_vertical_candidate_stack() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao xr");

    let renderer = WgpuCandidateRenderer::new(1280.0, 800.0);
    let scene = renderer.build_scene(&snapshot);

    assert!(scene.quads.len() >= 2);
    assert!(scene.quads[1].rect[1] > scene.quads[0].rect[1]);
    assert_eq!(
        scene.selected_label.as_deref(),
        Some(snapshot.candidate_labels[0].as_str())
    );
    assert_eq!(scene.draft_text, snapshot.draft_text);
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_hit_test_returns_clicked_candidate() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao xr");
    let renderer = WgpuCandidateRenderer::new(1280.0, 800.0);
    let scene = renderer.build_scene(&snapshot);

    let target = scene.hit_targets[1];
    let hit = scene.hit_test(target.rect[0] + 10.0, target.rect[1] + 10.0);

    assert_eq!(hit, Some(1));
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_contains_text_geometry_for_panel_copy() {
    use suzaku_map::ime::gpu::{TextRole, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("tablet ime");
    let renderer = WgpuCandidateRenderer::new(1280.0, 800.0);
    let scene = renderer.build_scene(&snapshot);

    assert!(!scene.text_quads.is_empty());
    assert!(!scene.atlas_glyphs.is_empty());
    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| matches!(layout.role, TextRole::InputLabel | TextRole::InputValue))
    );
}

#[cfg(feature = "gpu")]
#[test]
fn text_block_wraps_and_ellipsizes_long_copy() {
    use suzaku_map::ime::gpu::{TextAlign, TextBlock, TextRole};

    let layout = TextBlock {
        text: "selected candidate for xr tablet testing long phrase with extra wrapping pressure"
            .into(),
        origin: [40.0, 40.0],
        max_width: 90.0,
        pixel_size: 3.0,
        letter_spacing: 0.0,
        line_gap: 8.0,
        max_lines: 2,
        color: [1.0, 1.0, 1.0, 1.0],
        align: TextAlign::Left,
        role: TextRole::CandidatePrimary,
    }
    .layout();

    assert_eq!(layout.lines.len(), 2);
    assert!(layout.truncated);
    assert!(layout.lines[1].ends_with('…'));
    assert!(!layout.quads.is_empty());
}

#[cfg(feature = "gpu")]
#[test]
fn text_block_center_alignment_offsets_bounds_inside_max_width() {
    use suzaku_map::ime::gpu::{TextAlign, TextBlock, TextRole};

    let layout = TextBlock {
        text: "draft".into(),
        origin: [20.0, 20.0],
        max_width: 240.0,
        pixel_size: 4.0,
        letter_spacing: 0.0,
        line_gap: 10.0,
        max_lines: 1,
        color: [1.0, 1.0, 1.0, 1.0],
        align: TextAlign::Center,
        role: TextRole::HeaderTitle,
    }
    .layout();

    assert!(!layout.quads.is_empty());
    assert!(layout.bounds[0] >= 20.0);
    assert!(layout.bounds[2] <= 240.0);
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_exposes_hierarchical_text_sections() {
    use suzaku_map::ime::gpu::{PanelChromeState, TextRole, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao xr");
    let renderer = WgpuCandidateRenderer::new(1280.0, 800.0);
    let scene = renderer.build_scene(&snapshot);

    assert!(scene.text_sections.len() >= 3);
    assert_eq!(scene.text_sections[0].layouts.len(), 2);
    assert!(
        scene
            .text_sections
            .iter()
            .any(|section| section.role == TextRole::ToolButton)
    );
    assert!(
        scene
            .text_sections
            .iter()
            .any(|section| section.role == TextRole::CandidatePrimary && section.layouts.len() >= 2)
    );

    let settings_scene = renderer.build_settings_scene(&PanelChromeState {
        settings_open: true,
        ..PanelChromeState::default()
    });
    assert!(
        settings_scene
            .text_sections
            .iter()
            .any(|section| section.role == TextRole::SettingOption)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_candidate_primary_prefers_continuation_not_full_repeat() {
    use suzaku_map::ime::gpu::{TextRole, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("tablet ime");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);
    let scene = renderer.build_scene(&snapshot);

    let first_candidate = scene
        .text_sections
        .iter()
        .skip(1)
        .find_map(|section| {
            section
                .layouts
                .iter()
                .find(|layout| layout.role == TextRole::CandidatePrimary)
        })
        .expect("candidate primary layout");

    assert!(!first_candidate.lines.is_empty());
    assert!(
        !first_candidate.lines[0]
            .to_lowercase()
            .contains("tablet ime")
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_contains_seed_input_and_input_method_controls() {
    use suzaku_map::ime::gpu::{InputMode, InteractionKind, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);
    let scene = renderer.build_scene(&snapshot);

    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SeedInput)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::InputModesToggle)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind
                == InteractionKind::InputModeButton(InputMode::VirtualKeyboard))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SettingsToggle)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_prefers_settings_toggle_over_seed_input_when_overlapping() {
    use suzaku_map::ime::gpu::{InteractionKind, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);
    let scene = renderer.build_scene(&snapshot);
    let toggle_rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::SettingsToggle)
        .map(|target| target.rect)
        .expect("settings toggle");

    let hit = scene.hit_interaction(toggle_rect[0] + 4.0, toggle_rect[1] + 4.0);

    assert_eq!(hit, Some(InteractionKind::SettingsToggle));
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_can_collapse_input_method_buttons() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: false,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: suzaku_map::ime::gpu::DisplayTextScale::Medium,
            candidate_density: suzaku_map::ime::gpu::CandidateDensity::Cozy,
            preview_style: suzaku_map::ime::gpu::PreviewStyle::Compact,
            font_face: suzaku_map::ime::gpu::FontFaceChoice::Auto,
            text_spacing: suzaku_map::ime::gpu::TextSpacing::Normal,
            text_smoothing: suzaku_map::ime::gpu::TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Unknown,
            voice_backend_label: "Fallback Samples".into(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .all(|target| !matches!(target.kind, InteractionKind::InputModeButton(_)))
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_exposes_virtual_keyboard_keys_in_keyboard_mode() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, VirtualKeyboardKey, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 700.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: suzaku_map::ime::gpu::DisplayTextScale::Medium,
            candidate_density: suzaku_map::ime::gpu::CandidateDensity::Cozy,
            preview_style: suzaku_map::ime::gpu::PreviewStyle::Compact,
            font_face: suzaku_map::ime::gpu::FontFaceChoice::Auto,
            text_spacing: suzaku_map::ime::gpu::TextSpacing::Normal,
            text_smoothing: suzaku_map::ime::gpu::TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Unknown,
            voice_backend_label: "Fallback Samples".into(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(scene.interactive_targets.iter().any(|target| target.kind
        == InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character('q'))));
    assert!(scene.interactive_targets.iter().any(|target| target.kind
        == InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::ToggleNumeric)));
    assert!(
        scene.interactive_targets.iter().any(|target| target.kind
            == InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Backspace))
    );
    let keyboard_text = scene
        .text_sections
        .iter()
        .flat_map(|section| section.layouts.iter())
        .filter(|layout| layout.role == suzaku_map::ime::gpu::TextRole::KeyboardKey)
        .flat_map(|layout| layout.lines.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join("");
    assert!(keyboard_text.contains("123"));
    assert!(keyboard_text.contains("Space"));
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_hides_virtual_keyboard_keys_outside_keyboard_mode() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 700.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::Dictation,
            input_focused: false,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: suzaku_map::ime::gpu::DisplayTextScale::Medium,
            candidate_density: suzaku_map::ime::gpu::CandidateDensity::Cozy,
            preview_style: suzaku_map::ime::gpu::PreviewStyle::Compact,
            font_face: suzaku_map::ime::gpu::FontFaceChoice::Auto,
            text_spacing: suzaku_map::ime::gpu::TextSpacing::Normal,
            text_smoothing: suzaku_map::ime::gpu::TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Listening,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Ready,
            voice_backend_label: "Apple Speech".into(),
            voice_supports_live_capture: true,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: "hello xr panel".into(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .all(|target| !matches!(target.kind, InteractionKind::VirtualKeyboardKey(_)))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::ToggleVoiceCapture)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_switches_to_numeric_keyboard_layout() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, VirtualKeyboardKey, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 700.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: true,
            settings_open: false,
            text_scale: suzaku_map::ime::gpu::DisplayTextScale::Medium,
            candidate_density: suzaku_map::ime::gpu::CandidateDensity::Cozy,
            preview_style: suzaku_map::ime::gpu::PreviewStyle::Compact,
            font_face: suzaku_map::ime::gpu::FontFaceChoice::Auto,
            text_spacing: suzaku_map::ime::gpu::TextSpacing::Normal,
            text_smoothing: suzaku_map::ime::gpu::TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Unknown,
            voice_backend_label: "Fallback Samples".into(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(scene.interactive_targets.iter().any(|target| target.kind
        == InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character('1'))));
    assert!(scene.interactive_targets.iter().any(|target| target.kind
        == InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::ToggleAlphabetic)));
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_exposes_display_settings_when_open() {
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, InteractionKind,
        PanelChromeState, PreviewStyle, TextSmoothing, TextSpacing, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: true,
            text_scale: DisplayTextScale::Large,
            candidate_density: CandidateDensity::Compact,
            preview_style: PreviewStyle::Full,
            font_face: FontFaceChoice::Geneva,
            text_spacing: TextSpacing::Relaxed,
            text_smoothing: TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Ready,
            voice_backend_label: "Apple Speech".into(),
            voice_supports_live_capture: true,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetTextScale(DisplayTextScale::Large))
    );
    assert!(scene.interactive_targets.iter().any(
        |target| target.kind == InteractionKind::SetCandidateDensity(CandidateDensity::Compact)
    ));
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetFontFace(FontFaceChoice::Geneva))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetFontFace(FontFaceChoice::Menlo))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetFontFace(FontFaceChoice::PingFang))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetPreviewStyle(PreviewStyle::Full))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetTextSpacing(TextSpacing::Relaxed))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetTextSmoothing(TextSmoothing::Smooth))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetVoiceAutoInsert(true))
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetLlmEnabled(true))
    );
    assert!(scene.interactive_targets.iter().any(|target| target.kind
        == InteractionKind::SetLlmTemperature(
            suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced
        )));
}

#[cfg(feature = "gpu")]
#[test]
fn settings_scene_exposes_settings_controls_in_a_standalone_window() {
    use suzaku_map::ime::gpu::{
        DisplayTextScale, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let renderer = WgpuCandidateRenderer::new(520.0, 340.0);
    let scene = renderer.build_settings_scene(&PanelChromeState {
        settings_open: true,
        text_scale: DisplayTextScale::Large,
        ..PanelChromeState::default()
    });

    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SettingsToggle)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::SetTextScale(DisplayTextScale::Large))
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_switches_to_compact_floating_bubble_mode() {
    use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(96.0, 96.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            compact_mode: true,
            seed_text: "ni hao".into(),
            ..PanelChromeState::default()
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::ToggleCompactMode)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .all(|target| !matches!(target.kind, InteractionKind::Candidate(_)))
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_allows_wrapped_candidate_preview_in_full_mode() {
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, PanelChromeState,
        PreviewStyle, TextRole, TextSmoothing, TextSpacing, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("tablet ime");
    let renderer = WgpuCandidateRenderer::new(680.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "tablet ime".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 10,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: true,
            text_scale: DisplayTextScale::Large,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Full,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Relaxed,
            text_smoothing: TextSmoothing::Smooth,
            voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
            voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Ready,
            voice_backend_label: "Apple Speech".into(),
            voice_supports_live_capture: true,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            sentence_candidates: vec![
                "tablet ime keeps composing into a much longer preview sentence that should wrap inside the panel".into(),
            ],
            sentence_candidate_source_indices: vec![0],
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| layout.role == TextRole::CandidatePrimary && layout.lines.len() >= 2)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_shows_full_strings_without_compacting_to_ellipsis() {
    use suzaku_map::ime::gpu::{InputMode, PanelChromeState, TextRole, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("tablet ime");
    let renderer = WgpuCandidateRenderer::new(900.0, 900.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "tablet ime can continue by tapping the next suggestion".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 54,
            sentence_candidates: vec![
                "tablet ime can continue by tapping the next suggestion and then commit a full sentence".into(),
            ],
            ..PanelChromeState::default()
        },
    );

    let input_value = scene
        .text_sections
        .iter()
        .flat_map(|section| section.layouts.iter())
        .find(|layout| layout.role == TextRole::InputValue)
        .expect("input value layout");
    assert!(
        input_value
            .lines
            .join(" ")
            .contains("tapping the next suggestion")
    );
    assert!(!input_value.lines.join(" ").contains("..."));

    let candidate_meta = scene
        .text_sections
        .iter()
        .flat_map(|section| section.layouts.iter())
        .find(|layout| layout.role == TextRole::CandidateMeta)
        .expect("candidate meta layout");
    assert!(candidate_meta.lines.join(" ").contains("sentence"));
    assert!(!candidate_meta.lines.join(" ").contains("..."));
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_sentence_cards_show_full_sentence_not_continuation_fragment() {
    use suzaku_map::ime::gpu::{InputMode, PanelChromeState, TextRole, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple can");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "apple can".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 9,
            sentence_candidates: vec!["Apple can continue with the next suggestion.".into()],
            ..PanelChromeState::default()
        },
    );

    let candidate_primary = scene
        .text_sections
        .iter()
        .flat_map(|section| section.layouts.iter())
        .find(|layout| layout.role == TextRole::CandidatePrimary)
        .expect("candidate primary");

    assert!(
        candidate_primary
            .lines
            .join(" ")
            .contains("Apple can continue with the next suggestion.")
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_uses_full_width_primary_sentence_card() {
    use suzaku_map::ime::gpu::{InputMode, PanelChromeState, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("now can as");
    let renderer = WgpuCandidateRenderer::new(980.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "now can as".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 10,
            sentence_candidates: vec![
                "Now can as is ready.".into(),
                "Now expands into a complete candidate.".into(),
                "Can continue with the next suggestion.".into(),
            ],
            sentence_candidate_source_indices: vec![0, 1, 2],
            ..PanelChromeState::default()
        },
    );

    let candidate_targets: Vec<_> = scene
        .interactive_targets
        .iter()
        .filter(|target| {
            matches!(
                target.kind,
                suzaku_map::ime::gpu::InteractionKind::Candidate(_)
            )
        })
        .collect();

    assert!(candidate_targets.len() >= 3);
    assert!(candidate_targets[0].rect[2] > candidate_targets[1].rect[2]);
    assert!(candidate_targets[0].rect[3] > candidate_targets[1].rect[3]);

    let meta_text = scene
        .text_sections
        .iter()
        .flat_map(|section| section.layouts.iter())
        .flat_map(|layout| layout.lines.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(meta_text.contains("Best"));
    assert!(meta_text.contains("Guided") || meta_text.contains("Expanded"));
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_shows_voice_permission_denied_message() {
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, PanelChromeState,
        PreviewStyle, TextRole, TextSmoothing, TextSpacing, VoiceCaptureState,
        VoicePermissionState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::Dictation,
            input_focused: false,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            voice_state: VoiceCaptureState::Idle,
            voice_permission: VoicePermissionState::Denied,
            voice_backend_label: "Apple Speech".into(),
            voice_supports_live_capture: true,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: String::new(),
        },
    );

    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| {
                layout.role == TextRole::VoiceLabel
                    && layout
                        .lines
                        .iter()
                        .any(|line| line.to_lowercase().contains("denied"))
            })
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_exposes_handwriting_canvas_and_candidates() {
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, InputMode, InteractionKind,
        PanelChromeState, PreviewStyle, TextRole, TextSmoothing, TextSpacing, VoiceCaptureState,
        VoicePermissionState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "ni hao".into(),
            compact_mode: false,
            input_modes_expanded: true,
            active_input_mode: InputMode::Handwriting,
            input_focused: false,
            caret_index: 5,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            voice_state: VoiceCaptureState::Idle,
            voice_permission: VoicePermissionState::Unknown,
            voice_backend_label: "Fallback Samples".into(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
            llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: vec![vec![[320.0, 220.0], [350.0, 250.0]]],
            handwriting_candidates: vec!["apple".into(), "input".into()],
            handwriting_hint: "Tap a recognized seed to insert it.".into(),
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::HandwritingCanvas)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::UndoHandwritingStroke)
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::UseHandwritingCandidate(0))
    );
    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| layout.role == TextRole::HandwritingCandidate)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_caps_next_token_chips_at_six_and_sentences_at_four() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "apple".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            composed_tokens: vec!["apple".into()],
            next_token_candidates: vec![
                "is".into(),
                "can".into(),
                "will".into(),
                "for".into(),
                "with".into(),
                "next".into(),
                "extra".into(),
            ],
            sentence_candidates: vec![
                "apple is ready for xr input".into(),
                "apple can become a full sentence".into(),
                "apple will keep composing from taps".into(),
                "apple works well with panel input".into(),
                "apple extra sentence should be hidden".into(),
            ],
            ..PanelChromeState::default()
        },
    );

    assert_eq!(
        scene
            .interactive_targets
            .iter()
            .filter(|target| matches!(target.kind, InteractionKind::SelectNextToken(_)))
            .count(),
        6
    );
    assert_eq!(
        scene
            .interactive_targets
            .iter()
            .filter(|target| matches!(target.kind, InteractionKind::Candidate(_)))
            .count(),
        4
    );
    assert!(
        scene
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::RewindNextToken)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_hides_sentence_cards_until_sentence_stage_is_ready() {
    use suzaku_map::ime::gpu::{
        InputMode, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let renderer = WgpuCandidateRenderer::new(900.0, 760.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            seed_text: "apple".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 5,
            next_token_candidates: vec!["is".into(), "can".into(), "will".into()],
            sentence_candidates: Vec::new(),
            ..PanelChromeState::default()
        },
    );

    assert!(
        scene
            .interactive_targets
            .iter()
            .all(|target| !matches!(target.kind, InteractionKind::Candidate(_)))
    );
    assert_eq!(
        scene
            .interactive_targets
            .iter()
            .filter(|target| matches!(target.kind, InteractionKind::SelectNextToken(_)))
            .count(),
        3
    );
}

#[cfg(feature = "gpu")]
#[test]
fn text_block_tracking_changes_layout_width() {
    use suzaku_map::ime::gpu::{TextAlign, TextBlock, TextRole};

    let tight = TextBlock {
        text: "suzaku".into(),
        origin: [10.0, 10.0],
        max_width: 240.0,
        pixel_size: 3.0,
        letter_spacing: -0.4,
        line_gap: 6.0,
        max_lines: 1,
        color: [1.0, 1.0, 1.0, 1.0],
        align: TextAlign::Left,
        role: TextRole::InputValue,
    }
    .layout();

    let relaxed = TextBlock {
        text: "suzaku".into(),
        origin: [10.0, 10.0],
        max_width: 240.0,
        pixel_size: 3.0,
        letter_spacing: 0.8,
        line_gap: 6.0,
        max_lines: 1,
        color: [1.0, 1.0, 1.0, 1.0],
        align: TextAlign::Left,
        role: TextRole::InputValue,
    }
    .layout();

    assert!(relaxed.bounds[2] > tight.bounds[2]);
}

#[cfg(feature = "gpu")]
#[test]
fn panel_chrome_state_supports_real_text_editing() {
    use suzaku_map::ime::gpu::PanelChromeState;

    let mut chrome = PanelChromeState::default();
    chrome.insert_text("apple");
    chrome.move_caret_left();
    chrome.move_caret_left();
    chrome.insert_text(" ");
    chrome.backspace();
    chrome.move_caret_to_end();
    chrome.insert_text(" pie");

    assert_eq!(chrome.seed_text, "apple pie");
    assert_eq!(chrome.caret_index, chrome.seed_text.chars().count());
}

#[cfg(feature = "gpu")]
#[test]
fn panel_chrome_state_clamps_caret_before_backspace_after_text_normalization() {
    use suzaku_map::ime::gpu::PanelChromeState;

    let mut chrome = PanelChromeState {
        seed_text: "apple".into(),
        caret_index: 7,
        keyboard_shifted: false,
        keyboard_numeric: false,
        voice_state: suzaku_map::ime::gpu::VoiceCaptureState::Idle,
        voice_permission: suzaku_map::ime::gpu::VoicePermissionState::Unknown,
        voice_transcript: String::new(),
        voice_visual_phase: 0,
        voice_auto_insert: true,
        llm_enabled: true,
        llm_model: suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b,
        llm_temperature: suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced,
        ..PanelChromeState::default()
    };

    chrome.backspace();

    assert_eq!(chrome.seed_text, "appl");
    assert_eq!(chrome.caret_index, 4);
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_centers_compact_panel_in_large_viewport() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let renderer = WgpuCandidateRenderer::new(1200.0, 900.0);
    let scene = renderer.build_scene(&snapshot);

    let min_x = scene
        .quads
        .iter()
        .map(|quad| quad.rect[0])
        .fold(f32::INFINITY, f32::min);
    assert!(min_x > 100.0);
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_scales_panel_width_with_viewport_size() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let small = WgpuCandidateRenderer::new(900.0, 700.0).build_scene(&snapshot);
    let large = WgpuCandidateRenderer::new(1440.0, 900.0).build_scene(&snapshot);

    let small_panel_w = small.quads.first().expect("small panel").rect[2];
    let large_panel_w = large.quads.first().expect("large panel").rect[2];

    assert!(large_panel_w > small_panel_w + 120.0);
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_uses_two_column_candidates_in_wide_viewports() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let scene = WgpuCandidateRenderer::new(1440.0, 960.0).build_scene(&snapshot);

    assert!(scene.hit_targets.len() >= 2);
    let first = scene.hit_targets[0].rect;
    let second = scene.hit_targets[1].rect;
    assert!((first[1] - second[1]).abs() < 1.0);
    assert!(second[0] > first[0] + first[2] * 0.5);
}
