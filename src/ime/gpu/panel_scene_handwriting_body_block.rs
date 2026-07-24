{
                    let handwriting_scale = (drawer_rect[3] / (246.0 * responsive_scale)).clamp(0.82, 1.0);
                    let handwriting_y = drawer_rect[1] + 16.0 * responsive_scale * handwriting_scale;
                    let handwriting_title_y = handwriting_y + 6.0 * responsive_scale * handwriting_scale;
                    let handwriting_status_y = handwriting_title_y;
                    let handwriting_hint_y = handwriting_y + 30.0 * responsive_scale * handwriting_scale;
                    let handwriting_padding_x = 10.0 * responsive_scale * handwriting_scale;
                    let handwriting_footer_row_h = 18.0 * responsive_scale * handwriting_scale;
                    let handwriting_footer_gap_x = 8.0 * responsive_scale * handwriting_scale;
                    let handwriting_footer_gap_y = 6.0 * responsive_scale * handwriting_scale;
                    let base_action_w = (68.0 * handwriting_scale).max(58.0 * responsive_scale * handwriting_scale);
                    let candidate_text_max = 3usize;
                    let max_candidate_rows = 2usize;

                    let footer_left = drawer_rect[0] + handwriting_padding_x;
                    let footer_right = drawer_rect[0] + drawer_rect[2] - handwriting_padding_x;
                    let footer_width = (footer_right - footer_left).max(112.0);

                    let available_action_width = (footer_width * 0.38)
                        .clamp(base_action_w * 2.0 + handwriting_footer_gap_x, 190.0 * handwriting_scale);
                    let action_w = {
                        let candidate_area_min = 76.0 * responsive_scale * handwriting_scale;
                        let can_show_candidates = footer_width > available_action_width + candidate_area_min + handwriting_footer_gap_x;
                        let total_action = if can_show_candidates {
                            available_action_width.min(footer_width - handwriting_footer_gap_x - candidate_area_min)
                        } else {
                            footer_width - handwriting_footer_gap_x
                        }
                        .max(base_action_w * 2.0 + handwriting_footer_gap_x);

                        let undo_width = (total_action * 0.46).max(base_action_w);
                        let clear_width = (total_action - undo_width - handwriting_footer_gap_x).max(base_action_w * 0.8);
                        let candidate_area_x = footer_left + undo_width + clear_width + handwriting_footer_gap_x * 1.4;
                        if can_show_candidates && candidate_area_x + 56.0 * responsive_scale * handwriting_scale <= footer_right {
                            undo_width
                        } else {
                            footer_width * 0.5
                        }
                    };
                    let clear_w = (action_w * 1.0).max(base_action_w * 0.95);
                    let candidate_area_start = if action_w + clear_w + handwriting_footer_gap_x <= footer_width {
                        footer_left + action_w + clear_w + handwriting_footer_gap_x * 1.4
                    } else {
                        footer_left
                    };
                    let candidate_area_right = footer_right - 6.0 * responsive_scale * handwriting_scale;
                    let _candidate_area_width = (candidate_area_right - candidate_area_start).max(0.0);

                    let candidate_width = |text: &str, max_width: f32| {
                        let raw = (text.chars().count() as f32 * 9.5 + 16.0)
                            .max(48.0)
                            * handwriting_scale;
                        raw.min(max_width).max(44.0 * handwriting_scale)
                    };

                    let mut candidate_rows = 1usize;
                    let mut candidate_x_cursor = candidate_area_start;
                    let mut candidate_widths = Vec::with_capacity(candidate_text_max);
                    for candidate in chrome.handwriting_candidates.iter().take(candidate_text_max) {
                        if candidate_area_start >= candidate_area_right {
                            break;
                        }

                        let max_chip_w = candidate_area_right - candidate_area_start;
                        let w = candidate_width(candidate, max_chip_w);
                        let need_wrap = candidate_x_cursor + w > candidate_area_right && candidate_rows < max_candidate_rows;
                        if need_wrap {
                            candidate_rows += 1;
                            if candidate_rows > max_candidate_rows {
                                break;
                            }
                            candidate_x_cursor = candidate_area_start;
                        }

                        if candidate_rows > max_candidate_rows {
                            break;
                        }

                        candidate_widths.push(w);
                        candidate_x_cursor = (candidate_x_cursor + w + handwriting_footer_gap_x).max(candidate_area_start + w);
                    }
                    if candidate_widths.is_empty() {
                        candidate_rows = 1;
                    }

                    let handwriting_footer_rows = candidate_rows as f32;
                    let handwriting_footer_rect = [
                        footer_left,
                        drawer_rect[1] + drawer_rect[3]
                            - (handwriting_footer_rows * (handwriting_footer_row_h + handwriting_footer_gap_y)
                                + 10.0 * responsive_scale * handwriting_scale),
                        footer_width,
                        handwriting_footer_rows * (handwriting_footer_row_h + handwriting_footer_gap_y)
                            + 10.0 * responsive_scale * handwriting_scale,
                    ];
                    let canvas_top = handwriting_y + 50.0 * responsive_scale * handwriting_scale;
                    let canvas_rect = [
                        drawer_rect[0],
                        canvas_top,
                        drawer_rect[2],
                        (handwriting_footer_rect[1] - 8.0 * responsive_scale * handwriting_scale - canvas_top)
                            .max(104.0 * responsive_scale * handwriting_scale),
                    ];

                    append_soft_card_quads(
                        &mut quads,
                        canvas_rect,
                        [0.98, 0.99, 1.0, 1.0],
                        border_dark,
                        soft_shadow,
                        surface,
                        10.0 * responsive_scale * handwriting_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::HandwritingCanvas,
                        rect: canvas_rect,
                    });

                    for stroke in &chrome.handwriting_strokes {
                        for point in sample_stroke_points(stroke) {
                            quads.push(CandidateQuad {
                                rect: [
                                    point[0] - 2.5 * handwriting_scale,
                                    point[1] - 2.5 * handwriting_scale,
                                    5.0 * handwriting_scale,
                                    5.0 * handwriting_scale,
                                ],
                                color: accent,
                            });
                        }
                    }

                    let handwriting_layouts = vec![
                        TextBlock {
                            text: "Handwrite input".to_string(),
                            origin: [drawer_rect[0] + 14.0 * responsive_scale * handwriting_scale, handwriting_title_y],
                            max_width: (drawer_rect[2] - 160.0 * responsive_scale * handwriting_scale).max(108.0),
                            pixel_size: title_px * handwriting_scale,
                            letter_spacing: heading_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: "Trace on canvas".to_string(),
                            origin: [
                                drawer_rect[0] + drawer_rect[2]
                                    - 118.0 * responsive_scale * handwriting_scale,
                                handwriting_status_y,
                            ],
                            max_width: 108.0 * responsive_scale * handwriting_scale,
                            pixel_size: helper_px * handwriting_scale,
                            letter_spacing: ui_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 1,
                            color: text_primary,
                            align: TextAlign::Center,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: chrome.handwriting_hint.clone(),
                            origin: [drawer_rect[0] + 14.0 * responsive_scale * handwriting_scale, handwriting_hint_y],
                            max_width: drawer_rect[2] - 28.0 * responsive_scale * handwriting_scale,
                            pixel_size: helper_px * handwriting_scale,
                            letter_spacing: ui_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 2,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                    ];
                    for layout in &handwriting_layouts {
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    }
                    text_sections.push(TextSection {
                        role: TextRole::HandwritingLabel,
                        layouts: handwriting_layouts,
                    });

                    append_soft_card_quads(
                        &mut quads,
                        handwriting_footer_rect,
                        surface,
                        border_dark,
                        [0.25, 0.34, 0.48, 0.06],
                        shell,
                        8.0 * responsive_scale * handwriting_scale,
                    );

                    let undo_rect = [
                        handwriting_footer_rect[0] + 6.0 * responsive_scale * handwriting_scale,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale * handwriting_scale,
                        action_w.min(handwriting_footer_rect[2] * 0.5),
                        handwriting_footer_row_h,
                    ];
                    let undo_enabled = !chrome.handwriting_strokes.is_empty();
                    let (undo_hovered, undo_pressed) =
                        interaction_state(InteractionKind::UndoHandwritingStroke);
                    let undo_visual_rect = animated_rect(undo_rect, undo_hovered, undo_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        undo_visual_rect,
                        if undo_pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if undo_hovered {
                            [0.95, 0.97, 1.0, 1.0]
                        } else if undo_enabled {
                            [0.92, 0.95, 0.99, 1.0]
                        } else {
                            [0.97, 0.98, 1.0, 1.0]
                        },
                        border_dark,
                        animated_shadow(soft_shadow, undo_hovered, undo_pressed),
                        surface,
                        8.0 * responsive_scale * handwriting_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::UndoHandwritingStroke,
                        rect: undo_rect,
                    });

                    let clear_rect = [
                        undo_rect[0]
                            + undo_rect[2]
                            + handwriting_footer_gap_x,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale * handwriting_scale,
                        clear_w.min(undo_rect[2] * 1.05),
                        handwriting_footer_row_h,
                    ];
                    let clear_rect = if clear_rect[0] + clear_rect[2] <= handwriting_footer_rect[0] + handwriting_footer_rect[2] - 6.0 * responsive_scale * handwriting_scale
                    {
                        clear_rect
                    } else {
                        [
                            handwriting_footer_rect[0] + handwriting_footer_rect[2] * 0.5 + handwriting_footer_gap_x,
                            handwriting_footer_rect[1] + 2.0 * responsive_scale * handwriting_scale,
                            (handwriting_footer_rect[2] * 0.25).max(base_action_w),
                            handwriting_footer_row_h,
                        ]
                    };
                    let (clear_hovered, clear_pressed) =
                        interaction_state(InteractionKind::ClearHandwriting);
                    let clear_visual_rect = animated_rect(clear_rect, clear_hovered, clear_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        clear_visual_rect,
                        if clear_pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if clear_hovered {
                            [0.95, 0.97, 1.0, 1.0]
                        } else {
                            [0.92, 0.95, 0.99, 1.0]
                        },
                        border_dark,
                        animated_shadow(soft_shadow, clear_hovered, clear_pressed),
                        surface,
                        8.0 * responsive_scale * handwriting_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ClearHandwriting,
                        rect: clear_rect,
                    });

                    let mut handwriting_action_layouts = Vec::new();
                    append_undo_icon_quads(
                        &mut quads,
                        undo_visual_rect,
                        if undo_enabled {
                            text_primary
                        } else {
                            text_muted
                        },
                    );
                    append_trash_icon_quads(&mut quads, clear_visual_rect, text_primary);

                    let mut chip_x = candidate_area_start;
                    let mut chip_row = 0usize;
                    let mut chip_index = 0usize;
                    for (index, candidate) in chrome
                        .handwriting_candidates
                        .iter()
                        .take(candidate_text_max)
                        .enumerate()
                    {
                        if chip_row >= max_candidate_rows {
                            break;
                        }

                        let max_chip_w = candidate_area_right - chip_x;
                        let chip_w = candidate_width(candidate, max_chip_w);
                        if chip_x > candidate_area_start && chip_x + chip_w > candidate_area_right {
                            chip_row += 1;
                            if chip_row >= max_candidate_rows {
                                break;
                            }
                            chip_x = candidate_area_start;
                        }

                        let chip_y = handwriting_footer_rect[1]
                            + 2.0 * responsive_scale * handwriting_scale
                            + chip_row as f32 * (handwriting_footer_row_h + handwriting_footer_gap_y);
                        let rect = [chip_x, chip_y, chip_w, handwriting_footer_row_h];
                        let kind = InteractionKind::UseHandwritingCandidate(index);
                        let (hovered, pressed) = interaction_state(kind);
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                [0.67, 0.82, 0.97, 1.0]
                            } else if index == 0 {
                                accent_soft
                            } else if hovered {
                                [0.93, 0.96, 1.0, 1.0]
                            } else {
                                [0.97, 0.98, 1.0, 1.0]
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
                            8.0 * responsive_scale * handwriting_scale,
                        );
                        interactive_targets.push(InteractiveTarget { kind, rect });
                        let layout = TextBlock {
                            text: candidate.clone(),
                            origin: [
                                visual_rect[0] + 8.0 * responsive_scale * handwriting_scale,
                                visual_rect[1] + 6.0 * responsive_scale * handwriting_scale,
                            ],
                            max_width: (visual_rect[2] - 16.0 * responsive_scale * handwriting_scale).max(6.0),
                            pixel_size: 2.0 * handwriting_scale,
                            letter_spacing: ui_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 1,
                            color: if index == 0 {
                                accent_text
                            } else {
                                text_primary
                            },
                            align: TextAlign::Center,
                            role: TextRole::HandwritingCandidate,
                        }
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        handwriting_action_layouts.push(layout);
                        chip_x += chip_w + handwriting_footer_gap_x;
                        chip_index += 1;
                    }
                    if chip_index == 0 {
                        let no_candidates = [
                            TextBlock {
                                text: "Write to generate".to_string(),
                                origin: [
                                    candidate_area_start + 6.0 * responsive_scale * handwriting_scale,
                                    handwriting_footer_rect[1]
                                        + 2.0 * responsive_scale * handwriting_scale,
                                ],
                                max_width: (candidate_area_right - candidate_area_start - 12.0 * responsive_scale * handwriting_scale)
                                    .max(80.0),
                                pixel_size: 2.0 * handwriting_scale,
                                letter_spacing: ui_tracking * handwriting_scale,
                                line_gap: base_line_gap * handwriting_scale,
                                max_lines: 1,
                                color: text_muted,
                                align: TextAlign::Center,
                                role: TextRole::HandwritingCandidate,
                            }
                            .layout(),
                        ];
                        for layout in &no_candidates {
                            text_quads.extend(layout.quads.iter().copied());
                            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        }
                        text_sections.push(TextSection {
                            role: TextRole::HandwritingButton,
                            layouts: no_candidates.to_vec(),
                        });
                    } else {
                        text_sections.push(TextSection {
                            role: TextRole::HandwritingButton,
                            layouts: handwriting_action_layouts,
                        });
                    }
}
