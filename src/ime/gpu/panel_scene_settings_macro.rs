macro_rules! panel_scene_settings {
    () => {
            if chrome.settings_open {
                let settings_rect = [
                    panel_x,
                    settings_y + 8.0,
                    panel_width,
                    metrics.settings_panel_h - 8.0,
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

                let mut row_y = settings_rect[1] + 10.0;
                let chip_start_x = panel_x + 86.0;
                let chip_max_x = settings_rect[0] + settings_rect[2] - 12.0;
                for (label, options) in sections.iter() {
                    let label_layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [panel_x + 14.0, row_y + 2.0],
                        max_width: 72.0,
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
                    let mut row_bottom = chip_y + 20.0;
                    for (kind, chip_label, selected) in options {
                        let chip_w = (chip_label.chars().count() as f32 * 10.0).max(34.0) + 12.0;
                        if chip_x + chip_w > chip_max_x {
                            chip_x = chip_start_x;
                            chip_y += 24.0;
                        }
                        let rect = [chip_x, chip_y, chip_w, 20.0];
                        append_soft_card_quads(
                            &mut quads,
                            rect,
                            if *selected { accent_soft } else { surface },
                            if *selected { accent } else { border_dark },
                            soft_shadow,
                            surface,
                            7.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                        let option_layout = TextBlock {
                            text: (*chip_label).to_string(),
                            origin: [chip_x + 6.0, chip_y + 5.0],
                            max_width: chip_w - 12.0,
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
                        row_bottom = chip_y + 20.0;
                        chip_x += chip_w + 8.0;
                    }
                    row_y = row_bottom + 4.0;
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

    };
}
