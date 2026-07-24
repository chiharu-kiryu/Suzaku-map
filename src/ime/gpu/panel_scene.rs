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
        let responsive_scale = self.responsive_scale();
        let input_value_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.2,
            DisplayTextScale::Medium => 3.25,
            DisplayTextScale::Large => 4.1,
        } * responsive_scale;
        let label_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.1,
            DisplayTextScale::Medium => 2.35,
            DisplayTextScale::Large => 3.0,
        } * responsive_scale;
        let tracking = match chrome.text_spacing {
            TextSpacing::Tight => -0.42,
            TextSpacing::Normal => -0.34,
            TextSpacing::Relaxed => 0.06,
        } * responsive_scale;
        let base_line_gap = match chrome.text_spacing {
            TextSpacing::Tight => 4.0,
            TextSpacing::Normal => 5.0,
            TextSpacing::Relaxed => 7.0,
        } * responsive_scale;
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
        quads.push(CandidateQuad {
            rect: [0.0, 0.0, self.scene_width, self.scene_height],
            color: page_bg,
        });
        let panel_shell_y = (panel_y - 10.0 * responsive_scale).max(0.0);
        let panel_shell_h = (metrics.panel_height + 20.0 * responsive_scale)
            .min((self.scene_height - panel_shell_y).max(1.0));
        append_soft_card_quads(
            &mut quads,
            [
                panel_x - 10.0 * responsive_scale,
                panel_shell_y,
                panel_width + 20.0 * responsive_scale,
                panel_shell_h,
            ],
            shell,
            shell_border,
            soft_shadow,
            page_bg,
            12.0 * responsive_scale,
        );
        quads.push(CandidateQuad {
            rect: [
                panel_x + 8.0 * responsive_scale,
                panel_shell_y + 8.0 * responsive_scale,
                panel_width - 16.0 * responsive_scale,
                2.0 * responsive_scale,
            ],
            color: [1.0, 1.0, 1.0, 0.16],
        });
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
            10.0 * responsive_scale,
        );
        if chrome.input_focused {
            append_rounded_rect_quads(
                &mut quads,
                [
                    panel_x + 3.0 * responsive_scale,
                    input_box_y + 3.0 * responsive_scale,
                    panel_width - 6.0 * responsive_scale,
                    metrics.input_box_h - 6.0 * responsive_scale,
                ],
                [0.72, 0.87, 1.0, 0.12],
                8.0 * responsive_scale,
            );
        }
        if chrome.input_focused {
            append_rounded_rect_quads(
                &mut quads,
                [
                    panel_x - 1.0 * responsive_scale,
                    input_box_y - 1.0 * responsive_scale,
                    panel_width + 2.0 * responsive_scale,
                    metrics.input_box_h + 2.0 * responsive_scale,
                ],
                [0.56, 0.80, 1.0, 0.12],
                11.0 * responsive_scale,
            );
            quads.push(CandidateQuad {
                rect: [
                    panel_x + 12.0 * responsive_scale,
                    input_box_y + metrics.input_box_h - 5.0 * responsive_scale,
                    panel_width - 24.0 * responsive_scale,
                    2.0 * responsive_scale,
                ],
                color: [0.38, 0.69, 0.96, 0.30],
            });
        }
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::SeedInput,
            rect: [panel_x, input_box_y, panel_width, metrics.input_box_h],
        });

        let header_layouts = vec![
            TextBlock {
                text: "Seed input".to_string(),
                origin: [
                    panel_x + 16.0 * responsive_scale,
                    input_box_y + 11.5 * responsive_scale,
                ],
                max_width: panel_width - 58.0 * responsive_scale,
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
                    input_box_y + 30.0 * responsive_scale,
                ],
                max_width: panel_width - 58.0 * responsive_scale,
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
                    input_box_y + 28.0 * responsive_scale,
                    2.0 * responsive_scale,
                    20.0 * responsive_scale,
                ],
                color: accent,
            });
        }

        let compact_button_rect = [
            panel_x + panel_width - 34.0 * responsive_scale,
            input_box_y + 8.0 * responsive_scale,
            20.0 * responsive_scale,
            20.0 * responsive_scale,
        ];
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
            rect: compact_button_rect,
        });
        let compact_icon = TextBlock {
            text: "".to_string(),
            origin: [
                compact_button_rect[0] + 4.0 * responsive_scale,
                compact_button_rect[1] + 5.0 * responsive_scale,
            ],
            max_width: compact_button_rect[2] - 8.0 * responsive_scale,
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
        append_chevron_icon_quads(&mut quads, compact_button_rect, text_secondary, false);

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
        let toolbar_buttons = [
            (
                InteractionKind::InputModeButton(InputMode::VirtualKeyboard),
                chrome.active_input_mode == InputMode::VirtualKeyboard
                    && chrome.input_modes_expanded,
            ),
            (
                InteractionKind::InputModeButton(InputMode::Dictation),
                chrome.active_input_mode == InputMode::Dictation && chrome.input_modes_expanded,
            ),
            (
                InteractionKind::InputModeButton(InputMode::Handwriting),
                chrome.active_input_mode == InputMode::Handwriting && chrome.input_modes_expanded,
            ),
            (InteractionKind::SettingsToggle, chrome.settings_open),
        ];
        let toolbar_margin_x = 10.0 * responsive_scale;
        let toolbar_button_count = toolbar_buttons.len() as f32;
        let toolbar_total_w =
            toolbar_button_count * toolbar_button_size + (toolbar_button_count - 1.0) * icon_gap;
        let trailing_toggle_w = if chrome.input_modes_expanded {
            toolbar_button_size + 8.0 * responsive_scale
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
            interactive_targets.push(InteractiveTarget { kind: *kind, rect });
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
        if chrome.input_modes_expanded {
            let toggle_rect = [
                panel_x + panel_width - toolbar_button_size - 8.0 * responsive_scale,
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
                rect: toggle_rect,
            });
            append_chevron_icon_quads(&mut quads, toggle_visual_rect, accent_text, true);
        }

        let tools_panel_rect = [
            panel_x,
            tools_y + metrics.tools_header_h + 4.0 * responsive_scale,
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
                10.0 * responsive_scale,
            );
            quads.push(CandidateQuad {
                rect: [
                    tools_panel_rect[0] + 12.0 * responsive_scale,
                    tools_panel_rect[1] + 8.0 * responsive_scale,
                    tools_panel_rect[2] - 24.0 * responsive_scale,
                    2.0 * responsive_scale,
                ],
                color: [1.0, 1.0, 1.0, 0.14],
            });
            let drawer_rect = [
                tools_panel_rect[0] + 10.0 * responsive_scale,
                tools_panel_rect[1] + 6.0 * responsive_scale,
                tools_panel_rect[2] - 20.0 * responsive_scale,
                tools_panel_rect[3] - 10.0 * responsive_scale,
            ];
            append_soft_card_quads(
                &mut quads,
                drawer_rect,
                surface_alt,
                border_dark,
                soft_shadow,
                surface,
                9.0 * responsive_scale,
            );
            let handle_w = 40.0 * responsive_scale;
            quads.push(CandidateQuad {
                rect: [
                    drawer_rect[0] + (drawer_rect[2] - handle_w) * 0.5,
                    drawer_rect[1] + 7.0 * responsive_scale,
                    handle_w,
                    3.0 * responsive_scale,
                ],
                color: [1.0, 1.0, 1.0, 0.22],
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

        include!("panel_scene_candidates_block.rs")
    }
}
