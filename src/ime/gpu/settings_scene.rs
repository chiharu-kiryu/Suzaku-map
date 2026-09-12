use super::*;
use std::time::Instant;

impl WgpuCandidateRenderer {
    pub fn build_settings_scene(
        &self,
        chrome: &PanelChromeState,
        settings_option_text_scroll: Option<(InteractionKind, Instant)>,
    ) -> RenderScene {
        let ui = chrome.ui_language;
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
        let ui_scale = (self.scene_width / 640.0).clamp(0.86, 1.08);
        let tracking = match chrome.text_spacing {
            TextSpacing::Tight => -0.41,
            TextSpacing::Normal => -0.32,
            TextSpacing::Relaxed => 0.08,
        };
        let ui_tracking = tracking * 0.16 - 0.02 * ui_scale;
        let heading_tracking = tracking * 0.10 - 0.01 * ui_scale;
        let label_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.2,
            DisplayTextScale::Medium => 2.45,
            DisplayTextScale::Large => 3.12,
        } * ui_scale;
        let base_line_gap = if chrome.candidate_density == CandidateDensity::Compact {
            5.2 * ui_scale
        } else {
            6.4 * ui_scale
        };
        let title_px = (label_px * 1.32_f32).max(2.8_f32 * ui_scale);
        let section_px = (label_px * 1.01_f32).max(2.0_f32 * ui_scale);
        let chip_px = (label_px * 0.94_f32).max(1.88_f32 * ui_scale);
        let panel_width = (self.scene_width - 16.0).max(0.0);
        let panel_x = (self.scene_width - panel_width) / 2.0;
        let row_height = (24.8 * ui_scale).max(section_px.max(chip_px) * 7.0 + 8.0 * ui_scale);
        let section_gap_y = 4.8 * ui_scale;
        let label_col_x = panel_x + 30.0 * ui_scale;
        let label_max_width = [
            "Text",
            "Theme",
            "Title Bar",
            "Font",
            "Density",
            "Spacing",
            "Smooth",
            "Preview",
            "Tap Slop",
            "Tap Timeout",
            "Target Slop",
            "Voice Auto",
            "LLM",
            "Provider",
            "Tone",
            "Interface",
        ]
        .into_iter()
        .map(|label| {
            let label = ui.tr(label);
            measure_text_prefix_width(label, label.chars().count(), section_px, heading_tracking)
        })
        .fold(0.0_f32, f32::max)
        .ceil()
            + 1.0;
        let stacked_labels = label_max_width > panel_width * 0.32;
        let chip_start_x = if stacked_labels {
            label_col_x
        } else {
            label_col_x + label_max_width + 12.0 * ui_scale
        };
        let scroll_track_width = 5.8 * ui_scale;
        let scroll_track_padding = 8.0 * ui_scale;
        let chip_max_x = panel_x + panel_width - (scroll_track_width + scroll_track_padding * 2.0);
        let chip_gap_x = 5.8 * ui_scale;
        let chip_gap_y = 4.8 * ui_scale;
        let section_margin = 2.4 * ui_scale;
        let settings_panel_radius = 13.0 * ui_scale;
        let settings_section_radius = 10.0 * ui_scale;
        let settings_close_radius = 8.8 * ui_scale;
        let settings_chip_radius = 8.4 * ui_scale;
        let interaction_hit_rect = |rect: [f32; 4]| {
            self.interaction_hit_rect(
                rect,
                ui_scale,
                chrome.pointer_target_slop_tenths,
                1.0,
                2.0,
                2.0,
                true,
            )
        };
        let min_panel_height = 166.0;
        let chip_area_width = (chip_max_x - chip_start_x).max(0.0);

