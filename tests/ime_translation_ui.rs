#![cfg(feature = "gpu")]
use suzaku_map::{
    ime::{XRTabletImeEngine, gpu::*},
    languages::translation::TranslationLanguage,
};

fn scene(width: f32, chrome: &PanelChromeState) -> RenderScene {
    let mut engine = XRTabletImeEngine::new(Default::default());
    let snapshot = engine.seed(&chrome.seed_text);
    let height = WgpuCandidateRenderer::new(width, 1.0).preferred_input_panel_height(chrome);
    WgpuCandidateRenderer::new(width, height)
        .build_panel_scene(&snapshot, chrome, None, None, None, None)
}

#[test]
fn translation_selectors_and_actions_are_reachable_without_hiding_drafts() {
    for width in [420.0, 620.0, 900.0, 1395.0] {
        for text_scale in [
            DisplayTextScale::Small,
            DisplayTextScale::Medium,
            DisplayTextScale::Large,
        ] {
            let chrome = PanelChromeState {
                seed_text: "Hello, world!".into(),
                active_input_mode: InputMode::Translation,
                input_modes_expanded: true,
                text_scale,
                sentence_candidates: vec!["unrelated candidate".into()],
                next_token_candidates: vec!["unrelated word".into()],
                ..Default::default()
            };
            let rendered = scene(width, &chrome);
            for kind in std::iter::once(InteractionKind::SetTranslationSource(None))
                .chain(
                    TranslationLanguage::ALL
                        .map(|l| InteractionKind::SetTranslationSource(Some(l))),
                )
                .chain(TranslationLanguage::ALL.map(InteractionKind::SetTranslationTarget))
                .chain([InteractionKind::TranslateText, InteractionKind::SeedInput])
            {
                let target = rendered
                    .interactive_targets
                    .iter()
                    .find(|t| t.kind == kind)
                    .unwrap_or_else(|| panic!("missing {kind:?}"));
                assert!(target.rect[2] > 10.0 && target.rect[3] > 10.0);
                assert_eq!(
                    rendered.hit_interaction(
                        target.rect[0] + target.rect[2] / 2.0,
                        target.rect[1] + target.rect[3] / 2.0
                    ),
                    Some(kind),
                    "{width} {text_scale:?}"
                );
            }
            assert!(!rendered.interactive_targets.iter().any(|t| matches!(
                t.kind,
                InteractionKind::ApplyTranslation
                    | InteractionKind::Candidate(_)
                    | InteractionKind::SelectNextToken(_)
            )));
            assert!(
                rendered
                    .text_sections
                    .iter()
                    .flat_map(|s| &s.layouts)
                    .any(|l| l.lines.iter().any(|line| line.contains("Hello")))
            );
            let mut pending = chrome.clone();
            pending.translation.phase = TranslationPhase::Pending;
            let rendered = scene(width, &pending);
            assert!(
                rendered
                    .interactive_targets
                    .iter()
                    .any(|t| t.kind == InteractionKind::CancelTranslation)
            );
            assert!(!rendered.interactive_targets.iter().any(|t| matches!(
                t.kind,
                InteractionKind::ApplyTranslation | InteractionKind::TranslateText
            )));
        }
    }
}

#[test]
fn long_translations_can_be_read_to_the_end_and_controls_do_not_overlap() {
    for width in [420.0, 720.0, 1250.0] {
        let mut chrome = PanelChromeState {
            active_input_mode: InputMode::Translation,
            input_modes_expanded: true,
            seed_text: "Original".into(),
            ..Default::default()
        };
        chrome.translation.phase = TranslationPhase::Ready;
        chrome.translation.text =
            format!("{}FINAL-END", "你好 hello 안녕하세요 Bonjour! ".repeat(50));
        let mut reached_end = false;
        for page in 0..100 {
            chrome.translation.page = page;
            let rendered = scene(width, &chrome);
            let texts: Vec<_> = rendered
                .text_sections
                .iter()
                .filter(|s| s.role == TextRole::TranslationText)
                .flat_map(|s| &s.layouts)
                .flat_map(|l| &l.lines)
                .cloned()
                .collect();
            if texts.join("").contains("FINAL-END") {
                reached_end = true;
                break;
            }
            let next = InteractionKind::TranslationPage(page + 1);
            assert!(rendered.interactive_targets.iter().any(|t| t.kind == next));
            for target in rendered.interactive_targets.iter().filter(|t| {
                matches!(
                    t.kind,
                    InteractionKind::TranslateText
                        | InteractionKind::ApplyTranslation
                        | InteractionKind::TranslationPage(_)
                )
            }) {
                assert_eq!(
                    rendered.hit_interaction(
                        target.rect[0] + target.rect[2] / 2.0,
                        target.rect[1] + target.rect[3] / 2.0
                    ),
                    Some(target.kind)
                );
            }
        }
        assert!(reached_end);
    }
}
