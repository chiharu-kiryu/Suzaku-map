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
                            text_char_advance(ch, pixel_size, letter_spacing)
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

                    // Width controls typography; growing the drawer must not enlarge its text again.
                    let handwriting_scale = (drawer_rect[2] / (640.0 * responsive_scale)).clamp(0.9, 1.1);
                    let handwriting_title_px = (title_px * handwriting_scale).max(2.4 * responsive_scale);
                    let handwriting_hint_px = (helper_px * 0.9).max(2.2 * responsive_scale);
                    let handwriting_padding_x = 8.0 * responsive_scale;
                    let handwriting_footer_gap_x = 8.0 * responsive_scale;
                    let candidate_text_max = 2usize;
                    let footer_left = drawer_rect[0] + handwriting_padding_x;
                    let footer_right = drawer_rect[0] + drawer_rect[2] - handwriting_padding_x;
                    let footer_width = (footer_right - footer_left).max(0.0);

                    // A glyph is seven pixel_size units high, not one. Reserve full rows
                    // before assigning the remaining space to ink. Hint rows are fixed by
                    // width, so changing "Tracing..." does not move the canvas mid-stroke.
                    let hint_lines = if footer_width < 500.0 * responsive_scale { 2 } else { 1 };
                    let header_h = handwriting_title_px.max(handwriting_hint_px) * 7.0 + 4.0 * responsive_scale;
                    let hint_h = handwriting_hint_px * 7.0 * hint_lines as f32
                        + base_line_gap * handwriting_scale * (hint_lines - 1) as f32
                        + 4.0 * responsive_scale;
                    let footer_h = (handwriting_hint_px * 7.0 + 8.0 * responsive_scale).max(26.0 * responsive_scale);
                    let vertical_gap = 8.0 * responsive_scale;
                    let top_padding = 12.0 * responsive_scale;
                    let bottom_padding = 6.0 * responsive_scale;
                    let reserved_h = top_padding + header_h + hint_h + footer_h
                        + vertical_gap * 3.0 + bottom_padding;
                    // During a pending native resize, fit text within the old short viewport.
                    let vertical_fit = (drawer_rect[3].max(0.0) / reserved_h).clamp(0.0, 1.0);
                    let header_y = drawer_rect[1] + top_padding * vertical_fit;
                    let status_w = measure_text_prefix_width(ui.tr("Trace on canvas"), ui.tr("Trace on canvas").chars().count(), handwriting_hint_px, ui_tracking * handwriting_scale)
                        .min(footer_width * 0.46);
                    let title_rect = [footer_left, header_y,
                        (footer_width - status_w - handwriting_footer_gap_x).max(0.0), header_h * vertical_fit];
                    let status_rect = [footer_right - status_w, header_y, status_w, header_h * vertical_fit];
                    let hint_rect = [footer_left, header_y + (header_h + vertical_gap) * vertical_fit,
                        footer_width, hint_h * vertical_fit];
                    let handwriting_footer_row_h = footer_h * vertical_fit;
                    let handwriting_footer_rect = [footer_left,
                        drawer_rect[1] + drawer_rect[3].max(0.0) - (footer_h + bottom_padding) * vertical_fit,
                        footer_width, handwriting_footer_row_h];
                    let canvas_top = hint_rect[1] + hint_rect[3] + vertical_gap * vertical_fit;
                    let canvas_rect = [footer_left, canvas_top, footer_width,
                        (handwriting_footer_rect[1] - vertical_gap * vertical_fit - canvas_top).max(0.0)];

                    // One left-to-right footer flow: candidates start after the actual Clear
                    // button edge, not after a separate estimate of the action group width.
                    let action_w = (44.0 * responsive_scale).min(((footer_width - handwriting_footer_gap_x) * 0.5).max(0.0));
                    let undo_rect = [footer_left, handwriting_footer_rect[1], action_w, handwriting_footer_row_h];
                    let clear_rect = [undo_rect[0] + undo_rect[2] + handwriting_footer_gap_x,
                        undo_rect[1], action_w, handwriting_footer_row_h];
                    let candidate_area_start = clear_rect[0] + clear_rect[2] + handwriting_footer_gap_x;
                    let candidate_area_right = footer_right;
                    let candidate_area_width = (candidate_area_right - candidate_area_start).max(0.0);
                    let visible_candidate_count = chrome.handwriting_candidates.len().min(candidate_text_max)
                        .min(((candidate_area_width + handwriting_footer_gap_x) / (40.0 * responsive_scale + handwriting_footer_gap_x)).floor() as usize);
                    let candidate_slot_width = ((candidate_area_width
                        - handwriting_footer_gap_x * visible_candidate_count.saturating_sub(1) as f32)
                        / visible_candidate_count.max(1) as f32).max(0.0);
                    let candidate_width = |text: &str, max_width: f32| {
                        let raw = measure_text_prefix_width(text, text.chars().count(), handwriting_hint_px, ui_tracking * handwriting_scale)
                            + 16.0 * responsive_scale * handwriting_scale;
                        raw.max(48.0 * responsive_scale).min(candidate_slot_width).min(max_width.max(0.0))
                    };

                    append_soft_card_quads(
                        &mut quads,
                        canvas_rect,
                            keyboard_surface,
                            border_dark,
                            soft_shadow,
                            surface,
                            8.8 * responsive_scale * handwriting_scale,
                    );
                    if canvas_rect[2] > 0.0 && canvas_rect[3] > 0.0 {
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::HandwritingCanvas,
                            rect: canvas_rect,
                        });
                    }

                    for stroke in &chrome.handwriting_strokes {
                        for point in sample_stroke_points(stroke) {
                            quads.push(CandidateQuad { shape: Default::default(), clip_rect: None,
                                rect: intersect_rect([
                                    point[0] - 2.5 * responsive_scale * handwriting_scale,
                                    point[1] - 2.5 * responsive_scale * handwriting_scale,
                                    5.0 * responsive_scale * handwriting_scale,
                                    5.0 * responsive_scale * handwriting_scale,
                                ], canvas_rect),
                                color: accent,
                            });
                        }
                    }

                    let handwriting_layouts = vec![
                        TextBlock {
                            text: ui.tr("Handwrite input").to_string(),
                            origin: [title_rect[0], title_rect[1]],
                            max_width: title_rect[2],
                            pixel_size: handwriting_title_px,
                            letter_spacing: heading_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout_in_rect(title_rect, [0.0, 2.0 * responsive_scale * vertical_fit]),
                        TextBlock {
                            text: ui.tr("Trace on canvas").to_string(),
                            origin: [status_rect[0], status_rect[1]],
                            max_width: status_rect[2],
                            pixel_size: handwriting_hint_px,
                            letter_spacing: ui_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: 1,
                            color: text_primary,
                            align: TextAlign::Center,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout_in_rect(status_rect, [0.0, 2.0 * responsive_scale * vertical_fit]),
                        TextBlock {
                            text: ui.message(&chrome.handwriting_hint).into_owned(),
                            origin: [hint_rect[0], hint_rect[1]],
                            max_width: hint_rect[2],
                            pixel_size: handwriting_hint_px,
                            letter_spacing: ui_tracking * handwriting_scale,
                            line_gap: base_line_gap * handwriting_scale,
                            max_lines: hint_lines,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout_in_rect(hint_rect, [0.0, 2.0 * responsive_scale * vertical_fit]),
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
                        soft_shadow,
                        shell,
                        7.2 * responsive_scale * handwriting_scale,
                    );

                    let undo_enabled = !chrome.handwriting_strokes.is_empty();
                    let (undo_hovered, undo_pressed) =
                        interaction_state(InteractionKind::UndoHandwritingStroke);
                    let undo_visual_rect = animated_rect(undo_rect, undo_hovered, undo_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        undo_visual_rect,
                        if undo_pressed {
                            press_surface
                        } else if undo_hovered {
                            hover_surface
                        } else if undo_enabled {
                            surface_alt
                        } else {
                            surface
                        },
                        border_dark,
                        animated_shadow(soft_shadow, undo_hovered, undo_pressed),
                        surface,
                        8.0 * responsive_scale * handwriting_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::UndoHandwritingStroke,
                        rect: interaction_hit_rect(undo_rect),
                    });

                    let (clear_hovered, clear_pressed) =
                        interaction_state(InteractionKind::ClearHandwriting);
                    let clear_visual_rect = animated_rect(clear_rect, clear_hovered, clear_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        clear_visual_rect,
                        if clear_pressed {
                            press_surface
                        } else if clear_hovered {
                            hover_surface
                        } else {
                            surface_alt
                        },
                        border_dark,
                        animated_shadow(soft_shadow, clear_hovered, clear_pressed),
                        surface,
                        8.0 * responsive_scale * handwriting_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ClearHandwriting,
                        rect: interaction_hit_rect(clear_rect),
                    });

                    let mut handwriting_action_layouts = Vec::new();
                    append_undo_icon_quads(
                        &mut quads,
                        centered_icon_rect(undo_visual_rect),
                        if undo_enabled {
                            text_primary
                        } else {
                            text_muted
                        },
                    );
                    append_trash_icon_quads(&mut quads, centered_icon_rect(clear_visual_rect), text_primary);

                    let mut chip_x = candidate_area_start;
                    let mut chip_index = 0usize;
                    for (index, candidate) in chrome
                        .handwriting_candidates
                        .iter()
                        .take(candidate_text_max)
                        .enumerate()
                    {
                        if chip_index >= visible_candidate_count {
                            break;
                        }

                        let max_chip_w = candidate_area_right - chip_x;
                        let chip_w = candidate_width(candidate, max_chip_w);
                        if chip_x + chip_w > candidate_area_right {
                            break;
                        }
                        let chip_y = handwriting_footer_rect[1];
                        let rect = [chip_x, chip_y, chip_w, handwriting_footer_row_h];
                        let kind = InteractionKind::UseHandwritingCandidate(index);
                        let (hovered, pressed) = interaction_state(kind);
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                press_surface
                            } else if index == 0 {
                                accent_soft
                            } else if hovered {
                                hover_surface
                            } else {
                                surface
                            },
                            if pressed {
                                accent
                            } else if index == 0 {
                                accent
                            } else if hovered {
                                hover_border
                            } else {
                                border_dark
                            },
                            animated_shadow(soft_shadow, hovered, pressed),
                            surface,
                            8.0 * responsive_scale * handwriting_scale,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind,
                            rect: interaction_hit_rect(rect),
                        });
                        let layout = TextBlock {
                            text: candidate.clone(),
                            origin: [
                                visual_rect[0] + 8.0 * responsive_scale * handwriting_scale,
                                visual_rect[1] + 2.8 * responsive_scale * handwriting_scale,
                            ],
                            max_width: (visual_rect[2] - 16.0 * responsive_scale * handwriting_scale).max(6.0),
                            pixel_size: (helper_px * 0.9).max(2.2 * responsive_scale),
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
                        .layout_in_rect(visual_rect, [8.0 * responsive_scale * handwriting_scale, 2.0 * responsive_scale * handwriting_scale]);
                        let mut final_text = candidate.clone();
                        if layout.truncated || layout.lines.len() > 1 {
                            handwriting_candidate_truncated.push(index);
                            if let Some(&(scroll_index, started_at)) = handwriting_candidate_scroll {
                                if scroll_index == index && layout.truncated {
                                    final_text = scroll_text_for_candidate(
                                        &final_text,
                                        started_at,
                                        (helper_px * 0.9).max(2.2 * responsive_scale),
                                        ui_tracking * handwriting_scale,
                                        (visual_rect[2] - 16.0 * responsive_scale * handwriting_scale)
                                            .max(6.0),
                                    );
                                }
                            }
                        }

                        let layout = TextBlock {
                            text: final_text,
                            origin: [
                                visual_rect[0] + 8.0 * responsive_scale * handwriting_scale,
                                visual_rect[1] + 2.8 * responsive_scale * handwriting_scale,
                            ],
                            max_width: (visual_rect[2] - 16.0 * responsive_scale * handwriting_scale)
                                .max(6.0),
                            pixel_size: (helper_px * 0.9).max(2.2 * responsive_scale),
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
                        .layout_in_rect(visual_rect, [8.0 * responsive_scale * handwriting_scale, 2.0 * responsive_scale * handwriting_scale]);
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        handwriting_action_layouts.push(layout);
                        chip_x += chip_w + handwriting_footer_gap_x;
                        chip_index += 1;
                    }
                    if !handwriting_action_layouts.is_empty() {
                        text_sections.push(TextSection {
                            role: TextRole::HandwritingButton,
                            layouts: handwriting_action_layouts,
                        });
                    }
}
