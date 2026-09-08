#![cfg(feature = "gpu")]

use suzaku_map::ime::XRTabletImeEngine;
use suzaku_map::ime::gpu::{
    DisplayTextScale, InputMode, InteractionKind, PanelChromeState, RenderScene,
    WgpuCandidateRenderer,
};

fn contains(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && y >= rect[1] && x <= rect[0] + rect[2] && y <= rect[1] + rect[3]
}

fn assert_background_and_controls(scene: &RenderScene, width: f32, height: f32) {
    let mut background_points = 0;
    let mut control_points = 0;
    for row in 0..40 {
        for col in 0..60 {
            let x = (col as f32 + 0.5) * width / 60.0;
            let y = (row as f32 + 0.5) * height / 40.0;
            let control = scene.interactive_targets.iter().rev().find(|target| {
                target.kind != InteractionKind::DragWindow && contains(target.rect, x, y)
            });
            if let Some(control) = control {
                control_points += 1;
                assert_eq!(scene.hit_interaction(x, y), Some(control.kind));
                assert_eq!(scene.hit_interactive_target(x, y), Some(*control));
            } else {
                background_points += 1;
                assert_eq!(
                    scene.hit_interaction(x, y),
                    Some(InteractionKind::DragWindow),
                    "unclaimed background at {x},{y} in {width}x{height} must move the window"
                );
            }
        }
    }
    assert!(background_points > 0 && control_points > 0);
    assert_eq!(scene.hit_interaction(-10.0, -10.0), None);
    assert_eq!(scene.hit_interaction(width + 10.0, height + 10.0), None);
}

#[test]
fn non_functional_panel_regions_drag_at_every_size_and_input_mode() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    for width in [420.0, 675.0, 900.0, 1170.0, 1800.0] {
        for expanded in [false, true] {
            for mode in [
                InputMode::VirtualKeyboard,
                InputMode::Handwriting,
                InputMode::Dictation,
            ] {
                for text_scale in [DisplayTextScale::Small, DisplayTextScale::Large] {
                    let chrome = PanelChromeState {
                        input_modes_expanded: expanded,
                        active_input_mode: mode,
                        text_scale,
                        seed_text: "hel".into(),
                        sentence_candidates: vec!["hel".into(), "hello".into(), "help".into()],
                        sentence_candidate_source_indices: vec![0, 1, 2],
                        next_token_candidates: vec!["hello".into(), "help".into()],
                        ..Default::default()
                    };
                    let height = WgpuCandidateRenderer::new(width, 1.0)
                        .preferred_input_panel_height(&chrome);
                    let scene = WgpuCandidateRenderer::new(width, height)
                        .build_panel_scene(&snapshot, &chrome, None, None, None, None);
                    assert_background_and_controls(&scene, width, height);

                    let input = scene
                        .interactive_targets
                        .iter()
                        .find(|target| target.kind == InteractionKind::SeedInput)
                        .unwrap();
                    // The label/header is not an editable field. Keep the broad text row
                    // clickable, but make the space above it usable as a title bar.
                    assert_eq!(
                        scene.hit_interaction(input.rect[0] + 24.0, input.rect[1] - 5.0),
                        Some(InteractionKind::DragWindow)
                    );
                    assert_eq!(
                        scene.hit_interaction(
                            input.rect[0] + input.rect[2] * 0.5,
                            input.rect[1] + input.rect[3] * 0.5
                        ),
                        Some(InteractionKind::SeedInput)
                    );
                }
            }
        }
    }
}

#[test]
fn settings_background_drags_without_stealing_search_scroll_or_options() {
    for width in [420.0, 640.0, 900.0] {
        for offset in [0.0, 77.0, 10_000.0] {
            let scene = WgpuCandidateRenderer::new(width, 340.0).build_settings_scene(
                &PanelChromeState {
                    settings_scroll_offset: offset,
                    ..Default::default()
                },
                None,
            );
            assert_background_and_controls(&scene, width, 340.0);
        }
    }
}

#[test]
fn compact_background_drags_while_bubble_keeps_its_click_target() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    let scene = WgpuCandidateRenderer::new(92.0, 92.0).build_compact_scene(
        &snapshot,
        &PanelChromeState::default(),
        false,
        false,
    );
    assert_background_and_controls(&scene, 92.0, 92.0);
    assert_eq!(
        scene.hit_interaction(46.0, 46.0),
        Some(InteractionKind::ToggleCompactMode)
    );
    assert_eq!(
        scene.hit_interaction(0.1, 0.1),
        Some(InteractionKind::DragWindow)
    );
}
