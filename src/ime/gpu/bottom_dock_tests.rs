use super::*;
use crate::ime::{EngineConfig, XRTabletImeEngine};

fn fixture(native: bool, expanded: bool, mode: InputMode) -> (Snapshot, PanelChromeState) {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.seed("hello");
    let mut snapshot = engine.snapshot();
    snapshot.candidate_labels = (0..13).map(|index| format!("choice{index}")).collect();
    snapshot.selected_index = 6;
    let indices: Vec<_> = if native {
        (6..12).collect()
    } else {
        (0..4).collect()
    };
    let chrome = PanelChromeState {
        panel_layout_mode: PanelLayoutMode::BottomDock,
        seed_text: "hello".into(),
        input_modes_expanded: expanded,
        active_input_mode: mode,
        sentence_candidates: indices
            .iter()
            .map(|&index| snapshot.candidate_labels[index].clone())
            .collect(),
        sentence_candidate_source_indices: indices,
        native_candidate_page: native.then(|| NativeCandidatePage::new(6, 13, false)),
        next_token_candidates: if native {
            Vec::new()
        } else {
            vec!["hello".into(), "help".into()]
        },
        ..Default::default()
    };
    (snapshot, chrome)
}

fn render(width: f32, snapshot: &Snapshot, chrome: &PanelChromeState) -> (RenderScene, f32) {
    let mut renderer = WgpuCandidateRenderer::new(width, 1.0);
    renderer.scene_height = renderer.preferred_input_panel_height(chrome);
    let scene = renderer.build_panel_scene(snapshot, chrome, None, None, None, None);
    (scene, renderer.scene_height)
}

#[test]
fn automatic_layout_uses_the_detected_scene_without_overriding_manual_choice() {
    for native in [false, true] {
        for expanded in [false, true] {
            for detected in [PanelLayoutMode::FollowCaret, PanelLayoutMode::BottomDock] {
                let (snapshot, mut chrome) = fixture(native, expanded, InputMode::VirtualKeyboard);
                chrome.panel_layout_mode = detected;
                chrome.detected_panel_layout = if detected == PanelLayoutMode::BottomDock {
                    PanelLayoutMode::FollowCaret
                } else {
                    PanelLayoutMode::BottomDock
                };
                let (manual, manual_height) = render(1000.0, &snapshot, &chrome);
                chrome.panel_layout_mode = PanelLayoutMode::Auto;
                chrome.detected_panel_layout = detected;
                let (automatic, automatic_height) = render(1000.0, &snapshot, &chrome);
                assert_eq!(automatic_height, manual_height);
                assert_eq!(automatic.interactive_targets, manual.interactive_targets);
                assert_eq!(automatic.hit_targets, manual.hit_targets);
                assert_eq!(automatic.text_sections, manual.text_sections);
            }
        }
    }
}

fn assert_bounded(rect: [f32; 4], width: f32, height: f32) {
    let [x, y, w, h] = rect;
    assert!(
        x >= 0.0 && y >= 0.0 && w > 0.0 && h > 0.0 && x + w <= width + 0.1 && y + h <= height + 0.1,
        "{rect:?} escapes {width} × {height}"
    );
}

#[test]
fn bottom_dock_native_page_keeps_six_choices_above_touch_keyboard() {
    let (snapshot, mut chrome) = fixture(true, true, InputMode::VirtualKeyboard);
    for width in [320.0, 520.0, 900.0, 1100.0, 2200.0] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            for numeric in [false, true] {
                chrome.text_scale = text_scale;
                chrome.keyboard_numeric = numeric;
                let (scene, height) = render(width, &snapshot, &chrome);
                assert_eq!(
                    scene
                        .hit_targets
                        .iter()
                        .map(|t| t.index)
                        .collect::<Vec<_>>(),
                    (6..12).collect::<Vec<_>>()
                );
                let keys: Vec<_> = scene
                    .interactive_targets
                    .iter()
                    .filter(|t| matches!(t.kind, InteractionKind::VirtualKeyboardKey(_)))
                    .collect();
                assert!(keys.len() >= 25);
                let keys_top = keys.iter().map(|t| t.rect[1]).fold(f32::INFINITY, f32::min);
                for target in &scene.hit_targets {
                    assert_bounded(target.rect, width, height);
                    assert!(target.rect[1] + target.rect[3] < keys_top);
                    assert_eq!(
                        scene.hit_interaction(
                            target.rect[0] + target.rect[2] * 0.5,
                            target.rect[1] + target.rect[3] * 0.5
                        ),
                        Some(InteractionKind::Candidate(target.index))
                    );
                }
                if width >= 900.0 {
                    let row_y = scene.hit_targets[0].rect[1];
                    assert_eq!(
                        scene
                            .hit_targets
                            .iter()
                            .filter(|t| (t.rect[1] - row_y).abs() < 0.1)
                            .count(),
                        3
                    );
                }
                for target in keys {
                    assert_bounded(target.rect, width, height);
                    assert!(
                        target.rect[3] >= 37.0,
                        "touch key too short: {:?}",
                        target.rect
                    );
                    assert_eq!(
                        scene.hit_interaction(
                            target.rect[0] + target.rect[2] * 0.5,
                            target.rect[1] + target.rect[3] * 0.5
                        ),
                        Some(target.kind)
                    );
                }
            }
        }
    }
}

