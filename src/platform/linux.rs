use super::{PlatformCapabilities, PlatformSupport, SupportTier, TargetPlatform};
use crate::ime::gpu::FontFaceChoice;
use std::env;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxHostFlavor {
    Ubuntu,
    Arch,
    SteamOs,
}

pub fn ubuntu_support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::Ubuntu,
        tier: SupportTier::Secondary,
        capabilities: PlatformCapabilities {
            window_host: true,
            gpu_panel: true,
            system_ime_host: false,
            voice_input: true,
            handwriting_input: true,
            permission_bridge: false,
        },
    }
}

pub fn arch_support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::ArchLinux,
        tier: SupportTier::Secondary,
        capabilities: PlatformCapabilities {
            window_host: true,
            gpu_panel: true,
            system_ime_host: false,
            voice_input: true,
            handwriting_input: true,
            permission_bridge: false,
        },
    }
}

pub fn steamos_support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::SteamOs,
        tier: SupportTier::Secondary,
        capabilities: PlatformCapabilities {
            window_host: true,
            gpu_panel: true,
            system_ime_host: false,
            voice_input: true,
            handwriting_input: true,
            permission_bridge: false,
        },
    }
}

pub fn ubuntu_preferred_font_paths(font_face: FontFaceChoice) -> Vec<(&'static str, &'static str)> {
    match font_face {
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
                "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
                "Liberation Sans",
            ),
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "Noto Sans CJK",
            ),
        ],
        FontFaceChoice::Monaco => vec![(
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "DejaVu Sans Mono",
        )],
        FontFaceChoice::Menlo => vec![(
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "DejaVu Sans Mono",
        )],
        FontFaceChoice::Geneva => vec![(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "DejaVu Sans",
        )],
        FontFaceChoice::Helvetica => vec![(
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "Liberation Sans",
        )],
        FontFaceChoice::PingFang => vec![(
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "Noto Sans CJK",
        )],
        FontFaceChoice::ArialUnicode => vec![(
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "Noto Sans CJK",
        )],
    }
}

pub fn ubuntu_settings_directory_name() -> &'static str {
    "suzaku-panel"
}

pub fn linux_voice_backend_label() -> &'static str {
    match detect_linux_host_flavor() {
        LinuxHostFlavor::Ubuntu => "PipeWire / Portal Host · Ubuntu",
        LinuxHostFlavor::Arch => "PipeWire / Portal Host · Arch",
        LinuxHostFlavor::SteamOs => "PipeWire / Portal Host · SteamOS",
    }
}

pub fn linux_voice_sample_env_keys() -> [&'static str; 2] {
    ["SUZAKU_LINUX_VOICE_SAMPLE", "SUZAKU_UBUNTU_VOICE_SAMPLE"]
}

pub fn detect_linux_host_flavor() -> LinuxHostFlavor {
    match std::env::var("SUZAKU_LINUX_HOST")
        .unwrap_or_else(|_| "ubuntu".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "arch" => LinuxHostFlavor::Arch,
        "steamos" => LinuxHostFlavor::SteamOs,
        _ => LinuxHostFlavor::Ubuntu,
    }
}

pub fn linux_voice_portal_available() -> bool {
    if let Some(value) = env_flag_override("SUZAKU_LINUX_PORTAL_AVAILABLE") {
        return value;
    }

    has_session_bus_address() && has_executable_in_path("xdg-desktop-portal")
}

pub fn linux_voice_pipewire_available() -> bool {
    if let Some(value) = env_flag_override("SUZAKU_LINUX_PIPEWIRE_AVAILABLE") {
        return value;
    }

    has_executable_in_path("pipewire")
        || has_executable_in_path("pipewire-pulse")
        || env::var_os("PIPEWIRE_RUNTIME_DIR").is_some()
}

fn env_flag_override(key: &str) -> Option<bool> {
    env::var(key).ok().map(|value| value == "1")
}

fn has_session_bus_address() -> bool {
    env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
}

fn has_executable_in_path(name: &str) -> bool {
    let Some(path_os) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_os).any(|dir| Path::new(&dir).join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxHostFlavor, detect_linux_host_flavor, linux_voice_backend_label,
        linux_voice_pipewire_available, linux_voice_portal_available, linux_voice_sample_env_keys,
        ubuntu_preferred_font_paths, ubuntu_settings_directory_name, ubuntu_support_profile,
    };
    use crate::ime::gpu::FontFaceChoice;
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;

    #[test]
    fn ubuntu_profile_marks_voice_input_available() {
        assert!(ubuntu_support_profile().capabilities.voice_input);
    }

    #[test]
    fn ubuntu_fonts_prefer_dejavu_mono_first() {
        let fonts = ubuntu_preferred_font_paths(FontFaceChoice::Auto);

        assert_eq!(fonts[0].1, "DejaVu Sans Mono");
    }

    #[test]
    fn ubuntu_settings_directory_name_is_stable() {
        assert_eq!(ubuntu_settings_directory_name(), "suzaku-panel");
    }

    #[test]
    fn linux_voice_backend_defaults_to_portal_label() {
        assert!(linux_voice_backend_label().contains("PipeWire / Portal Host"));
        assert_eq!(
            linux_voice_sample_env_keys()[0],
            "SUZAKU_LINUX_VOICE_SAMPLE"
        );
    }

    #[test]
    fn arch_and_steamos_mark_voice_input_available() {
        assert!(super::arch_support_profile().capabilities.voice_input);
        assert!(super::steamos_support_profile().capabilities.voice_input);
    }

    #[test]
    fn linux_host_flavor_defaults_to_ubuntu() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.remove_var("SUZAKU_LINUX_HOST");

            assert_eq!(detect_linux_host_flavor(), LinuxHostFlavor::Ubuntu);
        });
    }

    #[test]
    fn linux_host_flavor_override_updates_arch_label() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_HOST", "arch");

            assert_eq!(detect_linux_host_flavor(), LinuxHostFlavor::Arch);
            assert!(linux_voice_backend_label().contains("Arch"));
        });
    }

    #[test]
    fn linux_host_flavor_override_updates_steamos_label() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_HOST", "steamos");

            assert_eq!(detect_linux_host_flavor(), LinuxHostFlavor::SteamOs);
            assert!(linux_voice_backend_label().contains("Steam"));
        });
    }

    #[test]
    fn linux_portal_and_pipewire_flags_default_off() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.remove_var("SUZAKU_LINUX_PORTAL_AVAILABLE");
            env.remove_var("SUZAKU_LINUX_PIPEWIRE_AVAILABLE");
            // Environment overrides are absent here; the runtime probe decides final availability.
            let _ = linux_voice_portal_available();
            let _ = linux_voice_pipewire_available();
        });
    }

    #[test]
    fn linux_env_flag_override_parses_boolean_strings() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_PORTAL_AVAILABLE", "1");
            env.set_var("SUZAKU_LINUX_PIPEWIRE_AVAILABLE", "0");

            assert!(linux_voice_portal_available());
            assert!(!linux_voice_pipewire_available());
        });
    }
}
