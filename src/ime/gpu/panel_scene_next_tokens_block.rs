{
    let chip_section_y = suggestions_y;
    if !chrome.next_token_candidates.is_empty() {
        let chip_section_rect = [
            panel_x,
            if collapsed_daily_mode {
                chip_section_y
            } else {
                chip_section_y - 3.0 * responsive_scale
            },
            panel_width,
            metrics.chip_section_h,
        ];
        append_soft_card_quads(
            &mut quads,
            chip_section_rect,
            if collapsed_daily_mode {
                surface_alt
            } else {
                surface
            },
            border_dark,
            soft_shadow,
            shell,
            10.0 * responsive_scale,
        );
        if !collapsed_daily_mode {
            let next_label_layouts = vec![
                TextBlock {
                    text: if chrome.composed_tokens.is_empty() {
                        "Next tokens".to_string()
                    } else {
                        format!("Next tokens  |  {}", chrome.composed_tokens.join(" "))
                    },
                    origin: [
                        panel_x + 6.0 * responsive_scale,
                        chip_section_y + 5.0 * responsive_scale,
                    ],
                    max_width: panel_width - 96.0 * responsive_scale,
                    pixel_size: title_px,
                    letter_spacing: heading_tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Left,
                    role: TextRole::NextTokenLabel,
                }
                .layout(),
            ];
            for layout in &next_label_layouts {
                text_quads.extend(layout.quads.iter().copied());
                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            }
            text_sections.push(TextSection {
                role: TextRole::NextTokenLabel,
                layouts: next_label_layouts,
            });
        }

        if !chrome.composed_tokens.is_empty() {
            let back_rect = if collapsed_daily_mode {
                [
                    panel_x + panel_width - 66.0 * responsive_scale,
                    chip_section_y + 8.0 * responsive_scale,
                    58.0 * responsive_scale,
                    20.0 * responsive_scale,
                ]
            } else if metrics.stacked_token_header {
                [
                    panel_x + 2.0 * responsive_scale,
                    chip_section_y + 24.0 * responsive_scale,
                    78.0 * responsive_scale,
                    22.0 * responsive_scale,
                ]
            } else {
                [
                    panel_x + panel_width - 78.0 * responsive_scale,
                    chip_section_y - 2.0 * responsive_scale,
                    78.0 * responsive_scale,
                    22.0 * responsive_scale,
                ]
            };
            let (hovered, pressed) = interaction_state(InteractionKind::RewindNextToken);
            let visual_back_rect = animated_rect(back_rect, hovered, pressed);
            append_soft_card_quads(
                &mut quads,
                visual_back_rect,
                if pressed {
                    [0.67, 0.82, 0.97, 1.0]
                } else if hovered {
                    surface_bright
                } else {
                    surface_muted
                },
                if pressed {
                    [0.08, 0.35, 0.68, 1.0]
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
                kind: InteractionKind::RewindNextToken,
                rect: back_rect,
            });
            let back_layout = TextBlock {
                text: if collapsed_daily_mode {
                    "←".to_string()
                } else {
                    "Back".to_string()
                },
                origin: [
                    visual_back_rect[0] + 8.0 * responsive_scale,
                    visual_back_rect[1] + 6.0 * responsive_scale,
                ],
                max_width: visual_back_rect[2] - 16.0 * responsive_scale,
                pixel_size: 2.0 * responsive_scale,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_primary,
                align: TextAlign::Center,
                role: TextRole::NextTokenChip,
            }
            .layout();
            text_quads.extend(back_layout.quads.iter().copied());
            atlas_glyphs.extend(back_layout.atlas_glyphs.iter().cloned());
            text_sections.push(TextSection {
                role: TextRole::NextTokenChip,
                layouts: vec![back_layout],
            });
        }

        let chip_y = if collapsed_daily_mode {
            chip_section_y + 9.0 * responsive_scale
        } else if metrics.stacked_token_header {
            chip_section_y + 48.0 * responsive_scale
        } else {
            chip_section_y + 30.0 * responsive_scale
        };
        let chip_inner_x = panel_x + 6.0 * responsive_scale;
        let chip_inner_width = panel_width - 12.0 * responsive_scale;
        let chip_inner_right = chip_inner_x + chip_inner_width;
        let max_rows = if collapsed_daily_mode { 1 } else { 2 };
        let chip_height = if collapsed_daily_mode {
            22.0 * responsive_scale
        } else {
            24.0 * responsive_scale
        };
        let row_gap = if collapsed_daily_mode {
            0.0
        } else {
            chip_height + 6.0 * responsive_scale
        };
        let chip_h_gap = if collapsed_daily_mode {
            6.0 * responsive_scale
        } else {
            8.0 * responsive_scale
        };

        let mut chip_x = chip_inner_x;
        let mut row = 0;
        let mut chip_layouts = Vec::new();
        for (index, token) in chrome.next_token_candidates.iter().take(6).enumerate() {
            let kind = InteractionKind::SelectNextToken(index);
            let (hovered, pressed) = interaction_state(kind);
            let base_chip_w = if collapsed_daily_mode {
                ((token.chars().count() as f32 * 10.0).max(54.0)
                    + if index == 0 { 30.0 } else { 20.0 })
                    * responsive_scale
            } else {
                ((token.chars().count() as f32 * 12.0).max(60.0) + 20.0) * responsive_scale
            };
            if chip_x > chip_inner_x && chip_x + base_chip_w > chip_inner_right {
                row += 1;
                chip_x = chip_inner_x;
            }
            if row >= max_rows {
                break;
            }

            let available_chip_w = (chip_inner_right - chip_x).max(28.0 * responsive_scale);
            let chip_w = base_chip_w.min(available_chip_w);
            let rect = [
                chip_x,
                chip_y + row as f32 * row_gap,
                chip_w,
                chip_height,
            ];
            let visual_rect = animated_rect(rect, hovered, pressed);
            append_soft_card_quads(
                &mut quads,
                visual_rect,
                if pressed {
                    [0.67, 0.82, 0.97, 1.0]
                } else if index == 0 {
                    accent_soft
                } else if hovered {
                    surface_bright
                } else {
                    surface
                },
                if pressed {
                    [0.08, 0.35, 0.68, 1.0]
                } else if index == 0 {
                    accent
                } else if hovered {
                    [0.46, 0.62, 0.82, 1.0]
                } else {
                    border_dark
                },
                animated_shadow(soft_shadow, hovered, pressed),
                surface,
                8.0 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget { kind, rect });
            let layout = TextBlock {
                text: if collapsed_daily_mode {
                    format!("{} {}", index + 1, token)
                } else {
                    token.clone()
                },
                origin: [
                    visual_rect[0] + 8.0 * responsive_scale,
                    visual_rect[1] + 6.0 * responsive_scale,
                ],
                max_width: visual_rect[2] - 16.0 * responsive_scale,
                pixel_size: if collapsed_daily_mode { chip_px * 0.95 } else { chip_px },
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: if index == 0 { accent_text } else { text_primary },
                align: TextAlign::Center,
                role: TextRole::NextTokenChip,
            }
            .layout();
            text_quads.extend(layout.quads.iter().copied());
            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            chip_layouts.push(layout);
            chip_x += chip_w + chip_h_gap;
        }
        if !chip_layouts.is_empty() {
            text_sections.push(TextSection {
                role: TextRole::NextTokenChip,
                layouts: chip_layouts,
            });
        }
    }
}
