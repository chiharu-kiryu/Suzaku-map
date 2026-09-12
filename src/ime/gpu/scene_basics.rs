use super::*;

impl WgpuCandidateRenderer {
    pub fn new(scene_width: f32, scene_height: f32) -> Self {
        Self {
            scene_width,
            scene_height,
        }
    }

    pub(super) fn responsive_scale(&self) -> f32 {
        // Height is an output of layout, not an input to control scaling.
        self.scene_width / 1000.0
    }

    /// Natural height in scene pixels, independent of the current viewport height.
    /// Settings use a separate desktop window and are excluded here.
    pub fn preferred_input_panel_height(&self, chrome: &PanelChromeState) -> f32 {
        let scale = (self.responsive_scale() * 1.12).clamp(0.9, 2.4);
        let metrics = PanelSceneMetrics::new(
            self.scene_width,
            f32::MAX,
            scale,
            chrome,
            chrome.sentence_candidates.len().min(4),
            self.scene_width < 1360.0 && chrome.preview_style == PreviewStyle::Compact,
        );
        (metrics.panel_height + 12.0 * scale).ceil()
    }

    pub(super) fn interaction_hit_rect(
        &self,
        rect: [f32; 4],
        scale: f32,
        pointer_target_slop_tenths: u16,
        pad_ratio: f32,
        width_growth: f32,
        height_growth: f32,
        include_min_size: bool,
    ) -> [f32; 4] {
        let interaction_hit_padding = (2.2 * scale) + (pointer_target_slop_tenths as f32 / 10.0);
        let mut hit_w = rect[2] + interaction_hit_padding * width_growth;
        let mut hit_h = rect[3] + interaction_hit_padding * height_growth;
        if include_min_size {
            let min_w = (22.0 + pointer_target_slop_tenths as f32 * 0.08) * scale;
            let min_h = (18.0 + pointer_target_slop_tenths as f32 * 0.06) * scale;
            hit_w = hit_w.max(min_w);
            hit_h = hit_h.max(min_h);
        }
        let pad_x = (interaction_hit_padding * pad_ratio).max(0.0);
        let pad_y = (interaction_hit_padding * pad_ratio).max(0.0);
        [
            (rect[0] - pad_x).max(0.0),
            (rect[1] - pad_y).max(0.0),
            hit_w,
            hit_h,
        ]
    }

    pub fn build_scene(&self, snapshot: &Snapshot) -> RenderScene {
        let chrome = PanelChromeState {
            seed_text: snapshot.seed_text.clone(),
            sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
            sentence_candidate_source_indices: (0..snapshot.candidate_labels.len())
                .take(4)
                .collect(),
            input_modes_expanded: true,
            ..PanelChromeState::default()
        };
        self.build_panel_scene(snapshot, &chrome, None, None, None, None)
    }

