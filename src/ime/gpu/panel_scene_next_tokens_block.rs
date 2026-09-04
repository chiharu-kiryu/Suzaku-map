{
    let scroll_text_for_candidate = |text: &str,
                                    started_at: Instant,
                                    pixel_size: f32,
                                    letter_spacing: f32,
                                    max_width: f32| {
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() || max_width <= 0.0 {
            return "…".to_string();
        }

        let glyph_advance = text_glyph_advance(pixel_size, letter_spacing);
        let width_for_char = |ch: char| {
            if ch == ' ' {
                text_space_advance(pixel_size)
            } else {
                glyph_advance
            }
        };

        let doubled: Vec<char> = chars.iter().chain(chars.iter()).cloned().collect();
        let cycle_step = (started_at.elapsed().as_millis() as f32 / 220.0).floor() as usize;
        let start = cycle_step % chars.len();
        let viewport_capacity = ((max_width / glyph_advance).floor() as usize).max(1);

        let mut text = String::new();
        let mut width = 0.0_f32;

        for (offset, ch) in doubled.iter().skip(start).take(chars.len()).enumerate() {
            if width_for_char(*ch) + width > max_width && !text.is_empty() {
                break;
            }
            if text.is_empty() {
                text.push(*ch);
                width = width_for_char(*ch);
            } else if offset < viewport_capacity {
                text.push(*ch);
                width += width_for_char(*ch);
            }
        }

        text
    };

    let chip_section_y = suggestions_y;
    let chip_section_h = if chrome.next_token_candidates.is_empty() {
        0.0
    } else {
        (candidate_area_bottom - chip_section_y - 4.0 * responsive_scale).max(0.0)
    };
    if chip_section_h > 0.0 {
        let available_section_h = chip_section_h.min(metrics.chip_section_h);
        let chip_section_rect = [
            panel_x,
            if collapsed_daily_mode {
                chip_section_y
            } else {
                chip_section_y - 3.0 * responsive_scale
            },
            panel_width,
            available_section_h,
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
                9.4 * responsive_scale,
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
                    max_width: (panel_width - 96.0 * responsive_scale).max(0.0),
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
                    chip_section_y + 7.0 * responsive_scale,
                    60.0 * responsive_scale,
                    21.0 * responsive_scale,
                ]
            } else if metrics.stacked_token_header {
                [
                    panel_x + 2.2 * responsive_scale,
                    chip_section_y + 23.4 * responsive_scale,
                    76.0 * responsive_scale,
                    22.0 * responsive_scale,
                ]
            } else {
                [
                    panel_x + panel_width - 80.0 * responsive_scale,
                    chip_section_y - 1.4 * responsive_scale,
                    80.0 * responsive_scale,
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
                7.2 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::RewindNextToken,
                rect: interaction_hit_rect(back_rect),
            });
            let back_layout = TextBlock {
                text: if collapsed_daily_mode {
                    "←".to_string()
                } else {
                    "Back".to_string()
                },
            origin: [
                    visual_back_rect[0] + 6.8 * responsive_scale,
                    visual_back_rect[1] + 5.2 * responsive_scale,
                ],
                max_width: (visual_back_rect[2] - 16.0 * responsive_scale).max(0.0),
                pixel_size: micro_px,
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
            chip_section_y + 6.8 * responsive_scale
        } else if metrics.stacked_token_header {
            chip_section_y + 46.0 * responsive_scale
        } else {
            chip_section_y + 27.5 * responsive_scale
        };
        let chip_content_span = if !collapsed_daily_mode {
            available_section_h - (chip_y - chip_section_y)
        } else {
            available_section_h
        };
        let chip_height = if collapsed_daily_mode {
            20.0 * responsive_scale
        } else {
            22.2 * responsive_scale
        };
        let chip_inner_x = panel_x + 6.0 * responsive_scale;
        let chip_inner_width = panel_width - 12.0 * responsive_scale;
        let chip_inner_right = chip_inner_x + chip_inner_width;
        let row_gap = if collapsed_daily_mode {
            0.0
        } else {
            chip_height + 4.5 * responsive_scale
        };
        let chip_content_span = chip_content_span.max(0.0);
        let max_rows = if chip_content_span < chip_height + 2.0 * responsive_scale {
            0
        } else if collapsed_daily_mode {
            1
        } else {
            (((chip_content_span - 2.0 * responsive_scale - chip_height) / row_gap)
                .floor()
                .max(0.0) as usize
                + 1)
            .min(2)
        };
        let chip_h_gap = if collapsed_daily_mode {
            5.2 * responsive_scale
        } else {
            5.6 * responsive_scale
        };

        let mut chip_x = chip_inner_x;
        let mut row = 0;
        let mut chip_layouts = Vec::new();
        for (index, token) in chrome.next_token_candidates.iter().take(6).enumerate() {
            let kind = InteractionKind::SelectNextToken(index);
            let (hovered, pressed) = interaction_state(kind);
            let base_chip_w = if collapsed_daily_mode {
                ((token.chars().count() as f32 * 10.0).max(52.0)
                    + if index == 0 { 28.0 } else { 18.0 })
                    * responsive_scale
            } else {
                ((token.chars().count() as f32 * 10.9).max(56.0) + 16.0) * responsive_scale
            };
            if chip_x > chip_inner_x && chip_x + base_chip_w > chip_inner_right {
                row += 1;
                chip_x = chip_inner_x;
            }
            if row >= max_rows {
                break;
            }
            let mut available_chip_w = chip_inner_right - chip_x;
            while available_chip_w <= 0.0 && row + 1 < max_rows {
                row += 1;
                chip_x = chip_inner_x;
                available_chip_w = chip_inner_right - chip_x;
            }
            if row >= max_rows || available_chip_w <= 0.0 {
                break;
            }

            let chip_w = base_chip_w.min(available_chip_w);
            if chip_w <= 0.0 {
                continue;
            }
            let rect = [
                chip_x,
                chip_y + row as f32 * row_gap,
                chip_w,
                chip_height,
            ];
            if rect[1] + chip_height > chip_section_y + available_section_h {
                break;
            }
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
                8.4 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind,
                rect: interaction_hit_rect(rect),
            });
            let layout = TextBlock {
                text: if collapsed_daily_mode {
                    format!("{} {}", index + 1, token)
                } else {
                    token.clone()
                },
                origin: [
                    visual_rect[0] + 7.2 * responsive_scale,
                    visual_rect[1] + 5.8 * responsive_scale,
                ],
                max_width: (visual_rect[2] - 14.0 * responsive_scale).max(0.0),
                pixel_size: if collapsed_daily_mode { chip_px * 0.95 } else { chip_px },
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: if index == 0 { accent_text } else { text_primary },
                align: TextAlign::Center,
                role: TextRole::NextTokenChip,
            }
            .layout();
            let is_wrapped = layout.lines.len() > 1;
            let needs_single_line_truncate = layout.truncated || is_wrapped;

            let display_text = if collapsed_daily_mode {
                format!("{} {}", index + 1, token)
            } else {
                token.clone()
            };

            let mut final_text = display_text;
            if needs_single_line_truncate {
                next_token_candidate_truncated.push(index);
            }
            if let Some(&(scroll_index, started_at)) = next_token_candidate_scroll {
                if scroll_index == index && layout.truncated {
                    final_text = scroll_text_for_candidate(
                        &final_text,
                        started_at,
                        if collapsed_daily_mode { chip_px * 0.95 } else { chip_px },
                        ui_tracking,
                        (visual_rect[2] - 14.0 * responsive_scale).max(0.0),
                    );
                }
            }

            let layout = TextBlock {
                text: final_text,
                origin: [
                    visual_rect[0] + 7.2 * responsive_scale,
                    visual_rect[1] + 5.8 * responsive_scale,
                ],
                max_width: (visual_rect[2] - 14.0 * responsive_scale).max(0.0),
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
