#![cfg(feature = "gpu")]

use suzaku_map::{ime::gpu::*, ui::UiLanguage};

fn settings(width: f32, chrome: &PanelChromeState) -> RenderScene {
    let probe = WgpuCandidateRenderer::new(width, 2000.0).build_settings_scene(chrome, None);
    let height = probe
        .settings_scroll_metadata
        .unwrap()
        .preferred_window_height;
    WgpuCandidateRenderer::new(width, height).build_settings_scene(chrome, None)
}

#[test]
fn panel_layout_modes_have_stable_ids_and_default_to_safe_platform_detection() {
    assert_eq!(PanelLayoutMode::default(), PanelLayoutMode::Auto);
    assert_eq!(
        PanelChromeState::default().panel_layout_mode,
        PanelLayoutMode::Auto
    );
    assert_eq!(
        PanelChromeState::default().effective_panel_layout(),
        PanelLayoutMode::FollowCaret
    );
    for mode in PanelLayoutMode::ALL {
        assert_eq!(PanelLayoutMode::from_id(mode.id()), Some(mode));
        assert!(!mode.label().is_empty());
    }
    for invalid in ["", "floating", "BOTTOM-DOCK", "bottom_dock"] {
        assert_eq!(PanelLayoutMode::from_id(invalid), None);
    }
}

#[test]
fn effective_layout_uses_detection_only_for_the_automatic_preference() {
    for detected in PanelLayoutMode::ALL {
        for preference in PanelLayoutMode::ALL {
            let chrome = PanelChromeState {
                panel_layout_mode: preference,
                detected_panel_layout: detected,
                ..Default::default()
            };
            let expected = if preference != PanelLayoutMode::Auto {
                preference
            } else if detected == PanelLayoutMode::BottomDock {
                PanelLayoutMode::BottomDock
            } else {
                PanelLayoutMode::FollowCaret
            };
            assert_eq!(chrome.effective_panel_layout(), expected);
            assert_eq!(chrome.panel_layout_mode, preference);
        }
    }
}

#[test]
fn panel_layout_is_localized_searchable_and_has_keyboard_targets() {
    for ui_language in UiLanguage::ALL {
        for width in [420.0, 620.0, 960.0] {
            for text_scale in [DisplayTextScale::Medium, DisplayTextScale::Large] {
                for search in [
                    "Panel Layout",
                    ui_language.tr("Panel Layout"),
                    "tablet",
                    "手机",
                    "下屏幕",
                ] {
                    for detected_panel_layout in PanelLayoutMode::ALL {
                        let chrome = PanelChromeState {
                            ui_language,
                            text_scale,
                            detected_panel_layout,
                            settings_category: SettingsCategory::Model,
                            settings_search_query: search.into(),
                            ..Default::default()
                        };
                        let scene = settings(width, &chrome);
                        let lines: Vec<_> = scene
                            .text_sections
                            .iter()
                            .flat_map(|section| &section.layouts)
                            .flat_map(|layout| &layout.lines)
                            .map(String::as_str)
                            .collect();
                        assert!(lines.contains(&ui_language.tr("Panel Layout")));
                        for mode in PanelLayoutMode::ALL {
                            let kind = InteractionKind::SetPanelLayoutMode(mode);
                            let rect = scene
                                .interactive_targets
                                .iter()
                                .find(|target| target.kind == kind)
                                .unwrap_or_else(|| {
                                    panic!("missing {kind:?}: {ui_language:?}/{width}/{search}")
                                })
                                .rect;
                            assert_eq!(
                                scene.hit_interaction(
                                    rect[0] + rect[2] / 2.0,
                                    rect[1] + rect[3] / 2.0
                                ),
                                Some(kind)
                            );
                            assert!(
                                scene
                                    .settings_focus_targets
                                    .iter()
                                    .any(|target| target.kind == kind)
                            );
                            assert!(
                                !scene.settings_option_truncated.contains(&kind),
                                "{ui_language:?}/{width}/{text_scale:?}/{kind:?}"
                            );
                            let label = if mode == PanelLayoutMode::Auto {
                                if detected_panel_layout == PanelLayoutMode::BottomDock {
                                    "Auto · Bottom dock"
                                } else {
                                    "Auto · Follow caret"
                                }
                            } else {
                                mode.label()
                            };
                            assert!(
                                lines.contains(&ui_language.tr(label)),
                                "{ui_language:?}/{width}/{text_scale:?}/{label}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn automatic_option_describes_detection_even_when_a_manual_mode_is_selected() {
    for ui_language in UiLanguage::ALL {
        for (detected_panel_layout, panel_layout_mode, label) in [
            (
                PanelLayoutMode::BottomDock,
                PanelLayoutMode::FollowCaret,
                "Auto · Bottom dock",
            ),
            (
                PanelLayoutMode::FollowCaret,
                PanelLayoutMode::BottomDock,
                "Auto · Follow caret",
            ),
        ] {
            let chrome = PanelChromeState {
                ui_language,
                panel_layout_mode,
                detected_panel_layout,
                settings_search_query: "Panel Layout".into(),
                ..Default::default()
            };
            let scene = settings(620.0, &chrome);
            assert!(
                scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .flat_map(|layout| &layout.lines)
                    .any(|line| line == ui_language.tr(label))
            );
            assert!(
                scene.interactive_targets.iter().any(|target| target.kind
                    == InteractionKind::SetPanelLayoutMode(PanelLayoutMode::Auto))
            );
            assert_eq!(chrome.effective_panel_layout(), panel_layout_mode);
            assert_eq!(chrome.panel_layout_mode, panel_layout_mode);
        }
    }
}

#[test]
fn layout_keyboard_focus_does_not_change_the_selected_mode() {
    let mut chrome = PanelChromeState {
        settings_search_query: "Panel Layout".into(),
        ..Default::default()
    };
    let before = settings(620.0, &chrome);
    let kind = InteractionKind::SetPanelLayoutMode(PanelLayoutMode::BottomDock);
    chrome.settings_keyboard_focus = Some(kind);
    let after = settings(620.0, &chrome);
    assert_eq!(chrome.panel_layout_mode, PanelLayoutMode::Auto);
    assert_eq!(after.interactive_targets, before.interactive_targets);
    assert_eq!(after.text_sections, before.text_sections);
    assert_eq!(after.quads.len(), before.quads.len() + 1);
    let target = after
        .settings_focus_targets
        .iter()
        .find(|target| target.kind == kind)
        .unwrap();
    assert_eq!(after.quads.last().unwrap().rect, target.rect);
}
