#![cfg(feature = "gpu")]

use suzaku_map::ime::gpu::{InputMode, PanelChromeState, WgpuCandidateRenderer};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};
use suzaku_map::panel_support::composition_candidate_previews;

#[test]
fn fitted_panels_keep_small_gutters_across_modes_scales_and_candidate_counts() {
    for language in ["en", "zh-Hans", "ja"] {
        for seed in [
            "",
            if language == "en" {
                "hel"
            } else if language == "ja" {
                "nihongo"
            } else {
                "nihao"
            },
        ] {
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: language.into(),
                ..Default::default()
            });
            let snapshot = engine.seed(seed);
            for width in [630.0, 900.0, 1170.0, 1600.0, 2500.0] {
                for expanded in [false, true] {
                    for mode in [
                        InputMode::VirtualKeyboard,
                        InputMode::Dictation,
                        InputMode::Handwriting,
                    ] {
                        let previews = composition_candidate_previews(
                            seed,
                            language,
                            engine.candidates(),
                            6,
                            4,
                        );
                        let (indices, labels) = previews.sentences.into_iter().unzip();
                        let chrome = PanelChromeState {
                            seed_text: seed.into(),
                            input_modes_expanded: expanded,
                            active_input_mode: mode,
                            sentence_candidates: labels,
                            sentence_candidate_source_indices: indices,
                            next_token_candidates: previews
                                .next_tokens
                                .into_iter()
                                .map(|edit| edit.label)
                                .collect(),
                            ..Default::default()
                        };
                        let height = WgpuCandidateRenderer::new(width, 1.0)
                            .preferred_input_panel_height(&chrome);
                        // Measuring from the result or an old tall viewport cannot cause a feedback loop.
                        assert_eq!(
                            WgpuCandidateRenderer::new(width, height)
                                .preferred_input_panel_height(&chrome),
                            height
                        );
                        assert_eq!(
                            WgpuCandidateRenderer::new(width, 5000.0)
                                .preferred_input_panel_height(&chrome),
                            height
                        );
                        let scene = WgpuCandidateRenderer::new(width, height)
                            .build_panel_scene(&snapshot, &chrome, None, None, None, None);
                        let left = scene
                            .quads
                            .iter()
                            .map(|q| q.rect[0])
                            .fold(f32::INFINITY, f32::min);
                        let right = scene
                            .quads
                            .iter()
                            .map(|q| q.rect[0] + q.rect[2])
                            .fold(0.0, f32::max);
                        let top = scene
                            .quads
                            .iter()
                            .map(|q| q.rect[1])
                            .fold(f32::INFINITY, f32::min);
                        let bottom = scene
                            .quads
                            .iter()
                            .map(|q| q.rect[1] + q.rect[3])
                            .fold(0.0, f32::max);
                        assert!(top < 14.0 && left < width * 0.025);
                        assert!(
                            width - right < width * 0.025,
                            "wide windows must not retain the old 1360px content cap"
                        );
                        assert!(
                            height - bottom < 18.0,
                            "{language} {width} {expanded} {mode:?}: bottom gutter {}",
                            height - bottom
                        );
                        assert!(
                            bottom <= height + 1.0,
                            "{language} {width} {expanded} {mode:?}: bottom {bottom}, height {height}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fold_changes_height_not_control_scale_or_top_anchor() {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed("hel");
    let expanded = PanelChromeState {
        input_modes_expanded: true,
        ..Default::default()
    };
    let folded = PanelChromeState {
        input_modes_expanded: false,
        ..expanded.clone()
    };
    let expanded_height =
        WgpuCandidateRenderer::new(900.0, 1.0).preferred_input_panel_height(&expanded);
    let folded_height =
        WgpuCandidateRenderer::new(900.0, 1.0).preferred_input_panel_height(&folded);
    assert!(expanded_height > folded_height + 100.0);
    let old = WgpuCandidateRenderer::new(900.0, expanded_height)
        .build_panel_scene(&snapshot, &folded, None, None, None, None);
    let fitted = WgpuCandidateRenderer::new(900.0, folded_height)
        .build_panel_scene(&snapshot, &folded, None, None, None, None);
    assert_eq!(
        old.atlas_glyphs, fitted.atlas_glyphs,
        "folding must not shrink fonts or vertically center content in the old viewport"
    );
}
