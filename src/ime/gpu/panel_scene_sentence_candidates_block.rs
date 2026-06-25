{
let sentence_y = suggestions_y + metrics.chip_section_h + metrics.sentence_section_gap;
let candidate_columns = metrics.candidate_columns;
let candidate_gap_x = if collapsed_daily_mode {
    6.0 * responsive_scale
} else {
    14.0 * responsive_scale
};
let collapsed_primary_w = if collapsed_daily_mode {
    (panel_width * 0.42).clamp(180.0 * responsive_scale, 280.0 * responsive_scale)
} else if candidate_columns == 2 {
    0.0
} else {
    0.0
};
let candidate_card_w = if collapsed_daily_mode {
    let trailing_count = visible_sentence_candidates.len().saturating_sub(1);
    if trailing_count == 0 {
        panel_width
    } else {
        (panel_width - collapsed_primary_w - candidate_gap_x * trailing_count as f32)
            / trailing_count as f32
    }
} else if candidate_columns == 2 {
    (panel_width - candidate_gap_x) / 2.0
} else {
    panel_width
};
if !visible_sentence_candidates.is_empty() {
    let alternate_count = visible_sentence_candidates.len().saturating_sub(1);
    let alternate_rows = if collapsed_daily_mode {
        0
    } else if alternate_count == 0 {
        0
    } else {
        alternate_count.div_ceil(candidate_columns)
    };
    let sentence_section_h = if collapsed_daily_mode {
        46.0 * responsive_scale
    } else if alternate_rows == 0 {
        metrics.hero_item_height
    } else {
        metrics.hero_item_height
            + metrics.item_gap
            + alternate_rows as f32 * metrics.item_height
            + (alternate_rows as f32 - 1.0).max(0.0) * metrics.item_gap
    };
    append_soft_card_quads(
        &mut quads,
        [panel_x, sentence_y, panel_width, sentence_section_h],
        if collapsed_daily_mode {
            surface_muted
        } else {
            surface
        },
        border_dark,
        soft_shadow,
        shell,
        if collapsed_daily_mode {
            10.0 * responsive_scale
        } else {
            12.0 * responsive_scale
        },
    );
}
for (display_index, (source_index, label)) in visible_sentence_candidates.iter().enumerate() {
    let is_hero = !collapsed_daily_mode && display_index == 0;
    let style_label = crate::panel_support::sentence_candidate_style_label(label, is_hero);
    let (x, y, card_width, card_height) = if is_hero {
        (panel_x, sentence_y, panel_width, metrics.hero_item_height)
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
            36.0 * responsive_scale,
        )
    } else {
        let alternate_index = display_index - 1;
        let column = alternate_index % candidate_columns;
        let row = alternate_index / candidate_columns;
        (
            panel_x + column as f32 * (candidate_card_w + candidate_gap_x),
            sentence_y
                + metrics.hero_item_height
                + metrics.item_gap
                + row as f32 * (metrics.item_height + metrics.item_gap),
            candidate_card_w,
            metrics.item_height,
        )
    };
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
            [0.08, 0.35, 0.68, 1.0]
        } else if collapsed_primary || selected || is_hero {
            accent
        } else if hovered {
            hover_border
        } else {
            border_dark
        },
        animated_shadow(soft_shadow, hovered, pressed),
        surface,
        if is_hero {
            12.0 * responsive_scale
        } else if collapsed_daily_mode {
            10.0 * responsive_scale
        } else {
            10.0 * responsive_scale
        },
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
            [1.0, 1.0, 1.0, 0.08],
            8.0 * responsive_scale,
        );
    } else if collapsed_daily_mode && display_index > 0 {
        quads.push(CandidateQuad {
            rect: [
                visual_rect[0] - candidate_gap_x * 0.5,
                visual_rect[1] + 7.0 * responsive_scale,
                1.0,
                visual_rect[3] - 14.0 * responsive_scale,
            ],
            color: [1.0, 1.0, 1.0, 0.10],
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
            [1.0, 1.0, 1.0, 0.08],
            10.0 * responsive_scale,
        );
    }
    if is_hero {
        let badge_rect = [
            visual_rect[0] + visual_rect[2] - 88.0 * responsive_scale,
            visual_rect[1] + 10.0 * responsive_scale,
            68.0 * responsive_scale,
            18.0 * responsive_scale,
        ];
        quads.push(CandidateQuad {
            rect: badge_rect,
            color: badge_fill,
        });
        let badge_layout = TextBlock {
            text: style_label.to_string(),
            origin: [
                badge_rect[0] + 8.0 * responsive_scale,
                badge_rect[1] + 4.0 * responsive_scale,
            ],
            max_width: badge_rect[2] - 16.0 * responsive_scale,
            pixel_size: 1.7 * responsive_scale,
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
    hit_targets.push(HitTarget {
        index: *source_index,
        rect: quad.rect,
    });
    interactive_targets.push(InteractiveTarget { kind, rect: quad.rect });
    let primary_layout = TextBlock {
        text: if collapsed_daily_mode {
            if display_index == 0 {
                format!("{}  {}", display_index + 1, label)
            } else {
                format!("{} {}", display_index + 1, label)
            }
        } else {
            label.clone()
        },
        origin: [
            visual_rect[0]
                + if collapsed_daily_mode {
                    10.0 * responsive_scale
                } else {
                    18.0 * responsive_scale
                },
            visual_rect[1]
                + if collapsed_daily_mode {
                    9.0 * responsive_scale
                } else {
                    16.0 * responsive_scale
                },
        ],
        max_width: visual_rect[2]
            - if collapsed_daily_mode {
                20.0 * responsive_scale
            } else {
                36.0 * responsive_scale
            },
        pixel_size: if is_hero {
            hero_px
        } else if collapsed_daily_mode {
            chip_px * 1.02
        } else {
            input_value_px * 1.02
        },
        letter_spacing: heading_tracking,
        line_gap: base_line_gap,
        max_lines: if collapsed_daily_mode {
            1
        } else if is_hero || chrome.preview_style == PreviewStyle::Full {
            3
        } else {
            2
        },
        color: if collapsed_primary || selected {
            accent_text
        } else {
            text_primary
        },
        align: TextAlign::Left,
        role: TextRole::CandidatePrimary,
    }
    .layout();
    let candidate_layouts = if collapsed_daily_mode {
        vec![primary_layout]
    } else {
        let meta_y =
            (primary_layout.bounds[1] + primary_layout.bounds[3] + 10.0 * responsive_scale)
                .max(visual_rect[1] + 54.0 * responsive_scale);
        vec![
            primary_layout,
            TextBlock {
                text: if selected && is_hero {
                    "Primary sentence selected".to_string()
                } else if selected {
                    "Alternate sentence selected".to_string()
                } else if is_hero {
                    "Best match · tap to commit".to_string()
                } else {
                    format!("{style_label} phrasing · tap to commit")
                },
                origin: [visual_rect[0] + 18.0 * responsive_scale, meta_y],
                max_width: visual_rect[2] - 36.0 * responsive_scale,
                pixel_size: helper_px,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 2,
                color: if selected {
                    selected_meta_text
                } else {
                    text_secondary
                },
                align: TextAlign::Left,
                role: TextRole::CandidateMeta,
            }
            .layout(),
        ]
    };
    for layout in &candidate_layouts {
        text_quads.extend(layout.quads.iter().copied());
        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
    }
    text_sections.push(TextSection {
        role: TextRole::CandidatePrimary,
        layouts: candidate_layouts,
    });
}
}
