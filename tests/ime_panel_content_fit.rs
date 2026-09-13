#![cfg(feature = "gpu")]

use suzaku_map::ime::gpu::{
    DisplayTextScale, InputMode, InteractionKind, PanelChromeState, QuadShape, TextRole,
    WgpuCandidateRenderer,
};
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

#[test]
fn completion_history_keeps_a_compact_back_row_without_next_word_candidates() {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: "en".into(),
        ..Default::default()
    });
    let snapshot = engine.seed("hello world");
    for width in [420.0, 630.0, 900.0, 1600.0, 2500.0] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            for expanded in [false, true] {
                for mode in [
                    InputMode::VirtualKeyboard,
                    InputMode::Dictation,
                    InputMode::Handwriting,
                    InputMode::Translation,
                ] {
                    for sentence_count in [0, 3] {
                        let chrome = PanelChromeState {
                            seed_text: "hello world".into(),
                            composed_tokens: vec!["hello".into(), "world".into()],
                            next_token_candidates: Vec::new(),
                            sentence_candidates: (0..sentence_count)
                                .map(|i| format!("hello world {i}"))
                                .collect(),
                            input_modes_expanded: expanded,
                            active_input_mode: mode,
                            text_scale,
                            ..Default::default()
                        };
                        let sizing = WgpuCandidateRenderer::new(width, 1.0);
                        let height = sizing.preferred_input_panel_height(&chrome);
                        let scene = WgpuCandidateRenderer::new(width, height)
                            .build_panel_scene(&snapshot, &chrome, None, None, None, None);
                        let back = scene
                            .interactive_targets
                            .iter()
                            .find(|target| target.kind == InteractionKind::RewindNextToken);
                        let without_history = PanelChromeState {
                            composed_tokens: Vec::new(),
                            ..chrome.clone()
                        };
                        let empty_height = sizing.preferred_input_panel_height(&without_history);
                        if expanded && mode == InputMode::Translation {
                            assert!(back.is_none(), "translation must not expose draft undo");
                            assert_eq!(height, empty_height);
                            continue;
                        }
                        let back = back.unwrap_or_else(|| {
                            panic!(
                                "N18: missing Back with pending history at {width}, {text_scale:?}, expanded={expanded}, {mode:?}, sentences={sentence_count}"
                            )
                        });
                        assert_eq!(
                            scene.hit_interaction(
                                back.rect[0] + back.rect[2] * 0.5,
                                back.rect[1] + back.rect[3] * 0.5,
                            ),
                            Some(InteractionKind::RewindNextToken),
                        );
                        assert!(back.rect[1] >= 0.0 && back.rect[1] + back.rect[3] <= height);
                        assert!(height > empty_height, "history must reserve its own row");
                        assert_eq!(
                            WgpuCandidateRenderer::new(width, height)
                                .preferred_input_panel_height(&chrome),
                            height,
                            "history-only sizing must not create a resize loop"
                        );
                        let full_height = sizing.preferred_input_panel_height(&PanelChromeState {
                            next_token_candidates: vec!["again".into()],
                            ..chrome.clone()
                        });
                        if expanded {
                            assert!(height < full_height, "do not keep empty word-chip rows");
                        } else {
                            assert_eq!(height, full_height);
                        }
                        for layout in scene
                            .text_sections
                            .iter()
                            .filter(|section| {
                                matches!(
                                    section.role,
                                    TextRole::NextTokenLabel | TextRole::NextTokenChip
                                )
                            })
                            .flat_map(|section| &section.layouts)
                        {
                            assert!(
                                scene.quads.iter().any(|quad| {
                                    // Ignore the enclosing panel and drop shadows:
                                    // the compact row/button itself must fit its text.
                                    matches!(quad.shape, QuadShape::Rounded { stroke, .. } if stroke > 0.0)
                                        && quad.rect[3] <= height - empty_height + 1.0
                                        && layout.bounds[0] >= quad.rect[0] - 0.05
                                        && layout.bounds[1] >= quad.rect[1] - 0.05
                                        && layout.bounds[0] + layout.bounds[2]
                                            <= quad.rect[0] + quad.rect[2] + 0.05
                                        && layout.bounds[1] + layout.bounds[3]
                                            <= quad.rect[1] + quad.rect[3] + 0.05
                                }),
                                "history text {:?} escapes its compact card at {width}, {text_scale:?}, expanded={expanded}, {mode:?}, sentences={sentence_count}",
                                layout.bounds
                            );
                        }
                        let mut candidates = 0;
                        for target in &scene.interactive_targets {
                            assert!(!matches!(target.kind, InteractionKind::SelectNextToken(_)));
                            if let InteractionKind::Candidate(index) = target.kind {
                                candidates += 1;
                                let card = scene
                                    .hit_targets
                                    .iter()
                                    .find(|card| card.index == index)
                                    .unwrap();
                                // Touch padding can extend into the inter-row gap,
                                // but undo must not encroach on a sentence card.
                                assert!(
                                    back.rect[1] + back.rect[3] <= card.rect[1],
                                    "Back {:?} overlaps sentence {:?} at {width}, {text_scale:?}, expanded={expanded}, {mode:?}, height={height}",
                                    back.rect,
                                    card.rect
                                );
                                assert_eq!(
                                    scene.hit_interaction(
                                        card.rect[0] + card.rect[2] * 0.5,
                                        card.rect[1] + card.rect[3] * 0.5,
                                    ),
                                    Some(target.kind)
                                );
                                for layout in scene
                                    .text_sections
                                    .iter()
                                    .filter(|section| section.role == TextRole::NextTokenLabel)
                                    .flat_map(|section| &section.layouts)
                                {
                                    assert!(
                                        layout.bounds[1] + layout.bounds[3] <= card.rect[1],
                                        "history label {:?} overlaps sentence {:?} at {width}, {text_scale:?}, expanded={expanded}, {mode:?}",
                                        layout.bounds,
                                        card.rect
                                    );
                                }
                            }
                        }
                        assert_eq!(candidates, sentence_count);
                        let empty_scene = WgpuCandidateRenderer::new(width, empty_height)
                            .build_panel_scene(&snapshot, &without_history, None, None, None, None);
                        assert!(
                            empty_scene
                                .interactive_targets
                                .iter()
                                .all(|target| { target.kind != InteractionKind::RewindNextToken })
                        );
                    }
                }
            }
        }
    }
}
