use super::{CandidateDensity, PanelChromeState, PreviewStyle, ThemePreset};

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

#[derive(Debug, Clone, Copy)]
pub(super) struct PanelSceneMetrics {
    pub(super) panel_width: f32,
    pub(super) panel_x: f32,
    pub(super) panel_y: f32,
    pub(super) panel_height: f32,
    pub(super) section_gap: f32,
    pub(super) input_box_h: f32,
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
    ) -> Self {
        let collapsed_daily_mode = !chrome.input_modes_expanded;
        let scene_margin = (4.2 * responsive_scale).max(3.0);
        let max_panel_width = (scene_width - scene_margin * 2.0).max(0.0);
        let min_panel_width = (max_panel_width * 0.78)
            .clamp(190.0, 310.0)
            .min(max_panel_width);
        let desired_panel_width = (scene_width - 220.0).clamp(190.0, 1200.0);
        let panel_width = desired_panel_width
            .min(max_panel_width)
            .max(min_panel_width);

        let spacing_scale = if chrome.candidate_density == CandidateDensity::Compact {
            0.97
        } else {
            1.0
        };

        let input_box_h = 46.0 * responsive_scale * spacing_scale;
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
                crate::ime::gpu::InputMode::VirtualKeyboard => 156.0 * responsive_scale,
                crate::ime::gpu::InputMode::Dictation => 170.0 * responsive_scale,
                crate::ime::gpu::InputMode::Handwriting => 173.0 * responsive_scale,
            }
        } else {
            0.0
        };

        let stack_cap = (scene_height - scene_margin * 2.0).max(0.0);
        let mut tools_content_h = expanded_input_panel_h;
        let mut item_height = match (chrome.candidate_density, chrome.preview_style) {
            (CandidateDensity::Compact, PreviewStyle::Compact) => 48.0,
            (CandidateDensity::Compact, PreviewStyle::Full) => 60.0,
            (CandidateDensity::Cozy, PreviewStyle::Compact) => 54.0,
            (CandidateDensity::Cozy, PreviewStyle::Full) => 71.0,
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

        let chip_section_h = if chrome.next_token_candidates.is_empty() {
            0.0
        } else if collapsed_daily_mode {
            32.5 * responsive_scale
        } else {
            (if stacked_token_header { 45.0 } else { 72.0 }) * responsive_scale
        };

        let sentence_columns = if collapsed_daily_mode {
            sentence_count.clamp(1, 4)
        } else if panel_width >= 748.0 && sentence_count > 2 {
            2
        } else {
            1
        };

        let sentence_rows = if collapsed_daily_mode {
            if sentence_count == 0 { 0 } else { 1 }
        } else if sentence_count == 0 {
            0
        } else {
            (sentence_count - 1).div_ceil(sentence_columns)
        };

        let mut sentence_height = if collapsed_daily_mode {
            if sentence_count == 0 {
                0.0
            } else {
                43.0 * responsive_scale
            }
        } else if sentence_rows == 0 {
            if sentence_count == 0 {
                0.0
            } else {
                hero_item_height
            }
        } else {
            hero_item_height
                + item_gap
                + sentence_rows as f32 * item_height
                + (sentence_rows as f32 - 1.0) * item_gap
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
                } else if sentence_rows == 0 {
                    if sentence_count == 0 {
                        0.0
                    } else {
                        hero_item_height
                    }
                } else {
                    hero_item_height
                        + item_gap
                        + sentence_rows as f32 * item_height
                        + (sentence_rows as f32 - 1.0) * item_gap
                };
            }
        }

        let panel_height = (fixed_without_sentence + sentence_height).min(stack_cap);
        let panel_y = if panel_height < scene_height - scene_margin * 2.0 {
            scene_margin
        } else {
            0.0
        };

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
