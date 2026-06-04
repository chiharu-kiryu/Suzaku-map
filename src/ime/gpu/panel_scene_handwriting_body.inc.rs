                    let handwriting_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let handwriting_title_y = handwriting_y + 6.0 * responsive_scale;
                    let handwriting_hint_y = handwriting_y + 28.0 * responsive_scale;
                    let handwriting_footer_rect = [
                        drawer_rect[0] + 10.0 * responsive_scale,
                        drawer_rect[1] + drawer_rect[3] - 34.0 * responsive_scale,
                        drawer_rect[2] - 20.0 * responsive_scale,
                        26.0 * responsive_scale,
                    ];
                    let canvas_top = handwriting_y + 56.0 * responsive_scale;
                    let canvas_rect = [
                        drawer_rect[0],
                        canvas_top,
                        drawer_rect[2],
                        (handwriting_footer_rect[1] - 10.0 * responsive_scale - canvas_top)
                            .max(56.0 * responsive_scale),
                    ];
                    append_soft_card_quads(
                        &mut quads,
                        canvas_rect,
                        [0.98, 0.99, 1.0, 1.0],
                        border_dark,
                        soft_shadow,
                        surface,
                        10.0 * responsive_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::HandwritingCanvas,
                            rect: canvas_rect,
                        });

                    for stroke in &chrome.handwriting_strokes {
                        for point in sample_stroke_points(stroke) {
                            quads.push(CandidateQuad {
                                rect: [point[0] - 2.5, point[1] - 2.5, 5.0, 5.0],
                                color: accent,
                            });
                        }
                    }

                    let handwriting_layouts = vec![
                        TextBlock {
                            text: "Trace handwriting".to_string(),
                            origin: [drawer_rect[0] + 14.0, handwriting_title_y],
                            max_width: drawer_rect[2] - 28.0,
                            pixel_size: title_px,
                            letter_spacing: heading_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: chrome.handwriting_hint.clone(),
                            origin: [drawer_rect[0] + 14.0, handwriting_hint_y],
                            max_width: drawer_rect[2] - 28.0,
                            pixel_size: helper_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
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
                        8.0 * responsive_scale,
                    );
                    let undo_rect = [
                        handwriting_footer_rect[0] + 6.0 * responsive_scale,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale,
                        68.0,
                        20.0,
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
                        8.0,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::UndoHandwritingStroke,
                        rect: undo_rect,
                    });
                    let clear_rect = [
                        handwriting_footer_rect[0] + 82.0 * responsive_scale,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale,
                        68.0,
                        20.0,
                    ];
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
                        8.0,
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

                    let mut chip_x = handwriting_footer_rect[0] + 158.0 * responsive_scale;
                    for (index, candidate) in
                        chrome.handwriting_candidates.iter().take(3).enumerate()
                    {
                        let kind = InteractionKind::UseHandwritingCandidate(index);
                        let (hovered, pressed) = interaction_state(kind);
                        let chip_w = (candidate.chars().count() as f32 * 9.5).max(52.0) + 16.0;
                        let rect = [
                            chip_x,
                            handwriting_footer_rect[1] + 2.0 * responsive_scale,
                            chip_w,
                            20.0,
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
                            8.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind, rect });
                        let layout = TextBlock {
                            text: candidate.clone(),
                            origin: [visual_rect[0] + 8.0, visual_rect[1] + 6.0],
                            max_width: visual_rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
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
                        chip_x += chip_w + 10.0;
                    }
                    text_sections.push(TextSection {
                        role: TextRole::HandwritingButton,
                        layouts: handwriting_action_layouts,
                    });
