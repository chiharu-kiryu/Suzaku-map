use super::*;
use crate::platform::voice_host::{voice_status_text, voice_transcript_placeholder};

impl WgpuCandidateRenderer {
    pub fn build_panel_scene(&self, snapshot: &Snapshot, chrome: &PanelChromeState) -> RenderScene {
        if chrome.compact_mode {
            return self.build_compact_scene(snapshot, chrome, false, false);
        }
        let theme = PanelTheme::for_preset(chrome.theme_preset);
        let page_bg = theme.page_bg;
        let shell = theme.shell;
        let shell_border = theme.shell_border;
        let surface = theme.surface;
        let surface_alt = theme.surface_alt;
        let accent = theme.accent;
        let surface_muted = theme.surface_muted;
        let accent_soft = theme.accent_soft;
        let accent_text = theme.accent_text;
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;
        let text_muted = theme.text_muted;
        let keyboard_surface = theme.keyboard_surface;
        let keyboard_special_surface = theme.keyboard_special_surface;
        let keyboard_text = theme.keyboard_text;
        let keyboard_secondary_text = theme.keyboard_secondary_text;
        let border_dark = theme.border_dark;
        let soft_shadow = theme.soft_shadow;
        let surface_bright = match chrome.theme_preset {
            ThemePreset::Daylight => [0.93, 0.96, 0.99, 1.0],
            ThemePreset::DeviceDark => [0.28, 0.33, 0.43, 1.0],
        };
        let input_surface = match chrome.theme_preset {
            ThemePreset::Daylight => [0.92, 0.96, 0.99, 1.0],
            ThemePreset::DeviceDark => [0.25, 0.30, 0.39, 1.0],
        };
        let input_focus_surface = match chrome.theme_preset {
            ThemePreset::Daylight => [0.95, 0.98, 1.0, 1.0],
            ThemePreset::DeviceDark => [0.29, 0.35, 0.45, 1.0],
        };
        let badge_text = match chrome.theme_preset {
            ThemePreset::Daylight => [0.96, 0.99, 1.0, 1.0],
            ThemePreset::DeviceDark => [0.97, 0.99, 1.0, 1.0],
        };
        let selected_meta_text = match chrome.theme_preset {
            ThemePreset::Daylight => [0.14, 0.28, 0.40, 1.0],
            ThemePreset::DeviceDark => [0.90, 0.96, 1.0, 1.0],
        };
        let voice_success_text = match chrome.theme_preset {
            ThemePreset::Daylight => [0.20, 0.44, 0.24, 1.0],
            ThemePreset::DeviceDark => [0.84, 0.96, 0.86, 1.0],
        };
        let voice_error_text = match chrome.theme_preset {
            ThemePreset::Daylight => [0.86, 0.38, 0.38, 1.0],
            ThemePreset::DeviceDark => [1.0, 0.74, 0.74, 1.0],
        };
        let voice_hint_fill = match chrome.theme_preset {
            ThemePreset::Daylight => [0.92, 0.96, 1.0, 1.0],
            ThemePreset::DeviceDark => [0.27, 0.35, 0.48, 1.0],
        };
        let voice_hint_border = match chrome.theme_preset {
            ThemePreset::Daylight => [0.53, 0.68, 0.86, 1.0],
            ThemePreset::DeviceDark => [0.54, 0.72, 0.96, 1.0],
        };
        let voice_listening_fill = match chrome.theme_preset {
            ThemePreset::Daylight => [0.84, 0.95, 0.87, 1.0],
            ThemePreset::DeviceDark => [0.23, 0.39, 0.28, 1.0],
        };
        let voice_listening_border = match chrome.theme_preset {
            ThemePreset::Daylight => [0.32, 0.62, 0.38, 1.0],
            ThemePreset::DeviceDark => [0.47, 0.80, 0.54, 1.0],
        };
        let voice_visual_bar = match chrome.theme_preset {
            ThemePreset::Daylight => [0.36, 0.72, 0.43, 0.95],
            ThemePreset::DeviceDark => [0.58, 0.90, 0.64, 0.95],
        };
        let hover_surface = match chrome.theme_preset {
            ThemePreset::Daylight => [0.93, 0.96, 1.0, 1.0],
            ThemePreset::DeviceDark => [0.28, 0.35, 0.46, 1.0],
        };
        let press_surface = match chrome.theme_preset {
            ThemePreset::Daylight => [0.67, 0.82, 0.97, 1.0],
            ThemePreset::DeviceDark => [0.32, 0.48, 0.68, 1.0],
        };
        let hover_border = match chrome.theme_preset {
            ThemePreset::Daylight => [0.46, 0.62, 0.82, 1.0],
            ThemePreset::DeviceDark => [0.56, 0.76, 1.0, 1.0],
        };
        let badge_fill = match chrome.theme_preset {
            ThemePreset::Daylight => [0.24, 0.56, 0.86, 1.0],
            ThemePreset::DeviceDark => [0.32, 0.60, 0.98, 1.0],
        };
        let responsive_scale = (self.responsive_scale() * chrome.window_scale * 1.12).clamp(0.9, 2.0);
        let input_value_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.4,
            DisplayTextScale::Medium => 3.4,
            DisplayTextScale::Large => 4.2,
        } * responsive_scale;
        let label_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.2,
            DisplayTextScale::Medium => 2.45,
            DisplayTextScale::Large => 3.12,
        } * responsive_scale;
        let tracking = match chrome.text_spacing {
            TextSpacing::Tight => -0.41,
            TextSpacing::Normal => -0.32,
            TextSpacing::Relaxed => 0.08,
        } * responsive_scale;
        let base_line_gap = match chrome.text_spacing {
            TextSpacing::Tight => 4.2,
            TextSpacing::Normal => 5.2,
            TextSpacing::Relaxed => 7.2,
        } * responsive_scale;
        let separator_divider = match chrome.theme_preset {
            ThemePreset::Daylight => [1.0, 1.0, 1.0, 0.16],
            ThemePreset::DeviceDark => [0.94, 0.98, 1.0, 0.08],
        };
        let ui_tracking = tracking * 0.16 - 0.02 * responsive_scale;
        let heading_tracking = tracking * 0.10 - 0.01 * responsive_scale;
        let visible_sentence_candidates: Vec<(usize, String)> = chrome
            .sentence_candidates
            .iter()
            .take(4)
            .enumerate()
            .map(|(display_index, label)| {
                (
                    chrome
                        .sentence_candidate_source_indices
                        .get(display_index)
                        .copied()
                        .unwrap_or(display_index),
                    label.clone(),
                )
            })
            .collect();
        let hero_cards_enabled =
            self.scene_width < 1360.0 && chrome.preview_style == PreviewStyle::Compact;
        let metrics = PanelSceneMetrics::new(
            self.scene_width,
            self.scene_height,
            responsive_scale,
            chrome,
            visible_sentence_candidates.len(),
        );
        let narrow_layout_scale = if metrics.panel_width < 680.0 {
            0.90
        } else if metrics.panel_width < 760.0 {
            0.96
        } else {
            1.0
        };
        let input_value_px = input_value_px * narrow_layout_scale;
        let label_px = label_px * narrow_layout_scale;
        let tracking = tracking * if narrow_layout_scale < 1.0 { 0.75 } else { 1.0 };
        let base_line_gap = (base_line_gap
            * if narrow_layout_scale < 1.0 {
                0.88_f32
            } else {
                1.0_f32
            })
        .max(3.0_f32);
        let title_px = (label_px * 1.24_f32).max(2.35_f32 * responsive_scale);
        let helper_px = (label_px * 1.02_f32).max(2.0_f32 * responsive_scale);
        let chip_px = (label_px * 1.08_f32).max(2.05_f32 * responsive_scale);
        let hero_px = input_value_px * 1.10;
        let panel_width = metrics.panel_width;
        let panel_x = metrics.panel_x;
        let panel_y = metrics.panel_y;
        let input_box_y = panel_y;
        let tools_y = input_box_y + metrics.input_box_h + metrics.section_gap;
        let suggestions_y =
            tools_y + metrics.tools_header_h + metrics.tools_content_h + metrics.section_gap;
        let settings_panel_h = if chrome.settings_open {
            let max_allowed_settings = (self.scene_height - panel_y - metrics.panel_height)
                .max(0.0)
                .min(380.0 * responsive_scale);
            if max_allowed_settings <= 0.0 {
                0.0
            } else {
                let min_settings_height = 150.0 * responsive_scale;
                let preferred_settings_height = (metrics.panel_height * 1.4)
                    .clamp(min_settings_height, 380.0 * responsive_scale);
                preferred_settings_height
                    .clamp(min_settings_height, 380.0 * responsive_scale)
                    .min(max_allowed_settings)
            }
        } else {
            0.0
        };
        let panel_height = if chrome.settings_open {
            (metrics.panel_height + settings_panel_h).min(self.scene_height - panel_y)
        } else {
            metrics.panel_height
        };
        let candidate_area_bottom = if chrome.settings_open {
            panel_y + metrics.panel_height
        } else {
            f32::INFINITY
        };
        let mut quads = Vec::with_capacity(snapshot.candidate_labels.len() + 6);
        let mut text_quads = Vec::new();
        let mut atlas_glyphs = Vec::new();
        let mut text_sections = Vec::new();
        let mut hit_targets = Vec::with_capacity(snapshot.candidate_labels.len());
        let mut interactive_targets = Vec::new();
        let interaction_state = |kind: InteractionKind| {
            (
                chrome.hovered_interaction == Some(kind),
                chrome.pressed_interaction == Some(kind),
            )
        };
        let animated_rect = |rect: [f32; 4], hovered: bool, pressed: bool| {
            if pressed {
                [rect[0], rect[1] + 1.5 * responsive_scale, rect[2], rect[3]]
            } else if hovered {
                [rect[0], rect[1] - 1.5 * responsive_scale, rect[2], rect[3]]
            } else {
                rect
            }
        };
        let animated_shadow = |shadow: [f32; 4], hovered: bool, pressed: bool| {
            let mut next = shadow;
            next[3] *= if pressed {
                0.72
            } else if hovered {
                1.22
            } else {
                1.0
            };
            next
        };
        let interaction_hit_rect = |rect: [f32; 4]| {
            self.interaction_hit_rect(
                rect,
                responsive_scale,
                chrome.pointer_target_slop_tenths,
                0.9,
                1.0,
                1.0,
                true,
            )
        };
        // No full-screen background here to keep panel bounds-based layout tests stable.
        // Candidate scenes are rendered on top of the host surface directly.
        if self.scene_height > 0.0 && self.scene_width > 0.0 {
            let bg_left = (panel_x - 1.6 * responsive_scale).max(0.0);
            let bg_width = panel_width + 3.2 * responsive_scale;
            quads.push(CandidateQuad {
                rect: [
                    bg_left,
                    panel_y - 2.0 * responsive_scale,
                    bg_width,
                    panel_height + 4.0 * responsive_scale,
                ],
                color: page_bg,
            });
        }
        let panel_shell_y = (panel_y - 2.0 * responsive_scale).max(0.0);
        let panel_shell_h = (panel_height + 4.2 * responsive_scale)
            .min((self.scene_height - panel_shell_y).max(1.0));
        let shell_left = panel_x.max(0.0);
        append_soft_card_quads(
            &mut quads,
            [
                shell_left,
                panel_shell_y,
                panel_width + 8.0 * responsive_scale,
                panel_shell_h,
            ],
            shell,
            shell_border,
            soft_shadow,
            page_bg,
            8.8 * responsive_scale,
        );
        if panel_height > 2.0 * responsive_scale {
            quads.push(CandidateQuad {
                rect: [
                    panel_x + 3.2 * responsive_scale,
                    panel_shell_y + 3.2 * responsive_scale,
                    panel_width - 6.4 * responsive_scale,
                    1.2 * responsive_scale,
                ],
                color: separator_divider,
            });
        }
        append_soft_card_quads(
            &mut quads,
            [panel_x, input_box_y, panel_width, metrics.input_box_h],
            if chrome.input_focused {
                input_focus_surface
            } else {
                input_surface
            },
            if chrome.input_focused {
                accent_soft
            } else {
                border_dark
            },
            soft_shadow,
            shell,
            9.0 * responsive_scale,
        );
        if chrome.input_focused {
            append_rounded_rect_quads(
                &mut quads,
                [
                    panel_x + 2.0 * responsive_scale,
                    input_box_y + 2.0 * responsive_scale,
                    panel_width - 4.0 * responsive_scale,
                    metrics.input_box_h - 4.0 * responsive_scale,
                ],
                [0.72, 0.87, 1.0, 0.12],
                6.0 * responsive_scale,
            );
        }
        if chrome.input_focused {
            append_rounded_rect_quads(
                &mut quads,
                [
                    panel_x - 0.4 * responsive_scale,
                    input_box_y - 0.4 * responsive_scale,
                    panel_width + 0.8 * responsive_scale,
                    metrics.input_box_h + 0.8 * responsive_scale,
                ],
                [0.56, 0.80, 1.0, 0.12],
                7.0 * responsive_scale,
            );
            quads.push(CandidateQuad {
                rect: [
                    panel_x + 10.0 * responsive_scale,
                    input_box_y + metrics.input_box_h - 5.0 * responsive_scale,
                    panel_width - 24.0 * responsive_scale,
                    1.6 * responsive_scale,
                ],
                color: [0.38, 0.69, 0.96, 0.30],
            });
        }
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::SeedInput,
            rect: interaction_hit_rect([panel_x, input_box_y, panel_width, metrics.input_box_h]),
        });

        let header_layouts = vec![
            TextBlock {
                text: "Seed input".to_string(),
                origin: [
                    panel_x + 16.0 * responsive_scale,
                    input_box_y + 9.0 * responsive_scale,
                ],
                max_width: (panel_width - 58.0 * responsive_scale).max(0.0),
                pixel_size: title_px,
                letter_spacing: heading_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Left,
                role: TextRole::InputLabel,
            }
            .layout(),
            TextBlock {
                text: if chrome.seed_text.is_empty() {
                    "Type a seed word or phrase".to_string()
                } else {
                    chrome.display_text()
                },
                origin: [
                    panel_x + 16.0 * responsive_scale,
                    input_box_y + 25.6 * responsive_scale,
                ],
                max_width: (panel_width - 58.0 * responsive_scale).max(0.0),
                pixel_size: input_value_px,
                letter_spacing: heading_tracking,
                line_gap: base_line_gap,
                max_lines: 3,
                color: if chrome.seed_text.is_empty() {
                    text_muted
                } else {
                    text_primary
                },
                align: TextAlign::Left,
                role: TextRole::InputValue,
            }
            .layout(),
        ];
        for layout in &header_layouts {
            text_quads.extend(layout.quads.iter().copied());
            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
        }
        text_sections.push(TextSection {
            role: TextRole::InputLabel,
            layouts: header_layouts,
        });
        if chrome.input_focused {
            let caret_x = panel_x
                + 16.0 * responsive_scale
                + measure_text_prefix_width(
                    &chrome.seed_text,
                    chrome.caret_index,
                    input_value_px,
                    tracking,
                );
            quads.push(CandidateQuad {
                rect: [
                    caret_x,
                    input_box_y + 25.5 * responsive_scale,
                    2.0 * responsive_scale,
                    17.0 * responsive_scale,
                ],
                color: accent,
            });
        }

        let compact_button_rect = [
            panel_x + panel_width - 32.0 * responsive_scale,
            input_box_y + 6.0 * responsive_scale,
            19.0 * responsive_scale,
            19.0 * responsive_scale,
        ];
        let scale_button_size = 15.0 * responsive_scale;
        let scale_button_spacing = 3.4 * responsive_scale;
        let scale_label_width = 24.0 * responsive_scale;
        let scale_plus_rect = [
            compact_button_rect[0] - scale_button_spacing - scale_button_size,
            compact_button_rect[1],
            scale_button_size,
            19.0 * responsive_scale,
        ];
        let scale_reset_rect = [
            scale_plus_rect[0] - scale_button_spacing - scale_button_size,
            compact_button_rect[1],
            scale_button_size,
            19.0 * responsive_scale,
        ];
        let scale_minus_rect = [
            scale_reset_rect[0] - scale_button_spacing - scale_button_size,
            compact_button_rect[1],
            scale_button_size,
            19.0 * responsive_scale,
        ];
        let scale_drag_rect = [
            scale_minus_rect[0],
            compact_button_rect[1],
            (scale_plus_rect[0] + scale_plus_rect[2] - scale_minus_rect[0]).max(scale_button_size),
            19.0 * responsive_scale,
        ];
        let scale_text_x = (scale_minus_rect[0] - scale_label_width - 3.0 * responsive_scale)
            .max(panel_x + 8.0 * responsive_scale);
        let can_decrease_scale = chrome.can_decrease_window_scale();
        let can_increase_scale = chrome.can_increase_window_scale();
        let can_reset_scale = chrome.can_reset_window_scale();
        if (scale_minus_rect[0] - scale_text_x) >= (18.0 * responsive_scale) {
            let scale_text = TextBlock {
                text: format!(
                    "{}%",
                    (chrome.window_scale * 100.0).round().clamp(1.0, 999.0) as i32
                ),
                origin: [
                    scale_text_x,
                    compact_button_rect[1] + 3.8 * responsive_scale,
                ],
                max_width: scale_label_width,
                pixel_size: 2.0 * responsive_scale,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: if chrome.window_scale == 1.0 {
                    text_muted
                } else {
                    text_primary
                },
                align: TextAlign::Left,
                role: TextRole::ToolButton,
            }
            .layout();
            text_quads.extend(scale_text.quads.iter().copied());
            atlas_glyphs.extend(scale_text.atlas_glyphs.iter().cloned());
            text_sections.push(TextSection {
                role: TextRole::ToolButton,
                layouts: vec![scale_text],
            });
        }
        let (drag_hovered, drag_pressed) = interaction_state(InteractionKind::DragWindowScale);
        let drag_visual_rect = animated_rect(scale_drag_rect, drag_hovered, drag_pressed);
        append_soft_card_quads(
            &mut quads,
            drag_visual_rect,
            if can_decrease_scale || can_increase_scale {
                if drag_hovered || drag_pressed {
                    surface_alt
                } else {
                    surface_muted
                }
            } else {
                surface_muted
            },
            border_dark,
            animated_shadow(soft_shadow, drag_hovered, drag_pressed),
            surface,
            6.2 * responsive_scale,
        );
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::DragWindowScale,
            rect: interaction_hit_rect(scale_drag_rect),
        });
        append_soft_card_quads(
            &mut quads,
            compact_button_rect,
            surface_alt,
            border_dark,
            soft_shadow,
            if chrome.input_focused {
                input_focus_surface
            } else {
                input_surface
            },
            6.0 * responsive_scale,
        );
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::ToggleCompactMode,
            rect: interaction_hit_rect(compact_button_rect),
        });
        let compact_icon = TextBlock {
            text: "".to_string(),
            origin: [
                compact_button_rect[0] + 3.4 * responsive_scale,
                compact_button_rect[1] + 3.8 * responsive_scale,
            ],
            max_width: (compact_button_rect[2] - 7.0 * responsive_scale).max(0.0),
            pixel_size: 2.0 * responsive_scale,
            letter_spacing: ui_tracking,
            line_gap: base_line_gap,
            max_lines: 1,
            color: text_secondary,
            align: TextAlign::Center,
            role: TextRole::ToolButton,
        }
        .layout();
        text_quads.extend(compact_icon.quads.iter().copied());
        atlas_glyphs.extend(compact_icon.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::ToolButton,
            layouts: vec![compact_icon],
        });
        append_chevron_icon_quads(&mut quads, compact_button_rect, text_secondary, false);

        let (decrease_hovered, decrease_pressed) = if can_decrease_scale {
            interaction_state(InteractionKind::DecreaseWindowScale)
        } else {
            (false, false)
        };
        let decrease_visual_rect =
            animated_rect(scale_minus_rect, decrease_hovered, decrease_pressed);
        append_soft_card_quads(
            &mut quads,
            decrease_visual_rect,
            if can_decrease_scale {
                surface_bright
            } else {
                surface_muted
            },
            border_dark,
            if can_decrease_scale {
                animated_shadow(soft_shadow, decrease_hovered, decrease_pressed)
            } else {
                animated_shadow(border_dark, false, false)
            },
            surface,
            5.4 * responsive_scale,
        );
        if can_decrease_scale {
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::DecreaseWindowScale,
                rect: interaction_hit_rect(scale_minus_rect),
            });
        }
        let minus_text = TextBlock {
            text: "-".to_string(),
            origin: [
                scale_minus_rect[0] + (scale_minus_rect[2] - 3.2 * responsive_scale) / 2.0,
                compact_button_rect[1] + 3.8 * responsive_scale,
            ],
            max_width: scale_minus_rect[2].max(6.0),
            pixel_size: 2.0 * responsive_scale,
            letter_spacing: ui_tracking,
            line_gap: base_line_gap,
            max_lines: 1,
            color: if can_decrease_scale {
                text_primary
            } else {
                text_muted
            },
            align: TextAlign::Center,
            role: TextRole::ToolButton,
        }
        .layout();
        text_quads.extend(minus_text.quads.iter().copied());
        atlas_glyphs.extend(minus_text.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::ToolButton,
            layouts: vec![minus_text],
        });

        let (reset_hovered, reset_pressed) = if can_reset_scale {
            interaction_state(InteractionKind::ResetWindowScale)
        } else {
            (false, false)
        };
        let reset_visual_rect = animated_rect(scale_reset_rect, reset_hovered, reset_pressed);
        append_soft_card_quads(
            &mut quads,
            reset_visual_rect,
            if !can_reset_scale {
                surface_muted
            } else if chrome.window_scale == 1.0 {
                surface
            } else {
                accent_soft
            },
            if chrome.window_scale == 1.0 {
                border_dark
            } else {
                accent
            },
            if can_reset_scale {
                animated_shadow(soft_shadow, reset_hovered, reset_pressed)
            } else {
                animated_shadow(border_dark, false, false)
            },
            surface,
            5.4 * responsive_scale,
        );
        if can_reset_scale {
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::ResetWindowScale,
                rect: interaction_hit_rect(scale_reset_rect),
            });
        }
        append_refresh_icon_quads(
            &mut quads,
            reset_visual_rect,
            if !can_reset_scale {
                text_muted
            } else if chrome.window_scale == 1.0 {
                text_secondary
            } else {
                accent_text
            },
        );
        let (increase_hovered, increase_pressed) = if can_increase_scale {
            interaction_state(InteractionKind::IncreaseWindowScale)
        } else {
            (false, false)
        };
        let increase_visual_rect =
            animated_rect(scale_plus_rect, increase_hovered, increase_pressed);
        append_soft_card_quads(
            &mut quads,
            increase_visual_rect,
            if can_increase_scale {
                surface_bright
            } else {
                surface_muted
            },
            border_dark,
            if can_increase_scale {
                animated_shadow(soft_shadow, increase_hovered, increase_pressed)
            } else {
                animated_shadow(border_dark, false, false)
            },
            surface,
            5.4 * responsive_scale,
        );
        if can_increase_scale {
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::IncreaseWindowScale,
                rect: interaction_hit_rect(scale_plus_rect),
            });
        }
        let plus_text = TextBlock {
            text: "+".to_string(),
            origin: [
                scale_plus_rect[0] + (scale_plus_rect[2] - 3.2 * responsive_scale) / 2.0,
                compact_button_rect[1] + 3.8 * responsive_scale,
            ],
            max_width: scale_plus_rect[2].max(6.0),
            pixel_size: 2.0 * responsive_scale,
            letter_spacing: ui_tracking,
            line_gap: base_line_gap,
            max_lines: 1,
            color: if can_increase_scale {
                text_primary
            } else {
                text_muted
            },
            align: TextAlign::Center,
            role: TextRole::ToolButton,
        }
        .layout();
        text_quads.extend(plus_text.quads.iter().copied());
        atlas_glyphs.extend(plus_text.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::ToolButton,
            layouts: vec![plus_text],
        });

        let tools_header_rect = [panel_x, tools_y, panel_width, metrics.tools_header_h];
        append_soft_card_quads(
            &mut quads,
            tools_header_rect,
            surface_alt,
            border_dark,
            soft_shadow,
            shell,
            10.0 * responsive_scale,
        );
        let toolbar_button_size = metrics.tool_button_h;
        let icon_gap = metrics.tool_gap;
        let mut toolbar_buttons: Vec<(InteractionKind, bool)> =
            vec![(InteractionKind::SettingsToggle, chrome.settings_open)];
        if chrome.input_modes_expanded {
            toolbar_buttons.extend([
                (
                    InteractionKind::InputModeButton(InputMode::VirtualKeyboard),
                    chrome.active_input_mode == InputMode::VirtualKeyboard,
                ),
                (
                    InteractionKind::InputModeButton(InputMode::Dictation),
                    chrome.active_input_mode == InputMode::Dictation,
                ),
                (
                    InteractionKind::InputModeButton(InputMode::Handwriting),
                    chrome.active_input_mode == InputMode::Handwriting,
                ),
            ]);
        }
        let toolbar_margin_x = 5.0 * responsive_scale;
        let toolbar_button_count = toolbar_buttons.len() as f32;
        let toolbar_total_w =
            toolbar_button_count * toolbar_button_size + (toolbar_button_count - 1.0) * icon_gap;
        let trailing_toggle_w = if chrome.input_modes_expanded {
            toolbar_button_size + 3.6 * responsive_scale
        } else {
            0.0
        };
        let toolbar_available_w =
            (panel_width - toolbar_margin_x * 2.0 - trailing_toggle_w).max(0.0);
        let toolbar_scale = if toolbar_total_w > toolbar_available_w && toolbar_total_w > 0.0 {
            (toolbar_available_w / toolbar_total_w).clamp(0.72, 1.0)
        } else {
            1.0
        };
        let toolbar_button_size = toolbar_button_size * toolbar_scale;
        let icon_gap = icon_gap * toolbar_scale;
        let toolbar_total_w =
            toolbar_button_count * toolbar_button_size + (toolbar_button_count - 1.0) * icon_gap;
        let icon_x = panel_x + panel_width - toolbar_total_w - trailing_toggle_w - toolbar_margin_x;
        let toolbar_y =
            tools_y + (metrics.tools_header_h - toolbar_button_size) * 0.5 + 0.5 * responsive_scale;
        for (index, (kind, selected)) in toolbar_buttons.iter().enumerate() {
            let rect = [
                icon_x + index as f32 * (toolbar_button_size + icon_gap),
                toolbar_y,
                toolbar_button_size,
                toolbar_button_size,
            ];
            let (hovered, pressed) = interaction_state(*kind);
            let visual_rect = animated_rect(rect, hovered, pressed);
            append_soft_card_quads(
                &mut quads,
                visual_rect,
                if *selected {
                    accent_soft
                } else if hovered {
                    surface_bright
                } else {
                    surface
                },
                if *selected {
                    accent
                } else if hovered {
                    [0.46, 0.62, 0.82, 1.0]
                } else {
                    border_dark
                },
                animated_shadow(soft_shadow, hovered, pressed),
                surface,
                7.0 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: *kind,
                rect: interaction_hit_rect(rect),
            });
            let icon_color = if *selected { accent_text } else { text_primary };
            match kind {
                InteractionKind::InputModeButton(InputMode::VirtualKeyboard) => {
                    append_keyboard_icon_quads(&mut quads, visual_rect, icon_color)
                }
                InteractionKind::InputModeButton(InputMode::Dictation) => {
                    append_mic_icon_quads(&mut quads, visual_rect, icon_color)
                }
                InteractionKind::InputModeButton(InputMode::Handwriting) => {
                    append_pen_icon_quads(&mut quads, visual_rect, icon_color)
                }
                InteractionKind::SettingsToggle => append_gear_icon_quads(
                    &mut quads,
                    visual_rect,
                    icon_color,
                    if *selected { accent_soft } else { surface },
                ),
                _ => {}
            }
        }
        {
            let toggle_rect = [
                panel_x + panel_width - toolbar_button_size - 3.8 * responsive_scale,
                toolbar_y,
                toolbar_button_size,
                toolbar_button_size,
            ];
            let (toggle_hovered, toggle_pressed) =
                interaction_state(InteractionKind::InputModesToggle);
            let toggle_visual_rect = animated_rect(toggle_rect, toggle_hovered, toggle_pressed);
            append_soft_card_quads(
                &mut quads,
                toggle_visual_rect,
                accent_soft,
                accent,
                animated_shadow(soft_shadow, toggle_hovered, toggle_pressed),
                surface,
                7.0 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::InputModesToggle,
                rect: interaction_hit_rect(toggle_rect),
            });
            append_chevron_icon_quads(
                &mut quads,
                toggle_visual_rect,
                accent_text,
                chrome.input_modes_expanded,
            );
        }

        let tools_panel_rect = [
            panel_x,
            tools_y + metrics.tools_header_h + 1.2 * responsive_scale,
            panel_width,
            metrics.tools_content_h,
        ];
        let show_expanded_tool_panel =
            chrome.input_modes_expanded && metrics.tools_content_h >= 40.0 * responsive_scale;
        if show_expanded_tool_panel {
            append_soft_card_quads(
                &mut quads,
                tools_panel_rect,
                surface,
                border_dark,
                soft_shadow,
                shell,
                8.8 * responsive_scale,
            );
            quads.push(CandidateQuad {
                rect: [
                    tools_panel_rect[0] + 9.0 * responsive_scale,
                    tools_panel_rect[1] + 5.0 * responsive_scale,
                    tools_panel_rect[2] - 15.0 * responsive_scale,
                    1.8 * responsive_scale,
                ],
                color: match chrome.theme_preset {
                    ThemePreset::Daylight => [1.0, 1.0, 1.0, 0.14],
                    ThemePreset::DeviceDark => [1.0, 1.0, 1.0, 0.10],
                },
            });
            let drawer_rect = [
                tools_panel_rect[0] + 5.0 * responsive_scale,
                tools_panel_rect[1] + 2.8 * responsive_scale,
                tools_panel_rect[2] - 10.0 * responsive_scale,
                tools_panel_rect[3] - 6.2 * responsive_scale,
            ];
            append_soft_card_quads(
                &mut quads,
                drawer_rect,
                surface_alt,
                border_dark,
                soft_shadow,
                surface,
                7.8 * responsive_scale,
            );
            let handle_w = 40.0 * responsive_scale;
            quads.push(CandidateQuad {
                rect: [
                    drawer_rect[0] + (drawer_rect[2] - handle_w) * 0.5,
                    drawer_rect[1] + 6.0 * responsive_scale,
                    handle_w,
                    2.8 * responsive_scale,
                ],
                color: match chrome.theme_preset {
                    ThemePreset::Daylight => [1.0, 1.0, 1.0, 0.22],
                    ThemePreset::DeviceDark => [0.96, 0.98, 1.0, 0.12],
                },
            });

            match chrome.active_input_mode {
                InputMode::VirtualKeyboard => {
                    include!("panel_scene_keyboard_body_block.rs");
                }
                InputMode::Dictation => {
                    include!("panel_scene_voice_body_block.rs");
                }
                InputMode::Handwriting => {
                    include!("panel_scene_handwriting_body_block.rs");
                }
            }
        }

        let settings_y = panel_y + metrics.panel_height;
        include!("panel_scene_settings_block.rs");

        let mut scene = { include!("panel_scene_candidates_block.rs") };

        if scene.quads.len() > 2 && scene.quads[1].color == scene.quads[2].color {
            scene.quads[2].color = [
                scene.quads[2].color[0],
                scene.quads[2].color[1],
                scene.quads[2].color[2],
                (scene.quads[2].color[3] + 0.04).min(1.0),
            ];
        }

        scene
    }
}
