use super::{CandidateDensity, DisplayTextScale, PanelChromeState, PreviewStyle, ThemePreset};

/// Theme swatches are authored in sRGB; wgpu's sRGB targets expect linear input.
pub fn srgb_color(hex: u32) -> [f32; 4] {
    let linear = |byte: u32| {
        let v = byte as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    [
        linear((hex >> 16) & 255),
        linear((hex >> 8) & 255),
        linear(hex & 255),
        1.0,
    ]
}

pub(super) struct PanelTheme {
    pub(super) page_bg: [f32; 4],
    pub(super) shell: [f32; 4],
    pub(super) shell_border: [f32; 4],
    pub(super) surface: [f32; 4],
    pub(super) surface_alt: [f32; 4],
    pub(super) surface_muted: [f32; 4],
    pub(super) keyboard_surface: [f32; 4],
    pub(super) keyboard_special_surface: [f32; 4],
    pub(super) keyboard_text: [f32; 4],
    pub(super) keyboard_secondary_text: [f32; 4],
    pub(super) accent: [f32; 4],
    pub(super) accent_soft: [f32; 4],
    pub(super) accent_text: [f32; 4],
    pub(super) text_primary: [f32; 4],
    pub(super) text_secondary: [f32; 4],
    pub(super) text_muted: [f32; 4],
    pub(super) border_dark: [f32; 4],
    pub(super) soft_shadow: [f32; 4],
}

impl PanelTheme {
    pub(super) fn for_preset(preset: ThemePreset) -> Self {
        match preset {
            ThemePreset::Suzaku => Self {
                page_bg: srgb_color(0xF5EEE7),
                shell: srgb_color(0xFCF8F3),
                shell_border: srgb_color(0xDDC9B9),
                surface: srgb_color(0xFFFCF8),
                surface_alt: srgb_color(0xF8EFE7),
                surface_muted: srgb_color(0xF0E4DA),
                keyboard_surface: srgb_color(0xFFFCF8),
                keyboard_special_surface: srgb_color(0xF2E6DC),
                keyboard_text: srgb_color(0x382A2B),
                keyboard_secondary_text: srgb_color(0x705958),
                accent: srgb_color(0xB5393F),
                accent_soft: srgb_color(0xF4DCD3),
                accent_text: srgb_color(0x8B2933),
                text_primary: srgb_color(0x34272A),
                text_secondary: srgb_color(0x72585A),
                text_muted: srgb_color(0x785F5D),
                border_dark: srgb_color(0xDFCCC0),
                soft_shadow: [0.12, 0.025, 0.03, 0.18],
            },
            ThemePreset::Baihu => Self {
                page_bg: srgb_color(0xF1EEE5),
                shell: srgb_color(0xF8F6EE),
                shell_border: srgb_color(0xCDC6B6),
                surface: srgb_color(0xFFFDF7),
                surface_alt: srgb_color(0xF3F0E6),
                surface_muted: srgb_color(0xE7E2D5),
                keyboard_surface: srgb_color(0xFFFDF7),
                keyboard_special_surface: srgb_color(0xEDE8DB),
                keyboard_text: srgb_color(0x2D3434),
                keyboard_secondary_text: srgb_color(0x5C625D),
                accent: srgb_color(0x796441),
                accent_soft: srgb_color(0xEAE1CE),
                accent_text: srgb_color(0x665132),
                text_primary: srgb_color(0x2D3434),
                text_secondary: srgb_color(0x5C625D),
                text_muted: srgb_color(0x5D625A),
                border_dark: srgb_color(0xCFC7B7),
                soft_shadow: [0.06, 0.05, 0.03, 0.16],
            },
            ThemePreset::Qinglong => Self {
                page_bg: srgb_color(0xEAEDF5),
                shell: srgb_color(0xF3F6FB),
                shell_border: srgb_color(0xBCCDD7),
                surface: srgb_color(0xFCFDFF),
                surface_alt: srgb_color(0xEAF4F2),
                surface_muted: srgb_color(0xE6E2F0),
                keyboard_surface: srgb_color(0xF7FBFC),
                keyboard_special_surface: srgb_color(0xE6E2F0),
                keyboard_text: srgb_color(0x243344),
                keyboard_secondary_text: srgb_color(0x496171),
                accent: srgb_color(0x367A80),
                accent_soft: srgb_color(0xE4DFF1),
                accent_text: srgb_color(0x644C83),
                text_primary: srgb_color(0x243344),
                text_secondary: srgb_color(0x496171),
                text_muted: srgb_color(0x566276),
                border_dark: srgb_color(0xC4BDD6),
                soft_shadow: [0.025, 0.055, 0.12, 0.17],
            },
            ThemePreset::Xuanwu => Self {
                page_bg: srgb_color(0x111629),
                shell: srgb_color(0x1A203A),
                shell_border: srgb_color(0x465379),
                surface: srgb_color(0x222A49),
                surface_alt: srgb_color(0x293357),
                surface_muted: srgb_color(0x303B5C),
                keyboard_surface: srgb_color(0x202742),
                keyboard_special_surface: srgb_color(0x303B5C),
                keyboard_text: srgb_color(0xEEF0FB),
                keyboard_secondary_text: srgb_color(0xB9C3E0),
                accent: srgb_color(0x9CAEEB),
                accent_soft: srgb_color(0x3C4874),
                accent_text: srgb_color(0xE1E7FF),
                text_primary: srgb_color(0xEEF0FB),
                text_secondary: srgb_color(0xB9C3E0),
                text_muted: srgb_color(0xABB9D6),
                border_dark: srgb_color(0x506087),
                soft_shadow: [0.004, 0.006, 0.018, 0.40],
            },
            ThemePreset::Daylight => Self {
                page_bg: [0.90, 0.93, 0.99, 1.0],
                shell: [0.97, 0.99, 1.0, 1.0],
                shell_border: [0.74, 0.84, 1.0, 1.0],
                surface: [0.98, 0.99, 1.0, 1.0],
                surface_alt: [0.94, 0.97, 1.0, 1.0],
                surface_muted: [0.90, 0.93, 0.98, 1.0],
                keyboard_surface: [0.98, 0.99, 1.0, 1.0],
                keyboard_special_surface: [0.85, 0.89, 0.96, 1.0],
                keyboard_text: [0.04, 0.07, 0.11, 1.0],
                keyboard_secondary_text: [0.16, 0.20, 0.27, 1.0],
                accent: [0.22, 0.49, 1.0, 1.0],
                accent_soft: [0.80, 0.89, 1.0, 1.0],
                accent_text: [0.10, 0.30, 0.70, 1.0],
                text_primary: [0.07, 0.10, 0.17, 1.0],
                text_secondary: [0.20, 0.27, 0.38, 1.0],
                text_muted: [0.34, 0.42, 0.55, 1.0],
                border_dark: [0.76, 0.84, 0.95, 1.0],
                soft_shadow: [0.22, 0.32, 0.50, 0.14],
            },
            ThemePreset::Solarized => Self {
                page_bg: [0.96, 0.93, 0.84, 1.0],
                shell: [0.98, 0.95, 0.87, 1.0],
                shell_border: [0.73, 0.63, 0.50, 1.0],
                surface: [0.97, 0.93, 0.84, 1.0],
                surface_alt: [0.93, 0.87, 0.78, 1.0],
                surface_muted: [0.89, 0.82, 0.71, 1.0],
                keyboard_surface: [0.98, 0.94, 0.84, 1.0],
                keyboard_special_surface: [0.93, 0.86, 0.74, 1.0],
                keyboard_text: [0.18, 0.16, 0.12, 1.0],
                keyboard_secondary_text: [0.34, 0.28, 0.20, 1.0],
                accent: [0.56, 0.44, 0.18, 1.0],
                accent_soft: [0.84, 0.76, 0.58, 1.0],
                accent_text: [0.31, 0.23, 0.12, 1.0],
                text_primary: [0.18, 0.16, 0.14, 1.0],
                text_secondary: [0.35, 0.28, 0.20, 1.0],
                text_muted: [0.57, 0.50, 0.40, 1.0],
                border_dark: [0.71, 0.56, 0.41, 1.0],
                soft_shadow: [0.40, 0.30, 0.19, 0.22],
            },
            ThemePreset::DeviceDark => Self {
                page_bg: [0.15, 0.18, 0.24, 1.0],
                shell: [0.19, 0.23, 0.31, 1.0],
                shell_border: [0.33, 0.40, 0.52, 1.0],
                surface: [0.23, 0.27, 0.36, 1.0],
                surface_alt: [0.26, 0.31, 0.41, 1.0],
                surface_muted: [0.29, 0.34, 0.44, 1.0],
                keyboard_surface: [0.18, 0.22, 0.30, 1.0],
                keyboard_special_surface: [0.28, 0.33, 0.43, 1.0],
                keyboard_text: [0.96, 0.98, 1.0, 1.0],
                keyboard_secondary_text: [0.93, 0.96, 1.0, 1.0],
                accent: [0.40, 0.68, 1.0, 1.0],
                accent_soft: [0.22, 0.41, 0.63, 1.0],
                accent_text: [0.93, 0.97, 1.0, 1.0],
                text_primary: [0.92, 0.95, 1.0, 1.0],
                text_secondary: [0.80, 0.86, 0.93, 1.0],
                text_muted: [0.68, 0.76, 0.86, 1.0],
                border_dark: [0.37, 0.46, 0.59, 1.0],
                soft_shadow: [0.03, 0.05, 0.09, 0.34],
            },
            ThemePreset::HighContrast => Self {
                page_bg: [0.03, 0.04, 0.07, 1.0],
                shell: [0.08, 0.11, 0.17, 1.0],
                shell_border: [0.42, 0.52, 0.84, 1.0],
                surface: [0.16, 0.21, 0.32, 1.0],
                surface_alt: [0.19, 0.25, 0.37, 1.0],
                surface_muted: [0.22, 0.29, 0.42, 1.0],
                keyboard_surface: [0.09, 0.13, 0.20, 1.0],
                keyboard_special_surface: [0.14, 0.20, 0.31, 1.0],
                keyboard_text: [1.0, 1.0, 1.0, 1.0],
                keyboard_secondary_text: [0.90, 0.94, 1.0, 1.0],
                accent: [0.15, 0.86, 1.0, 1.0],
                accent_soft: [0.20, 0.70, 1.0, 1.0],
                accent_text: [0.98, 0.99, 1.0, 1.0],
                text_primary: [1.0, 1.0, 1.0, 1.0],
                text_secondary: [0.92, 0.96, 1.0, 1.0],
                text_muted: [0.72, 0.82, 0.96, 1.0],
                border_dark: [0.84, 0.90, 1.0, 1.0],
                soft_shadow: [0.00, 0.00, 0.00, 0.76],
            },
        }
    }
}

impl ThemePreset {
    /// Linear RGBA, shared with the native window clear and tooltip overlay.
    pub fn page_background(self) -> [f32; 4] {
        PanelTheme::for_preset(self).page_bg
    }

    pub fn tooltip_colors(self) -> ([f32; 4], [f32; 4], [f32; 4]) {
        let theme = PanelTheme::for_preset(self);
        (theme.surface, theme.text_primary, theme.accent)
    }

    pub(super) fn secondary_accent(self) -> [f32; 4] {
        srgb_color(match self {
            Self::Suzaku => 0xB68B52,
            Self::Baihu => 0x8B8065,
            Self::Qinglong => 0x80639F,
            Self::Xuanwu => 0x89BCCA,
            _ => return PanelTheme::for_preset(self).accent,
        })
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn guardian_text_retains_readable_contrast_on_light_and_dark_surfaces() {
        let luminance = |color: [f32; 4]| color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
        let contrast = |a, b| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for preset in ThemePreset::ALL.into_iter().filter(|p| p.is_guardian()) {
            let theme = PanelTheme::for_preset(preset);
            for foreground in [theme.text_primary, theme.text_secondary, theme.text_muted] {
                for surface in [theme.surface, theme.surface_alt, theme.surface_muted] {
                    assert!(
                        contrast(surface, foreground) >= 4.5,
                        "{preset:?}: unreadable text"
                    );
                }
            }
            assert!(
                contrast(theme.accent_soft, theme.accent_text) >= 4.5,
                "{preset:?}: selection"
            );
            assert!(contrast(theme.keyboard_surface, theme.keyboard_text) >= 4.5);
            assert!(
                contrast(
                    theme.keyboard_special_surface,
                    theme.keyboard_secondary_text
                ) >= 4.5
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PanelSceneMetrics {
    pub(super) panel_width: f32,
    pub(super) panel_x: f32,
    pub(super) panel_y: f32,
    pub(super) panel_height: f32,
    pub(super) section_gap: f32,
    pub(super) input_box_h: f32,
    pub(super) input_value_px: f32,
    pub(super) tools_header_h: f32,
    pub(super) tool_button_h: f32,
    pub(super) tool_gap: f32,
    pub(super) tools_content_h: f32,
    pub(super) item_height: f32,
    pub(super) hero_item_height: f32,
    pub(super) item_gap: f32,
    pub(super) chip_section_h: f32,
    pub(super) sentence_section_gap: f32,
    pub(super) candidate_columns: usize,
    pub(super) stacked_token_header: bool,
}

impl PanelSceneMetrics {
    pub(super) fn new(
        scene_width: f32,
        scene_height: f32,
        responsive_scale: f32,
        chrome: &PanelChromeState,
        sentence_count: usize,
        hero_cards_enabled: bool,
    ) -> Self {
        let translating = chrome.input_modes_expanded
            && chrome.active_input_mode == super::InputMode::Translation;
        let sentence_count = if translating { 0 } else { sentence_count };
        let collapsed_daily_mode = !chrome.input_modes_expanded;
        let scene_margin = (4.2 * responsive_scale).max(3.0);
        let max_panel_width = (scene_width - scene_margin * 2.0).max(0.0);
        let min_panel_width = (max_panel_width * 0.78)
            .clamp(190.0, 310.0)
            .min(max_panel_width);
        let desired_panel_width = (scene_width - 24.0 * responsive_scale).max(190.0);
        let panel_width = desired_panel_width
            .min(max_panel_width)
            .max(min_panel_width);

        let spacing_scale = if chrome.candidate_density == CandidateDensity::Compact {
            0.97
        } else {
            1.0
        };

        let input_value_px = match chrome.text_scale {
            DisplayTextScale::Small => 2.4,
            DisplayTextScale::Medium => 3.4,
            DisplayTextScale::Large => 4.2,
        } * responsive_scale
            * if panel_width < 680.0 {
                0.90
            } else if panel_width < 760.0 {
                0.96
            } else {
                1.0
            };
        // Header controls and editable text occupy separate rows, including at Large text size.
        let input_box_h =
            29.0 * responsive_scale + input_value_px * 7.0 + 9.0 * responsive_scale * spacing_scale;
        let tools_header_h = if collapsed_daily_mode {
            21.0 * responsive_scale
        } else {
            19.0 * responsive_scale
        };
        let tool_button_h = 16.2 * responsive_scale * spacing_scale;
        let tool_gap = 2.8 * responsive_scale * spacing_scale;
        let section_gap = 2.9 * responsive_scale * spacing_scale;

        let expanded_input_panel_h = if chrome.input_modes_expanded {
            match chrome.active_input_mode {
                crate::ime::gpu::InputMode::VirtualKeyboard => 145.0 * responsive_scale,
                crate::ime::gpu::InputMode::Dictation => 170.0 * responsive_scale,
                // Real title/hint line heights plus a readable footer and drawing area.
                crate::ime::gpu::InputMode::Handwriting => 220.0 * responsive_scale,
                crate::ime::gpu::InputMode::Translation => {
                    super::translation_scene::preferred_translation_height(
                        panel_width - 12.0 * responsive_scale,
                        responsive_scale,
                        chrome.text_scale,
                        chrome.ui_language,
                    ) + 6.4 * responsive_scale
                }
            }
        } else {
            0.0
        };

        let stack_cap = (scene_height - scene_margin * 2.0).max(0.0);
        let mut tools_content_h = expanded_input_panel_h;
        let mut item_height = match (chrome.candidate_density, chrome.preview_style) {
            (CandidateDensity::Compact, PreviewStyle::Compact) => 48.0,
            (CandidateDensity::Compact, PreviewStyle::Full) => 66.0,
            (CandidateDensity::Cozy, PreviewStyle::Compact) => 54.0,
            (CandidateDensity::Cozy, PreviewStyle::Full) => 72.0,
        } * responsive_scale;

        let mut hero_item_height = item_height + 10.0 * responsive_scale;
        let mut item_gap = if chrome.candidate_density == CandidateDensity::Compact {
            2.6
        } else {
            3.0
        } * responsive_scale;

        let stacked_token_header = !collapsed_daily_mode
            && max_panel_width < 560.0
            && !chrome.next_token_candidates.is_empty();

        let chip_section_h = if translating
            || (chrome.next_token_candidates.is_empty() && chrome.composed_tokens.is_empty())
        {
            0.0
        } else if collapsed_daily_mode {
            32.5 * responsive_scale
        } else if chrome.next_token_candidates.is_empty() {
            // Keep pending undo even when suggestions are exhausted. Fit the
            // Large history heading and card insets, without empty chip rows.
            40.0 * responsive_scale
        } else {
            (if stacked_token_header { 45.0 } else { 72.0 }) * responsive_scale
        };

        let alternate_count = if hero_cards_enabled {
            sentence_count.saturating_sub(1)
        } else {
            sentence_count
        };
        let sentence_columns = if collapsed_daily_mode {
            sentence_count.clamp(1, 4)
        } else if panel_width >= 900.0 && alternate_count >= 3 {
            3
        } else if panel_width >= 520.0 && alternate_count >= 2 {
            2
        } else {
            1
        };

        let sentence_rows = if collapsed_daily_mode {
            if sentence_count == 0 { 0 } else { 1 }
        } else {
            alternate_count.div_ceil(sentence_columns)
        };

        let mut sentence_height = if collapsed_daily_mode {
            if sentence_count == 0 {
                0.0
            } else {
                43.0 * responsive_scale
            }
        } else if sentence_count == 0 {
            0.0
        } else if hero_cards_enabled {
            hero_item_height
                + if sentence_rows == 0 {
                    0.0
                } else {
                    item_gap
                        + sentence_rows as f32 * item_height
                        + (sentence_rows as f32 - 1.0) * item_gap
                }
        } else {
            sentence_rows as f32 * item_height + (sentence_rows as f32 - 1.0) * item_gap
        };

        let fixed_without_sentence = input_box_h
            + section_gap
            + tools_header_h
            + tools_content_h
            + section_gap
            + chip_section_h
            + if chip_section_h > 0.0 && sentence_count > 0 {
                if collapsed_daily_mode {
                    2.8 * responsive_scale
                } else {
                    3.2 * responsive_scale
                }
            } else {
                0.0
            };

        if fixed_without_sentence > stack_cap {
            let base_without_tools = input_box_h
                + section_gap
                + tools_header_h
                + section_gap
                + chip_section_h
                + if chip_section_h > 0.0 && sentence_count > 0 {
                    if collapsed_daily_mode {
                        2.8 * responsive_scale
                    } else {
                        3.2 * responsive_scale
                    }
                } else {
                    0.0
                };
            tools_content_h = (stack_cap - base_without_tools).max(0.0);
        }

        let fixed_without_sentence = input_box_h
            + section_gap
            + tools_header_h
            + tools_content_h
            + section_gap
            + chip_section_h
            + if chip_section_h > 0.0 && sentence_count > 0 {
                if collapsed_daily_mode {
                    3.2 * responsive_scale
                } else {
                    4.6 * responsive_scale
                }
            } else {
                0.0
            };

        let available_sentence_h = (stack_cap - fixed_without_sentence).max(0.0);
        if sentence_count > 0 {
            let height_scale = (available_sentence_h / sentence_height).clamp(0.22, 1.0);
            if height_scale < 1.0 {
                item_height *= height_scale;
                item_gap *= height_scale;
                hero_item_height = item_height + 10.0 * responsive_scale * height_scale;
                sentence_height = if collapsed_daily_mode {
                    if sentence_count == 0 {
                        0.0
                    } else {
                        (43.0 * responsive_scale).max((40.0 * height_scale).max(item_height))
                    }
                } else if sentence_count == 0 {
                    0.0
                } else if hero_cards_enabled {
                    hero_item_height
                        + if sentence_rows == 0 {
                            0.0
                        } else {
                            item_gap
                                + sentence_rows as f32 * item_height
                                + (sentence_rows as f32 - 1.0) * item_gap
                        }
                } else {
                    sentence_rows as f32 * item_height + (sentence_rows as f32 - 1.0) * item_gap
                };
            }
        }

        let panel_height = (fixed_without_sentence + sentence_height).min(stack_cap);
        // Anchor the top edge while the native window catches up with a content
        // resize; never center a folded panel in the previous tall viewport.
        let panel_y = scene_margin.min((scene_height - panel_height).max(0.0));

        let panel_x = if scene_width > panel_width + scene_margin * 2.0 {
            ((scene_width - panel_width) / 2.0).max(scene_margin)
        } else {
            0.0
        };

        Self {
            panel_width,
            panel_x,
            panel_y,
            panel_height,
            section_gap,
            input_box_h,
            input_value_px,
            tools_header_h,
            tool_button_h,
            tool_gap,
            tools_content_h,
            item_height,
            hero_item_height,
            item_gap,
            chip_section_h,
            sentence_section_gap: if chip_section_h > 0.0 && sentence_count > 0 {
                if collapsed_daily_mode {
                    2.8 * responsive_scale
                } else {
                    4.6 * responsive_scale
                }
            } else {
                0.0
            },
            candidate_columns: sentence_columns,
            stacked_token_header,
        }
    }
}
