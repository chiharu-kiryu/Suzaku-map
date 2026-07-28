use super::*;

impl WgpuCandidateRenderer {
    pub fn build_settings_scene(&self, chrome: &PanelChromeState) -> RenderScene {
        let theme = PanelTheme::for_preset(chrome.theme_preset);
        let page_bg = theme.page_bg;
        let shell = theme.shell;
        let shell_border = theme.shell_border;
        let surface = theme.surface;
        let surface_alt = theme.surface_alt;
        let surface_muted = theme.surface_muted;
        let accent_soft = theme.accent_soft;
        let accent = theme.accent;
        let accent_text = theme.accent_text;
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;
        let border_dark = theme.border_dark;
        let soft_shadow = theme.soft_shadow;
        let tracking = match chrome.text_spacing {
            TextSpacing::Tight => -0.45,
            TextSpacing::Normal => -0.15,
            TextSpacing::Relaxed => 0.24,
        };
        let ui_tracking = tracking * 0.08 - 0.03;
        let heading_tracking = tracking * 0.04 - 0.02;
        let base_line_gap = if chrome.candidate_density == CandidateDensity::Compact {
            4.7
        } else {
            6.0
        };
        let ui_scale = (self.scene_width / 640.0).clamp(0.86, 1.08);
        let title_px = 3.82 * ui_scale;
        let section_px = 2.52 * ui_scale;
        let chip_px = 2.34 * ui_scale;
        let panel_width = (self.scene_width * 0.92).clamp(390.0, 740.0);
        let panel_x = ((self.scene_width - panel_width) / 2.0).max(6.0);
        let row_height = 24.8 * ui_scale;
        let section_gap_y = 4.8 * ui_scale;
        let chip_start_x = panel_x + 112.0 * ui_scale;
        let chip_max_x = panel_x + panel_width - 18.0;
        let chip_gap_x = 6.0 * ui_scale;
        let chip_gap_y = 5.0 * ui_scale;
        let label_col_x = panel_x + 17.0 * ui_scale;
        let label_max_width = (chip_start_x - label_col_x - 8.0).max(94.0);
        let row_label_height = 21.8 * ui_scale;
        let section_label_height = row_label_height;
        let section_margin = 2.4 * ui_scale;
        let min_panel_height = 166.0;
        let chip_area_width = (chip_max_x - chip_start_x).max(132.0);

        let sections: Vec<(&str, Vec<(InteractionKind, &str, bool)>)> = vec![
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
                vec![(
                    InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                    "Llama3.2 3B (default)",
                    chrome.llm_model == LlmModelPreset::Llama32_3b,
                )],
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
                .max(56.0)
                .min(chip_area_width)
                + 20.0 * ui_scale
        };
        let estimate_chip_rows = |options: &[(InteractionKind, &str, bool)]| {
            let mut cursor_x = chip_start_x;
            let mut rows = 1usize;
            for (_, chip_label, _) in options {
                let chip_w = estimated_chip_width(chip_label);
                if cursor_x + chip_w > chip_max_x {
                    rows += 1;
                    cursor_x = chip_start_x;
                }
                cursor_x += chip_w + chip_gap_x;
            }
            rows
        };
        let mut estimated_height = 0.0;

        for (_, options) in sections.iter() {
            let section_rows = estimate_chip_rows(options.as_slice());
            estimated_height += section_margin + section_label_height;
            estimated_height += section_rows as f32 * row_height;
            if section_rows > 1 {
                estimated_height += (section_rows as f32 - 1.0) * chip_gap_y;
            }
            estimated_height += section_gap_y;
        }

        let title_section_h = 35.0;
        let panel_padding_y = 8.0;
        let panel_height = (title_section_h + estimated_height + panel_padding_y)
            .max(min_panel_height)
            .min((self.scene_height - 16.0).max(224.0));
        let panel_y = if self.scene_height > panel_height + 20.0 {
            10.0
        } else {
            0.0
        };

        let mut quads = Vec::new();
        let mut text_quads = Vec::new();
        let mut atlas_glyphs = Vec::new();
        let mut text_sections = Vec::new();
        let hit_targets = Vec::new();
        let mut interactive_targets = Vec::new();
        let interaction_state = |kind: InteractionKind| {
            (
                chrome.hovered_interaction == Some(kind),
                chrome.pressed_interaction == Some(kind),
            )
        };
        let animated_rect = |rect: [f32; 4], hovered: bool, pressed: bool| {
            if pressed {
                [rect[0], rect[1] + 1.0, rect[2], rect[3]]
            } else if hovered {
                [rect[0], rect[1] - 1.0, rect[2], rect[3]]
            } else {
                rect
            }
        };
        let animated_shadow = |shadow: [f32; 4], hovered: bool, pressed: bool| {
            let mut next = shadow;
            next[3] *= if pressed {
                0.76
            } else if hovered {
                1.18
            } else {
                1.0
            };
            next
        };

        quads.push(CandidateQuad {
            rect: [0.0, 0.0, self.scene_width, self.scene_height],
            color: page_bg,
        });
        append_soft_card_quads(
            &mut quads,
            [panel_x, panel_y, panel_width, panel_height],
            shell,
            shell_border,
            soft_shadow,
            page_bg,
            12.0,
        );
        quads.push(CandidateQuad {
            rect: [panel_x + 10.0, panel_y + 8.0, panel_width - 20.0, 2.0],
            color: if chrome.theme_preset == ThemePreset::DeviceDark {
                [1.0, 1.0, 1.0, 0.06]
            } else {
                [1.0, 1.0, 1.0, 0.16]
            },
        });

        let close_rect = [panel_x + panel_width - 34.0, panel_y + 9.0, 20.0, 20.0];
        let (close_hovered, close_pressed) = interaction_state(InteractionKind::SettingsToggle);
        let close_visual_rect = animated_rect(close_rect, close_hovered, close_pressed);
        append_soft_card_quads(
            &mut quads,
            close_visual_rect,
            if close_pressed {
                [0.67, 0.82, 0.97, 1.0]
            } else if close_hovered {
                [0.93, 0.96, 1.0, 1.0]
            } else {
                surface_alt
            },
            if close_pressed {
                [0.08, 0.35, 0.68, 1.0]
            } else if close_hovered {
                [0.46, 0.62, 0.82, 1.0]
            } else {
                shell_border
            },
            animated_shadow(soft_shadow, close_hovered, close_pressed),
            shell,
            8.0,
        );
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::SettingsToggle,
            rect: close_rect,
        });
        append_gear_icon_quads(&mut quads, close_visual_rect, text_secondary, surface_alt);

        let title_layout = TextBlock {
            text: "Panel Settings".to_string(),
            origin: [panel_x + 16.0, panel_y + 11.0],
            max_width: panel_width - 60.0,
            pixel_size: title_px,
            letter_spacing: heading_tracking,
            line_gap: base_line_gap,
            max_lines: 1,
            color: text_primary,
            align: TextAlign::Left,
            role: TextRole::HeaderTitle,
        }
        .layout();
        text_quads.extend(title_layout.quads.iter().copied());
        atlas_glyphs.extend(title_layout.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::HeaderTitle,
            layouts: vec![title_layout],
        });

        let mut label_layouts = Vec::new();
        let mut option_layouts = Vec::new();
        let mut content_y = panel_y + 42.0;
        let panel_bottom_guard = panel_y + panel_height - 5.0;

        for (label, options) in sections.iter() {
            let section_rows = estimate_chip_rows(options.as_slice());
            let mut section_height =
                section_margin + section_label_height + section_rows as f32 * row_height;
            if section_rows > 1 {
                section_height += (section_rows as f32 - 1.0) * chip_gap_y;
            }
            let section_top = content_y - 3.2;
            if section_top + section_height > panel_bottom_guard {
                break;
            }
            append_soft_card_quads(
                &mut quads,
                [
                    panel_x + 8.0,
                    section_top,
                    panel_width - 16.0,
                    section_height,
                ],
                surface,
                [border_dark[0], border_dark[1], border_dark[2], 0.42],
                [
                    soft_shadow[0],
                    soft_shadow[1],
                    soft_shadow[2],
                    soft_shadow[3] * 0.45,
                ],
                surface_muted,
                10.0,
            );
            quads.push(CandidateQuad {
                rect: [
                    panel_x + 20.0,
                    section_top + section_height - 1.0,
                    panel_width - 40.0,
                    1.0,
                ],
                color: if chrome.theme_preset == ThemePreset::DeviceDark {
                    [1.0, 1.0, 1.0, 0.06]
                } else {
                    [1.0, 1.0, 1.0, 0.08]
                },
            });

            let mut chip_x = chip_start_x;
            let mut chip_y = content_y;
            let chip_area_right = chip_max_x;
            for (kind, chip_label, selected) in options {
                let (hovered, pressed) = interaction_state(*kind);
                let available_chip_w =
                    (chip_area_width + chip_start_x - chip_x).max(72.0 * ui_scale);
                let chip_w = estimated_chip_width(chip_label).min(available_chip_w);
                if chip_x > chip_start_x && chip_x + chip_w > chip_area_right {
                    chip_x = chip_start_x;
                    chip_y += row_height + chip_gap_y;
                }

                let rect = [chip_x, chip_y, chip_w, row_height];
                let visual_rect = animated_rect(rect, hovered, pressed);
                append_soft_card_quads(
                    &mut quads,
                    visual_rect,
                    if pressed {
                        [0.67, 0.82, 0.97, 1.0]
                    } else if *selected {
                        accent_soft
                    } else if hovered {
                        [0.93, 0.96, 1.0, 1.0]
                    } else {
                        surface
                    },
                    if pressed {
                        [0.08, 0.35, 0.68, 1.0]
                    } else if *selected {
                        accent
                    } else if hovered {
                        [0.46, 0.62, 0.82, 1.0]
                    } else {
                        shell_border
                    },
                    animated_shadow(soft_shadow, hovered, pressed),
                    shell,
                    8.0,
                );
                interactive_targets.push(InteractiveTarget { kind: *kind, rect });

                let option_layout = TextBlock {
                    text: (*chip_label).to_string(),
                    origin: [
                        visual_rect[0] + 10.0 * ui_scale,
                        visual_rect[1] + 6.2 * ui_scale,
                    ],
                    max_width: (visual_rect[2] - 20.0 * ui_scale).max(14.0),
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

                chip_x += chip_w + chip_gap_x;
            }

            let label_layout = TextBlock {
                text: (*label).to_string(),
                origin: [label_col_x, content_y + 6.8],
                max_width: label_max_width,
                pixel_size: section_px,
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

            content_y += section_height + section_gap_y;
        }

        text_sections.push(TextSection {
            role: TextRole::SettingLabel,
            layouts: label_layouts,
        });
        text_sections.push(TextSection {
            role: TextRole::SettingOption,
            layouts: option_layouts,
        });

        RenderScene {
            quads,
            text_quads,
            atlas_glyphs,
            text_sections,
            hit_targets,
            interactive_targets,
            labels: Vec::new(),
            selected_label: None,
            draft_text: String::new(),
        }
    }

    pub async fn request_adapter() -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
    }
}
