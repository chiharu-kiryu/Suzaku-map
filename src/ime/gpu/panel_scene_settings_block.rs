{
            if chrome.settings_open && settings_panel_h >= 8.0 {
                let effective_settings_h = settings_panel_h;
                let settings_rect = [
                    panel_x,
                    settings_y + 8.0,
                    panel_width,
                    effective_settings_h - 8.0,
                ];
                quads.push(CandidateQuad {
                    rect: settings_rect,
                    color: surface,
                });
                let mut settings_layouts = Vec::new();
                let mut option_layouts = Vec::new();

                let sections = [
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
                            (
                                InteractionKind::SetLlmEnabled(true),
                                "On",
                                chrome.llm_enabled,
                            ),
                            (
                                InteractionKind::SetLlmEnabled(false),
                                "Off",
                                !chrome.llm_enabled,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Model",
                        [(
                            InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                            "Llama3.2 3B",
                            chrome.llm_model == LlmModelPreset::Llama32_3b,
                        )]
                        .to_vec(),
                    ),
                    (
                        "Heat",
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
                                InteractionKind::SetLlmTemperature(
                                    LlmTemperaturePreset::Expressive,
                                ),
                                "Expressive",
                                chrome.llm_temperature == LlmTemperaturePreset::Expressive,
                            ),
                        ]
                        .to_vec(),
                    ),
                ];

                let row_gap = 5.4 * responsive_scale;
                let chip_height = 22.2 * responsive_scale;
                let chip_corner_radius = 9.0 * responsive_scale;
                let section_card_radius = 9.4 * responsive_scale;
                let chip_x_gap = 6.8 * responsive_scale;
                let label_y_pad = 3.0 * responsive_scale;
                let label_row_h = 21.8 * responsive_scale;
                let section_row_step = chip_height + row_gap;
                let settings_content_bottom = settings_rect[1] + settings_rect[3] - 2.0;
                let mut row_y = settings_rect[1] + 11.0 * responsive_scale;
                let chip_start_x = panel_x + 84.0 * responsive_scale;
                let chip_max_x = settings_rect[0] + settings_rect[2] - 10.0;
                let available_chip_width = (chip_max_x - chip_start_x).max(24.0 * responsive_scale);
                let section_card_fill = [
                    (surface[0] + (1.0 - surface[0]) * 0.04).min(1.0),
                    (surface[1] + (1.0 - surface[1]) * 0.03).min(1.0),
                    (surface[2] + (1.0 - surface[2]) * 0.01).min(1.0),
                    0.30,
                ];
                let section_card_border = [
                        (surface[0] * 0.92 + surface_muted[0] * 0.08),
                        (surface[1] * 0.92 + surface_muted[1] * 0.08),
                        (surface[2] * 0.92 + surface_muted[2] * 0.08),
                        0.30,
                    ];
                'section_loop: for (label, options) in sections.iter() {
                    if row_y + label_row_h > settings_content_bottom {
                        break 'section_loop;
                    }

                    let label_layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [panel_x + 14.0 * responsive_scale, row_y + label_y_pad],
                    max_width: (72.0 * responsive_scale).max(78.0),
                        pixel_size: title_px,
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
                    settings_layouts.push(label_layout);

                    let mut chip_x = chip_start_x;
                    let mut chip_y = row_y;
                    let mut row_bottom = chip_y + chip_height;
                    let mut section_has_options = false;
                    let section_row_start = row_y;

                    'chip_loop: for (kind, chip_label, selected) in options {
                        if chip_x >= chip_max_x - 2.0 * responsive_scale {
                            break 'chip_loop;
                        }

                        let chip_w = (chip_label.chars().count() as f32 * 10.0)
                            .max(32.0 * responsive_scale)
                            .min(available_chip_width)
                            + 10.0 * responsive_scale;
                        if chip_x + chip_w > chip_max_x {
                            chip_x = chip_start_x;
                            chip_y += section_row_step;
                            row_bottom = chip_y + chip_height;
                        }
                        if chip_x + chip_w > chip_max_x {
                            break 'chip_loop;
                        }

                        if chip_y + chip_height > settings_content_bottom {
                            break 'chip_loop;
                        }

                        let rect = [chip_x, chip_y, chip_w, chip_height];
                        append_soft_card_quads(
                            &mut quads,
                            rect,
                            if *selected { accent_soft } else { surface },
                            if *selected { accent } else { border_dark },
                            soft_shadow,
                            surface,
                            chip_corner_radius,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind: *kind,
                            rect: interaction_hit_rect(rect),
                        });
                        let option_layout = TextBlock {
                            text: (*chip_label).to_string(),
                            origin: [chip_x + 5.2 * responsive_scale, chip_y + 5.2 * responsive_scale],
                            max_width: (chip_w - 12.0 * responsive_scale).max(6.0 * responsive_scale),
                            pixel_size: chip_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if *selected { accent_text } else { text_primary },
                            align: TextAlign::Center,
                            role: TextRole::SettingOption,
                        }
                        .layout();
                        text_quads.extend(option_layout.quads.iter().copied());
                        atlas_glyphs.extend(option_layout.atlas_glyphs.iter().cloned());
                        option_layouts.push(option_layout);

                        section_has_options = true;
                        row_bottom = chip_y + chip_height;
                        chip_x += chip_w + chip_x_gap;
                    }

                    let section_card_top = (section_row_start - 3.6 * responsive_scale).max(settings_rect[1]);
                    let section_content_bottom = if section_has_options {
                        row_bottom
                    } else {
                        row_y + label_row_h
                    };
                    let section_card_bottom =
                        (section_content_bottom + 4.3 * responsive_scale).min(settings_content_bottom);
                    let section_card_h = (section_card_bottom - section_card_top).max(label_row_h);
                    if section_card_h > 0.0 {
                        append_soft_card_quads(
                            &mut quads,
                            [
                                panel_x + 6.0 * responsive_scale,
                                section_card_top,
                                panel_width - 12.0 * responsive_scale,
                                section_card_h,
                            ],
                            section_card_fill,
                            section_card_border,
                            soft_shadow,
                            surface,
                            section_card_radius,
                        );
                    }

                    if section_has_options {
                        row_y = row_bottom + row_gap;
                    } else {
                        row_y += label_row_h + row_gap;
                    }
                }

                    text_sections.push(TextSection {
                        role: TextRole::SettingLabel,
                        layouts: settings_layouts,
                    });
                    text_sections.push(TextSection {
                        role: TextRole::SettingOption,
                        layouts: option_layouts,
                    });
                }

}
