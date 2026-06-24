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
        let base_line_gap = match chrome.candidate_density {
            CandidateDensity::Compact => 4.0,
            CandidateDensity::Cozy => 6.0,
        };
        let title_px = 3.6;
        let section_px = 2.45;
        let chip_px = 2.2;
        let panel_width = self.scene_width.clamp(520.0, 900.0) - 24.0;
        let panel_x = ((self.scene_width - panel_width) / 2.0).max(8.0);
        let panel_y = 10.0;
        let row_h = 36.0;
        let title_h = 40.0;
        let settings_row_count = 13.0;
        let panel_height = title_h + 12.0 + settings_row_count * row_h + 12.0;

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
            color: [1.0, 1.0, 1.0, 0.16],
        });

        let close_rect = [panel_x + panel_width - 36.0, panel_y + 8.0, 22.0, 22.0];
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
            origin: [panel_x + 16.0, panel_y + 12.0],
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

        let sections: [(&str, Vec<(InteractionKind, &str, bool)>); 11] = [
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

        let mut label_layouts = Vec::new();
        let mut option_layouts = Vec::new();
        let mut row_y = panel_y + title_h + 10.0;
        let label_col_x = panel_x + 18.0;
        let chip_start_x = panel_x + 132.0;
        let chip_max_x = panel_x + panel_width - 20.0;
        let chip_gap_x = 12.0;
        let chip_gap_y = 10.0;
        for (label, options) in sections.iter() {
            let section_top = row_y - 4.0;
            let estimated_rows = options
                .iter()
                .fold(
                    (chip_start_x, 1usize),
                    |(cursor_x, rows), (_, chip_label, _): &(InteractionKind, &str, bool)| {
                        let chip_w = (chip_label.chars().count() as f32 * 10.8).max(58.0) + 22.0;
                        if cursor_x + chip_w > chip_max_x {
                            (chip_start_x + chip_w + chip_gap_x, rows + 1)
                        } else {
                            (cursor_x + chip_w + chip_gap_x, rows)
                        }
                    },
                )
                .1;
            let section_height = 18.0 + estimated_rows as f32 * row_h - (row_h - 26.0);
            append_soft_card_quads(
                &mut quads,
                [
                    panel_x + 10.0,
                    section_top,
                    panel_width - 20.0,
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
                    panel_x + 22.0,
                    section_top + section_height - 1.0,
                    panel_width - 44.0,
                    1.0,
                ],
                color: [1.0, 1.0, 1.0, 0.08],
            });
            let label_layout = TextBlock {
                text: match *label {
                    "LLM" => "LLM".to_string(),
                    "Voice Auto" => "Voice Auto".to_string(),
                    "Device Dark" => "Device Dark".to_string(),
                    other => other.to_string(),
                },
                origin: [label_col_x, row_y + 7.0],
                max_width: 100.0,
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

            let mut chip_x = chip_start_x;
            let mut chip_y = row_y;
            let mut row_bottom = chip_y + 26.0;
            for (kind, chip_label, selected) in options {
                let (hovered, pressed) = interaction_state(*kind);
                let chip_w = (chip_label.chars().count() as f32 * 10.8).max(58.0) + 22.0;
                if chip_x + chip_w > chip_max_x {
                    chip_x = chip_start_x;
                    chip_y += row_h;
                }
                let rect = [chip_x, chip_y, chip_w, 26.0];
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
                        surface_alt
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
                    9.0,
                );
                interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                let option_layout = TextBlock {
                    text: (*chip_label).to_string(),
                    origin: [visual_rect[0] + 10.0, visual_rect[1] + 7.0],
                    max_width: visual_rect[2] - 20.0,
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
                row_bottom = chip_y + 26.0;
                chip_x += chip_w + chip_gap_x;
            }
            row_y = row_bottom + chip_gap_y;
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
