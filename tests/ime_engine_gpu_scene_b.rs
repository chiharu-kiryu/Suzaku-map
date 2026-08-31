use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
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
        PanelChromeState, PreviewStyle, TextSmoothing, TextSpacing, ThemePreset,
        WgpuCandidateRenderer,
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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
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
            .any(|target| target.kind == InteractionKind::SetThemePreset(ThemePreset::DeviceDark))
    );
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
    let scene = renderer.build_settings_scene(
        &PanelChromeState {
            settings_open: true,
            text_scale: DisplayTextScale::Large,
            ..PanelChromeState::default()
        },
        None,
    );

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
fn settings_scene_target_slop_expands_interactive_hit_area() {
    use suzaku_map::ime::gpu::{
        DisplayTextScale, InteractionKind, PanelChromeState, WgpuCandidateRenderer,
    };

    let renderer = WgpuCandidateRenderer::new(520.0, 340.0);
    let base_scene = renderer.build_settings_scene(
        &PanelChromeState {
            pointer_target_slop_tenths: 20,
            ..PanelChromeState::default()
        },
        None,
    );
    let expanded_scene = renderer.build_settings_scene(
        &PanelChromeState {
            pointer_target_slop_tenths: 80,
            ..PanelChromeState::default()
        },
        None,
    );

    let target = InteractionKind::SetTextScale(DisplayTextScale::Large);
    let base_target = base_scene
        .interactive_targets
        .iter()
        .find(|entry| entry.kind == target)
        .expect("base large text scale target");
    let expanded_target = expanded_scene
        .interactive_targets
        .iter()
        .find(|entry| entry.kind == target)
        .expect("expanded large text scale target");

    assert!(expanded_target.rect[2] > base_target.rect[2] + 1.0);
    assert!(expanded_target.rect[3] > base_target.rect[3] + 1.0);

    let probe_x = base_target.rect[0]
        + base_target.rect[2]
        + ((expanded_target.rect[2] - base_target.rect[2]) * 0.4);
    let probe_y = base_target.rect[1] + base_target.rect[3] * 0.5;

    assert_eq!(base_scene.hit_interaction(probe_x, probe_y), None);
    assert_eq!(
        expanded_scene.hit_interaction(probe_x, probe_y),
        Some(target)
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
        None,
        None,
        None,
        None,
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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
            ..PanelChromeState::default()
        },
            None, None, None, None,
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
            None, None, None, None,
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
    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| layout.role == TextRole::CandidatePrimary)
    );
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
        None,
        None,
        None,
        None,
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
        None,
        None,
        None,
        None,
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
