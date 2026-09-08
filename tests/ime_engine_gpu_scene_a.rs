use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

#[cfg(feature = "gpu")]
#[test]
fn gpu_scene_builder_marks_selected_candidate() {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("hello");
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
    let snapshot = engine.seed("hello");
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

    let settings_scene = renderer.build_settings_scene(
        &PanelChromeState {
            settings_open: true,
            ..PanelChromeState::default()
        },
        None,
    );
    assert!(
        settings_scene
            .text_sections
            .iter()
            .any(|section| section.role == TextRole::SettingOption)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_candidate_primary_preserves_the_literal_fallback() {
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
        first_candidate.lines[0]
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
fn render_scene_target_slop_expands_interactive_hit_area() {
    use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);

    let base_scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            pointer_target_slop_tenths: 20,
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
    );
    let expanded_scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            pointer_target_slop_tenths: 80,
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
    );

    let base_toggle = base_scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::SettingsToggle)
        .expect("base settings toggle target");
    let expanded_toggle = expanded_scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::SettingsToggle)
        .expect("expanded settings toggle target");

    assert!(expanded_toggle.rect[2] > base_toggle.rect[2] + 1.0);
    assert!(expanded_toggle.rect[3] > base_toggle.rect[3] + 1.0);

    let probe_x = expanded_toggle.rect[0] + 1.0;
    let probe_y = expanded_toggle.rect[1] + expanded_toggle.rect[3] * 0.5;

    assert_eq!(
        base_scene.hit_interaction(probe_x, probe_y),
        Some(InteractionKind::DragWindow)
    );
    assert_eq!(
        expanded_scene.hit_interaction(probe_x, probe_y),
        Some(InteractionKind::SettingsToggle)
    );
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
            theme_preset: suzaku_map::ime::gpu::ThemePreset::Daylight,
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
fn render_scene_exposes_window_scale_reset_control() {
    use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState, WgpuCandidateRenderer};

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);
    let scene = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            window_scale: 1.2,
            sentence_candidates: vec!["ni hao example".into()],
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
            .any(|target| target.kind == InteractionKind::ResetWindowScale)
    );
    let reset = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::ResetWindowScale)
        .expect("reset window scale target");
    let hit = scene.hit_interaction(reset.rect[0] + 2.0, reset.rect[1] + 2.0);
    assert_eq!(hit, Some(InteractionKind::ResetWindowScale));

    let decrease = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::DecreaseWindowScale)
        .expect("decrease window scale target");
    let increase = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::IncreaseWindowScale)
        .expect("increase window scale target");
    assert_eq!(
        scene.hit_interaction(decrease.rect[0] + 2.0, decrease.rect[1] + 2.0),
        Some(InteractionKind::DecreaseWindowScale)
    );
    assert_eq!(
        scene.hit_interaction(increase.rect[0] + 2.0, increase.rect[1] + 2.0),
        Some(InteractionKind::IncreaseWindowScale)
    );
}

#[cfg(feature = "gpu")]
#[test]
fn render_scene_disables_scale_controls_at_boundaries() {
    use suzaku_map::ime::gpu::{
        InteractionKind, PANEL_SCALE_MAX, PANEL_SCALE_MIN, PanelChromeState, WgpuCandidateRenderer,
    };

    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    let renderer = WgpuCandidateRenderer::new(900.0, 520.0);

    let at_min = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            window_scale: PANEL_SCALE_MIN,
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
    );
    let at_max = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            window_scale: PANEL_SCALE_MAX,
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
    );
    let at_default = renderer.build_panel_scene(
        &snapshot,
        &PanelChromeState {
            window_scale: 1.0,
            ..PanelChromeState::default()
        },
        None,
        None,
        None,
        None,
    );

    assert!(
        !at_min
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::DecreaseWindowScale)
    );
    assert!(
        at_min
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::IncreaseWindowScale)
    );
    assert!(
        !at_max
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::IncreaseWindowScale)
    );
    assert!(
        at_max
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::DecreaseWindowScale)
    );
    assert!(
        !at_default
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::ResetWindowScale)
    );
}