        let custom_tone_label = ui
            .message(&format!(
                "Custom ({:.1})",
                chrome.llm_temperature.tenths() as f32 / 10.0
            ))
            .into_owned();
        let mut sections: Vec<(SettingsCategory, &str, Vec<(InteractionKind, &str, bool)>)> = vec![
            (
                SettingsCategory::Appearance,
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
                SettingsCategory::Appearance,
                "Theme",
                ThemePreset::ALL
                    .into_iter()
                    .map(|preset| {
                        (
                            InteractionKind::SetThemePreset(preset),
                            preset.label(),
                            chrome.theme_preset == preset,
                        )
                    })
                    .collect(),
            ),
            (
                SettingsCategory::Appearance,
                "Title Bar",
                vec![
                    (
                        InteractionKind::SetHideSystemTitlebar(false),
                        "Show",
                        !chrome.hide_system_titlebar,
                    ),
                    (
                        InteractionKind::SetHideSystemTitlebar(true),
                        "Hide",
                        chrome.hide_system_titlebar,
                    ),
                ],
            ),
            (
                SettingsCategory::Appearance,
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
                SettingsCategory::Input,
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
                SettingsCategory::Appearance,
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
                SettingsCategory::Appearance,
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
                SettingsCategory::Input,
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
                SettingsCategory::Input,
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
                SettingsCategory::Input,
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
                SettingsCategory::Input,
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
                SettingsCategory::Input,
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
                SettingsCategory::Model,
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
                SettingsCategory::Model,
                "Provider",
                vec![(
                    InteractionKind::SetLlmModel(LlmModelPreset::Configured),
                    "Configured service",
                    chrome.llm_model == LlmModelPreset::Configured,
                )],
            ),
            (
                SettingsCategory::Model,
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

        if matches!(chrome.llm_temperature, LlmTemperaturePreset::Custom(_)) {
            if let Some((_, _, options)) =
                sections.iter_mut().find(|(_, title, _)| *title == "Tone")
            {
                options.push((
                    InteractionKind::SetLlmTemperature(chrome.llm_temperature),
                    &custom_tone_label,
                    true,
                ));
            }
        }

        // Append instead of renumbering existing collapse identities.
        sections.push((
            SettingsCategory::Appearance,
            "Interface",
            crate::ui::UiLanguage::ALL
                .into_iter()
                .map(|language| {
                    (
                        InteractionKind::SetUiLanguage(language),
                        if language == crate::ui::UiLanguage::System {
                            ui.tr("System")
                        } else {
                            language.native_name()
                        },
                        chrome.ui_language == language,
                    )
                })
                .collect(),
        ));
        let original_sections = sections.clone();
        for (_, label, options) in &mut sections {
            *label = ui.tr(label);
            for (kind, text, _) in options {
                // Endonyms and font/model identities are not translated.
                if !matches!(
                    kind,
                    InteractionKind::SetUiLanguage(_) | InteractionKind::SetFontFace(_)
                ) || *text == "Auto"
                {
                    *text = ui.tr(text);
                }
            }
        }
        let estimated_chip_width = |label: &str| {
            (measure_text_prefix_width(label, label.chars().count(), chip_px, ui_tracking)
                + 20.0 * ui_scale
                + 1.0)
                .ceil()
                .max(48.0 * ui_scale)
                .min(chip_area_width)
        };
        let estimate_chip_rows = |options: &[(InteractionKind, &str, bool)]| {
            let mut cursor_x = chip_start_x;
            let mut rows = 1usize;
            for (_, chip_label, _) in options {
                let chip_w = estimated_chip_width(chip_label);
                if cursor_x > chip_start_x && cursor_x + chip_w > chip_max_x {
                    rows += 1;
                    cursor_x = chip_start_x;
                }
                cursor_x += chip_w + chip_gap_x;
            }
            rows
        };
        let search_query = chrome.settings_search_query.trim().to_lowercase();
        let is_searching = !search_query.is_empty();
        let mut visible_sections: Vec<(usize, &str, Vec<(InteractionKind, &str, bool)>, bool)> =
            Vec::new();

        for (index, (category, label, options)) in sections.iter().enumerate() {
            if !is_searching && *category != chrome.settings_category {
                continue;
            }
            let is_collapsed = chrome
                .settings_collapsed_sections
                .get(index)
                .copied()
                .unwrap_or(false);
            let mut filtered_options = Vec::new();

            if is_searching {
                let label_matches = label.to_lowercase().contains(&search_query)
                    || original_sections[index]
                        .1
                        .to_lowercase()
                        .contains(&search_query)
                    || (original_sections[index].1 == "Title Bar"
                        && ["system titlebar", "gnome", "系统标题栏", "隐藏标题栏"]
                            .iter()
                            .any(|alias| alias.contains(&search_query)));
                for (kind, option_label, selected) in options {
                    if label_matches
                        || option_label.to_lowercase().contains(&search_query)
                        || original_sections[index]
                            .2
                            .iter()
                            .any(|(original_kind, text, _)| {
                                original_kind == kind && text.to_lowercase().contains(&search_query)
                            })
                    {
                        filtered_options.push((*kind, *option_label, *selected));
                    }
                }
                if label_matches || !filtered_options.is_empty() {
                    visible_sections.push((index, label, filtered_options, false));
                }
            } else {
                for (kind, option_label, selected) in options {
                    filtered_options.push((*kind, *option_label, *selected));
                }
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
            let width_for_char = |ch: char| text_char_advance(ch, pixel_size, letter_spacing);

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

        // Labels share the first option row; estimation and drawing must use the same flow.
        let section_height_for = |options: &[(InteractionKind, &str, bool)], collapsed: bool| {
            let rows = if collapsed || options.is_empty() {
                1
            } else {
                estimate_chip_rows(options)
            };
            section_margin * 2.0
                + rows as f32 * row_height
                + (rows - 1) as f32 * chip_gap_y
                + if stacked_labels && !collapsed && !options.is_empty() {
                    row_height + chip_gap_y
                } else {
                    0.0
                }
        };
        let estimated_height: f32 = visible_sections
            .iter()
            .map(|(_, _, options, collapsed)| {
                section_height_for(options, *collapsed) + section_gap_y
            })
            .sum();

        let title_section_h = title_px * 7.0 + 12.0 * ui_scale;
        let search_bar_h = (24.0 * ui_scale).max(section_px * 7.0 + 8.0 * ui_scale);
        let tabs_h = row_height + 6.0 * ui_scale;
        let header_h = title_section_h + tabs_h + 6.0 * ui_scale + search_bar_h + 8.0;
        // Native windows use whole pixels. Round outward so fractional layout
        // arithmetic cannot create a scrollbar for an otherwise fully fitted page.
        let preferred_window_height = (header_h + estimated_height + 24.0).max(220.0).ceil();
        let panel_height = (preferred_window_height - 16.0)
            .max(min_panel_height)
            .min((self.scene_height - 16.0).max(0.0));
        let panel_y = 8.0;

        let mut quads = Vec::new();
        let mut text_quads = Vec::new();
        let mut atlas_glyphs = Vec::new();
        let mut text_sections = Vec::new();
        let hit_targets = Vec::new();
        let mut interactive_targets = Vec::new();
        let mut settings_option_truncated = Vec::new();
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

        if !chrome.hide_system_titlebar {
            quads.push(CandidateQuad {
                shape: Default::default(),
                clip_rect: None,
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
        }
        append_soft_card_quads(
            &mut quads,
            [panel_x, panel_y, panel_width, panel_height],
            shell,
            shell_border,
            soft_shadow,
            page_bg,
            settings_panel_radius,
        );
        let settings_divider = [
            (text_primary[0] * 0.84 + text_secondary[0] * 0.16),
            (text_primary[1] * 0.84 + text_secondary[1] * 0.16),
            (text_primary[2] * 0.84 + text_secondary[2] * 0.16),
            0.14,
        ];
        let settings_hover_surface = [
            (surface[0] + (1.0 - surface[0]) * 0.02).min(1.0),
            (surface[1] + (1.0 - surface[1]) * 0.02).min(1.0),
            (surface[2] + (1.0 - surface[2]) * 0.02).min(1.0),
            1.0,
        ];
        let settings_press_surface = [
            (surface[0] * 0.78 + accent[0] * 0.22),
            (surface[1] * 0.82 + accent[1] * 0.18),
            (surface[2] * 0.88 + accent[2] * 0.12),
            1.0,
        ];
        let settings_hover_border = [
            (shell_border[0] * 0.34 + accent[0] * 0.66),
            (shell_border[1] * 0.34 + accent[1] * 0.66),
            (shell_border[2] * 0.34 + accent[2] * 0.66),
            1.0,
        ];
        let settings_section_fill = [
            (surface[0] * 0.91 + surface_muted[0] * 0.09),
            (surface[1] * 0.91 + surface_muted[1] * 0.09),
            (surface[2] * 0.91 + surface_muted[2] * 0.09),
            0.30,
        ];
        let settings_section_border = [
            (surface[0] * 0.92 + border_dark[0] * 0.08),
            (surface[1] * 0.92 + border_dark[1] * 0.08),
            (surface[2] * 0.92 + border_dark[2] * 0.08),
            0.34,
        ];
        quads.push(CandidateQuad {
            shape: Default::default(),
            clip_rect: None,
            rect: [panel_x + 10.0, panel_y + 7.0, panel_width - 20.0, 1.6],
            color: settings_divider,
        });

        let close_size = (24.0 * ui_scale).max(20.0);
        let close_rect = [
            panel_x + panel_width - close_size - 12.0 * ui_scale,
            panel_y + (title_section_h - close_size) * 0.5,
            close_size,
            close_size,
        ];
        let (close_hovered, close_pressed) = interaction_state(InteractionKind::SettingsToggle);
        let close_visual_rect = animated_rect(close_rect, close_hovered, close_pressed);
        append_soft_card_quads(
            &mut quads,
            close_visual_rect,
            if close_pressed {
                settings_press_surface
            } else if close_hovered {
                settings_hover_surface
            } else {
                surface_alt
            },
            if close_pressed {
                accent
            } else if close_hovered {
                settings_hover_border
            } else {
                shell_border
            },
            animated_shadow(soft_shadow, close_hovered, close_pressed),
            shell,
            settings_close_radius,
        );
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::SettingsToggle,
            rect: interaction_hit_rect(close_rect),
        });
        let close_layout = TextBlock {
            text: String::new(),
            origin: [0.0; 2],
            max_width: 0.0,
            pixel_size: section_px,
            letter_spacing: ui_tracking,
            line_gap: 0.0,
            max_lines: 1,
            color: text_secondary,
            align: TextAlign::Center,
            role: TextRole::ToolButton,
        }
        .layout_in_rect(close_visual_rect, [3.0 * ui_scale, 3.0 * ui_scale]);
        append_close_icon_quads(&mut quads, close_visual_rect, text_secondary);
        text_quads.extend(close_layout.quads.iter().copied());
        atlas_glyphs.extend(close_layout.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::ToolButton,
            layouts: vec![close_layout],
        });

        let title_layout = TextBlock {
            text: ui.tr("Panel Settings").to_string(),
            origin: [panel_x + 15.0, panel_y + 9.0],
            max_width: (panel_width - 60.0).max(0.0),
            pixel_size: title_px,
            letter_spacing: heading_tracking,
            line_gap: base_line_gap,
            max_lines: 1,
            color: text_primary,
            align: TextAlign::Left,
            role: TextRole::HeaderTitle,
        }
        .layout_in_rect(
            [
                panel_x + 15.0 * ui_scale,
                panel_y,
                panel_width - close_size - 40.0 * ui_scale,
                title_section_h,
            ],
            [0.0, 4.0 * ui_scale],
        );
        text_quads.extend(title_layout.quads.iter().copied());
        atlas_glyphs.extend(title_layout.atlas_glyphs.iter().cloned());
        text_sections.push(TextSection {
            role: TextRole::HeaderTitle,
            layouts: vec![title_layout],
        });

        let mut label_layouts = Vec::new();
        let mut option_layouts = Vec::new();
        let search_bar_x = panel_x + 14.0 * ui_scale;
        let search_bar_w = (panel_width - 28.0 * ui_scale).max(0.0);
        let search_bar_y = panel_y + title_section_h + tabs_h + 6.0 * ui_scale;
        let search_clear_size = search_bar_h;
        let search_clear_x = search_bar_x + search_bar_w - search_clear_size;
        let search_clear_y = search_bar_y;
        let clear_search_text = !chrome.settings_search_query.is_empty();
        let clear_search_visible = clear_search_text;
        let search_bar_rect = [search_bar_x, search_bar_y, search_bar_w, search_bar_h];
        let clear_button_rect = [
            search_clear_x,
            search_clear_y,
            search_clear_size,
            search_bar_h,
        ];
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
            8.0 * ui_scale,
        );
        interactive_targets.push(InteractiveTarget {
            kind: InteractionKind::SettingsSearchInput,
            rect: interaction_hit_rect(search_bar_rect),
        });

        let search_text = if chrome.settings_search_query.is_empty() {
            ui.tr("Search all settings").to_string()
        } else {
            chrome.settings_search_query.clone()
        };
        let search_layout = TextBlock {
            text: search_text,
            origin: [
                search_bar_rect[0] + 8.0 * ui_scale,
                search_bar_rect[1] + 4.8 * ui_scale,
            ],
            max_width: (search_bar_rect[2] - 16.0 * ui_scale).max(40.0),
            pixel_size: section_px,
            letter_spacing: ui_tracking,
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
        .layout_in_rect(
            [
                search_bar_x,
                search_bar_y,
                (search_bar_w
                    - if clear_search_visible {
                        search_clear_size + 4.0 * ui_scale
                    } else {
                        0.0
                    })
                .max(0.0),
                search_bar_h,
            ],
            [8.0 * ui_scale, 3.0 * ui_scale],
        );
        text_quads.extend(search_layout.quads.iter().copied());
        atlas_glyphs.extend(search_layout.atlas_glyphs.iter().cloned());

        if clear_search_visible {
            let clear_fill = if clear_pressed {
                settings_press_surface
            } else if clear_hovered {
                settings_hover_surface
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
                    settings_hover_border
                } else {
                    shell_border
                },
                soft_shadow,
                surface,
                8.0 * ui_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsSearchClear,
                rect: interaction_hit_rect(clear_button_rect),
            });

            let clear_label = TextBlock {
                text: "×".to_string(),
                origin: [
                    clear_button_rect[0] + 6.0 * ui_scale,
                    clear_button_rect[1] + 4.0 * ui_scale,
                ],
                max_width: (clear_button_rect[2] - 2.0 * ui_scale).max(2.0),
                pixel_size: section_px,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Center,
                role: TextRole::SettingOption,
            }
            .layout_in_rect(clear_button_rect, [3.0 * ui_scale, 3.0 * ui_scale]);
            text_quads.extend(clear_label.quads.iter().copied());
            atlas_glyphs.extend(clear_label.atlas_glyphs.iter().cloned());
        }

        text_sections.push(TextSection {
            role: TextRole::InputLabel,
            layouts: vec![search_layout],
        });

        let tabs_x = search_bar_x;
        let tabs_y = panel_y + title_section_h;
        let tab_gap = 5.0 * ui_scale;
        let tab_content_width = (search_bar_w - 2.0 * tab_gap).max(0.0);
        let tab_label_widths = SettingsCategory::ALL.map(|category| {
            measure_text_prefix_width(
                ui.tr(category.label()),
                ui.tr(category.label()).chars().count(),
                chip_px,
                ui_tracking,
            )
            .ceil()
                + 16.0 * ui_scale
        });
        let tab_labels_width: f32 = tab_label_widths.iter().sum();
        let tab_extra_width = ((tab_content_width - tab_labels_width) / 3.0).max(0.0);
        let mut tab_x = tabs_x;
        let mut tab_layouts = Vec::new();
        for (index, category) in SettingsCategory::ALL.into_iter().enumerate() {
            // Give longer labels their actual text width before distributing
            // spare space; large text must not truncate the first tab at 400px.
            let tab_width = if tab_labels_width <= tab_content_width {
                tab_label_widths[index] + tab_extra_width
            } else {
                tab_label_widths[index] * tab_content_width / tab_labels_width
            };
            let rect = [tab_x, tabs_y, tab_width, tabs_h];
            tab_x += tab_width + tab_gap;
            let selected = !is_searching && chrome.settings_category == category;
            let kind = InteractionKind::SetSettingsCategory(category);
            let (hovered, pressed) = interaction_state(kind);
            append_rounded_rect_quads(
                &mut quads,
                rect,
                if selected || pressed {
                    accent_soft
                } else if hovered {
                    surface_alt
                } else {
                    shell
                },
                settings_chip_radius,
            );
            if selected {
                quads.push(CandidateQuad::outline(
                    rect,
                    accent,
                    settings_chip_radius,
                    0.8,
                ));
            }
            interactive_targets.push(InteractiveTarget { kind, rect });
            let layout = TextBlock {
                text: ui.tr(category.label()).into(),
                origin: [0.0; 2],
                max_width: tab_width,
                pixel_size: chip_px,
                letter_spacing: ui_tracking,
                line_gap: 0.0,
                max_lines: 1,
                color: if selected {
                    accent_text
                } else {
                    text_secondary
                },
                align: TextAlign::Center,
                role: TextRole::ToolButton,
            }
            .layout_in_rect(rect, [8.0 * ui_scale, 3.0 * ui_scale]);
            text_quads.extend(layout.quads.iter().copied());
            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            tab_layouts.push(layout);
        }
        text_sections.push(TextSection {
            role: TextRole::ToolButton,
            layouts: tab_layouts,
        });

        let settings_content_top = search_bar_y + search_bar_h + 8.0;
        let settings_content_bottom = panel_y + panel_height - 8.0;
        let visible_content_height = (settings_content_bottom - settings_content_top).max(0.0);
        let max_scroll_offset = (estimated_height - visible_content_height).max(0.0);
        let settings_scroll_offset = chrome
            .settings_scroll_offset
            .max(0.0)
            .min(max_scroll_offset);
        let settings_scroll_track_x =
            (panel_x + panel_width - scroll_track_width - scroll_track_padding).max(panel_x + 2.0);
        let settings_scroll_track_top = settings_content_top;
        let settings_scroll_track_height =
            (settings_content_bottom - settings_scroll_track_top).max(0.0);
        let mut content_y = settings_content_top - settings_scroll_offset;
        let has_settings_scroll = max_scroll_offset > 0.0 && settings_scroll_track_height > 0.0;
        let settings_scroll_handle_height = if has_settings_scroll {
            let ratio = (visible_content_height / estimated_height.max(visible_content_height))
                .clamp(0.12, 1.0);
            (settings_scroll_track_height * ratio)
                .max(14.0 * ui_scale)
                .min(settings_scroll_track_height)
        } else {
            settings_scroll_track_height.min(16.0 * ui_scale)
        };
        let settings_scroll_drag_range =
            (settings_scroll_track_height - settings_scroll_handle_height).max(0.0);
        let settings_scroll_handle_offset =
            if has_settings_scroll && settings_scroll_drag_range > 0.0 {
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
        let settings_scroll_metadata = SettingsScrollMetadata {
            preferred_window_height,
            track_rect: settings_scroll_track_rect,
            handle_rect: settings_scroll_handle_rect,
            content_height: estimated_height.max(0.0),
            visible_height: visible_content_height,
            max_scroll_offset,
            handle_drag_range: settings_scroll_drag_range,
        };
        let visible_in_settings = |top: f32, height: f32| {
            top + height > settings_content_top && top < settings_content_bottom
        };
        let content_viewport = [
            panel_x + 7.0,
            settings_content_top,
            panel_width - 14.0,
            visible_content_height,
        ];

        if has_settings_scroll {
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
            let (scroll_hovered_handle, _) =
                interaction_state(InteractionKind::SettingsScrollHandle);
            let handle_hovered = scroll_hovered_track || scroll_hovered_handle;
            let handle_color = if handle_hovered {
                [text_primary[0], text_primary[1], text_primary[2], 0.44]
            } else {
                [text_primary[0], text_primary[1], text_primary[2], 0.34]
            };
            let handle_rect = settings_scroll_handle_rect;
            quads.push(CandidateQuad {
                shape: Default::default(),
                clip_rect: None,
                rect: handle_rect,
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

        let content_quad_start = quads.len();
        let content_target_start = interactive_targets.len();
        if visible_sections.is_empty() {
            let mut empty_layout = TextBlock {
                text: ui.tr("No matching settings").to_string(),
                origin: [label_col_x, settings_content_top],
                max_width: panel_width - 40.0 * ui_scale,
                pixel_size: section_px,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Left,
                role: TextRole::SettingLabel,
            }
            .layout();
            empty_layout.clip_to_rect(content_viewport);
            text_quads.extend(empty_layout.quads.iter().copied());
            atlas_glyphs.extend(empty_layout.atlas_glyphs.iter().cloned());
            label_layouts.push(empty_layout);
        }
        for (_visible_index, (section_index, label, options, is_collapsed)) in
            visible_sections.iter().enumerate()
        {
            let section_effectively_collapsed = *is_collapsed;
            let section_height = section_height_for(options, section_effectively_collapsed);
            let section_top = content_y;
            let section_visible = visible_in_settings(section_top, section_height);
            if section_visible {
                append_soft_card_quads(
                    &mut quads,
                    [
                        panel_x + 8.0,
                        section_top,
                        chip_max_x - panel_x - 4.0,
                        section_height,
                    ],
                    settings_section_fill,
                    settings_section_border,
                    [
                        soft_shadow[0],
                        soft_shadow[1],
                        soft_shadow[2],
                        soft_shadow[3] * 0.45,
                    ],
                    surface_muted,
                    settings_section_radius,
                );
                quads.push(CandidateQuad {
                    shape: Default::default(),
                    clip_rect: None,
                    rect: [
                        panel_x + 20.0,
                        section_top + section_height - 1.0,
                        chip_max_x - panel_x - 20.0,
                        1.0,
                    ],
                    color: settings_divider,
                });
            }

            let row_y = content_y + section_margin;
            if visible_in_settings(row_y, row_height) {
                let toggle_size = 16.0 * ui_scale;
                append_chevron_icon_quads(
                    &mut quads,
                    [
                        panel_x + 12.0 * ui_scale,
                        row_y + (row_height - toggle_size) * 0.5,
                        toggle_size,
                        toggle_size,
                    ],
                    text_secondary,
                    section_effectively_collapsed,
                );
                if !is_searching {
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ToggleSettingsSection(*section_index),
                        rect: interaction_hit_rect([
                            panel_x + 10.0 * ui_scale,
                            row_y,
                            if stacked_labels {
                                chip_area_width
                            } else {
                                chip_start_x - panel_x - 16.0 * ui_scale
                            },
                            row_height,
                        ]),
                    });
                }
            }

            let mut chip_x = chip_start_x;
            let mut chip_y = row_y
                + if stacked_labels {
                    row_height + chip_gap_y
                } else {
                    0.0
                };
            if !section_effectively_collapsed {
                for (kind, chip_label, selected) in options {
                    let (hovered, pressed) = interaction_state(*kind);
                    let chip_w = estimated_chip_width(chip_label);
                    if chip_x > chip_start_x && chip_x + chip_w > chip_max_x {
                        chip_x = chip_start_x;
                        chip_y += row_height + chip_gap_y;
                    }
                    let rect = [chip_x, chip_y, chip_w, row_height];
                    let rect_visible = visible_in_settings(chip_y, row_height);
                    let visual_rect = animated_rect(rect, hovered, pressed);

                    let mut option_text = (*chip_label).to_string();
                    if rect_visible {
                        let base_layout = TextBlock {
                            text: option_text.clone(),
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
                        .layout_in_rect(visual_rect, [10.0 * ui_scale, 3.0 * ui_scale]);
                        if base_layout.truncated || base_layout.lines.len() > 1 {
                            settings_option_truncated.push(*kind);
                        }

                        if let Some((scroll_kind, started_at)) =
                            settings_option_text_scroll.as_ref()
                        {
                            if *scroll_kind == *kind && base_layout.truncated {
                                option_text = scroll_text_for_option(
                                    &option_text,
                                    *started_at,
                                    chip_px,
                                    ui_tracking,
                                    (visual_rect[2] - 20.0 * ui_scale).max(14.0),
                                );
                            }
                        }

                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                settings_press_surface
                            } else if *selected {
                                accent_soft
                            } else if hovered {
                                settings_hover_surface
                            } else {
                                surface
                            },
                            if pressed {
                                accent
                            } else if *selected {
                                accent
                            } else if hovered {
                                settings_hover_border
                            } else {
                                shell_border
                            },
                            animated_shadow(soft_shadow, hovered, pressed),
                            shell,
                            settings_chip_radius,
                        );
                        interactive_targets.push(InteractiveTarget {
                            kind: *kind,
                            rect: interaction_hit_rect(rect),
                        });

                        let mut option_layout = TextBlock {
                            text: option_text,
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
                        .layout_in_rect(visual_rect, [10.0 * ui_scale, 3.0 * ui_scale]);
                        option_layout.clip_to_rect(content_viewport);
                        text_quads.extend(option_layout.quads.iter().copied());
                        atlas_glyphs.extend(option_layout.atlas_glyphs.iter().cloned());
                        option_layouts.push(option_layout);
                    }
                    chip_x += chip_w + chip_gap_x;
                }
            }

            let mut label_layout = TextBlock {
                text: (*label).to_string(),
                origin: [label_col_x, content_y + 6.2],
                max_width: label_max_width,
                pixel_size: section_px,
                letter_spacing: heading_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Left,
                role: TextRole::SettingLabel,
            }
            .layout_in_rect(
                [label_col_x, row_y, label_max_width, row_height],
                [0.0, 3.0 * ui_scale],
            );
            label_layout.clip_to_rect(content_viewport);
            if visible_in_settings(row_y, row_height) {
                text_quads.extend(label_layout.quads.iter().copied());
                atlas_glyphs.extend(label_layout.atlas_glyphs.iter().cloned());
                label_layouts.push(label_layout);
            }
            content_y += section_height + section_gap_y;
        }

        for quad in &mut quads[content_quad_start..] {
            quad.clip_rect = Some(content_viewport);
        }
        for target in &mut interactive_targets[content_target_start..] {
            target.rect = intersect_rect(target.rect, content_viewport);
        }
        interactive_targets.retain(|target| target.rect[2] > 0.0 && target.rect[3] > 0.0);

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
            sentence_candidate_truncated: Vec::new(),
            next_token_candidate_truncated: Vec::new(),
            handwriting_candidate_truncated: Vec::new(),
            settings_option_truncated,
            settings_scroll_metadata: Some(settings_scroll_metadata),
            labels: Vec::new(),
            selected_label: None,
            draft_text: String::new(),
        }
        .with_window_drag_background(self.scene_width, self.scene_height)
    }

    pub async fn request_adapter() -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
    }
}
