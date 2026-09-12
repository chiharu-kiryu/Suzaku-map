#![cfg(feature = "gpu")]

use suzaku_map::ime::gpu::{
    DisplayTextScale, InputMode, InteractionKind, PanelChromeState, TextAlign, TextBlock, TextRole,
    ThemePreset, VirtualKeyboardKey, WgpuCandidateRenderer,
};
use suzaku_map::ime::{EngineConfig, XRTabletImeEngine};

fn assert_inside(inner: [f32; 4], outer: [f32; 4]) {
    assert!(
        inner[0] >= outer[0] - 0.01
            && inner[1] >= outer[1] - 0.01
            && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
            && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01,
        "{inner:?} escapes {outer:?}"
    );
}

#[test]
fn control_labels_center_in_the_visible_rect_at_every_size() {
    for text in ["+", "Backspace", "Use transcript"] {
        for height in [16.0, 26.0, 52.0] {
            let rect = [20.0, 40.0, 82.0, height];
            let layout = TextBlock {
                text: text.into(),
                origin: [900.0, 900.0],
                max_width: 400.0,
                pixel_size: 3.4,
                letter_spacing: 0.4,
                line_gap: 3.0,
                max_lines: 2,
                color: [1.0; 4],
                align: TextAlign::Center,
                role: TextRole::ToolButton,
            }
            .layout_in_rect(rect, [4.0, 2.0]);
            assert!(
                (layout.bounds[1] + layout.bounds[3] * 0.5 - (rect[1] + height * 0.5)).abs() < 0.01
            );
            assert!(
                (layout.bounds[0] + layout.bounds[2] * 0.5 - (rect[0] + rect[2] * 0.5)).abs()
                    < 0.01
            );
            for glyph in &layout.atlas_glyphs {
                assert_inside(glyph.rect, rect);
            }
            for quad in &layout.quads {
                assert_inside(quad.rect, rect);
            }
        }
    }
}

#[test]
fn single_symbol_buttons_fit_measured_glyph_widths_without_an_ellipsis() {
    use std::{collections::HashMap, sync::Arc};
    use suzaku_map::ime::gpu::with_font_metrics;
    with_font_metrics(
        Arc::new(HashMap::from([('+', 6.2), ('W', 7.0), ('あ', 7.0)])),
        || {
            for text in ["+", "W", "あ"] {
                for spacing in [-0.4, 0.0, 0.4] {
                    let rect = [10.0, 20.0, 14.04, 18.0];
                    let layout = TextBlock {
                        text: text.into(),
                        origin: [0.0; 2],
                        max_width: 100.0,
                        pixel_size: 3.0,
                        letter_spacing: spacing,
                        line_gap: 0.0,
                        max_lines: 1,
                        color: [1.0; 4],
                        align: TextAlign::Center,
                        role: TextRole::ToolButton,
                    }
                    .layout_in_rect(rect, [1.8, 1.8]);
                    assert_eq!(
                        layout.lines,
                        [text],
                        "a button symbol must not become an ellipsis"
                    );
                    assert!(!layout.truncated);
                    assert_eq!(layout.atlas_glyphs.len(), 1);
                    assert_inside(layout.atlas_glyphs[0].rect, rect);
                }
            }
        },
    );
}

#[test]
fn zero_sized_control_has_no_text_geometry() {
    let block = TextBlock {
        text: "label".into(),
        origin: [0.0; 2],
        max_width: 200.0,
        pixel_size: 3.0,
        letter_spacing: 0.0,
        line_gap: 2.0,
        max_lines: 1,
        color: [1.0; 4],
        align: TextAlign::Center,
        role: TextRole::ToolButton,
    };
    for rect in [[0.0, 0.0, 0.0, 20.0], [0.0, 0.0, 20.0, 0.0]] {
        let layout = block.layout_in_rect(rect, [2.0, 2.0]);
        assert!(layout.atlas_glyphs.is_empty() && layout.quads.is_empty());
    }
}

#[test]
fn keyboard_and_token_labels_match_their_click_targets_across_scales() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    for (width, height) in [(420.0, 300.0), (900.0, 480.0), (1260.0, 672.0)] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            let chrome = PanelChromeState {
                input_modes_expanded: true,
                active_input_mode: InputMode::VirtualKeyboard,
                seed_text: "ni hao".into(),
                text_scale,
                next_token_candidates: vec!["how".into(), "you".into()],
                ..PanelChromeState::default()
            };
            let scene = WgpuCandidateRenderer::new(width, height)
                .build_panel_scene(&snapshot, &chrome, None, None, None, None);
            assert!(
                scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .any(|layout| layout.role == TextRole::ToolButton && layout.lines == ["100%"])
            );
            for target in &scene.interactive_targets {
                let (label, role) = match target.kind {
                    InteractionKind::DecreaseWindowScale => ("-".into(), TextRole::ToolButton),
                    InteractionKind::IncreaseWindowScale => ("+".into(), TextRole::ToolButton),
                    InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character(ch)) => {
                        (ch.to_string(), TextRole::KeyboardKey)
                    }
                    InteractionKind::SelectNextToken(index) => (
                        chrome.next_token_candidates[index].clone(),
                        TextRole::NextTokenChip,
                    ),
                    _ => continue,
                };
                let layout = scene
                    .text_sections
                    .iter()
                    .filter(|section| section.role == role)
                    .flat_map(|section| &section.layouts)
                    .find(|layout| layout.lines == [label.clone()])
                    .unwrap_or_else(|| {
                        panic!("missing {label} at {width}x{height} {text_scale:?}")
                    });
                assert_inside(layout.bounds, target.rect);
                for glyph in &layout.atlas_glyphs {
                    assert_inside(glyph.rect, target.rect);
                }
                assert_eq!(
                    scene.hit_interaction(
                        layout.bounds[0] + layout.bounds[2] * 0.5,
                        layout.bounds[1] + layout.bounds[3] * 0.5
                    ),
                    Some(target.kind)
                );
            }
        }
    }
}

