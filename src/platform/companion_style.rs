use std::ffi::CString;
use std::fs;

use crate::ime::gpu::ThemePreset;
use crate::platform::settings_host::display_settings_path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateCompanionStyle {
    pub window_title: &'static str,
    pub header_title: &'static str,
    pub show_header: bool,
    pub min_width: u16,
    pub max_width: u16,
    pub row_height: u16,
    pub max_text_lines: u8,
    pub horizontal_padding: u16,
    pub vertical_padding: u16,
    pub panel_background: u32,
    pub title_text: u32,
    pub row_normal_text: u32,
    pub row_hover_text: u32,
    pub row_selected_text: u32,
    pub accent: u32,
    pub border: u32,
}

pub fn detached_candidate_companion_style_for_preset(
    preset: ThemePreset,
) -> CandidateCompanionStyle {
    match preset {
        ThemePreset::Daylight => CandidateCompanionStyle {
            window_title: "Suzaku Candidates",
            header_title: "Suzaku Candidates",
            show_header: false,
            min_width: 220,
            max_width: 420,
            row_height: 22,
            max_text_lines: 2,
            horizontal_padding: 12,
            vertical_padding: 10,
            panel_background: 0xE3EBF7F7,
            title_text: 0x27415CFF,
            row_normal_text: 0x3E5874FF,
            row_hover_text: 0x23425FFF,
            row_selected_text: 0x143450FF,
            accent: 0x3D8EEBFF,
            border: 0x8CA4C2FF,
        },
        ThemePreset::DeviceDark => CandidateCompanionStyle {
            window_title: "Suzaku Candidates",
            header_title: "Suzaku Candidates",
            show_header: false,
            min_width: 220,
            max_width: 420,
            row_height: 22,
            max_text_lines: 2,
            horizontal_padding: 12,
            vertical_padding: 10,
            panel_background: 0x1E2735EF,
            title_text: 0xEAF3FFFF,
            row_normal_text: 0xD3E0F4FF,
            row_hover_text: 0xDFF0FFFF,
            row_selected_text: 0x91CCFFFF,
            accent: 0x4FA3F5FF,
            border: 0x5E7594FF,
        },
    }
}

pub fn current_companion_theme_preset() -> ThemePreset {
    let Ok(contents) = fs::read_to_string(display_settings_path()) else {
        return ThemePreset::Daylight;
    };

    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "theme_preset" {
            continue;
        }
        return match value.trim() {
            "device_dark" => ThemePreset::DeviceDark,
            _ => ThemePreset::Daylight,
        };
    }

    ThemePreset::Daylight
}

pub fn detached_candidate_companion_style() -> CandidateCompanionStyle {
    detached_candidate_companion_style_for_preset(current_companion_theme_preset())
}

fn into_raw_c_string(value: &str) -> *mut std::os::raw::c_char {
    CString::new(value)
        .expect("companion style strings must not contain interior nulls")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_window_title_utf8() -> *mut std::os::raw::c_char {
    into_raw_c_string(detached_candidate_companion_style().window_title)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_header_title_utf8() -> *mut std::os::raw::c_char {
    into_raw_c_string(detached_candidate_companion_style().header_title)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_panel_background_rgba8() -> u32 {
    detached_candidate_companion_style().panel_background
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_show_header() -> bool {
    detached_candidate_companion_style().show_header
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_min_width() -> u16 {
    detached_candidate_companion_style().min_width
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_max_width() -> u16 {
    detached_candidate_companion_style().max_width
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_row_height() -> u16 {
    detached_candidate_companion_style().row_height
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_max_text_lines() -> u8 {
    detached_candidate_companion_style().max_text_lines
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_horizontal_padding() -> u16 {
    detached_candidate_companion_style().horizontal_padding
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_vertical_padding() -> u16 {
    detached_candidate_companion_style().vertical_padding
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_title_text_rgba8() -> u32 {
    detached_candidate_companion_style().title_text
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_row_normal_text_rgba8() -> u32 {
    detached_candidate_companion_style().row_normal_text
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_row_hover_text_rgba8() -> u32 {
    detached_candidate_companion_style().row_hover_text
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_row_selected_text_rgba8() -> u32 {
    detached_candidate_companion_style().row_selected_text
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_accent_rgba8() -> u32 {
    detached_candidate_companion_style().accent
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_companion_border_rgba8() -> u32 {
    detached_candidate_companion_style().border
}

#[cfg(test)]
mod tests {
    use super::{current_companion_theme_preset, detached_candidate_companion_style_for_preset};
    use crate::ime::gpu::ThemePreset;

    #[test]
    fn detached_candidate_companion_style_uses_nonempty_titles() {
        let style = detached_candidate_companion_style_for_preset(ThemePreset::Daylight);
        assert!(!style.window_title.is_empty());
        assert!(!style.header_title.is_empty());
        assert!(style.min_width <= style.max_width);
    }

    #[test]
    fn detached_candidate_companion_style_has_distinct_interaction_colors() {
        let style = detached_candidate_companion_style_for_preset(ThemePreset::DeviceDark);
        assert_ne!(style.row_normal_text, style.row_hover_text);
        assert_ne!(style.row_hover_text, style.row_selected_text);
    }

    #[test]
    fn current_companion_theme_preset_defaults_to_known_theme() {
        assert!(matches!(
            current_companion_theme_preset(),
            ThemePreset::Daylight | ThemePreset::DeviceDark
        ));
    }
}
