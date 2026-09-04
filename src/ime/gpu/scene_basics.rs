use super::*;

impl WgpuCandidateRenderer {
    pub fn new(scene_width: f32, scene_height: f32) -> Self {
        Self {
            scene_width,
            scene_height,
        }
    }

    pub(super) fn responsive_scale(&self) -> f32 {
        let width_factor = self.scene_width / 900.0;
        let height_factor = self.scene_height / 780.0;
        (width_factor * 0.65 + height_factor * 0.35).clamp(0.8, 1.8)
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
        let page_bg = theme.page_bg;
        let halo = if pressed {
            [0.42, 0.70, 0.97, 0.24]
        } else if hovered {
            [0.48, 0.75, 1.0, 0.20]
        } else {
            [0.40, 0.66, 0.95, 0.12]
        };
        let shell = if pressed {
            [0.71, 0.83, 0.96, 1.0]
        } else if hovered {
            [0.78, 0.88, 0.98, 1.0]
        } else {
            [0.73, 0.84, 0.97, 1.0]
        };
        let shell_inner = if pressed {
            [0.87, 0.93, 0.99, 1.0]
        } else {
            [0.91, 0.96, 1.0, 1.0]
        };
        let shell_core = if pressed {
            [0.96, 0.98, 1.0, 1.0]
        } else {
            [0.98, 0.99, 1.0, 1.0]
        };
        let shell_shadow = match chrome.theme_preset {
            ThemePreset::Daylight => [0.19, 0.28, 0.41, 0.24],
            ThemePreset::Solarized => [0.31, 0.22, 0.12, 0.24],
            ThemePreset::DeviceDark => [0.01, 0.03, 0.07, 0.42],
            ThemePreset::HighContrast => [0.00, 0.00, 0.00, 0.52],
        };
        let glass_ring = if pressed {
            [0.90, 0.95, 1.0, 0.22]
        } else if hovered {
            [0.93, 0.97, 1.0, 0.24]
        } else {
            [0.88, 0.94, 1.0, 0.18]
        };
        let contact_shadow = match chrome.theme_preset {
            ThemePreset::Daylight => [0.15, 0.23, 0.35, 0.16],
            ThemePreset::Solarized => [0.24, 0.16, 0.09, 0.16],
            ThemePreset::DeviceDark => [0.01, 0.02, 0.05, 0.28],
            ThemePreset::HighContrast => [0.06, 0.06, 0.06, 0.38],
        };
        let bird_primary = if pressed {
            [0.77, 0.16, 0.16, 1.0]
        } else if hovered {
            [0.84, 0.19, 0.19, 1.0]
        } else {
            [0.85, 0.23, 0.23, 1.0]
        };
        let bird_secondary = [0.95, 0.47, 0.40, 1.0];
        let bird_beak = [0.96, 0.67, 0.30, 1.0];
        let bird_eye = [0.44, 0.09, 0.09, 1.0];
        let mut quads = Vec::new();
        let text_quads = Vec::new();
        let atlas_glyphs = Vec::new();
        let text_sections = Vec::new();
        let hit_targets = Vec::new();
        quads.push(CandidateQuad {
            rect: [0.0, 0.0, self.scene_width, self.scene_height],
            color: page_bg,
        });
        let orb_size = self.scene_width.min(self.scene_height) - 16.0;
        let orb_size = orb_size.clamp(56.0, 92.0);
        let orb_rect = [
            (self.scene_width - orb_size) / 2.0,
            (self.scene_height - orb_size) / 2.0,
            orb_size,
            orb_size,
        ];
        let halo_rect = [
            orb_rect[0] - orb_size * 0.10,
            orb_rect[1] - orb_size * 0.10,
            orb_size * 1.20,
            orb_size * 1.20,
        ];
        let contact_rect = [
            orb_rect[0] + orb_size * 0.18,
            orb_rect[1] + orb_size * 0.90,
            orb_size * 0.64,
            orb_size * 0.09,
        ];
        let ring_rect = [
            orb_rect[0] + orb_size * 0.08,
            orb_rect[1] + orb_size * 0.08,
            orb_size * 0.84,
            orb_size * 0.84,
        ];
        let inner_rect = [
            orb_rect[0] + orb_size * 0.11,
            orb_rect[1] + orb_size * 0.11,
            orb_size * 0.78,
            orb_size * 0.78,
        ];
        let core_rect = [
            orb_rect[0] + orb_size * 0.24,
            orb_rect[1] + orb_size * 0.24,
            orb_size * 0.52,
            orb_size * 0.52,
        ];
        append_rounded_rect_quads(&mut quads, halo_rect, halo, halo_rect[2] * 0.5);
        append_rounded_rect_quads(
            &mut quads,
            contact_rect,
            contact_shadow,
            contact_rect[3] * 0.5,
        );
        append_soft_card_quads(
            &mut quads,
            orb_rect,
            shell,
            [0.41, 0.55, 0.73, 1.0],
            shell_shadow,
            theme.shell,
            orb_rect[2] * 0.5,
        );
        append_rounded_rect_quads(&mut quads, ring_rect, glass_ring, ring_rect[2] * 0.5);
        append_rounded_rect_quads(
            &mut quads,
            [
                orb_rect[0] + orb_size * 0.04,
                orb_rect[1] + orb_size * 0.04,
                orb_size * 0.92,
                orb_size * 0.18,
            ],
            [1.0, 1.0, 1.0, 0.10],
            orb_rect[2] * 0.26,
        );
        append_rounded_rect_quads(&mut quads, inner_rect, shell_inner, inner_rect[2] * 0.5);
        append_rounded_rect_quads(
            &mut quads,
            [
                inner_rect[0] + orb_size * 0.03,
                inner_rect[1] + orb_size * 0.03,
                inner_rect[2] - orb_size * 0.06,
                inner_rect[3] * 0.28,
            ],
            [1.0, 1.0, 1.0, 0.12],
            inner_rect[2] * 0.18,
        );
        append_rounded_rect_quads(&mut quads, core_rect, shell_core, core_rect[2] * 0.5);
        append_rounded_rect_quads(
            &mut quads,
            [
                orb_rect[0] + orb_size * 0.22,
                orb_rect[1] + orb_size * 0.80,
                orb_size * 0.56,
                orb_size * 0.06,
            ],
            [0.33, 0.63, 0.96, 0.85],
            orb_size * 0.03,
        );
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
        let icon_rect = [
            orb_rect[0] + orb_size * 0.15,
            orb_rect[1] + orb_size * 0.15,
            orb_size * 0.70,
            orb_size * 0.70,
        ];
        append_suzaku_bird_icon_quads(
            &mut quads,
            icon_rect,
            bird_primary,
            bird_secondary,
            bird_beak,
            bird_eye,
        );
        if !snapshot.candidate_labels.is_empty() {
            append_rounded_rect_quads(
                &mut quads,
                [
                    orb_rect[0] + orb_size * 0.74,
                    orb_rect[1] + orb_size * 0.18,
                    orb_size * 0.12,
                    orb_size * 0.12,
                ],
                [0.96, 0.33, 0.30, 1.0],
                orb_size * 0.06,
            );
            append_rounded_rect_quads(
                &mut quads,
                [
                    orb_rect[0] + orb_size * 0.78,
                    orb_rect[1] + orb_size * 0.22,
                    orb_size * 0.04,
                    orb_size * 0.04,
                ],
                [1.0, 0.95, 0.95, 0.95],
                orb_size * 0.02,
            );
        }
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
    fn default_panel_spans_an_unpaired_candidate_across_the_last_row() {
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.seed("ni hao");
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
