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
                page_bg: [0.72, 0.78, 0.87, 1.0],
                shell: [0.83, 0.88, 0.95, 1.0],
                shell_border: [0.49, 0.58, 0.71, 1.0],
                surface: [0.89, 0.93, 0.98, 1.0],
                surface_alt: [0.81, 0.87, 0.95, 1.0],
                surface_muted: [0.73, 0.79, 0.88, 1.0],
                keyboard_surface: [0.90, 0.94, 0.99, 1.0],
                keyboard_special_surface: [0.74, 0.80, 0.89, 1.0],
                keyboard_text: [0.06, 0.09, 0.14, 1.0],
                keyboard_secondary_text: [0.08, 0.12, 0.18, 1.0],
                accent: [0.13, 0.44, 0.82, 1.0],
                accent_soft: [0.63, 0.81, 1.0, 1.0],
                accent_text: [0.04, 0.13, 0.26, 1.0],
                text_primary: [0.06, 0.10, 0.16, 1.0],
                text_secondary: [0.10, 0.15, 0.22, 1.0],
                text_muted: [0.18, 0.25, 0.34, 1.0],
                border_dark: [0.45, 0.55, 0.68, 1.0],
                soft_shadow: [0.22, 0.30, 0.43, 0.16],
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
                keyboard_secondary_text: [0.88, 0.93, 0.99, 1.0],
                accent: [0.40, 0.68, 1.0, 1.0],
                accent_soft: [0.22, 0.41, 0.63, 1.0],
                accent_text: [0.93, 0.97, 1.0, 1.0],
                text_primary: [0.92, 0.95, 1.0, 1.0],
                text_secondary: [0.74, 0.81, 0.90, 1.0],
                text_muted: [0.56, 0.65, 0.77, 1.0],
                border_dark: [0.37, 0.46, 0.59, 1.0],
                soft_shadow: [0.03, 0.05, 0.09, 0.34],
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
    pub(super) settings_panel_h: f32,
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
        let scene_margin = (7.0 * responsive_scale).max(6.0);
        let max_panel_width = (scene_width - scene_margin * 2.0).max(320.0);
        let min_panel_width = 380.0_f32.min(max_panel_width);
        let desired_panel_width = scene_width * 0.94;
        let panel_width = desired_panel_width
            .min(max_panel_width)
            .max(min_panel_width);
        let input_box_h = 58.0 * responsive_scale;
        let tools_header_h = if collapsed_daily_mode {
            34.0 * responsive_scale
        } else {
            28.0 * responsive_scale
        };
        let tool_button_h = 22.0 * responsive_scale;
        let tool_gap = 5.0 * responsive_scale;
        let section_gap = 6.0 * responsive_scale;
        let extended_input_panel_h = if chrome.input_modes_expanded {
            228.0 * responsive_scale
        } else {
            0.0
        };
        let tools_content_h = if chrome.input_modes_expanded {
            extended_input_panel_h
        } else {
            0.0
        };
        let settings_panel_h = if chrome.settings_open { 286.0 } else { 0.0 };
        let item_height = match (chrome.candidate_density, chrome.preview_style) {
            (CandidateDensity::Compact, PreviewStyle::Compact) => 62.0,
            (CandidateDensity::Compact, PreviewStyle::Full) => 82.0,
            (CandidateDensity::Cozy, PreviewStyle::Compact) => 72.0,
            (CandidateDensity::Cozy, PreviewStyle::Full) => 92.0,
        } * responsive_scale;
        let hero_item_height = item_height + 12.0 * responsive_scale;
        let item_gap = if chrome.candidate_density == CandidateDensity::Compact {
            5.0
        } else {
            6.0
        } * responsive_scale;
        let stacked_token_header = !collapsed_daily_mode
            && max_panel_width < 560.0
            && !chrome.next_token_candidates.is_empty();
        let chip_section_h = if chrome.next_token_candidates.is_empty() {
            0.0
        } else if collapsed_daily_mode {
            42.0 * responsive_scale
        } else {
            (if stacked_token_header { 74.0 } else { 52.0 }) * responsive_scale
        };
        let sentence_section_gap = if chip_section_h > 0.0 && sentence_count > 0 {
            if collapsed_daily_mode {
                4.0 * responsive_scale
            } else {
                6.0 * responsive_scale
            }
        } else {
            0.0
        };
        let candidate_columns = if collapsed_daily_mode {
            sentence_count.clamp(1, 4)
        } else if panel_width >= 760.0 && sentence_count > 2 {
            2
        } else {
            1
        };
        let sentence_rows = if collapsed_daily_mode {
            if sentence_count == 0 { 0 } else { 1 }
        } else if sentence_count == 0 {
            0
        } else {
            (sentence_count - 1).div_ceil(candidate_columns)
        };
        let sentence_height = if collapsed_daily_mode {
            if sentence_count == 0 {
                0.0
            } else {
                52.0 * responsive_scale
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
        let panel_height = input_box_h
            + section_gap
            + tools_header_h
            + tools_content_h
            + settings_panel_h
            + section_gap
            + chip_section_h
            + sentence_section_gap
            + sentence_height;
        let panel_x = ((scene_width - panel_width) / 2.0).max(scene_margin);
        let centered_panel_y = (scene_height - panel_height) / 2.0;
        let panel_y = centered_panel_y.max(14.0 * responsive_scale);

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
            settings_panel_h,
            item_height,
            hero_item_height,
            item_gap,
            chip_section_h,
            sentence_section_gap,
            candidate_columns,
            stacked_token_header,
        }
    }
}