    pub fn build_compact_scene(
        &self,
        snapshot: &Snapshot,
        chrome: &PanelChromeState,
        hovered: bool,
        pressed: bool,
    ) -> RenderScene {
        let theme = PanelTheme::for_preset(chrome.theme_preset);
        let orb_size = (self.scene_width.min(self.scene_height) - 16.0).clamp(1.0, 92.0);
        let orb_rect = [
            (self.scene_width - orb_size) * 0.5,
            (self.scene_height - orb_size) * 0.5,
            orb_size,
            orb_size,
        ];
        let mut quads = Vec::new();
        // No opaque viewport quad: compositing-capable desktops get a truly round orb.
        for (spread, alpha) in [(5.0, 0.025), (3.0, 0.04), (1.0, 0.07)] {
            quads.push(CandidateQuad::rounded(
                [
                    orb_rect[0] - spread,
                    orb_rect[1] - spread + 1.5,
                    orb_size + spread * 2.0,
                    orb_size + spread * 2.0,
                ],
                [
                    theme.soft_shadow[0],
                    theme.soft_shadow[1],
                    theme.soft_shadow[2],
                    if pressed { alpha * 0.5 } else { alpha },
                ],
                orb_size,
            ));
        }
        if chrome.theme_preset.is_guardian() {
            quads.extend(theme_badge_quads(
                chrome.theme_preset,
                orb_rect,
                hovered,
                pressed,
            ));
        } else {
            let mut fill = theme.surface;
            if hovered || pressed {
                fill = theme.accent_soft;
            }
            quads.push(CandidateQuad::rounded(orb_rect, fill, orb_size * 0.5));
            quads.push(CandidateQuad::outline(
                orb_rect,
                theme.accent,
                orb_size * 0.5,
                if hovered { 1.6 } else { 0.9 },
            ));
            append_suzaku_bird_icon_quads(
                &mut quads,
                [
                    orb_rect[0] + orb_size * 0.12,
                    orb_rect[1] + orb_size * 0.10,
                    orb_size * 0.80,
                    orb_size * 0.80,
                ],
                theme.accent,
                theme.accent_text,
                srgb_color(0xC79B59),
                theme.text_primary,
            );
        }
        if !snapshot.candidate_labels.is_empty() {
            let dot = [
                orb_rect[0] + orb_size * 0.79,
                orb_rect[1] + orb_size * 0.12,
                orb_size * 0.10,
                orb_size * 0.10,
            ];
            let (dot_fill, dot_border) = if chrome.theme_preset == ThemePreset::Suzaku {
                (srgb_color(0xFFF1D0), srgb_color(0x8B2933))
            } else {
                (theme.accent_soft, theme.accent)
            };
            quads.push(CandidateQuad::rounded(dot, dot_fill, orb_size));
            quads.push(CandidateQuad::outline(dot, dot_border, orb_size, 0.7));
        }
        let text_quads = Vec::new();
        let atlas_glyphs = Vec::new();
        let text_sections = Vec::new();
        let hit_targets = Vec::new();
        let compact_hit_rect = self.interaction_hit_rect(
            orb_rect,
            1.0,
            chrome.pointer_target_slop_tenths,
            1.0,
            2.0,
            2.0,
            false,
        );
        let mut targets = vec![InteractiveTarget {
            kind: InteractionKind::ToggleCompactMode,
            rect: compact_hit_rect,
        }];
        RenderScene {
            quads,
            text_quads,
            atlas_glyphs,
            text_sections,
            hit_targets,
            interactive_targets: std::mem::take(&mut targets),
            labels: snapshot.candidate_labels.clone(),
            sentence_candidate_truncated: Vec::new(),
            next_token_candidate_truncated: Vec::new(),
            handwriting_candidate_truncated: Vec::new(),
            settings_option_truncated: Vec::new(),
            settings_scroll_metadata: None,
            selected_label: snapshot
                .candidate_labels
                .get(snapshot.selected_index)
                .cloned(),
            draft_text: snapshot.draft_text.clone(),
        }
        .with_window_drag_background(self.scene_width, self.scene_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ime::{EngineConfig, XRTabletImeEngine};

    #[test]
    fn scaled_expanded_panel_keeps_text_inside_the_viewport() {
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.seed("ni hao");
        let snapshot = engine.snapshot();
        let chrome = PanelChromeState {
            seed_text: "ni hao".to_string(),
            caret_index: "ni hao".chars().count(),
            input_modes_expanded: true,
            window_scale: 1.4,
            next_token_candidates: vec![
                "how".to_string(),
                "you".to_string(),
                "new".to_string(),
                "is".to_string(),
            ],
            sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
            sentence_candidate_source_indices: (0..snapshot.candidate_labels.len())
                .take(4)
                .collect(),
            ..PanelChromeState::default()
        };
        let renderer = WgpuCandidateRenderer::new(1_260.0, 728.0);
        let scene = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);

        let lowest_glyph = scene
            .atlas_glyphs
            .iter()
            .max_by(|left, right| {
                (left.rect[1] + left.rect[3]).total_cmp(&(right.rect[1] + right.rect[3]))
            })
            .expect("rendered text");
        let lowest_text_edge = lowest_glyph.rect[1] + lowest_glyph.rect[3];
        let lowest_layout = scene
            .text_sections
            .iter()
            .flat_map(|section| section.layouts.iter())
            .max_by(|left, right| {
                (left.bounds[1] + left.bounds[3]).total_cmp(&(right.bounds[1] + right.bounds[3]))
            })
            .expect("text layout");
        assert!(
            lowest_text_edge <= renderer.scene_height + 0.01,
            "glyph {:?} extends to {lowest_text_edge}, beyond {} (rect {:?}); lowest layout role {:?}, bounds {:?}",
            lowest_glyph.ch,
            renderer.scene_height,
            lowest_glyph.rect,
            lowest_layout.role,
            lowest_layout.bounds,
        );

        let drag_target = scene
            .interactive_targets
            .iter()
            .rev()
            .find(|target| target.kind == InteractionKind::DragWindow)
            .expect("expanded panel drag target");
        let drag_center_x = drag_target.rect[0] + drag_target.rect[2] * 0.5;
        let drag_center_y = drag_target.rect[1] + drag_target.rect[3] * 0.5;
        assert_eq!(
            scene.hit_interaction(drag_center_x, drag_center_y),
            Some(InteractionKind::DragWindow)
        );
    }

    #[test]
    fn collapsed_cjk_candidates_share_width_instead_of_hiding_secondary_readings() {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: "ja".into(),
            ..Default::default()
        });
        let snapshot = engine.seed("nihongo");
        let chrome = PanelChromeState {
            seed_text: "nihongo".into(),
            input_modes_expanded: false,
            sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
            sentence_candidate_source_indices: (0..4).collect(),
            ..Default::default()
        };
        let scene = WgpuCandidateRenderer::new(1000.0, 620.0)
            .build_panel_scene(&snapshot, &chrome, None, None, None, None);
        assert_eq!(scene.hit_targets.len(), 4);
        let width = scene.hit_targets[0].rect[2];
        assert!(
            scene
                .hit_targets
                .iter()
                .all(|target| (target.rect[2] - width).abs() < 0.01)
        );
        assert!(
            scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .any(|layout| layout.lines.iter().any(|line| line.contains("にほんご")))
        );
    }

    #[test]
    fn short_english_word_chips_fit_in_expanded_and_collapsed_panels() {
        let mut engine = XRTabletImeEngine::new(Default::default());
        let snapshot = engine.seed("hel");
        for expanded in [false, true] {
            for text_scale in [
                DisplayTextScale::Small,
                DisplayTextScale::Medium,
                DisplayTextScale::Large,
            ] {
                let chrome = PanelChromeState {
                    seed_text: "hel".into(),
                    input_modes_expanded: expanded,
                    text_scale,
                    next_token_candidates: vec!["hello".into(), "help".into(), "helpful".into()],
                    sentence_candidates: snapshot
                        .candidate_labels
                        .iter()
                        .take(4)
                        .cloned()
                        .collect(),
                    sentence_candidate_source_indices: (0..4).collect(),
                    ..Default::default()
                };
                let scene = WgpuCandidateRenderer::new(1000.0, 620.0)
                    .build_panel_scene(&snapshot, &chrome, None, None, None, None);
                let chips: Vec<_> = scene
                    .text_sections
                    .iter()
                    .filter(|section| section.role == TextRole::NextTokenChip)
                    .flat_map(|section| &section.layouts)
                    .collect();
                assert_eq!(chips.len(), 3);
                assert!(
                    chips.iter().all(|layout| !layout.truncated),
                    "{expanded} / {text_scale:?}"
                );
                assert!(chips[0].lines[0].contains("hello"));
                if !expanded {
                    let width = scene.hit_targets[0].rect[2];
                    assert!(
                        scene
                            .hit_targets
                            .iter()
                            .all(|target| (target.rect[2] - width).abs() < 0.01)
                    );
                }
            }
        }
    }

    #[test]
    fn default_panel_spans_an_unpaired_candidate_across_the_last_row() {
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.seed("hello");
        let snapshot = engine.snapshot();
        let chrome = PanelChromeState {
            seed_text: "ni hao".to_string(),
            input_modes_expanded: true,
            next_token_candidates: vec!["how".to_string(), "you".to_string()],
            sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
            sentence_candidate_source_indices: (0..snapshot.candidate_labels.len())
                .take(4)
                .collect(),
            ..PanelChromeState::default()
        };
        let renderer = WgpuCandidateRenderer::new(900.0, 480.0);
        let scene = renderer.build_panel_scene(&snapshot, &chrome, None, None, None, None);
        assert!(scene.hit_targets.len() >= 4, "four sentence candidates");

        let hero = scene.hit_targets[0].rect;
        let first_alternate = scene.hit_targets[1].rect;
        let second_alternate = scene.hit_targets[2].rect;
        let last_alternate = scene.hit_targets[3].rect;
        assert!((first_alternate[1] - second_alternate[1]).abs() < 0.01);
        assert!(last_alternate[1] > first_alternate[1]);
        assert!((last_alternate[0] - hero[0]).abs() < 0.01);
        assert!((last_alternate[2] - hero[2]).abs() < 0.01);
        assert!(hero[0] <= 16.0, "left gutter should stay compact");
        assert!(
            900.0 - hero[0] - hero[2] <= 16.0,
            "right gutter should stay compact"
        );

        let close_target = scene
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::ClosePanel)
            .expect("expanded panel close target");
        let close_center_x = close_target.rect[0] + close_target.rect[2] * 0.5;
        let close_center_y = close_target.rect[1] + close_target.rect[3] * 0.5;
        assert_eq!(
            scene.hit_interaction(close_center_x, close_center_y),
            Some(InteractionKind::ClosePanel)
        );
    }
}
