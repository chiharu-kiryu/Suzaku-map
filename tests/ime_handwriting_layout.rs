#![cfg(feature = "gpu")]

use suzaku_map::ime::XRTabletImeEngine;
use suzaku_map::ime::gpu::{
    DisplayTextScale, InputMode, InteractionKind, PanelChromeState, TextRole, TextSpacing,
    WgpuCandidateRenderer,
};

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] - 0.01
        && b[0] < a[0] + a[2] - 0.01
        && a[1] < b[1] + b[3] - 0.01
        && b[1] < a[1] + a[3] - 0.01
}

#[test]
fn handwriting_text_canvas_and_actions_do_not_overlap() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    for width in [420.0, 675.0, 900.0, 1170.0, 1800.0, 2500.0] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            for text_spacing in [
                TextSpacing::Tight,
                TextSpacing::Normal,
                TextSpacing::Relaxed,
            ] {
                for candidates in [
                    vec![],
                    vec!["O".into(), "A".into()],
                    vec!["你好世界".into(), "handwriting".into()],
                ] {
                    let chrome = PanelChromeState {
                        input_modes_expanded: true,
                        active_input_mode: InputMode::Handwriting,
                        text_scale,
                        text_spacing,
                        handwriting_candidates: candidates,
                        handwriting_hint: "Try a clearer trace, undo a stroke, or tap Clear."
                            .into(),
                        ..Default::default()
                    };
                    let height = WgpuCandidateRenderer::new(width, 1.0)
                        .preferred_input_panel_height(&chrome);
                    let scene = WgpuCandidateRenderer::new(width, height)
                        .build_panel_scene(&snapshot, &chrome, None, None, None, None);
                    let labels: Vec<_> = scene
                        .text_sections
                        .iter()
                        .flat_map(|section| &section.layouts)
                        .filter(|layout| {
                            matches!(
                                layout.role,
                                TextRole::HandwritingLabel | TextRole::HandwritingCandidate
                            )
                        })
                        .collect();
                    if width == 420.0 && text_scale == DisplayTextScale::Large {
                        let hint = labels
                            .iter()
                            .find(|label| label.lines[0].starts_with("Try a clearer"))
                            .unwrap();
                        assert_eq!(
                            hint.lines.len(),
                            2,
                            "narrow hints use both reserved text rows"
                        );
                        assert!(!hint.truncated, "the full guidance fits in two lines");
                    }
                    for (index, a) in labels.iter().enumerate() {
                        for b in &labels[index + 1..] {
                            assert!(
                                !overlap(a.bounds, b.bounds),
                                "{width} {text_scale:?}: {:?} {:?} overlaps {:?} {:?}",
                                a.lines,
                                a.bounds,
                                b.lines,
                                b.bounds
                            );
                        }
                    }
                    let canvas = scene
                        .interactive_targets
                        .iter()
                        .find(|target| target.kind == InteractionKind::HandwritingCanvas)
                        .unwrap()
                        .rect;
                    assert!(canvas[3] >= 50.0, "canvas must retain useful drawing space");
                    for label in &labels {
                        assert!(
                            !overlap(label.bounds, canvas),
                            "text {:?} overlaps the drawing canvas",
                            label.lines
                        );
                        for glyph in &label.atlas_glyphs {
                            assert!(glyph.rect[0] >= 0.0 && glyph.rect[1] >= 0.0);
                            assert!(glyph.rect[0] + glyph.rect[2] <= width + 0.01);
                            assert!(glyph.rect[1] + glyph.rect[3] <= height + 0.01);
                        }
                    }
                    for kind in [
                        InteractionKind::UndoHandwritingStroke,
                        InteractionKind::ClearHandwriting,
                    ] {
                        let action = scene
                            .interactive_targets
                            .iter()
                            .find(|target| target.kind == kind)
                            .unwrap();
                        for label in labels
                            .iter()
                            .filter(|label| label.role == TextRole::HandwritingCandidate)
                        {
                            assert!(
                                !overlap(label.bounds, action.rect),
                                "candidate text overlaps {kind:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn handwriting_canvas_stays_fixed_when_status_and_candidates_change() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    for width in [420.0, 900.0, 1800.0] {
        let mut chrome = PanelChromeState {
            input_modes_expanded: true,
            active_input_mode: InputMode::Handwriting,
            text_scale: DisplayTextScale::Large,
            ..Default::default()
        };
        let height = WgpuCandidateRenderer::new(width, 1.0).preferred_input_panel_height(&chrome);
        let renderer = WgpuCandidateRenderer::new(width, height);
        let mut original = None;
        for hint in [
            "Draw a seed word with mouse or touch.",
            "Tracing… release to recognize",
            "Try a clearer trace, undo a stroke, or tap Clear.",
        ] {
            chrome.handwriting_hint = hint.into();
            for candidates in [vec![], vec!["O".into(), "A".into()]] {
                chrome.handwriting_candidates = candidates;
                let scene = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
                let canvas = scene
                    .interactive_targets
                    .iter()
                    .find(|target| target.kind == InteractionKind::HandwritingCanvas)
                    .unwrap()
                    .rect;
                assert_eq!(
                    *original.get_or_insert(canvas),
                    canvas,
                    "status changes must not shift an active stroke's coordinate system"
                );
            }
        }
    }
}

#[test]
fn handwriting_ink_cannot_paint_over_text_after_a_resize() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    let mut chrome = PanelChromeState {
        input_modes_expanded: true,
        active_input_mode: InputMode::Handwriting,
        // Keep Undo enabled in both scenes so only the inserted ink differs.
        handwriting_strokes: vec![vec![]],
        ..Default::default()
    };
    let renderer = WgpuCandidateRenderer::new(420.0, 420.0);
    let empty = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
    let canvas = empty
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::HandwritingCanvas)
        .unwrap()
        .rect;
    chrome.handwriting_strokes = vec![vec![
        [-20.0, -20.0],
        [canvas[0] + 10.0, canvas[1] + 10.0],
        [1200.0, 900.0],
    ]];
    let ink = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
    let inserted = ink.quads.len() - empty.quads.len();
    assert!(inserted > 0);
    let start = ink
        .quads
        .iter()
        .zip(&empty.quads)
        .position(|(a, b)| a != b)
        .unwrap();
    for quad in &ink.quads[start..start + inserted] {
        assert!(quad.rect[0] >= canvas[0] && quad.rect[1] >= canvas[1]);
        assert!(quad.rect[0] + quad.rect[2] <= canvas[0] + canvas[2] + 0.01);
        assert!(quad.rect[1] + quad.rect[3] <= canvas[1] + canvas[3] + 0.01);
    }
}
