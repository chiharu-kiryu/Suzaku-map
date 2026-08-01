{
                    let voice_scale = (drawer_rect[3] / (230.0 * responsive_scale)).clamp(0.76, 1.0);
                    let voice_button_label_px = (helper_px * 0.84).max(1.85 * responsive_scale) * voice_scale;
                    let voice_content_x_pad = 8.6 * responsive_scale * voice_scale;
                    let voice_y = drawer_rect[1] + 10.5 * responsive_scale * voice_scale;
                    let voice_rect = [
                        drawer_rect[0],
                        voice_y,
                        drawer_rect[2],
                        (drawer_rect[3] - 16.0 * responsive_scale).max(142.0 * responsive_scale * voice_scale),
                    ];
                    let voice_content_x = voice_rect[0] + voice_content_x_pad;
                    let voice_header_y = voice_y + 8.0 * responsive_scale * voice_scale;
                    let voice_transcript_y = voice_y + 28.0 * responsive_scale * voice_scale;
                    let voice_transcript_bottom = (voice_rect[1] + voice_rect[3])
                        - (20.0 * responsive_scale * voice_scale);
                    let voice_transcript_room = voice_transcript_bottom - voice_transcript_y;
                    let voice_transcript_max_lines = if voice_transcript_room
                        >= 54.0 * responsive_scale * voice_scale
                    {
                        3
                    } else if voice_transcript_room >= 32.0 * responsive_scale * voice_scale {
                        2
                    } else {
                        1
                    };
                    append_soft_card_quads(
                        &mut quads,
                        voice_rect,
                        [0.97, 0.98, 1.0, 1.0],
                        border_dark,
                        soft_shadow,
                        surface,
                        8.8 * responsive_scale * voice_scale,
                    );

                    let transcript = if chrome.voice_transcript.is_empty() {
                        voice_transcript_placeholder(
                            chrome.voice_permission,
                            &chrome.voice_backend_label,
                            chrome.voice_supports_live_capture,
                        )
                    } else {
                        chrome.voice_transcript.clone()
                    };
                    let status_text = voice_status_text(
                        chrome.voice_state,
                        chrome.voice_permission,
                        !chrome.voice_transcript.is_empty(),
                        &chrome.voice_backend_label,
                    );
                    let live_hint = if chrome.voice_state == VoiceCaptureState::Listening {
                        if chrome.voice_transcript.is_empty() {
                            "Listening now..."
                        } else {
                            "Heard just now"
                        }
                    } else if !chrome.voice_transcript.is_empty() {
                        "Ready to insert"
                    } else {
                        ""
                    };
                    let live_hint_x = if live_hint.is_empty() {
                        drawer_rect[0] + drawer_rect[2] - 120.0 * responsive_scale * voice_scale
                    } else {
                        drawer_rect[0] + drawer_rect[2] - 116.0 * responsive_scale * voice_scale
                    };
                    if !live_hint.is_empty() {
                        let hint_rect = [
                            live_hint_x,
                            voice_y + 7.0 * responsive_scale * voice_scale,
                            112.0 * responsive_scale * voice_scale,
                            17.0 * responsive_scale * voice_scale,
                        ];
                        append_soft_card_quads(
                            &mut quads,
                            hint_rect,
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_listening_fill
                            } else {
                                voice_hint_fill
                            },
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_listening_border
                            } else {
                                voice_hint_border
                            },
                            [0.30, 0.40, 0.55, 0.10],
                            [0.97, 0.98, 1.0, 1.0],
                            7.0 * responsive_scale * voice_scale,
                        );
                    }
                    if chrome.voice_state == VoiceCaptureState::Listening {
                        let base_x =
                            drawer_rect[0] + drawer_rect[2] - 128.0 * responsive_scale * voice_scale;
                        let base_y = voice_y + 30.0 * responsive_scale * voice_scale;
                        let phase = chrome.voice_visual_phase as f32;
                        for index in 0..4 {
                            let pulse = ((phase + index as f32 * 3.0) % 12.0) / 12.0;
                            let mirrored = if pulse > 0.5 { 1.0 - pulse } else { pulse };
                            let bar_h = (8.0 + mirrored * 18.0) * voice_scale;
                            quads.push(CandidateQuad {
                                rect: [
                                    base_x + index as f32 * 12.0 * responsive_scale * voice_scale,
                                    base_y + (22.0 * responsive_scale * voice_scale - bar_h),
                                    7.0 * responsive_scale * voice_scale,
                                    bar_h,
                                ],
                                color: voice_visual_bar,
                            });
                        }
                    }
                    let voice_layouts = vec![
                        TextBlock {
                            text: "Voice input".to_string(),
                            origin: [voice_content_x, voice_header_y],
                            max_width: drawer_rect[2]
                                - if live_hint.is_empty() {
                                    26.0 * responsive_scale * voice_scale
                                } else {
                                    138.0 * responsive_scale * voice_scale
                                },
                            pixel_size: title_px * voice_scale,
                            letter_spacing: ui_tracking * voice_scale,
                            line_gap: base_line_gap * voice_scale,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                accent
                            } else if matches!(
                                chrome.voice_permission,
                                VoicePermissionState::Denied | VoicePermissionState::Error
                            ) {
                                voice_error_text
                            } else {
                                text_secondary
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: if live_hint.is_empty() {
                                status_text
                            } else {
                                live_hint.to_string()
                            },
                            origin: [
                                live_hint_x,
                                voice_header_y + 1.0 * responsive_scale * voice_scale,
                            ],
                            max_width: 110.0 * responsive_scale * voice_scale,
                            pixel_size: helper_px * voice_scale,
                            letter_spacing: ui_tracking * voice_scale,
                            line_gap: base_line_gap * voice_scale,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_success_text
                            } else if matches!(
                                chrome.voice_permission,
                                VoicePermissionState::Denied | VoicePermissionState::Error
                            ) {
                                voice_error_text
                            } else {
                                text_primary
                            },
                            align: TextAlign::Center,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: transcript,
                            origin: [voice_content_x, voice_transcript_y],
                            max_width: (drawer_rect[2] - 24.0 * responsive_scale * voice_scale).max(0.0),
                            pixel_size: if chrome.voice_transcript.is_empty() {
                                input_value_px * 0.58 * voice_scale
                            } else {
                                input_value_px * 0.74 * voice_scale
                            },
                            letter_spacing: if chrome.voice_transcript.is_empty() {
                                ui_tracking * voice_scale
                            } else {
                                heading_tracking * voice_scale
                            },
                            line_gap: base_line_gap * voice_scale,
                            max_lines: voice_transcript_max_lines,
                            color: if chrome.voice_transcript.is_empty() {
                                text_muted
                            } else {
                                text_primary
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceTranscript,
                        }
                        .layout(),
                    ];
                    for layout in &voice_layouts {
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    }
                    text_sections.push(TextSection {
                        role: TextRole::VoiceTranscript,
                        layouts: voice_layouts,
                    });

                    let mut voice_actions = vec![
                        (
                            InteractionKind::ToggleVoiceCapture,
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                "Stop"
                            } else if chrome.voice_permission == VoicePermissionState::Denied {
                                "Denied"
                            } else {
                                "Mic"
                            },
                            (76.0 * voice_scale).max(58.0 * responsive_scale * voice_scale),
                            chrome.voice_state == VoiceCaptureState::Listening,
                            chrome.voice_permission != VoicePermissionState::Denied
                                && chrome.voice_permission != VoicePermissionState::Error,
                        ),
                        (
                            InteractionKind::InsertVoiceTranscript,
                            "Seed",
                            (86.0 * voice_scale).max(60.0 * responsive_scale * voice_scale),
                            !chrome.voice_transcript.is_empty()
                                && chrome.voice_state != VoiceCaptureState::Listening,
                            !chrome.voice_transcript.is_empty()
                                && chrome.voice_state != VoiceCaptureState::Listening,
                        ),
                    ];
                    if !chrome.voice_supports_live_capture
                        || chrome.voice_permission == VoicePermissionState::Unavailable
                    {
                        voice_actions.push((
                            InteractionKind::CycleVoiceSample,
                            "Next",
                            (82.0 * voice_scale).max(60.0 * responsive_scale * voice_scale),
                            false,
                            true,
                        ));
                    }
                    if matches!(
                        chrome.voice_permission,
                        VoicePermissionState::Pending | VoicePermissionState::Denied
                    ) {
                        voice_actions.push((
                            InteractionKind::OpenVoiceSettings,
                            "Prefs",
                            (84.0 * voice_scale).max(60.0 * responsive_scale * voice_scale),
                            false,
                            true,
                        ));
                        voice_actions.push((
                            InteractionKind::RefreshVoicePermissions,
                            "Sync",
                            (70.0 * voice_scale).max(58.0 * responsive_scale * voice_scale),
                            false,
                            true,
                        ));
                    }
                    if !chrome.voice_transcript.is_empty() {
                        voice_actions.push((
                            InteractionKind::ClearVoiceTranscript,
                            "Clear",
                            (70.0 * voice_scale).max(58.0 * responsive_scale * voice_scale),
                            false,
                            true,
                        ));
                    }

                    let action_row_h = 20.6 * responsive_scale * voice_scale;
                    let action_row_gap_x = 5.0 * responsive_scale * voice_scale;
                    let action_row_gap_y = 5.0 * responsive_scale * voice_scale;
                    let action_start_x = drawer_rect[0] + 8.0 * responsive_scale * voice_scale;
                    let action_end_x = drawer_rect[0] + drawer_rect[2] - 8.0 * responsive_scale * voice_scale;
                    let action_row_width = (action_end_x - action_start_x).max(0.0);
                    let mut action_rows = 1usize;
                    let mut action_cursor_x = action_start_x;

                    for (_, _, width, _, _) in &voice_actions {
                        let next_width = width.min(action_row_width);
                        if action_cursor_x > action_start_x
                            && action_cursor_x + next_width + action_row_gap_x > action_end_x
                        {
                            action_rows += 1;
                            action_cursor_x = action_start_x;
                        }
                        action_cursor_x = (action_cursor_x + next_width + action_row_gap_x).max(action_end_x);
                    }
                    let voice_footer_rows = action_rows.max(1);
                    let voice_footer_height =
                        (action_row_h * voice_footer_rows as f32
                            + (action_rows.saturating_sub(1) as f32) * action_row_gap_y
                            + 8.0 * responsive_scale * voice_scale)
                        .max(30.0 * responsive_scale * voice_scale);
                    let voice_footer_rect = [
                        action_start_x,
                        voice_rect[1] + voice_rect[3]
                            - voice_footer_height
                            - 8.0 * responsive_scale * voice_scale,
                            drawer_rect[2] - 16.0 * responsive_scale * voice_scale,
                        voice_footer_height,
                    ];
                    append_soft_card_quads(
                        &mut quads,
                        voice_footer_rect,
                        surface,
                        border_dark,
                        [0.25, 0.34, 0.48, 0.06],
                        shell,
                        8.0 * responsive_scale * voice_scale,
                    );
                    let mut voice_action_layouts = Vec::new();
                    let mut action_cursor_x = voice_footer_rect[0] + 5.0 * responsive_scale * voice_scale;
                    let mut action_row = 0;
                    for (kind, label, width, emphasized, enabled) in voice_actions {
                        let (hovered, pressed) = interaction_state(kind);
                        let action_row_start = voice_footer_rect[0] + 5.0 * responsive_scale * voice_scale;
                        let row_right = action_row_start + action_row_width;
                        let next_width = width.min(action_row_width);

                        if action_cursor_x > action_row_start
                            && action_cursor_x + next_width + action_row_gap_x > row_right
                        {
                            action_row += 1;
                            if action_row >= voice_footer_rows {
                                break;
                            }
                            action_cursor_x = action_row_start;
                        }
                        let rect = [
                            action_cursor_x,
                            voice_footer_rect[1]
                                + 3.6 * responsive_scale * voice_scale
                                + action_row as f32
                                    * (action_row_h + action_row_gap_y),
                            next_width,
                            action_row_h,
                        ];
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        action_cursor_x += next_width + 6.4 * responsive_scale * voice_scale;
                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                [0.67, 0.82, 0.97, 1.0]
                            } else if emphasized {
                                accent_soft
                            } else if hovered {
                                [0.93, 0.96, 1.0, 1.0]
                            } else if !enabled {
                                [0.97, 0.98, 1.0, 1.0]
                            } else {
                                [0.92, 0.95, 0.99, 1.0]
                            },
                            if pressed {
                                [0.08, 0.35, 0.68, 1.0]
                            } else if emphasized {
                                accent
                            } else if hovered {
                                [0.46, 0.62, 0.82, 1.0]
                            } else {
                                border_dark
                            },
                            animated_shadow(soft_shadow, hovered, pressed),
                            surface,
                            8.0 * responsive_scale * voice_scale,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind,
                            rect: interaction_hit_rect(rect),
                        });
                        let icon_color = if emphasized {
                            accent_text
                        } else if !enabled {
                            text_muted
                        } else {
                            text_primary
                        };
                        match kind {
                            InteractionKind::ToggleVoiceCapture => {
                                append_mic_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::InsertVoiceTranscript => {
                                append_seed_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::CycleVoiceSample => {
                                append_next_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::OpenVoiceSettings => append_gear_icon_quads(
                                &mut quads,
                                visual_rect,
                                icon_color,
                                [0.0, 0.0, 0.0, 0.0],
                            ),
                            InteractionKind::RefreshVoicePermissions => {
                                append_refresh_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::ClearVoiceTranscript => {
                                append_trash_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                        _ => {
                            let layout = TextBlock {
                                text: label.to_string(),
                                    origin: [
                                visual_rect[0] + 7.2 * responsive_scale * voice_scale,
                                visual_rect[1] + 5.8 * responsive_scale * voice_scale,
                            ],
                                    max_width: (visual_rect[2] - 15.0 * responsive_scale * voice_scale).max(0.0),
                                    pixel_size: voice_button_label_px,
                                    letter_spacing: ui_tracking * voice_scale,
                                    line_gap: base_line_gap * voice_scale,
                                    max_lines: 1,
                                    color: icon_color,
                                    align: TextAlign::Center,
                                    role: TextRole::VoiceButton,
                                }
                                .layout();
                                text_quads.extend(layout.quads.iter().copied());
                                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                                voice_action_layouts.push(layout);
                            }
                        }
                    }
                    text_sections.push(TextSection {
                        role: TextRole::VoiceButton,
                        layouts: voice_action_layouts,
                    });

}
