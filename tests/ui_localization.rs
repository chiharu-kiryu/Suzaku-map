#![cfg(feature = "gpu")]
use suzaku_map::{
    ime::{XRTabletImeEngine, gpu::*},
    ui::UiLanguage,
};

fn settings(width: f32, chrome: &PanelChromeState) -> RenderScene {
    let probe = WgpuCandidateRenderer::new(width, 2000.0).build_settings_scene(chrome, None);
    let height = probe
        .settings_scroll_metadata
        .unwrap()
        .preferred_window_height;
    WgpuCandidateRenderer::new(width, height).build_settings_scene(chrome, None)
}

fn assert_hit(scene: &RenderScene, kind: InteractionKind) {
    let rect = scene
        .interactive_targets
        .iter()
        .find(|t| t.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?}"))
        .rect;
    assert_eq!(
        scene.hit_interaction(rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0),
        Some(kind)
    );
}

#[test]
fn eight_interfaces_reflow_and_search_in_both_localized_and_original_labels() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                let mut chrome = PanelChromeState {
                    ui_language,
                    text_scale,
                    ..Default::default()
                };
                let scene = settings(width, &chrome);
                for language in UiLanguage::ALL {
                    let kind = InteractionKind::SetUiLanguage(language);
                    assert_hit(&scene, kind);
                    assert!(
                        !scene.settings_option_truncated.contains(&kind),
                        "{ui_language:?} {width} {text_scale:?}"
                    );
                }
                for layout in scene
                    .text_sections
                    .iter()
                    .filter(|s| s.role == TextRole::SettingLabel)
                    .flat_map(|s| &s.layouts)
                {
                    assert!(
                        !layout.truncated,
                        "{ui_language:?} {width} {text_scale:?}: {:?}",
                        layout.lines
                    );
                }
                for search in [ui_language.tr("Theme"), "Theme"] {
                    chrome.settings_search_query = search.into();
                    let scene = settings(width, &chrome);
                    for theme in ThemePreset::ALL {
                        assert_hit(&scene, InteractionKind::SetThemePreset(theme));
                    }
                }
            }
        }
    }
}

#[test]
fn chrome_localization_never_translates_user_drafts_candidates_or_model_output() {
    for ui_language in UiLanguage::ALL {
        let mut chrome = PanelChromeState {
            ui_language,
            seed_text: "Settings".into(),
            input_modes_expanded: true,
            active_input_mode: InputMode::Translation,
            translation: TranslationView {
                phase: TranslationPhase::Ready,
                text: "Theme".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut engine = XRTabletImeEngine::new(Default::default());
        let snapshot = engine.seed(&chrome.seed_text);
        for width in [420.0, 720.0] {
            chrome.text_scale = DisplayTextScale::Large;
            let height =
                WgpuCandidateRenderer::new(width, 1.0).preferred_input_panel_height(&chrome);
            let scene = WgpuCandidateRenderer::new(width, height)
                .build_panel_scene(&snapshot, &chrome, None, None, None, None);
            for (role, expected) in [
                (TextRole::InputValue, "Settings"),
                (TextRole::TranslationText, "Theme"),
            ] {
                assert!(
                    scene
                        .text_sections
                        .iter()
                        .flat_map(|s| &s.layouts)
                        .filter(|layout| layout.role == role)
                        .flat_map(|l| &l.lines)
                        .any(|line| line == expected),
                    "{ui_language:?} {width} {role:?}: {:?}",
                    scene
                        .text_sections
                        .iter()
                        .map(|s| (
                            s.role,
                            s.layouts.iter().flat_map(|l| &l.lines).collect::<Vec<_>>()
                        ))
                        .collect::<Vec<_>>()
                );
            }
            for kind in [
                InteractionKind::TranslateText,
                InteractionKind::ApplyTranslation,
            ] {
                assert_hit(&scene, kind);
            }
            assert!(
                scene
                    .text_sections
                    .iter()
                    .filter(|s| s.role == TextRole::TranslationButton)
                    .flat_map(|s| &s.layouts)
                    .all(|l| !l.truncated),
                "{ui_language:?} {width}"
            );
        }
        chrome.active_input_mode = InputMode::VirtualKeyboard;
        chrome.input_modes_expanded = false;
        chrome.sentence_candidates = vec!["Settings".into()];
        chrome.sentence_candidate_source_indices = vec![0];
        let scene = WgpuCandidateRenderer::new(900.0, 500.0)
            .build_panel_scene(&snapshot, &chrome, None, None, None, None);
        assert!(
            scene
                .text_sections
                .iter()
                .filter(|s| s.role == TextRole::CandidatePrimary)
                .flat_map(|s| &s.layouts)
                .flat_map(|l| &l.lines)
                .any(|line| line.contains("Settings"))
        );
    }
}
