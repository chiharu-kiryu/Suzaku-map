#![cfg(feature = "gpu")]
use suzaku_map::ime::gpu::{
    FontFaceChoice, InteractionKind, PanelChromeState, QuadShape, TextSmoothing, ThemePreset,
    WgpuCandidateRenderer,
};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

#[test]
fn theme_ids_are_unique_and_old_settings_remain_compatible() {
    let mut ids = Vec::new();
    for theme in ThemePreset::ALL {
        assert_eq!(ThemePreset::from_id(theme.id()), Some(theme));
        assert!(!ids.contains(&theme.id()));
        ids.push(theme.id());
    }
    assert_eq!(ThemePreset::from_id("suzaku"), Some(ThemePreset::Suzaku));
    assert_eq!(
        ThemePreset::from_id("device_dark"),
        Some(ThemePreset::DeviceDark)
    );
    assert_eq!(ThemePreset::from_id("unknown"), None);
    assert_eq!(ThemePreset::from_id("BAIHU"), None);
}

#[test]
fn indigo_drawers_do_not_fall_back_to_white_surfaces() {
    use suzaku_map::ime::gpu::InputMode;
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("hello");
    for mode in [
        InputMode::VirtualKeyboard,
        InputMode::Dictation,
        InputMode::Handwriting,
    ] {
        let chrome = PanelChromeState {
            theme_preset: ThemePreset::Xuanwu,
            input_modes_expanded: true,
            active_input_mode: mode,
            ..Default::default()
        };
        let height = WgpuCandidateRenderer::new(640.0, 1.0).preferred_input_panel_height(&chrome);
        let scene = WgpuCandidateRenderer::new(640.0, height)
            .build_panel_scene(&snapshot, &chrome, None, None, None, None);
        let large_surfaces: Vec<_> = scene
            .quads
            .iter()
            .filter(|q| q.rect[2] > 150.0 && q.rect[3] > 40.0 && q.color[3] > 0.99)
            .filter(|q| {
                matches!(
                    q.shape,
                    QuadShape::Rectangle | QuadShape::Rounded { stroke: 0.0, .. }
                )
            })
            .collect();
        assert!(!large_surfaces.is_empty());
        assert!(
            large_surfaces
                .iter()
                .all(|q| q.color[..3].iter().all(|v| *v < 0.20)),
            "{mode:?} has a light fallback card"
        );
    }
}

#[test]
fn new_profiles_use_suzaku_and_smooth_system_text() {
    let chrome = PanelChromeState::default();
    assert_eq!(chrome.theme_preset, ThemePreset::Suzaku);
    assert_eq!(chrome.text_smoothing, TextSmoothing::Smooth);
    assert_eq!(chrome.font_face, FontFaceChoice::Auto);
}

#[test]
fn themes_keep_the_same_candidate_actions_and_curved_toolbar() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("hello");
    let renderer = WgpuCandidateRenderer::new(1000.0, 520.0);
    let mut reference = None;
    for theme in ThemePreset::ALL {
        let chrome = PanelChromeState {
            seed_text: "hello".into(),
            input_modes_expanded: true,
            theme_preset: theme,
            sentence_candidates: vec!["hello".into(), "hello world".into()],
            sentence_candidate_source_indices: vec![0, 1],
            ..Default::default()
        };
        let scene = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
        let actions: Vec<_> = scene
            .interactive_targets
            .iter()
            .map(|target| (target.kind, target.rect))
            .collect();
        if let Some(reference) = &reference {
            assert_eq!(&actions, reference);
        } else {
            reference = Some(actions);
        }
        assert!(
            scene
                .quads
                .iter()
                .any(|quad| matches!(quad.shape, QuadShape::Segment { .. }))
        );
        assert!(
            scene.quads.iter().any(
                |quad| matches!(quad.shape, QuadShape::Rounded { radius, .. } if radius > 5.0)
            )
        );
        assert!(scene.quads.len() < 1600, "bounded vector geometry");
        assert_eq!(scene.labels, snapshot.candidate_labels);
    }
}

#[test]
fn borderless_panel_and_settings_drop_the_backplate_without_moving_controls() {
    use suzaku_map::ime::gpu::InputMode;
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    for seed in ["", "hello"] {
        let snapshot = engine.seed(seed);
        for theme in ThemePreset::ALL {
            for width in [420.0, 720.0, 1040.0] {
                for mode in [
                    InputMode::VirtualKeyboard,
                    InputMode::Dictation,
                    InputMode::Handwriting,
                ] {
                    for settings in [false, true] {
                        let mut chrome = PanelChromeState {
                            theme_preset: theme,
                            active_input_mode: mode,
                            input_modes_expanded: true,
                            seed_text: seed.into(),
                            ..Default::default()
                        };
                        let height = if settings {
                            340.0
                        } else {
                            WgpuCandidateRenderer::new(width, 1.0)
                                .preferred_input_panel_height(&chrome)
                        };
                        let renderer = WgpuCandidateRenderer::new(width, height);
                        let build = |chrome: &PanelChromeState| {
                            if settings {
                                renderer.build_settings_scene(chrome, None)
                            } else {
                                renderer
                                    .build_panel_scene(&snapshot, chrome, None, None, None, None)
                            }
                        };
                        let decorated = build(&chrome);
                        chrome.hide_system_titlebar = true;
                        let floating = build(&chrome);
                        assert_eq!(floating.interactive_targets, decorated.interactive_targets);
                        // Show/Hide selected colors change, but text geometry must not.
                        assert_eq!(floating.atlas_glyphs.len(), decorated.atlas_glyphs.len());
                        for (actual, expected) in
                            floating.atlas_glyphs.iter().zip(&decorated.atlas_glyphs)
                        {
                            assert_eq!(
                                (actual.ch, actual.rect, actual.clip_rect),
                                (expected.ch, expected.rect, expected.clip_rect)
                            );
                        }
                        assert!(floating.quads.len() < decorated.quads.len());
                        for point in [
                            [0.5, 0.5],
                            [width - 0.5, 0.5],
                            [0.5, height - 0.5],
                            [width - 0.5, height - 0.5],
                        ] {
                            assert!(
                                floating.quads.iter().all(|quad| quad.color[3] == 0.0
                                    || quad.signed_distance(point) > 1.0),
                                "opaque corner at {point:?}: {theme:?}, {mode:?}, settings={settings}, {width}x{height}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn orb_keeps_drag_restore_hit_targets_but_has_no_square_background() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("hello");
    for (theme, size) in ThemePreset::ALL
        .into_iter()
        .flat_map(|preset| [64.0, 96.0, 144.0].map(|size| (preset, size)))
    {
        let scene = WgpuCandidateRenderer::new(size, size).build_compact_scene(
            &snapshot,
            &PanelChromeState {
                theme_preset: theme,
                ..Default::default()
            },
            false,
            false,
        );
        assert_eq!(
            scene.hit_interaction(size * 0.5, size * 0.5),
            Some(InteractionKind::ToggleCompactMode)
        );
        assert_eq!(
            scene.hit_interaction(0.0, 0.0),
            Some(InteractionKind::DragWindow)
        );
        assert!(
            scene
                .quads
                .iter()
                .all(|quad| quad.signed_distance([0.0, 0.0]) > 0.0)
        );
        assert!(scene.atlas_glyphs.is_empty());
        assert!(scene.quads.len() < 510);
    }
}