#[test]
fn handwriting_candidate_labels_stay_inside_short_footer_buttons() {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    let snapshot = engine.seed("ni hao");
    for width in [420.0, 900.0] {
        let scene = WgpuCandidateRenderer::new(width, 480.0).build_panel_scene(
            &snapshot,
            &PanelChromeState {
                active_input_mode: InputMode::Handwriting,
                input_modes_expanded: true,
                handwriting_candidates: vec!["O".into(), "A".into()],
                text_scale: DisplayTextScale::Large,
                ..PanelChromeState::default()
            },
            None,
            None,
            None,
            None,
        );
        let target = scene
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::UseHandwritingCandidate(0))
            .expect("handwriting candidate button");
        let layout = scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .find(|layout| layout.role == TextRole::HandwritingCandidate && layout.lines == ["O"])
            .expect("candidate label");
        assert_inside(layout.bounds, target.rect);
        assert_eq!(
            scene.hit_interaction(
                layout.bounds[0] + layout.bounds[2] * 0.5,
                layout.bounds[1] + layout.bounds[3] * 0.5
            ),
            Some(target.kind)
        );
    }
}

#[test]
fn settings_search_expands_matching_options_and_keeps_labels_aligned() {
    for width in [420.0, 520.0, 760.0] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            let chrome = PanelChromeState {
                settings_search_query: "theme".into(),
                settings_collapsed_sections: vec![true; 14],
                text_scale,
                ..PanelChromeState::default()
            };
            let scene =
                WgpuCandidateRenderer::new(width, 340.0).build_settings_scene(&chrome, None);
            for preset in ThemePreset::ALL {
                let label = preset.label();
                let target = scene
                    .interactive_targets
                    .iter()
                    .find(|target| target.kind == InteractionKind::SetThemePreset(preset))
                    .expect("search result target");
                let layout = scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .find(|layout| layout.lines == [label])
                    .expect("complete option label");
                assert!(
                    !layout.truncated,
                    "{label} unexpectedly truncated at {width} {text_scale:?}"
                );
                assert_inside(layout.bounds, target.rect);
                assert_eq!(
                    scene.hit_interaction(
                        layout.bounds[0] + layout.bounds[2] * 0.5,
                        layout.bounds[1] + layout.bounds[3] * 0.5
                    ),
                    Some(target.kind)
                );
            }
        }
    }
}

#[test]
fn settings_scroll_clips_text_and_click_targets_below_the_search_bar() {
    for offset in [0.0, 13.0, 77.0, 10_000.0] {
        let scene = WgpuCandidateRenderer::new(420.0, 300.0).build_settings_scene(
            &PanelChromeState {
                settings_scroll_offset: offset,
                text_scale: DisplayTextScale::Large,
                ..PanelChromeState::default()
            },
            None,
        );
        let scroll = scene.settings_scroll_metadata.expect("scroll metadata");
        let viewport = [0.0, scroll.track_rect[1], 420.0, scroll.visible_height];
        for target in &scene.interactive_targets {
            if matches!(
                target.kind,
                InteractionKind::DragWindow
                    | InteractionKind::SettingsToggle
                    | InteractionKind::SettingsSearchInput
                    | InteractionKind::SettingsSearchClear
                    | InteractionKind::SettingsScrollTrack
                    | InteractionKind::SettingsScrollHandle
            ) {
                continue;
            }
            assert_inside(target.rect, viewport);
        }
        for section in &scene.text_sections {
            if !matches!(
                section.role,
                TextRole::SettingLabel | TextRole::SettingOption
            ) {
                continue;
            }
            for layout in &section.layouts {
                assert_inside(layout.bounds, viewport);
                for glyph in &layout.atlas_glyphs {
                    assert_inside(
                        glyph.clip_rect.expect("scrolling text must be clipped"),
                        viewport,
                    );
                }
            }
        }
    }
}

#[test]
fn empty_settings_search_explains_that_no_options_match() {
    let scene = WgpuCandidateRenderer::new(520.0, 340.0).build_settings_scene(
        &PanelChromeState {
            settings_search_query: "not-a-setting".into(),
            ..PanelChromeState::default()
        },
        None,
    );
    assert!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .any(|layout| layout.lines == ["No matching settings"])
    );
    assert_eq!(
        scene.settings_scroll_metadata.unwrap().max_scroll_offset,
        0.0
    );
}
