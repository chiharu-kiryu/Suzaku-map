use crate::ime::gpu::FontFaceChoice;
#[cfg(target_os = "windows")]
use crate::platform::windows;
use winit::event_loop::EventLoopBuilder;
use winit::keyboard::ModifiersState;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowAttributesExtWindows;
#[cfg(target_os = "windows")]
use winit::window::Theme;
use winit::window::WindowAttributes;

pub fn configure_event_loop_builder(builder: &mut EventLoopBuilder<()>) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

        builder.with_activation_policy(ActivationPolicy::Regular);
        builder.with_default_menu(true);
        builder.with_activate_ignoring_other_apps(true);
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
            .with_movable_by_window_background(true)
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

pub fn preferred_font_paths(font_face: FontFaceChoice) -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "macos")]
    {
        return match font_face {
            FontFaceChoice::Auto => vec![
                ("/System/Library/Fonts/Monaco.ttf", "Monaco"),
                ("/System/Library/Fonts/Geneva.ttf", "Geneva"),
                ("/Library/Fonts/Arial Unicode.ttf", "Arial Unicode"),
            ],
            FontFaceChoice::Monaco => vec![("/System/Library/Fonts/Monaco.ttf", "Monaco")],
            FontFaceChoice::Geneva => vec![("/System/Library/Fonts/Geneva.ttf", "Geneva")],
            FontFaceChoice::ArialUnicode => {
                vec![("/Library/Fonts/Arial Unicode.ttf", "Arial Unicode")]
            }
        };
    }

    #[cfg(target_os = "windows")]
    {
        return windows::preferred_font_paths(font_face);
    }

    #[cfg(target_os = "linux")]
    {
        return match font_face {
            FontFaceChoice::Auto => vec![
                (
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                    "DejaVu Sans Mono",
                ),
                (
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "DejaVu Sans",
                ),
                (
                    "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
                    "Noto Sans CJK",
                ),
            ],
            FontFaceChoice::Monaco => {
                vec![(
                    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                    "DejaVu Sans Mono",
                )]
            }
            FontFaceChoice::Geneva => {
                vec![(
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                    "DejaVu Sans",
                )]
            }
            FontFaceChoice::ArialUnicode => {
                vec![(
                    "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
                    "Noto Sans CJK",
                )]
            }
        };
    }

    #[allow(unreachable_code)]
    Vec::new()
}
