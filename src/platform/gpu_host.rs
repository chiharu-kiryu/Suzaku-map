use crate::ime::gpu::FontFaceChoice;
#[cfg(target_os = "linux")]
use crate::platform::linux;
#[cfg(target_os = "windows")]
use crate::platform::windows;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder};
use winit::keyboard::ModifiersState;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowAttributesExtWindows;
#[cfg(target_os = "windows")]
use winit::window::Theme;
use winit::window::{Window, WindowAttributes};

const NON_FOCUSING_PANEL_SCREEN_MARGIN_PX: i32 = 24;
const SETTINGS_WINDOW_GAP_PX: i32 = 12;
const SETTINGS_WINDOW_SCREEN_MARGIN_PX: i32 = 18;

fn bottom_centered_window_position(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    window_size: PhysicalSize<u32>,
    margin: i32,
) -> PhysicalPosition<i32> {
    let margin = i64::from(margin.max(0));
    let monitor_x = i64::from(monitor_position.x);
    let monitor_y = i64::from(monitor_position.y);
    let monitor_width = i64::from(monitor_size.width);
    let monitor_height = i64::from(monitor_size.height);
    let window_width = i64::from(window_size.width);
    let window_height = i64::from(window_size.height);

    let fits_horizontally = window_width + margin.saturating_mul(2) <= monitor_width;
    let fits_vertically = window_height + margin.saturating_mul(2) <= monitor_height;
    let x = if fits_horizontally {
        monitor_x + (monitor_width - window_width) / 2
    } else {
        monitor_x
    };
    let y = if fits_vertically {
        monitor_y + monitor_height - window_height - margin
    } else {
        monitor_y
    };

    PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

fn settings_window_position(
    anchor_position: PhysicalPosition<i32>,
    anchor_size: PhysicalSize<u32>,
    settings_size: PhysicalSize<u32>,
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    gap: i32,
    margin: i32,
) -> PhysicalPosition<i32> {
    let anchor_x = i64::from(anchor_position.x);
    let anchor_y = i64::from(anchor_position.y);
    let anchor_width = i64::from(anchor_size.width);
    let anchor_height = i64::from(anchor_size.height);
    let settings_width = i64::from(settings_size.width);
    let settings_height = i64::from(settings_size.height);
    let monitor_x = i64::from(monitor_position.x);
    let monitor_y = i64::from(monitor_position.y);
    let monitor_width = i64::from(monitor_size.width);
    let monitor_height = i64::from(monitor_size.height);
    let gap = i64::from(gap.max(0));
    let margin = i64::from(margin.max(0));

    let desired_x = anchor_x + (anchor_width - settings_width) / 2;
    let above_y = anchor_y - settings_height - gap;
    let below_y = anchor_y + anchor_height + gap;
    let monitor_top = monitor_y + margin;
    let monitor_bottom = monitor_y + monitor_height - margin;
    let desired_y = if above_y >= monitor_top {
        above_y
    } else if below_y + settings_height <= monitor_bottom {
        below_y
    } else {
        monitor_y + (monitor_height - settings_height) / 2
    };

    let fits_horizontally = settings_width + margin.saturating_mul(2) <= monitor_width;
    let fits_vertically = settings_height + margin.saturating_mul(2) <= monitor_height;
    let x = if fits_horizontally {
        desired_x.clamp(
            monitor_x + margin,
            monitor_x + monitor_width - settings_width - margin,
        )
    } else {
        monitor_x
    };
    let y = if fits_vertically {
        desired_y.clamp(monitor_top, monitor_bottom - settings_height)
    } else {
        monitor_y
    };

    PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

pub fn main_window_runs_without_focus() -> bool {
    #[cfg(target_os = "linux")]
    {
        return linux::linux_panel_uses_x11_no_focus();
    }

    #[allow(unreachable_code)]
    false
}

pub fn finish_main_window_creation(window: &Window, event_loop: &ActiveEventLoop) {
    #[cfg(target_os = "linux")]
    if main_window_runs_without_focus() {
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .or_else(|| window.current_monitor());
        if let Some(monitor) = monitor {
            let position = bottom_centered_window_position(
                monitor.position(),
                monitor.size(),
                window.inner_size(),
                NON_FOCUSING_PANEL_SCREEN_MARGIN_PX,
            );
            window.set_outer_position(position);
        }
        window.set_visible(true);
    }
}

pub fn finish_settings_window_creation(
    settings_window: &Window,
    main_window: &Window,
    event_loop: &ActiveEventLoop,
) {
    settings_window.set_visible(true);
    let monitor = main_window
        .current_monitor()
        .or_else(|| event_loop.primary_monitor())
        .or_else(|| event_loop.available_monitors().next());
    if let (Some(monitor), Ok(main_position)) = (monitor, main_window.outer_position()) {
        let position = settings_window_position(
            main_position,
            main_window.outer_size(),
            settings_window.outer_size(),
            monitor.position(),
            monitor.size(),
            SETTINGS_WINDOW_GAP_PX,
            SETTINGS_WINDOW_SCREEN_MARGIN_PX,
        );
        settings_window.set_outer_position(position);
    }
}

pub fn configure_event_loop_builder<T: 'static>(_builder: &mut EventLoopBuilder<T>) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

        _builder.with_activation_policy(ActivationPolicy::Regular);
        _builder.with_default_menu(true);
        _builder.with_activate_ignoring_other_apps(true);
    }

    #[cfg(target_os = "linux")]
    if linux::linux_panel_uses_x11_no_focus() {
        use winit::platform::x11::EventLoopBuilderExtX11;

        _builder.with_x11();
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

    #[cfg(target_os = "linux")]
    if linux::linux_panel_uses_x11_no_focus() {
        use winit::platform::x11::{WindowAttributesExtX11, WindowType};

        return attrs
            .with_visible(false)
            .with_active(false)
            .with_override_redirect(true)
            .with_x11_window_type(vec![WindowType::Utility])
            .with_name("Suzaku", "suzaku-panel");
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
        bottom_centered_window_position, configure_event_loop_builder,
        decorate_main_window_attributes, decorate_settings_window_attributes, is_quit_shortcut,
        preferred_font_paths, settings_window_position, uses_super_for_quit,
    };
    use crate::ime::gpu::FontFaceChoice;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
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
            assert!(is_quit_shortcut(super_mod));
            assert!(!is_quit_shortcut(control_mod));
        } else {
            assert!(is_quit_shortcut(control_mod));
            assert!(!is_quit_shortcut(super_mod));
        }
        assert!(!is_quit_shortcut(modifiers_off));
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

    #[test]
    fn non_focusing_panel_starts_at_bottom_center() {
        let position = bottom_centered_window_position(
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(2_560, 1_440),
            PhysicalSize::new(1_260, 728),
            24,
        );

        assert_eq!(position, PhysicalPosition::new(650, 688));
    }

    #[test]
    fn non_focusing_panel_position_supports_negative_monitor_origins() {
        let position = bottom_centered_window_position(
            PhysicalPosition::new(-1_920, 120),
            PhysicalSize::new(1_920, 1_080),
            PhysicalSize::new(900, 520),
            24,
        );

        assert_eq!(position, PhysicalPosition::new(-1_410, 656));
    }

    #[test]
    fn oversized_non_focusing_panel_anchors_to_monitor_origin() {
        let position = bottom_centered_window_position(
            PhysicalPosition::new(300, -200),
            PhysicalSize::new(800, 600),
            PhysicalSize::new(900, 700),
            24,
        );

        assert_eq!(position, PhysicalPosition::new(300, -200));
    }

    #[test]
    fn settings_window_centers_above_the_main_panel() {
        let position = settings_window_position(
            PhysicalPosition::new(510, 576),
            PhysicalSize::new(900, 480),
            PhysicalSize::new(520, 340),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1_920, 1_080),
            12,
            18,
        );

        assert_eq!(position, PhysicalPosition::new(700, 224));
    }

    #[test]
    fn settings_window_uses_below_anchor_then_monitor_center_fallback() {
        let below = settings_window_position(
            PhysicalPosition::new(200, 40),
            PhysicalSize::new(600, 120),
            PhysicalSize::new(420, 240),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1_000, 800),
            12,
            18,
        );
        let centered = settings_window_position(
            PhysicalPosition::new(200, 250),
            PhysicalSize::new(600, 300),
            PhysicalSize::new(420, 400),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1_000, 800),
            12,
            18,
        );

        assert_eq!(below, PhysicalPosition::new(290, 172));
        assert_eq!(centered, PhysicalPosition::new(290, 200));
    }

    #[test]
    fn settings_window_is_clamped_inside_negative_origin_monitor() {
        let position = settings_window_position(
            PhysicalPosition::new(-1_900, 80),
            PhysicalSize::new(500, 320),
            PhysicalSize::new(520, 340),
            PhysicalPosition::new(-1_920, 0),
            PhysicalSize::new(1_920, 1_080),
            12,
            18,
        );

        assert_eq!(position, PhysicalPosition::new(-1_902, 412));
    }
}
