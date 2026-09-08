use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
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
    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .any(|layout| {
                layout.role == TextRole::VoiceLabel && layout.lines.join(" ").contains("denied")
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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
        None,
        None,
        None,
        None,
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
        None,
        None,
        None,
        None,
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
    use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("apple");
    let renderer = WgpuCandidateRenderer::new(1200.0, 900.0);
    let chrome = PanelChromeState {
        compact_mode: true,
        ..PanelChromeState::default()
    };
    let scene = renderer.build_compact_scene(&snapshot, &chrome, false, false);
    let compact_target = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::ToggleCompactMode)
        .expect("compact bubble target");

    let center_x = compact_target.rect[0] + compact_target.rect[2] * 0.5;
    let center_y = compact_target.rect[1] + compact_target.rect[3] * 0.5;
    assert!((center_x - renderer.scene_width * 0.5).abs() < 0.01);
    assert!((center_y - renderer.scene_height * 0.5).abs() < 0.01);
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
    let snapshot = engine.seed("hello");
    let scene = WgpuCandidateRenderer::new(1440.0, 960.0).build_scene(&snapshot);

    assert!(scene.hit_targets.len() >= 2);
    let first = scene.hit_targets[0].rect;
    let second = scene.hit_targets[1].rect;
    assert!((first[1] - second[1]).abs() < 1.0);
    assert!(second[0] > first[0] + first[2] * 0.5);
}