#[test]
fn bottom_dock_collapsing_keyboard_keeps_candidates_and_separate_controls() {
    for native in [false, true] {
        let (snapshot, mut chrome) = fixture(native, true, InputMode::VirtualKeyboard);
        let (_, expanded_height) = render(1000.0, &snapshot, &chrome);
        chrome.input_modes_expanded = false;
        let (scene, collapsed_height) = render(1000.0, &snapshot, &chrome);
        assert!(collapsed_height < expanded_height - 200.0);
        assert_eq!(scene.hit_targets.len(), if native { 6 } else { 4 });
        assert!(
            !scene
                .interactive_targets
                .iter()
                .any(|t| matches!(t.kind, InteractionKind::VirtualKeyboardKey(_)))
        );
        for kind in [
            InteractionKind::SettingsToggle,
            InteractionKind::InputModesToggle,
        ] {
            let target = scene
                .interactive_targets
                .iter()
                .find(|t| t.kind == kind)
                .unwrap();
            assert_eq!(
                scene.hit_interaction(
                    target.rect[0] + target.rect[2] * 0.5,
                    target.rect[1] + target.rect[3] * 0.5
                ),
                Some(kind)
            );
            for choice in &scene.hit_targets {
                assert!(choice.rect[1] + choice.rect[3] < target.rect[1]);
            }
        }
    }
}

#[test]
fn folded_toolbar_controls_keep_independent_centers_and_horizontal_edges_in_both_layouts() {
    for width in [320.0, 520.0, 1000.0, 2200.0] {
        for layout in [PanelLayoutMode::FollowCaret, PanelLayoutMode::BottomDock] {
            for native in [false, true] {
                let mut previous_height = None;
                for slop in [10, 50, 120] {
                    let (snapshot, mut chrome) = fixture(native, false, InputMode::VirtualKeyboard);
                    chrome.panel_layout_mode = layout;
                    chrome.pointer_target_slop_tenths = slop;
                    let (scene, height) = render(width, &snapshot, &chrome);
                    let target = |kind| {
                        scene
                            .interactive_targets
                            .iter()
                            .find(|target| target.kind == kind)
                            .unwrap()
                    };
                    let settings = target(InteractionKind::SettingsToggle);
                    let toggle = target(InteractionKind::InputModesToggle);
                    if let Some(previous) = previous_height {
                        assert!(
                            settings.rect[3] > previous + 1.0,
                            "vertical accessibility slop stopped growing at {width}/{layout:?}/{native}/{slop}"
                        );
                    }
                    previous_height = Some(settings.rect[3]);
                    assert!(
                        settings.rect[0] + settings.rect[2] < toggle.rect[0],
                        "folded controls overlap at {width}/{layout:?}/{native}/{slop}: {:?} / {:?}",
                        settings.rect,
                        toggle.rect
                    );
                    for target in [settings, toggle] {
                        assert_bounded(target.rect, width, height);
                        let [x, y, w, h] = target.rect;
                        // Above/below the visual row, slop follows the existing
                        // z-order against neighbouring input/candidate rows.
                        // The two side-by-side controls must remain independent
                        // at their centers and every horizontal boundary.
                        for point in [
                            [x + w * 0.5, y + h * 0.5],
                            [x, y + h * 0.5],
                            [x + w, y + h * 0.5],
                        ] {
                            assert_eq!(
                                scene.hit_interaction(point[0], point[1]),
                                Some(target.kind),
                                "wrong folded control at {width}/{layout:?}/{native}/{slop}: {point:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn bottom_dock_partial_last_page_retains_absolute_identity_and_keyboard_size() {
    let (mut snapshot, mut chrome) = fixture(true, true, InputMode::VirtualKeyboard);
    let (full, full_height) = render(1000.0, &snapshot, &chrome);
    snapshot.selected_index = 12;
    chrome.native_candidate_page = Some(NativeCandidatePage::new(12, 13, false));
    chrome.sentence_candidates = vec!["last choice".into()];
    chrome.sentence_candidate_source_indices = vec![12];
    let (last, last_height) = render(1000.0, &snapshot, &chrome);
    assert!(last_height < full_height);
    assert_eq!(last.hit_targets.len(), 1);
    assert_eq!(last.hit_targets[0].index, 12);
    assert!(
        last.interactive_targets
            .iter()
            .any(|t| t.kind == InteractionKind::NativeCandidatePage(false))
    );
    assert!(
        !last
            .interactive_targets
            .iter()
            .any(|t| t.kind == InteractionKind::NativeCandidatePage(true))
    );
    let keyboard_size = |scene: &RenderScene| {
        scene
            .interactive_targets
            .iter()
            .filter_map(|target| {
                if let InteractionKind::VirtualKeyboardKey(key) = target.kind {
                    Some((key, target.rect[2], target.rect[3]))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(keyboard_size(&full), keyboard_size(&last));
    for target in &last.hit_targets {
        assert_bounded(target.rect, 1000.0, last_height);
    }
}

#[test]
fn bottom_dock_all_tools_and_standalone_candidates_fit_natural_height() {
    for width in [520.0, 1000.0, 2200.0] {
        for native in [false, true] {
            for mode in [
                InputMode::VirtualKeyboard,
                InputMode::Dictation,
                InputMode::Handwriting,
                InputMode::Translation,
            ] {
                let (snapshot, mut chrome) = fixture(native, true, mode);
                chrome.text_scale = DisplayTextScale::Large;
                let (scene, height) = render(width, &snapshot, &chrome);
                if mode == InputMode::Translation {
                    assert!(scene.hit_targets.is_empty());
                } else {
                    assert_eq!(scene.hit_targets.len(), if native { 6 } else { 4 });
                }
                for target in &scene.interactive_targets {
                    if target.kind != InteractionKind::DragWindow {
                        assert_bounded(target.rect, width, height);
                    }
                }
                for glyph in &scene.atlas_glyphs {
                    assert_bounded(glyph.rect, width, height);
                }
            }
        }
    }
}
