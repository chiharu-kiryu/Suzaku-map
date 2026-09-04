{
    if !chrome.settings_open || settings_panel_h <= 8.0 {
        settings_scroll_metadata = None;
    } else {
        let settings_rect = [
            panel_x,
            settings_y + 8.0,
            panel_width,
            (settings_panel_h - 8.0).max(0.0),
        ];
        if settings_rect[3] <= 0.0 {
            settings_scroll_metadata = None;
        } else {
            let search_bar_x = settings_rect[0] + 14.0 * responsive_scale;
            let search_bar_w = (settings_rect[2] - 72.0 * responsive_scale).max(140.0 * responsive_scale);
            let search_bar_h = 19.2 * responsive_scale;
            let search_bar_y = settings_rect[1] + 7.0 * responsive_scale;
            let search_clear_size = (18.0 * responsive_scale).max(10.0);
            let search_clear_x = search_bar_x + search_bar_w + 6.0 * responsive_scale;
            let clear_search_visible = !chrome.settings_search_query.is_empty();
            let search_bar_rect = [search_bar_x, search_bar_y, search_bar_w, search_bar_h];
            let clear_button_rect = [search_clear_x, search_bar_y, search_clear_size, search_bar_h];
            let (search_hovered, search_pressed) =
                interaction_state(InteractionKind::SettingsSearchInput);
            let (clear_hovered, clear_pressed) =
                interaction_state(InteractionKind::SettingsSearchClear);
            let search_bar_fill = if search_pressed {
                surface_muted
            } else if search_hovered || chrome.settings_search_focused {
                surface
            } else {
                surface_alt
            };
            let settings_divider = [
                (text_primary[0] * 0.84 + text_secondary[0] * 0.16),
                (text_primary[1] * 0.84 + text_secondary[1] * 0.16),
                (text_primary[2] * 0.84 + text_secondary[2] * 0.16),
                0.14,
            ];
            let settings_section_fill = [
                (surface[0] * 0.91 + surface_muted[0] * 0.09),
                (surface[1] * 0.91 + surface_muted[1] * 0.09),
                (surface[2] * 0.91 + surface_muted[2] * 0.09),
                0.24,
            ];
            let settings_section_border = [
                (surface[0] * 0.92 + border_dark[0] * 0.08),
                (surface[1] * 0.92 + border_dark[1] * 0.08),
                (surface[2] * 0.92 + border_dark[2] * 0.08),
                0.34,
            ];
            let section_gap_y = 4.8 * responsive_scale;
            let section_label_height = (chip_px * 1.5).max(21.0 * responsive_scale);
            let section_margin = 2.4 * responsive_scale;
            let row_height = chip_px * 1.6;
            let chip_gap_x = 5.8 * responsive_scale;
            let chip_gap_y = 4.8 * responsive_scale;
            let chip_radius = 8.4 * responsive_scale;
            let chip_start_x = search_bar_x + 68.0 * responsive_scale;
            let chip_max_x = settings_rect[0] + settings_rect[2] - 20.0 * responsive_scale;
            let label_col_x = search_bar_x;
            let label_max_width = (chip_start_x - label_col_x - 8.0).max(72.0);
            let sections: Vec<(&str, Vec<(InteractionKind, &'static str, bool)>)> = vec![
                (
                    "Text",
                    [
                        (
                            InteractionKind::SetTextScale(DisplayTextScale::Small),
                            "S",
                            chrome.text_scale == DisplayTextScale::Small,
                        ),
                        (
                            InteractionKind::SetTextScale(DisplayTextScale::Medium),
                            "M",
                            chrome.text_scale == DisplayTextScale::Medium,
                        ),
                        (
                            InteractionKind::SetTextScale(DisplayTextScale::Large),
                            "L",
                            chrome.text_scale == DisplayTextScale::Large,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Theme",
                    [
                        (
                            InteractionKind::SetThemePreset(ThemePreset::Daylight),
                            "Daylight",
                            chrome.theme_preset == ThemePreset::Daylight,
                        ),
                        (
                            InteractionKind::SetThemePreset(ThemePreset::DeviceDark),
                            "Device Dark",
                            chrome.theme_preset == ThemePreset::DeviceDark,
                        ),
                        (
                            InteractionKind::SetThemePreset(ThemePreset::Solarized),
                            "Solarized",
                            chrome.theme_preset == ThemePreset::Solarized,
                        ),
                        (
                            InteractionKind::SetThemePreset(ThemePreset::HighContrast),
                            "High Contrast",
                            chrome.theme_preset == ThemePreset::HighContrast,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Font",
                    [
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Auto),
                            "Auto",
                            chrome.font_face == FontFaceChoice::Auto,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Monaco),
                            "Monaco",
                            chrome.font_face == FontFaceChoice::Monaco,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Menlo),
                            "Menlo",
                            chrome.font_face == FontFaceChoice::Menlo,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Geneva),
                            "Geneva",
                            chrome.font_face == FontFaceChoice::Geneva,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Helvetica),
                            "Helvetica",
                            chrome.font_face == FontFaceChoice::Helvetica,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::PingFang),
                            "PingFang",
                            chrome.font_face == FontFaceChoice::PingFang,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::ArialUnicode),
                            "Arial",
                            chrome.font_face == FontFaceChoice::ArialUnicode,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Density",
                    [
                        (
                            InteractionKind::SetCandidateDensity(CandidateDensity::Compact),
                            "Compact",
                            chrome.candidate_density == CandidateDensity::Compact,
                        ),
                        (
                            InteractionKind::SetCandidateDensity(CandidateDensity::Cozy),
                            "Cozy",
                            chrome.candidate_density == CandidateDensity::Cozy,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Spacing",
                    [
                        (
                            InteractionKind::SetTextSpacing(TextSpacing::Tight),
                            "Tight",
                            chrome.text_spacing == TextSpacing::Tight,
                        ),
                        (
                            InteractionKind::SetTextSpacing(TextSpacing::Normal),
                            "Normal",
                            chrome.text_spacing == TextSpacing::Normal,
                        ),
                        (
                            InteractionKind::SetTextSpacing(TextSpacing::Relaxed),
                            "Relaxed",
                            chrome.text_spacing == TextSpacing::Relaxed,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Smooth",
                    [
                        (
                            InteractionKind::SetTextSmoothing(TextSmoothing::Sharp),
                            "Sharp",
                            chrome.text_smoothing == TextSmoothing::Sharp,
                        ),
                        (
                            InteractionKind::SetTextSmoothing(TextSmoothing::Smooth),
                            "Smooth",
                            chrome.text_smoothing == TextSmoothing::Smooth,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Preview",
                    [
                        (
                            InteractionKind::SetPreviewStyle(PreviewStyle::Compact),
                            "Trim",
                            chrome.preview_style == PreviewStyle::Compact,
                        ),
                        (
                            InteractionKind::SetPreviewStyle(PreviewStyle::Full),
                            "Full",
                            chrome.preview_style == PreviewStyle::Full,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Tap Slop",
                    [
                        (
                            InteractionKind::SetPointerTapSlopTenths(40),
                            "4px",
                            chrome.pointer_tap_slop_tenths == 40,
                        ),
                        (
                            InteractionKind::SetPointerTapSlopTenths(75),
                            "7.5px",
                            chrome.pointer_tap_slop_tenths == 75,
                        ),
                        (
                            InteractionKind::SetPointerTapSlopTenths(100),
                            "10px",
                            chrome.pointer_tap_slop_tenths == 100,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Tap Timeout",
                    [
                        (
                            InteractionKind::SetPointerTapMaxMs(180),
                            "180ms",
                            chrome.pointer_tap_max_ms == 180,
                        ),
                        (
                            InteractionKind::SetPointerTapMaxMs(260),
                            "260ms",
                            chrome.pointer_tap_max_ms == 260,
                        ),
                        (
                            InteractionKind::SetPointerTapMaxMs(320),
                            "320ms",
                            chrome.pointer_tap_max_ms == 320,
                        ),
                        (
                            InteractionKind::SetPointerTapMaxMs(420),
                            "420ms",
                            chrome.pointer_tap_max_ms == 420,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Target Slop",
                    [
                        (
                            InteractionKind::SetPointerTargetSlopTenths(20),
                            "2px",
                            chrome.pointer_target_slop_tenths == 20,
                        ),
                        (
                            InteractionKind::SetPointerTargetSlopTenths(35),
                            "3.5px",
                            chrome.pointer_target_slop_tenths == 35,
                        ),
                        (
                            InteractionKind::SetPointerTargetSlopTenths(50),
                            "5px",
                            chrome.pointer_target_slop_tenths == 50,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "Voice Auto",
                    [
                        (
                            InteractionKind::SetVoiceAutoInsert(true),
                            "On",
                            chrome.voice_auto_insert,
                        ),
                        (
                            InteractionKind::SetVoiceAutoInsert(false),
                            "Off",
                            !chrome.voice_auto_insert,
                        ),
                    ]
                    .to_vec(),
                ),
                (
                    "LLM",
                    [
                        (InteractionKind::SetLlmEnabled(true), "On", chrome.llm_enabled),
                        (InteractionKind::SetLlmEnabled(false), "Off", !chrome.llm_enabled),
                    ]
                    .to_vec(),
                ),
                (
                    "Model",
                    [(
                        InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                        "Llama3.2 3B (default)",
                        chrome.llm_model == LlmModelPreset::Llama32_3b,
                    )]
                    .to_vec(),
                ),
                (
                    "Tone",
                    [
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Focused),
                            "Focused",
                            chrome.llm_temperature == LlmTemperaturePreset::Focused,
                        ),
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Balanced),
                            "Balanced",
                            chrome.llm_temperature == LlmTemperaturePreset::Balanced,
                        ),
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Expressive),
                            "Expressive",
                            chrome.llm_temperature == LlmTemperaturePreset::Expressive,
                        ),
                    ]
                    .to_vec(),
                ),
            ];
            let estimated_chip_width = |label: &str| {
                (label.chars().count() as f32 * 10.0)
                    .max(58.0 * responsive_scale)
                    .min((chip_max_x - chip_start_x).max(132.0))
                    + 20.0 * responsive_scale
            };
            let estimate_chip_rows = |options: &[(InteractionKind, &str, bool)]| {
                let mut chip_x = chip_start_x;
                let mut rows = 1usize;
                for (_, chip_label, _) in options {
                    let chip_w = estimated_chip_width(chip_label);
                    if chip_x + chip_w > chip_max_x {
                        rows += 1;
                        chip_x = chip_start_x;
                    }
                    chip_x += chip_w + chip_gap_x;
                }
                rows
            };
            let search_query = chrome.settings_search_query.trim().to_ascii_lowercase();
            let is_searching = !search_query.is_empty();
            let mut visible_sections: Vec<(usize, &str, Vec<(InteractionKind, &str, bool)>, bool)> =
                Vec::new();
            for (index, (label, options)) in sections.iter().enumerate() {
                let mut filtered_options = Vec::new();
                let is_collapsed = chrome
                    .settings_collapsed_sections
                    .get(index)
                    .copied()
                    .unwrap_or(false);
                let mut maybe_match = false;
                for (kind, option_label, selected) in options {
                    let match_by_label =
                        label.to_ascii_lowercase().contains(&search_query);
                    let match_by_option =
                        option_label.to_ascii_lowercase().contains(&search_query);
                    if is_searching && (match_by_label || match_by_option) {
                        filtered_options.push((*kind, *option_label, *selected));
                        maybe_match = true;
                    } else if !is_searching {
                        filtered_options.push((*kind, *option_label, *selected));
                    }
                }
                if maybe_match || !is_searching || (!filtered_options.is_empty()) {
                    visible_sections.push((index, label, filtered_options, is_collapsed));
                }
            }

            let scroll_text_for_option = |text: &str,
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
            let mut estimated_height = 0.0;
            for (_, _, options, collapsed) in visible_sections.iter() {
                if !is_searching && *collapsed || options.is_empty() {
                    estimated_height += section_margin + section_label_height + section_gap_y;
                } else {
                    let section_rows = estimate_chip_rows(options.as_slice());
                    estimated_height += section_margin + section_label_height;
                    estimated_height += section_rows as f32 * row_height;
                    if section_rows > 1 {
                        estimated_height += (section_rows as f32 - 1.0) * chip_gap_y;
                    }
                    estimated_height += section_gap_y;
                }
            }
            let settings_content_top = search_bar_y + search_bar_h + 8.0 * responsive_scale;
            let settings_content_bottom = settings_rect[1] + settings_rect[3] - 4.0;
            let visible_content_height = (settings_content_bottom - settings_content_top).max(0.0);
            let max_scroll_offset = (estimated_height - visible_content_height).max(0.0);
            let settings_scroll_offset = chrome
                .settings_scroll_offset
                .max(0.0)
                .min(max_scroll_offset);
            let scroll_track_width = 5.8 * responsive_scale;
            let scroll_track_padding = 8.0 * responsive_scale;
            let settings_scroll_track_x = (settings_rect[0] + settings_rect[2] - scroll_track_width - scroll_track_padding)
                .max(settings_rect[0] + 2.0);
            let settings_scroll_track_top = settings_content_top;
            let settings_scroll_track_height =
                (settings_content_bottom - settings_scroll_track_top).max(0.0);
            let chip_max_x_with_scrollbar = (settings_rect[0] + settings_rect[2]
                - (scroll_track_width + scroll_track_padding * 2.0))
            .max(chip_start_x + 24.0 * responsive_scale);
            let chip_max_x = chip_max_x_with_scrollbar;
            let has_settings_scroll = max_scroll_offset > 0.0 && settings_scroll_track_height > 0.0;
            let settings_scroll_handle_height = if has_settings_scroll {
                let ratio = (visible_content_height / estimated_height.max(visible_content_height))
                    .clamp(0.12, 1.0);
                (settings_scroll_track_height * ratio)
                    .clamp(14.0 * responsive_scale, settings_scroll_track_height)
            } else {
                settings_scroll_track_height.min(16.0 * responsive_scale)
            };
            let settings_scroll_drag_range = (settings_scroll_track_height - settings_scroll_handle_height).max(0.0);
            let settings_scroll_handle_offset = if has_settings_scroll && settings_scroll_drag_range > 0.0 {
                (settings_scroll_offset / max_scroll_offset * settings_scroll_drag_range)
                    .clamp(0.0, settings_scroll_drag_range)
            } else {
                0.0
            };
            let settings_scroll_track_rect = [
                settings_scroll_track_x,
                settings_scroll_track_top,
                scroll_track_width,
                settings_scroll_track_height,
            ];
            let settings_scroll_handle_rect = [
                settings_scroll_track_x,
                settings_scroll_track_top + settings_scroll_handle_offset,
                scroll_track_width,
                settings_scroll_handle_height,
            ];
            settings_scroll_metadata = Some(SettingsScrollMetadata {
                track_rect: settings_scroll_track_rect,
                handle_rect: settings_scroll_handle_rect,
                content_height: estimated_height.max(0.0),
                visible_height: visible_content_height,
                max_scroll_offset,
                handle_drag_range: settings_scroll_drag_range,
            });
            let visible_in_settings = |top: f32, height: f32| {
                top + height > settings_content_top && top < settings_content_bottom
            };

            append_soft_card_quads(
                &mut quads,
                settings_rect,
                settings_section_fill,
                settings_section_border,
                [soft_shadow[0], soft_shadow[1], soft_shadow[2], soft_shadow[3] * 0.33],
                surface_muted,
                10.0,
            );
            quads.push(CandidateQuad {
                rect: [
                    settings_rect[0] + 8.0 * responsive_scale,
                    settings_rect[1] + 8.0 * responsive_scale,
                    settings_rect[2] - 16.0 * responsive_scale,
                    1.8 * responsive_scale,
                ],
                color: settings_divider,
            });
            append_soft_card_quads(
                &mut quads,
                search_bar_rect,
                search_bar_fill,
                if search_hovered || clear_hovered {
                    accent
                } else {
                    border_dark
                },
                soft_shadow,
                surface,
                8.0 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsSearchInput,
                rect: interaction_hit_rect(search_bar_rect),
            });
            let search_text = if chrome.settings_search_query.is_empty() {
                "Search settings".to_string()
            } else {
                chrome.settings_search_query.clone()
            };
            let search_layout = TextBlock {
                text: search_text,
                origin: [
                    search_bar_rect[0] + 7.2 * responsive_scale,
                    search_bar_rect[1] + 4.2 * responsive_scale,
                ],
                max_width: (search_bar_rect[2] - 14.0 * responsive_scale).max(44.0),
                pixel_size: title_px,
                letter_spacing: heading_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: if chrome.settings_search_query.is_empty() {
                    text_secondary
                } else {
                    text_primary
                },
                align: TextAlign::Left,
                role: TextRole::InputValue,
            }
            .layout();
            text_quads.extend(search_layout.quads.iter().copied());
            atlas_glyphs.extend(search_layout.atlas_glyphs.iter().cloned());
            text_sections.push(TextSection {
                role: TextRole::InputLabel,
                layouts: vec![search_layout.clone()],
            });
            if clear_search_visible {
                let clear_fill = if clear_pressed {
                    accent
                } else if clear_hovered {
                    surface
                } else {
                    surface_alt
                };
                append_soft_card_quads(
                    &mut quads,
                    clear_button_rect,
                    clear_fill,
                    if clear_pressed {
                        accent
                    } else if clear_hovered {
                        settings_divider
                    } else {
                        border_dark
                    },
                    soft_shadow,
                    surface,
                    7.6 * responsive_scale,
                );
                interactive_targets.push(InteractiveTarget {
                    kind: InteractionKind::SettingsSearchClear,
                    rect: interaction_hit_rect(clear_button_rect),
                });
                let clear_label = TextBlock {
                    text: "×".to_string(),
                    origin: [
                        clear_button_rect[0] + 4.2 * responsive_scale,
                        clear_button_rect[1] + 3.4 * responsive_scale,
                    ],
                    max_width: (clear_button_rect[2] - 2.0).max(1.0),
                    pixel_size: title_px,
                    letter_spacing: heading_tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Center,
                    role: TextRole::InputValue,
                }
                .layout();
                text_quads.extend(clear_label.quads.iter().copied());
                atlas_glyphs.extend(clear_label.atlas_glyphs.iter().cloned());
                text_sections.push(TextSection {
                    role: TextRole::InputLabel,
                    layouts: vec![clear_label.clone()],
                });
            }
            if has_settings_scroll && settings_scroll_track_height > 0.0 {
                let track_color = [
                    (text_primary[0] + text_secondary[0] * 0.7) / 1.7,
                    (text_primary[1] + text_secondary[1] * 0.7) / 1.7,
                    (text_primary[2] + text_secondary[2] * 0.7) / 1.7,
                    0.2,
                ];
                append_soft_card_quads(
                    &mut quads,
                    settings_scroll_track_rect,
                    track_color,
                    [
                        (text_secondary[0] * 0.2),
                        (text_secondary[1] * 0.2),
                        (text_secondary[2] * 0.2),
                        0.4,
                    ],
                    [0.0, 0.0, 0.0, 0.0],
                    surface,
                    scroll_track_width * 0.55,
                );
                let (scroll_hovered_track, _) = interaction_state(InteractionKind::SettingsScrollTrack);
                let (scroll_hovered_handle, _) = interaction_state(InteractionKind::SettingsScrollHandle);
                let handle_hovered = scroll_hovered_track || scroll_hovered_handle;
                let handle_color = if chrome.settings_open && handle_hovered {
                    [text_primary[0], text_primary[1], text_primary[2], 0.44]
                } else {
                    [text_primary[0], text_primary[1], text_primary[2], 0.34]
                };
                quads.push(CandidateQuad {
                    rect: settings_scroll_handle_rect,
                    color: handle_color,
                });
                interactive_targets.push(InteractiveTarget {
                    kind: InteractionKind::SettingsScrollTrack,
                    rect: interaction_hit_rect(settings_scroll_track_rect),
                });
                interactive_targets.push(InteractiveTarget {
                    kind: InteractionKind::SettingsScrollHandle,
                    rect: interaction_hit_rect(settings_scroll_handle_rect),
                });
            }

            let mut content_y = settings_content_top - settings_scroll_offset;
            let mut label_layouts = Vec::new();
            let mut option_layouts = Vec::new();
            if visible_sections.is_empty() {
                let empty_layout = TextBlock {
                    text: "No matching settings".to_string(),
                    origin: [
                        search_bar_x,
                        settings_content_top + 6.0 * responsive_scale,
                    ],
                    max_width: (settings_rect[2] - 36.0 * responsive_scale).max(120.0),
                    pixel_size: chip_px,
                    letter_spacing: ui_tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Left,
                    role: TextRole::SettingLabel,
                }
                .layout();
                text_sections.push(TextSection {
                    role: TextRole::SettingLabel,
                    layouts: vec![empty_layout.clone()],
                });
                text_quads.extend(empty_layout.quads.iter().copied());
                atlas_glyphs.extend(empty_layout.atlas_glyphs.iter().cloned());
            } else {
                for (_visible, label, options, is_collapsed) in visible_sections.iter() {
                    let section_index = sections
                        .iter()
                        .position(|(name, _)| *name == *label)
                        .unwrap_or(0);
                    let section_effectively_collapsed = is_searching || *is_collapsed;
                    let section_rows = if section_effectively_collapsed || options.is_empty() {
                        1.0
                    } else {
                        estimate_chip_rows(options.as_slice()) as f32
                    };
                    let section_height = section_margin + section_label_height
                        + if !section_effectively_collapsed && !options.is_empty() {
                            section_rows * row_height + (section_rows - 1.0) * chip_gap_y
                        } else {
                            0.0
                        }
                        + section_gap_y;
                    let section_top = content_y - 2.4 * responsive_scale;
                    let section_visible = visible_in_settings(section_top, section_height);
                    if section_visible {
                        append_soft_card_quads(
                            &mut quads,
                            [
                                settings_rect[0] + 8.0 * responsive_scale,
                                section_top,
                                settings_rect[2] - 16.0 * responsive_scale,
                                section_height,
                            ],
                            settings_section_fill,
                            settings_section_border,
                            [
                                soft_shadow[0],
                                soft_shadow[1],
                                soft_shadow[2],
                                soft_shadow[3] * 0.55,
                            ],
                            surface_muted,
                            9.8 * responsive_scale,
                        );
                        quads.push(CandidateQuad {
                            rect: [
                                settings_rect[0] + 20.0 * responsive_scale,
                                section_top + section_height - 1.0,
                                settings_rect[2] - 40.0 * responsive_scale,
                                1.0,
                            ],
                            color: settings_divider,
                        });
                    }

                    let toggle_layout = TextBlock {
                        text: if section_effectively_collapsed { "▸" } else { "▾" }.to_string(),
                        origin: [
                            settings_rect[0] + settings_rect[2] - 24.0 * responsive_scale,
                            section_top + 6.2 * responsive_scale,
                        ],
                        max_width: 10.0 * responsive_scale,
                        pixel_size: label_px,
                        letter_spacing: heading_tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: text_secondary,
                        align: TextAlign::Center,
                        role: TextRole::SettingOption,
                    }
                    .layout();
                    if section_visible {
                        text_quads.extend(toggle_layout.quads.iter().copied());
                        atlas_glyphs.extend(toggle_layout.atlas_glyphs.iter().cloned());
                    }
                    if section_visible && section_effectively_collapsed {
                        text_sections.push(TextSection {
                            role: TextRole::SettingOption,
                            layouts: vec![toggle_layout.clone()],
                        });
                    } else if section_visible {
                        text_sections.push(TextSection {
                            role: TextRole::SettingOption,
                            layouts: vec![toggle_layout.clone()],
                        });
                    }
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ToggleSettingsSection(section_index),
                        rect: interaction_hit_rect([
                            settings_rect[0] + 8.0 * responsive_scale,
                            section_top,
                            settings_rect[2] - 16.0 * responsive_scale,
                            section_label_height,
                        ]),
                    });

                    if section_visible {
                        let label_layout = TextBlock {
                            text: (*label).to_string(),
                            origin: [label_col_x, content_y + 4.8 * responsive_scale],
                            max_width: label_max_width,
                            pixel_size: section_label_height * 0.63,
                            letter_spacing: heading_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::SettingLabel,
                        }
                        .layout();
                        text_quads.extend(label_layout.quads.iter().copied());
                        atlas_glyphs.extend(label_layout.atlas_glyphs.iter().cloned());
                        label_layouts.push(label_layout);
                    }

                    let mut chip_x = chip_start_x;
                    let mut chip_y = content_y;
                    if !section_effectively_collapsed {
                        for (kind, option_label, selected) in options {
                            let (hovered, pressed) = interaction_state(*kind);
                            let available_chip_w = (chip_max_x - chip_x).max(72.0 * responsive_scale);
                            let chip_w = estimated_chip_width(option_label).min(available_chip_w);
                            if chip_x > chip_start_x && chip_x + chip_w > chip_max_x {
                                chip_x = chip_start_x;
                                chip_y += row_height + chip_gap_y;
                            }
                            let rect = [chip_x, chip_y, chip_w, row_height];
                            let is_visible = visible_in_settings(chip_y, row_height);
                            let visual_rect = animated_rect(rect, hovered, pressed);
                            if is_visible {
                                let mut option_text = (*option_label).to_string();
                                let base_layout = TextBlock {
                                    text: option_text.clone(),
                                    origin: [
                                        visual_rect[0] + 10.0 * responsive_scale,
                                        visual_rect[1] + 6.2 * responsive_scale,
                                    ],
                                    max_width: (visual_rect[2] - 20.0 * responsive_scale).max(14.0),
                                    pixel_size: chip_px,
                                    letter_spacing: ui_tracking,
                                    line_gap: base_line_gap,
                                    max_lines: 1,
                                    color: if *selected { accent_text } else { text_primary },
                                    align: TextAlign::Center,
                                    role: TextRole::SettingOption,
                                }
                                .layout();
                                if base_layout.truncated || base_layout.lines.len() > 1 {
                                    settings_option_truncated.push(*kind);
                                }
                                if let Some((scroll_kind, started_at)) = settings_option_text_scroll {
                                    if *scroll_kind == *kind && base_layout.truncated {
                                        option_text = scroll_text_for_option(
                                            &option_text,
                                            *started_at,
                                            chip_px,
                                            ui_tracking,
                                            (visual_rect[2] - 20.0 * responsive_scale).max(14.0),
                                        );
                                    }
                                }
                                append_soft_card_quads(
                                    &mut quads,
                                    visual_rect,
                                    if pressed {
                                        accent
                                    } else if *selected {
                                        accent_soft
                                    } else if hovered {
                                        surface
                                    } else {
                                        surface_alt
                                    },
                                    if pressed {
                                        accent
                                    } else if *selected {
                                        accent
                                    } else if hovered {
                                        settings_divider
                                    } else {
                                        border_dark
                                    },
                                    animated_shadow(soft_shadow, hovered, pressed),
                                    shell,
                                    chip_radius,
                                );
                                interactive_targets.push(InteractiveTarget {
                                    kind: *kind,
                                    rect: interaction_hit_rect(rect),
                                });
                                let final_layout = TextBlock {
                                    text: option_text,
                                    origin: [
                                        visual_rect[0] + 10.0 * responsive_scale,
                                        visual_rect[1] + 6.2 * responsive_scale,
                                    ],
                                    max_width: (visual_rect[2] - 20.0 * responsive_scale).max(14.0),
                                    pixel_size: chip_px,
                                    letter_spacing: ui_tracking,
                                    line_gap: base_line_gap,
                                    max_lines: 1,
                                    color: if *selected { accent_text } else { text_primary },
                                    align: TextAlign::Center,
                                    role: TextRole::SettingOption,
                                }
                                .layout();
                                text_quads.extend(final_layout.quads.iter().copied());
                                atlas_glyphs.extend(final_layout.atlas_glyphs.iter().cloned());
                                option_layouts.push(final_layout);
                            }
                            interactive_targets.push(InteractiveTarget {
                                kind: *kind,
                                rect: interaction_hit_rect(rect),
                            });
                            chip_x += chip_w + chip_gap_x;
                        }
                    }

                    content_y += section_height + section_gap_y;
                }
            }
            if !label_layouts.is_empty() {
                text_sections.push(TextSection {
                    role: TextRole::SettingLabel,
                    layouts: label_layouts,
                });
            }
            if !option_layouts.is_empty() {
                text_sections.push(TextSection {
                    role: TextRole::SettingOption,
                    layouts: option_layouts,
                });
            }
        }
    }
}
