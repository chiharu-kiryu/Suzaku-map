{
        let separator_divider = compact_divider;

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

    let actual_chip_section_h = if chrome.next_token_candidates.is_empty() {
        0.0
    } else {
        ((candidate_area_bottom - suggestions_y - 4.0 * responsive_scale).max(0.0))
            .min(metrics.chip_section_h)
    };
    let sentence_y = suggestions_y + actual_chip_section_h + metrics.sentence_section_gap;
    let available_sentence_area = (candidate_area_bottom - sentence_y).max(0.0);
    let panel_right = panel_x + panel_width;
    let candidate_columns = metrics.candidate_columns;
    let candidate_gap_x = if collapsed_daily_mode {
        4.8 * responsive_scale
    } else {
        7.0 * responsive_scale
    };
    let candidate_count = visible_sentence_candidates.len();
    let has_hero_card = !collapsed_daily_mode
        && hero_cards_enabled
        && !visible_sentence_candidates.is_empty();
    let alternate_count = candidate_count.saturating_sub(has_hero_card as usize);
    let candidate_count_f = candidate_count as f32;
    let min_candidate_w = 40.0 * responsive_scale;
    let collapsed_fallback_card_w = {
        let gap_count = (candidate_count_f - 1.0).max(0.0);
        ((panel_width - candidate_gap_x * gap_count).max(0.0) / candidate_count_f.max(1.0))
            .max(min_candidate_w)
    };
    let (collapsed_primary_w, collapsed_primary_uses_equal_width) = if collapsed_daily_mode {
        let trailing_count = candidate_count.saturating_sub(1);
        if trailing_count == 0 {
            (panel_width, false)
        } else {
            let trailing_gap = candidate_gap_x * trailing_count as f32;
            let preferred_primary = (panel_width * 0.68).max(min_candidate_w);
            let trailing_width_guess =
                ((panel_width - preferred_primary - trailing_gap) / trailing_count as f32)
                    .max(min_candidate_w);
            let required_width =
                preferred_primary + trailing_gap + trailing_width_guess * trailing_count as f32;
            if required_width <= panel_width {
                (preferred_primary, false)
            } else {
                (collapsed_fallback_card_w, true)
            }
        }
    } else if candidate_columns > 1 {
        (0.0, false)
    } else {
        (0.0, false)
    };
    let candidate_card_w = if collapsed_daily_mode {
        let trailing_count = candidate_count.saturating_sub(1);
        if trailing_count == 0 {
            panel_width
        } else {
            let remaining = panel_width - collapsed_primary_w - candidate_gap_x * trailing_count as f32;
            let normal = (remaining / trailing_count as f32).max(min_candidate_w);
            if collapsed_primary_uses_equal_width {
                collapsed_fallback_card_w
            } else {
                normal
            }
        }
    } else if candidate_columns > 1 {
        (panel_width - candidate_gap_x * (candidate_columns.saturating_sub(1) as f32))
            / candidate_columns as f32
    } else {
        panel_width
    };
    let hero_card_height = if collapsed_daily_mode {
        33.0 * responsive_scale
} else {
    metrics.hero_item_height
};
    let hero_row_offset = if !collapsed_daily_mode && hero_cards_enabled && !visible_sentence_candidates.is_empty() {
        hero_card_height + metrics.item_gap
    } else {
        0.0
    };
    if !visible_sentence_candidates.is_empty() && available_sentence_area > 0.0 {
        let alternate_rows = if collapsed_daily_mode {
            0
        } else if alternate_count == 0 {
            0
        } else {
        alternate_count.div_ceil(candidate_columns)
    };
    let sentence_section_h = if collapsed_daily_mode {
        42.0 * responsive_scale
        } else if has_hero_card {
            hero_card_height
                + if alternate_rows == 0 {
                    0.0
                } else {
                    metrics.item_gap
                        + alternate_rows as f32 * metrics.item_height
                        + (alternate_rows as f32 - 1.0).max(0.0) * metrics.item_gap
                }
        } else {
            alternate_rows as f32 * metrics.item_height
                + (alternate_rows as f32 - 1.0).max(0.0) * metrics.item_gap
        };
        let sentence_section_h = sentence_section_h.min(available_sentence_area);
        let sentence_section_top = sentence_y;
        let sentence_interaction_hovered =
            matches!(chrome.hovered_interaction, Some(InteractionKind::Candidate(_)));
        let sentence_interaction_pressed =
            matches!(chrome.pressed_interaction, Some(InteractionKind::Candidate(_)));
        if sentence_section_h > 0.0 {
            let section_radius = (10.8 * responsive_scale).max(6.0);
            append_soft_card_quads(
                &mut quads,
                [panel_x, sentence_section_top, panel_width, sentence_section_h],
                if collapsed_daily_mode {
                    surface_muted
                } else {
                    surface
                },
                border_dark,
                soft_shadow,
                shell,
                section_radius,
            );
                quads.push(CandidateQuad {
                    rect: [
                        panel_x + 8.0 * responsive_scale,
                        sentence_section_top + 4.0 * responsive_scale,
                        panel_width - 16.0 * responsive_scale,
                        1.8 * responsive_scale,
                    ],
                    color: panel_glass_layer_color(
                        sentence_interaction_hovered,
                        chrome.input_focused,
                        sentence_interaction_pressed,
                    ),
                });
                if !collapsed_daily_mode && hero_cards_enabled {
                    quads.push(CandidateQuad {
                        rect: [
                            panel_x + 14.0 * responsive_scale,
                            sentence_section_top + hero_card_height + 2.4 * responsive_scale,
                            panel_width - 28.0 * responsive_scale,
                            1.6 * responsive_scale,
                        ],
                        color: panel_glass_layer_color(
                            sentence_interaction_hovered,
                            chrome.input_focused,
                            sentence_interaction_pressed,
                        ),
                    });
                }
            }
    }
    for (display_index, (source_index, label)) in visible_sentence_candidates.iter().enumerate() {
        let is_hero = !collapsed_daily_mode && hero_cards_enabled && display_index == 0;
        let style_label = crate::panel_support::sentence_candidate_style_label(label, is_hero);
        let (x, y, mut card_width, card_height) = if is_hero {
            (panel_x, sentence_y, panel_width, hero_card_height)
    } else if collapsed_daily_mode {
        let trailing_index = display_index.saturating_sub(1);
        (
            if display_index == 0 {
                panel_x
            } else {
                panel_x
                    + collapsed_primary_w
                    + candidate_gap_x
                    + trailing_index as f32 * (candidate_card_w + candidate_gap_x)
            },
            sentence_y,
            if display_index == 0 {
                collapsed_primary_w
            } else {
                candidate_card_w
            },
            33.0 * responsive_scale,
        )
        } else {
            let alternate_index = display_index.saturating_sub(has_hero_card as usize);
            let column = alternate_index % candidate_columns;
            let row = alternate_index / candidate_columns;
            let is_last_single_card = candidate_columns > 1
                && alternate_count % candidate_columns == 1
                && alternate_index + 1 == alternate_count;
            (
                panel_x + column as f32 * (candidate_card_w + candidate_gap_x),
                sentence_y + hero_row_offset + row as f32 * (metrics.item_height + metrics.item_gap),
                if is_last_single_card {
                    panel_width
                } else {
                    candidate_card_w
                },
                metrics.item_height,
            )
        };
        if x + card_width > panel_right {
            card_width = (panel_right - x).max(0.0);
        }
        if card_width <= 0.0 {
            continue;
        }
        if y + card_height > candidate_area_bottom + 0.01 {
            break;
        }
        let selected = *source_index == snapshot.selected_index;
        let kind = InteractionKind::Candidate(*source_index);
        let (hovered, pressed) = interaction_state(kind);
        let quad = CandidateQuad {
        rect: [x, y, card_width, card_height],
        color: [0.0, 0.0, 0.0, 0.0],
    };
    let visual_rect = animated_rect(quad.rect, hovered, pressed);
    let collapsed_primary = collapsed_daily_mode && display_index == 0;
    append_soft_card_quads(
        &mut quads,
        visual_rect,
        if pressed {
            press_surface
        } else if collapsed_primary || selected {
            accent_soft
        } else if is_hero {
            surface_bright
        } else if hovered {
            hover_surface
        } else if collapsed_daily_mode {
            surface_alt
        } else if snapshot.degraded {
            surface_muted
        } else {
            surface
        },
            if pressed {
                accent
        } else if collapsed_primary || selected || is_hero {
            accent
        } else if hovered {
            hover_border
        } else {
            border_dark
        },
        animated_shadow(soft_shadow, hovered, pressed),
        surface,
            (10.2 * responsive_scale).max(6.0),
    );
    if collapsed_primary {
        append_rounded_rect_quads(
            &mut quads,
            [
                visual_rect[0] + 3.0 * responsive_scale,
                visual_rect[1] + 3.0 * responsive_scale,
                visual_rect[2] - 6.0 * responsive_scale,
                visual_rect[3] - 6.0 * responsive_scale,
            ],
            separator_divider,
            9.0 * responsive_scale,
        );
    } else if collapsed_daily_mode && display_index > 0 {
        quads.push(CandidateQuad {
            rect: [
                visual_rect[0] - candidate_gap_x * 0.5,
                visual_rect[1] + 7.0 * responsive_scale,
                1.0,
                visual_rect[3] - 14.0 * responsive_scale,
            ],
            color: compact_divider,
        });
    }
    if is_hero {
            append_rounded_rect_quads(
                &mut quads,
                [
                    visual_rect[0] + 4.0 * responsive_scale,
                    visual_rect[1] + 4.0 * responsive_scale,
                    visual_rect[2] - 8.0 * responsive_scale,
                    visual_rect[3] - 8.0 * responsive_scale,
                ],
                panel_glass_layer_color(hovered, chrome.input_focused, pressed),
                9.8 * responsive_scale,
            );
    }
    hit_targets.push(HitTarget {
        index: *source_index,
        rect: quad.rect,
    });
    interactive_targets.push(InteractiveTarget {
        kind,
        rect: interaction_hit_rect(quad.rect),
    });
    let primary_pixel_size = if is_hero {
        hero_px
    } else if collapsed_daily_mode {
        chip_px * 1.02
    } else if candidate_columns >= 3 {
        // Preserve complete short sentences when the default desktop layout fills one row.
        input_value_px * 0.80
    } else {
        input_value_px * 1.02
    };
    let mut primary_text = if collapsed_daily_mode {
        if display_index == 0 {
            format!("{}  {}", display_index + 1, label)
        } else {
            format!("{} {}", display_index + 1, label)
        }
    } else {
        label.clone()
    };
    let primary_max_width = (visual_rect[2]
        - if collapsed_daily_mode {
            20.0 * responsive_scale
        } else {
            36.0 * responsive_scale
        })
        .max(4.0 * responsive_scale);
    let primary_top_padding = if collapsed_daily_mode {
        9.0 * responsive_scale
    } else {
        8.0 * responsive_scale
    };
    let primary_origin = [
        visual_rect[0]
            + if collapsed_daily_mode {
                10.0 * responsive_scale
            } else {
                18.0 * responsive_scale
            },
        visual_rect[1] + primary_top_padding,
    ];
    let primary_color = if collapsed_primary || selected {
        accent_text
    } else {
        text_primary
    };
    let single_line_height = primary_pixel_size * 7.0;
    let primary_available_height =
        (visual_rect[3] - primary_top_padding - 3.0 * responsive_scale).max(0.0);
    let primary_max_lines = if !collapsed_daily_mode
        && primary_available_height >= single_line_height * 2.0 + base_line_gap
    {
        2
    } else {
        1
    };
    let build_primary_layout = |text: &str, max_lines: usize| {
        TextBlock {
            text: text.to_string(),
            origin: primary_origin,
            max_width: primary_max_width,
            pixel_size: primary_pixel_size,
            letter_spacing: heading_tracking,
            line_gap: base_line_gap,
            max_lines,
            color: primary_color,
            align: TextAlign::Left,
            role: TextRole::CandidatePrimary,
        }
        .layout()
    };
    let mut primary_layout = build_primary_layout(&primary_text, primary_max_lines);
    if primary_layout.lines.is_empty() {
        primary_layout = build_primary_layout("", primary_max_lines.min(1).max(1));
    }
    if primary_layout.truncated {
        sentence_candidate_truncated.push(*source_index);
    }

    if let Some(&(scroll_index, started_at)) = sentence_candidate_scroll {
        if scroll_index == *source_index && primary_layout.truncated {
            let scrolled = scroll_text_for_candidate(
                &primary_text,
                started_at,
                primary_pixel_size,
                heading_tracking,
                (visual_rect[2]
                    - if collapsed_daily_mode {
                        20.0 * responsive_scale
                    } else {
                        36.0 * responsive_scale
                    })
                    .max(4.0 * responsive_scale),
            );
            primary_text = scrolled;
            primary_layout = TextBlock {
                text: primary_text,
                origin: primary_origin,
                max_width: (visual_rect[2]
                    - if collapsed_daily_mode {
                        20.0 * responsive_scale
                    } else {
                        36.0 * responsive_scale
                    })
                    .max(4.0 * responsive_scale),
                pixel_size: primary_pixel_size,
                letter_spacing: heading_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: if collapsed_primary || selected {
                    accent_text
                } else {
                    text_primary
                },
                align: TextAlign::Left,
                role: TextRole::CandidatePrimary,
            }
            .layout();
        }
    }
    let mut candidate_layouts = Vec::new();
    let primary_bottom = primary_layout.bounds[1] + primary_layout.bounds[3];
    candidate_layouts.push(primary_layout);

    if !collapsed_daily_mode {
        let meta_label = if selected && is_hero {
            "primary sentence selected".to_string()
        } else if selected {
            "alternate sentence selected".to_string()
        } else if is_hero {
            "sentence best match · tap to commit".to_string()
        } else {
            format!("sentence {style_label} phrase · tap to commit")
        };

        let meta_height = helper_px * 7.0;
        let preferred_meta_y = visual_rect[1] + visual_rect[3]
            - meta_height
            - 1.5 * responsive_scale;
        let meta_max_lines = usize::from(
            preferred_meta_y >= primary_bottom + 2.0 * responsive_scale,
        );

        if meta_max_lines > 0 {
            candidate_layouts.push(
                TextBlock {
                    text: meta_label,
                    origin: [visual_rect[0] + 18.0 * responsive_scale, preferred_meta_y],
                    max_width: (visual_rect[2] - 36.0 * responsive_scale)
                        .max(4.0 * responsive_scale),
                    pixel_size: helper_px,
                    letter_spacing: ui_tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: if selected {
                        selected_meta_text
                    } else {
                        text_secondary
                    },
                    align: TextAlign::Left,
                    role: TextRole::CandidateMeta,
                }
                .layout(),
            );
        }
    }

    if is_hero {
        if collapsed_daily_mode {
            let available_badge_width = (visual_rect[2] - 6.0 * responsive_scale).max(0.0);
            if available_badge_width > 0.0 {
                let badge_width = (66.0 * responsive_scale).min(available_badge_width);
                if badge_width > 0.0 {
                    let badge_rect = [
                        visual_rect[0] + visual_rect[2] - badge_width,
                        visual_rect[1] + 9.2 * responsive_scale,
                        badge_width,
                        17.0 * responsive_scale,
                    ];
                    quads.push(CandidateQuad {
                        rect: badge_rect,
                        color: badge_fill,
                    });
                }
            }
        }
    }
    for layout in &candidate_layouts {
        text_quads.extend(layout.quads.iter().copied());
        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
    }
    text_sections.push(TextSection {
        role: TextRole::CandidatePrimary,
        layouts: candidate_layouts,
    });
    if is_hero {
        let available_badge_width = (visual_rect[2] - 6.0 * responsive_scale).max(0.0);
        let badge_width = (66.0 * responsive_scale).min(available_badge_width);
        let badge_layout = TextBlock {
            text: style_label.to_string(),
            origin: [
                visual_rect[0]
                    + visual_rect[2]
                    - badge_width
                    + 8.0 * responsive_scale,
                visual_rect[1] + 10.0 * responsive_scale,
            ],
            max_width: (badge_width - 16.0 * responsive_scale).max(0.0),
            pixel_size: badge_meta_px,
            letter_spacing: ui_tracking * 0.7,
            line_gap: base_line_gap,
            max_lines: 1,
            color: badge_text,
            align: TextAlign::Center,
            role: TextRole::CandidateMeta,
        }
        .layout();
        text_quads.extend(badge_layout.quads.iter().copied());
        atlas_glyphs.extend(badge_layout.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::CandidateMeta,
            layouts: vec![badge_layout],
        });
    }
}
}
