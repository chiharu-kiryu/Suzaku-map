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
        ThemePreset::Solarized => CandidateCompanionStyle {
            window_title: "Suzaku Candidates",
            header_title: "Suzaku Candidates",
            show_header: false,
            min_width: 220,
            max_width: 420,
            row_height: 22,
            max_text_lines: 2,
            horizontal_padding: 12,
            vertical_padding: 10,
            panel_background: 0xF3EBDDFF,
            title_text: 0x2F2920FF,
            row_normal_text: 0x4E4438FF,
            row_hover_text: 0x392D1EFF,
            row_selected_text: 0x20140AFF,
            accent: 0x8A6F38FF,
            border: 0xB39A70FF,
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
        ThemePreset::HighContrast => CandidateCompanionStyle {
            window_title: "Suzaku Candidates",
            header_title: "Suzaku Candidates",
            show_header: false,
            min_width: 220,
            max_width: 420,
            row_height: 22,
            max_text_lines: 2,
            horizontal_padding: 12,
            vertical_padding: 10,
            panel_background: 0x111726FF,
            title_text: 0xEDF5FFFF,
            row_normal_text: 0xDDE8F7FF,
            row_hover_text: 0xD7F2FFFF,
            row_selected_text: 0x7EE4FFFF,
            accent: 0x39B9EFFF,
            border: 0x6B88AFFF,
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
            "solarized" => ThemePreset::Solarized,
            "device_dark" => ThemePreset::DeviceDark,
            "high_contrast" => ThemePreset::HighContrast,
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
    use std::ffi::{CStr, CString};
    #[cfg(target_os = "linux")]
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::path::Path;
    #[cfg(target_os = "linux")]
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        current_companion_theme_preset, detached_candidate_companion_style,
        detached_candidate_companion_style_for_preset, suzaku_host_companion_accent_rgba8,
        suzaku_host_companion_border_rgba8, suzaku_host_companion_header_title_utf8,
        suzaku_host_companion_horizontal_padding, suzaku_host_companion_max_text_lines,
        suzaku_host_companion_max_width, suzaku_host_companion_min_width,
        suzaku_host_companion_panel_background_rgba8, suzaku_host_companion_row_height,
        suzaku_host_companion_row_hover_text_rgba8, suzaku_host_companion_row_normal_text_rgba8,
        suzaku_host_companion_row_selected_text_rgba8, suzaku_host_companion_show_header,
        suzaku_host_companion_title_text_rgba8, suzaku_host_companion_vertical_padding,
        suzaku_host_companion_window_title_utf8,
    };
    use crate::ime::gpu::ThemePreset;
    #[cfg(target_os = "linux")]
    use crate::platform::test_env;
    #[cfg(target_os = "linux")]
    use crate::platform::test_env::ScopedEnv;
    use std::os::raw::c_char;

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
    fn detached_candidate_companion_style_exports_are_consistent() {
        let style = detached_candidate_companion_style();

        let title = suzaku_host_companion_window_title_utf8();
        let header = suzaku_host_companion_header_title_utf8();

        let title = c_string_to_owned(title);
        let header = c_string_to_owned(header);

        assert_eq!(title.as_deref(), Some(style.window_title));
        assert_eq!(header.as_deref(), Some(style.header_title));
        assert_eq!(
            suzaku_host_companion_panel_background_rgba8(),
            style.panel_background
        );
        assert_eq!(
            suzaku_host_companion_horizontal_padding(),
            style.horizontal_padding
        );
    }

    #[test]
    fn detached_candidate_companion_style_matches_preset_selector() {
        let theme = current_companion_theme_preset();
        let preset_style = detached_candidate_companion_style_for_preset(theme);
        let current_style = detached_candidate_companion_style();

        assert_eq!(preset_style.window_title, current_style.window_title);
        assert_eq!(preset_style.header_title, current_style.header_title);
        assert_eq!(preset_style.show_header, current_style.show_header);
        assert_eq!(preset_style.min_width, current_style.min_width);
        assert_eq!(preset_style.max_width, current_style.max_width);
        assert_eq!(preset_style.row_height, current_style.row_height);
        assert_eq!(preset_style.max_text_lines, current_style.max_text_lines);
    }

    #[test]
    fn detached_candidate_companion_style_exports_are_numeric() {
        let style = detached_candidate_companion_style();

        assert_eq!(suzaku_host_companion_min_width(), style.min_width);
        assert_eq!(suzaku_host_companion_max_width(), style.max_width);
        assert_eq!(suzaku_host_companion_row_height(), style.row_height);
        assert_eq!(suzaku_host_companion_max_text_lines(), style.max_text_lines);
        assert_eq!(
            suzaku_host_companion_vertical_padding(),
            style.vertical_padding
        );
        assert_eq!(suzaku_host_companion_title_text_rgba8(), style.title_text);
        assert_eq!(
            suzaku_host_companion_row_normal_text_rgba8(),
            style.row_normal_text
        );
        assert_eq!(
            suzaku_host_companion_row_hover_text_rgba8(),
            style.row_hover_text
        );
        assert_eq!(
            suzaku_host_companion_row_selected_text_rgba8(),
            style.row_selected_text
        );
        assert_eq!(suzaku_host_companion_accent_rgba8(), style.accent);
        assert_eq!(suzaku_host_companion_border_rgba8(), style.border);
        assert_eq!(suzaku_host_companion_show_header(), style.show_header);
    }

    #[test]
    fn current_companion_theme_preset_defaults_to_known_theme() {
        assert!(matches!(
            current_companion_theme_preset(),
            ThemePreset::Daylight
                | ThemePreset::DeviceDark
                | ThemePreset::HighContrast
                | ThemePreset::Solarized
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn current_companion_theme_preset_reads_device_dark_setting() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            write_theme_setting_file("theme_preset=device_dark", env);
            assert_eq!(current_companion_theme_preset(), ThemePreset::DeviceDark);
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn current_companion_theme_preset_falls_back_for_unknown_lines() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            write_theme_setting_file("something else\nnot_a_key=42\ntheme_preset=starlight", env);
            assert_eq!(current_companion_theme_preset(), ThemePreset::Daylight);
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn current_companion_theme_preset_ignores_whitespace_around_key_value() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            write_theme_setting_file(" theme_preset = device_dark \n", env);
            assert_eq!(current_companion_theme_preset(), ThemePreset::DeviceDark);
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn current_companion_theme_preset_defaults_on_empty_or_missing_value() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            write_theme_setting_file("theme_preset=\n", env);
            assert_eq!(current_companion_theme_preset(), ThemePreset::Daylight);
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn current_companion_theme_preset_uses_first_matching_entry_when_duplicated() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            write_theme_setting_file("theme_preset=device_dark\ntheme_preset=daylight", env);
            assert_eq!(current_companion_theme_preset(), ThemePreset::DeviceDark);
        });
    }

    #[cfg(target_os = "linux")]
    fn write_theme_setting_file(contents: &str, env: &mut ScopedEnv) {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let mut temp_root = std::env::temp_dir();
        temp_root.push(format!(
            "suzaku-theme-settings-{}-{}",
            std::process::id(),
            now_nanos
        ));
        env.set_var("XDG_CONFIG_HOME", temp_root.to_str().expect("temp path"));

        let settings_path = super::super::settings_host::display_settings_path();
        let parent = settings_path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).expect("create settings directory");
        fs::write(settings_path, contents).expect("write settings file");
    }

    fn c_string_to_owned(ptr: *mut c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        let _ = unsafe { CString::from_raw(ptr) };
        Some(text)
    }
}
