use crate::ime::gpu::FontFaceChoice;
#[cfg(target_os = "linux")]
use crate::platform::linux;
#[cfg(target_os = "windows")]
use crate::platform::windows;
use winit::event_loop::EventLoopBuilder;
use winit::keyboard::ModifiersState;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowAttributesExtWindows;
#[cfg(target_os = "windows")]
use winit::window::Theme;
use winit::window::WindowAttributes;

pub fn configure_event_loop_builder(_builder: &mut EventLoopBuilder<()>) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

        _builder.with_activation_policy(ActivationPolicy::Regular);
        _builder.with_default_menu(true);
        _builder.with_activate_ignoring_other_apps(true);
    }
}

pub fn decorate_main_window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;

        return attrs
            .with_title_hidden(true)
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_movable_by_window_background(false)
            .with_accepts_first_mouse(true)
            .with_tabbing_identifier("suzaku.xr.panel");
    }

    #[cfg(target_os = "windows")]
    {
        return attrs
            .with_theme(Some(Theme::Dark))
            .with_drag_and_drop(false)
            .with_skip_taskbar(false);
    }

    #[allow(unreachable_code)]
    attrs
}

pub fn decorate_settings_window_attributes(attrs: WindowAttributes) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;

        return attrs
            .with_title_hidden(false)
            .with_titlebar_transparent(false)
            .with_fullsize_content_view(false)
            .with_accepts_first_mouse(true)
            .with_tabbing_identifier("suzaku.xr.settings");
    }

    #[cfg(target_os = "windows")]
    {
        return attrs
            .with_theme(Some(Theme::Dark))
            .with_drag_and_drop(false)
            .with_skip_taskbar(false);
    }

    #[allow(unreachable_code)]
    attrs
}

pub fn uses_super_for_quit() -> bool {
    cfg!(target_os = "macos")
}

pub fn is_quit_shortcut(modifiers: ModifiersState) -> bool {
    if uses_super_for_quit() {
        modifiers.super_key()
    } else {
        modifiers.control_key()
    }
}

pub fn preferred_font_paths(_font_face: FontFaceChoice) -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "macos")]
    {
        return match _font_face {
            FontFaceChoice::Auto => vec![
                ("/System/Library/Fonts/Monaco.ttf", "Monaco"),
                ("/System/Library/Fonts/Supplemental/Menlo.ttc", "Menlo"),
                ("/System/Library/Fonts/Helvetica.ttc", "Helvetica"),
                ("/System/Library/Fonts/PingFang.ttc", "PingFang"),
                ("/System/Library/Fonts/Geneva.ttf", "Geneva"),
                ("/Library/Fonts/Arial Unicode.ttf", "Arial Unicode"),
            ],
            FontFaceChoice::Monaco => vec![("/System/Library/Fonts/Monaco.ttf", "Monaco")],
            FontFaceChoice::Menlo => {
                vec![("/System/Library/Fonts/Supplemental/Menlo.ttc", "Menlo")]
            }
            FontFaceChoice::Geneva => vec![("/System/Library/Fonts/Geneva.ttf", "Geneva")],
            FontFaceChoice::Helvetica => {
                vec![("/System/Library/Fonts/Helvetica.ttc", "Helvetica")]
            }
            FontFaceChoice::PingFang => {
                vec![("/System/Library/Fonts/PingFang.ttc", "PingFang")]
            }
            FontFaceChoice::ArialUnicode => {
                vec![("/Library/Fonts/Arial Unicode.ttf", "Arial Unicode")]
            }
        };
    }

    #[cfg(target_os = "windows")]
    {
        return windows::preferred_font_paths(_font_face);
    }

    #[cfg(target_os = "linux")]
    {
        return linux::ubuntu_preferred_font_paths(_font_face);
    }

    #[allow(unreachable_code)]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::{
        configure_event_loop_builder, decorate_main_window_attributes,
        decorate_settings_window_attributes, is_quit_shortcut, preferred_font_paths,
        uses_super_for_quit,
    };
    use crate::ime::gpu::FontFaceChoice;
    use winit::event_loop::EventLoop;
    use winit::keyboard::ModifiersState;
    use winit::window::WindowAttributes;

    #[test]
    fn event_loop_builder_configures_macos_attributes_if_available() {
        let mut builder = EventLoop::<()>::builder();
        configure_event_loop_builder(&mut builder);
    }

    #[test]
    fn window_attributes_have_macos_variants_if_available() {
        let attrs = WindowAttributes::default();
        let _ = decorate_main_window_attributes(attrs);
        let attrs = WindowAttributes::default();
        let _ = decorate_settings_window_attributes(attrs);
    }

    #[test]
    fn quit_shortcut_tracks_platform_primary_modifier() {
        let super_mod = ModifiersState::SUPER;
        let control_mod = ModifiersState::CONTROL;
        let modifiers_off = ModifiersState::default();

        if cfg!(target_os = "macos") {
            assert_eq!(is_quit_shortcut(super_mod), true);
            assert_eq!(is_quit_shortcut(control_mod), false);
        } else {
            assert_eq!(is_quit_shortcut(control_mod), true);
            assert_eq!(is_quit_shortcut(super_mod), false);
        }
        assert_eq!(is_quit_shortcut(modifiers_off), false);
    }

    #[test]
    fn quit_shortcut_ignores_non_primary_modifiers() {
        let alt_or_shift_only = ModifiersState::ALT | ModifiersState::SHIFT;
        let ctrl_with_shift = ModifiersState::CONTROL | ModifiersState::SHIFT;
        let super_with_alt = ModifiersState::SUPER | ModifiersState::ALT;

        if cfg!(target_os = "macos") {
            assert!(!is_quit_shortcut(alt_or_shift_only));
            assert!(is_quit_shortcut(super_with_alt));
            assert!(!is_quit_shortcut(ctrl_with_shift));
        } else {
            assert!(!is_quit_shortcut(alt_or_shift_only));
            assert!(is_quit_shortcut(ctrl_with_shift));
            assert!(!is_quit_shortcut(super_with_alt));
        }
    }

    #[test]
    fn uses_super_for_quit_matches_platform_guard() {
        let expected = cfg!(target_os = "macos");
        assert_eq!(uses_super_for_quit(), expected);
    }

    #[test]
    fn preferred_fonts_available_for_all_choices() {
        assert!(!preferred_font_paths(FontFaceChoice::Auto).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::Monaco).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::Menlo).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::Geneva).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::Helvetica).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::PingFang).is_empty());
        assert!(!preferred_font_paths(FontFaceChoice::ArialUnicode).is_empty());
    }
}
