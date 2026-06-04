{
                    let voice_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let voice_rect = [
                        drawer_rect[0],
                        voice_y,
                        drawer_rect[2],
                        drawer_rect[3] - 24.0 * responsive_scale,
                    ];
                    let voice_footer_rows = if panel_width < 760.0 { 2 } else { 1 };
                    let voice_footer_height =
                        (voice_footer_rows as f32 * 26.0 + 10.0) * responsive_scale;
                    let voice_footer_rect = [
                        drawer_rect[0] + 10.0 * responsive_scale,
                        voice_rect[1] + voice_rect[3]
                            - voice_footer_height
                            - 8.0 * responsive_scale,
                        drawer_rect[2] - 20.0 * responsive_scale,
                        voice_footer_height,
                    ];
                    let voice_content_x = voice_rect[0] + 12.0 * responsive_scale;
                    let voice_header_y = voice_y + 12.0 * responsive_scale;
                    let voice_transcript_y = voice_y + 38.0 * responsive_scale;
                    let voice_transcript_bottom =
                        voice_footer_rect[1] - 14.0 * responsive_scale;
                    let voice_transcript_room = voice_transcript_bottom - voice_transcript_y;
                    let voice_transcript_max_lines = if voice_transcript_room
                        >= 56.0 * responsive_scale
                    {
                        3
                    } else if voice_transcript_room >= 34.0 * responsive_scale {
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
                        10.0 * responsive_scale,
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
                    if !live_hint.is_empty() {
                        let hint_rect = [
                            drawer_rect[0] + drawer_rect[2] - 128.0 * responsive_scale,
                            voice_y + 8.0 * responsive_scale,
                            116.0 * responsive_scale,
                            18.0 * responsive_scale,
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
                            7.0 * responsive_scale,
                        );
                    }
                    if chrome.voice_state == VoiceCaptureState::Listening {
                        let base_x =
                            drawer_rect[0] + drawer_rect[2] - 128.0 * responsive_scale;
                        let base_y = voice_y + 30.0 * responsive_scale;
                        let phase = chrome.voice_visual_phase as f32;
                        for index in 0..4 {
                            let pulse = ((phase + index as f32 * 3.0) % 12.0) / 12.0;
                            let mirrored = if pulse > 0.5 { 1.0 - pulse } else { pulse };
                            let bar_h = 8.0 + mirrored * 18.0;
                            quads.push(CandidateQuad {
                                rect: [
                                    base_x + index as f32 * 12.0,
                                    base_y + (22.0 - bar_h),
                                    7.0,
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
                                    28.0 * responsive_scale
                                } else {
                                    176.0 * responsive_scale
                                },
                            pixel_size: title_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
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
                                drawer_rect[0] + drawer_rect[2] - 122.0 * responsive_scale,
                                voice_header_y + 1.0 * responsive_scale,
                            ],
                            max_width: 104.0 * responsive_scale,
                            pixel_size: helper_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 2,
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
                            max_width: drawer_rect[2] - 24.0 * responsive_scale,
                            pixel_size: if chrome.voice_transcript.is_empty() {
                                input_value_px * 0.58
                            } else {
                                input_value_px * 0.74
                            },
                            letter_spacing: if chrome.voice_transcript.is_empty() {
                                ui_tracking
                            } else {
                                heading_tracking
                            },
                            line_gap: base_line_gap,
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
                            78.0,
                            chrome.voice_state == VoiceCaptureState::Listening,
                            chrome.voice_permission != VoicePermissionState::Denied
                                && chrome.voice_permission != VoicePermissionState::Error,
                        ),
                        (
                            InteractionKind::InsertVoiceTranscript,
                            "Seed",
                            88.0,
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
                            84.0,
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
                            86.0,
                            false,
                            true,
                        ));
                        voice_actions.push((
                            InteractionKind::RefreshVoicePermissions,
                            "Sync",
                            72.0,
                            false,
                            true,
                        ));
                    }
                    if !chrome.voice_transcript.is_empty() {
                        voice_actions.push((
                            InteractionKind::ClearVoiceTranscript,
                            "Clear",
                            72.0,
                            false,
                            true,
                        ));
                    }
                    append_soft_card_quads(
                        &mut quads,
                        voice_footer_rect,
                        surface,
                        border_dark,
                        [0.25, 0.34, 0.48, 0.06],
                        shell,
                        8.0 * responsive_scale,
                    );
                    let mut voice_action_layouts = Vec::new();
                    let mut action_cursor_x = voice_footer_rect[0] + 6.0 * responsive_scale;
                    let mut action_row = 0;
                    for (kind, label, width, emphasized, enabled) in voice_actions {
                        let (hovered, pressed) = interaction_state(kind);
                        if action_cursor_x > voice_footer_rect[0] + 6.0 * responsive_scale
                            && action_cursor_x + width
                                > voice_footer_rect[0] + voice_footer_rect[2]
                        {
                            action_row += 1;
                            action_cursor_x = voice_footer_rect[0] + 6.0 * responsive_scale;
                        }
                        let rect = [
                            action_cursor_x,
                            voice_footer_rect[1]
                                + 4.0 * responsive_scale
                                + action_row as f32 * 24.0 * responsive_scale,
                            width.min(voice_footer_rect[2] - 12.0 * responsive_scale),
                            20.0,
                        ];
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        action_cursor_x += width + 8.0 * responsive_scale;
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
                            8.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind, rect });
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
                                    origin: [visual_rect[0] + 8.0, visual_rect[1] + 6.0],
                                    max_width: visual_rect[2] - 16.0,
                                    pixel_size: 2.0,
                                    letter_spacing: ui_tracking,
                                    line_gap: base_line_gap,
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
