#![cfg(feature = "gpu")]

use std::{collections::HashMap, sync::Arc};
use suzaku_map::ime::gpu::{
    CandidateDensity, DisplayTextScale, FontLayoutMetrics, InteractionKind, PanelChromeState,
    TextRole, WgpuCandidateRenderer, with_font_metrics,
};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

fn contains(outer: [f32; 4], inner: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.01
        && inner[1] >= outer[1] - 0.01
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

#[test]
fn long_input_and_caret_stay_inside_the_editor_at_every_text_scale() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("hello");
    let seed = "Wi  hello  你好 日本語  a very long editable phrase ".repeat(4);
    let metrics: FontLayoutMetrics = Arc::new(HashMap::from([('W', 5.8), ('i', 1.7)]));
    with_font_metrics(metrics, || {
        for width in [420.0, 750.0, 1000.0, 1500.0, 2500.0] {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                for density in [CandidateDensity::Compact, CandidateDensity::Cozy] {
                    for caret_index in [0, 3, seed.chars().count() / 2, seed.chars().count()] {
                        let chrome = PanelChromeState {
                            seed_text: seed.clone(),
                            input_focused: true,
                            caret_index,
                            text_scale,
                            candidate_density: density,
                            ..Default::default()
                        };
                        let renderer = WgpuCandidateRenderer::new(width, 1200.0);
                        let scene =
                            renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
                        let editor = scene
                            .interactive_targets
                            .iter()
                            .find(|target| target.kind == InteractionKind::SeedInput)
                            .unwrap()
                            .rect;
                        let layout = scene
                            .text_sections
                            .iter()
                            .flat_map(|section| &section.layouts)
                            .find(|layout| layout.role == TextRole::InputValue)
                            .unwrap();
                        assert_eq!(
                            layout.lines,
                            [seed.clone()],
                            "editing must preserve spaces and not wrap"
                        );
                        assert!(
                            contains(editor, layout.bounds),
                            "input text escaped the editor at {width}, {text_scale:?}"
                        );
                        assert!(
                            !layout.truncated,
                            "scroll the draft without adding an ellipsis"
                        );
                        for glyph in &layout.atlas_glyphs {
                            let clip = glyph.clip_rect.expect("long input needs a viewport");
                            assert!(contains(editor, clip));
                        }
                        let caret = scene
                            .quads
                            .iter()
                            .find(|quad| {
                                quad.clip_rect.is_some()
                                    && quad.rect[2] < 6.0
                                    && quad.rect[3] > quad.rect[2]
                            })
                            .expect("input caret with the editor viewport");
                        assert!(
                            contains(editor, caret.rect),
                            "caret escaped at index {caret_index}"
                        );
                        let title = scene
                            .text_sections
                            .iter()
                            .flat_map(|section| &section.layouts)
                            .find(|layout| layout.role == TextRole::InputLabel)
                            .unwrap();
                        assert!(
                            title.bounds[1] + title.bounds[3] <= layout.bounds[1] + 0.01,
                            "input label overlaps the editable text"
                        );
                    }
                }
            }
        }
    });
}
