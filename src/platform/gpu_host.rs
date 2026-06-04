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
