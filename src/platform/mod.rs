//! Platform capability matrix and host/companion adapters.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetPlatform {
    MacOs,
    Windows,
    Android,
    Ubuntu,
    ArchLinux,
    SteamOs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportTier {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub window_host: bool,
    pub gpu_panel: bool,
    pub system_ime_host: bool,
    pub voice_input: bool,
    pub handwriting_input: bool,
    pub permission_bridge: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformSupport {
    pub platform: TargetPlatform,
    pub tier: SupportTier,
    pub capabilities: PlatformCapabilities,
}

pub fn host_platform() -> TargetPlatform {
    #[cfg(target_os = "macos")]
    {
        return TargetPlatform::MacOs;
    }
    #[cfg(target_os = "windows")]
    {
        return TargetPlatform::Windows;
    }
    #[cfg(target_os = "linux")]
    {
        return TargetPlatform::Ubuntu;
    }
    #[allow(unreachable_code)]
    TargetPlatform::Ubuntu
}

pub fn support_for(platform: TargetPlatform) -> PlatformSupport {
    match platform {
        TargetPlatform::MacOs => macos::support_profile(),
        TargetPlatform::Windows => windows::support_profile(),
        TargetPlatform::Android => android::support_profile(),
        TargetPlatform::Ubuntu => linux::ubuntu_support_profile(),
        TargetPlatform::ArchLinux => linux::arch_support_profile(),
        TargetPlatform::SteamOs => linux::steamos_support_profile(),
    }
}

pub mod android;
pub mod android_ime;
pub mod android_jni_bridge;
pub mod companion_style;
pub mod fallback_voice;
pub mod gpu_host;
pub mod ime_host_adapter;
pub mod ime_host_dispatch;
pub mod ime_host_runtime;
pub mod linux;
#[cfg(target_os = "linux")]
#[path = "linux_ime_command.rs"]
pub mod linux_command;
#[cfg(all(target_os = "linux", feature = "linux-ibus"))]
pub mod linux_ibus_host;
pub mod linux_ime;
#[cfg(target_os = "linux")]
pub mod linux_ime_control;
#[cfg(target_os = "linux")]
pub mod linux_ime_sync;
#[cfg(target_os = "linux")]
pub mod linux_ipc;
pub mod linux_voice;
pub mod macos;
pub mod macos_ime;
#[cfg(target_os = "macos")]
pub mod macos_voice;
pub mod panel_companion_dispatch;
#[cfg(feature = "gpu")]
pub mod panel_text_focus;
pub mod settings_host;
#[cfg(test)]
mod test_env;
pub mod text_output_host;
pub mod ubuntu_voice;
pub mod voice_host;
pub mod windows;
pub mod windows_ime;
pub mod windows_voice;

pub const PLATFORM_SUPPORT_ROADMAP: [PlatformSupport; 6] = [
    support_for_const(TargetPlatform::MacOs),
    support_for_const(TargetPlatform::Windows),
    support_for_const(TargetPlatform::Android),
    support_for_const(TargetPlatform::Ubuntu),
    support_for_const(TargetPlatform::ArchLinux),
    support_for_const(TargetPlatform::SteamOs),
];

#[cfg(test)]
mod platform_tests {
    use super::{
        PLATFORM_SUPPORT_ROADMAP, SupportTier, TargetPlatform, host_platform, support_for,
        support_for_const,
    };

    #[test]
    fn host_platform_returns_a_supported_target() {
        let platform = host_platform();
        assert!(matches!(
            platform,
            TargetPlatform::MacOs
                | TargetPlatform::Windows
                | TargetPlatform::Ubuntu
                | TargetPlatform::ArchLinux
                | TargetPlatform::SteamOs
                | TargetPlatform::Android
        ));
    }

    #[test]
    fn support_for_matches_const_profiles_for_all_roadmap_entries() {
        for support in PLATFORM_SUPPORT_ROADMAP {
            assert_eq!(support, support_for_const(support.platform));
            assert_eq!(support_for(support.platform), support);
        }
    }

    #[test]
    fn support_for_matches_const_profiles_for_known_targets() {
        for support in PLATFORM_SUPPORT_ROADMAP {
            assert_eq!(support_for(support.platform), support);
        }
    }

    #[test]
    fn linux_profiles_are_secondary_tier() {
        let ubuntu = support_for(TargetPlatform::Ubuntu);
        let arch = support_for(TargetPlatform::ArchLinux);
        let steam = support_for(TargetPlatform::SteamOs);

        assert_eq!(ubuntu.tier, SupportTier::Secondary);
        assert_eq!(arch.tier, SupportTier::Secondary);
        assert_eq!(steam.tier, SupportTier::Secondary);
    }

    #[test]
    fn support_roadmap_keeps_expected_platform_order() {
        let expected = [
            TargetPlatform::MacOs,
            TargetPlatform::Windows,
            TargetPlatform::Android,
            TargetPlatform::Ubuntu,
            TargetPlatform::ArchLinux,
            TargetPlatform::SteamOs,
        ];

        for (index, platform) in expected.iter().enumerate() {
            assert_eq!(PLATFORM_SUPPORT_ROADMAP[index].platform, *platform);
        }
        assert_eq!(PLATFORM_SUPPORT_ROADMAP.len(), expected.len());
    }

    #[test]
    fn support_profile_marked_as_readable() {
        let windows = support_for_const(TargetPlatform::Windows);
        assert!(windows.capabilities.voice_input);
        assert!(windows.capabilities.permission_bridge);
    }
}

const fn support_for_const(platform: TargetPlatform) -> PlatformSupport {
    match platform {
        TargetPlatform::MacOs => PlatformSupport {
            platform: TargetPlatform::MacOs,
            tier: SupportTier::Primary,
            capabilities: PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        },
        TargetPlatform::Windows => PlatformSupport {
            platform: TargetPlatform::Windows,
            tier: SupportTier::Primary,
            capabilities: PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        },
        TargetPlatform::Android => PlatformSupport {
            platform: TargetPlatform::Android,
            tier: SupportTier::Secondary,
            capabilities: PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        },
        TargetPlatform::Ubuntu => PlatformSupport {
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
        },
        TargetPlatform::ArchLinux => PlatformSupport {
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
        },
        TargetPlatform::SteamOs => PlatformSupport {
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
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PLATFORM_SUPPORT_ROADMAP, PlatformCapabilities, SupportTier, TargetPlatform, host_platform,
        support_for,
    };

    #[test]
    fn roadmap_prioritizes_macos_and_windows_first() {
        assert_eq!(PLATFORM_SUPPORT_ROADMAP[0].platform, TargetPlatform::MacOs);
        assert_eq!(
            PLATFORM_SUPPORT_ROADMAP[1].platform,
            TargetPlatform::Windows
        );
        assert_eq!(PLATFORM_SUPPORT_ROADMAP[0].tier, SupportTier::Primary);
        assert_eq!(PLATFORM_SUPPORT_ROADMAP[1].tier, SupportTier::Primary);
    }

    #[test]
    fn linux_targets_remain_in_secondary_rollout() {
        for target in [
            TargetPlatform::Android,
            TargetPlatform::Ubuntu,
            TargetPlatform::ArchLinux,
            TargetPlatform::SteamOs,
        ] {
            assert_eq!(support_for(target).tier, SupportTier::Secondary);
        }
    }

    #[test]
    fn host_platform_is_present_in_roadmap() {
        let host = host_platform();

        assert_eq!(support_for(host).platform, host);
    }

    #[test]
    fn support_profile_tier_distribution_has_primary_and_secondary() {
        let mut has_primary = false;
        let mut has_secondary = false;

        for support in &PLATFORM_SUPPORT_ROADMAP {
            match support.tier {
                SupportTier::Primary => has_primary = true,
                SupportTier::Secondary => has_secondary = true,
            }
        }

        assert!(has_primary);
        assert!(has_secondary);
    }

    fn support_matches_expected(
        actual: super::PlatformSupport,
        expected_platform: TargetPlatform,
        expected_tier: SupportTier,
        expected_capabilities: PlatformCapabilities,
    ) {
        assert_eq!(actual.platform, expected_platform);
        assert_eq!(actual.tier, expected_tier);
        assert_eq!(actual.capabilities, expected_capabilities);
    }

    #[test]
    fn support_profiles_define_expected_platform_capabilities() {
        support_matches_expected(
            support_for(TargetPlatform::MacOs),
            TargetPlatform::MacOs,
            SupportTier::Primary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        );
        support_matches_expected(
            support_for(TargetPlatform::Windows),
            TargetPlatform::Windows,
            SupportTier::Primary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        );
        support_matches_expected(
            support_for(TargetPlatform::Android),
            TargetPlatform::Android,
            SupportTier::Secondary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: true,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: true,
            },
        );
        support_matches_expected(
            support_for(TargetPlatform::Ubuntu),
            TargetPlatform::Ubuntu,
            SupportTier::Secondary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: false,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: false,
            },
        );
        support_matches_expected(
            support_for(TargetPlatform::ArchLinux),
            TargetPlatform::ArchLinux,
            SupportTier::Secondary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: false,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: false,
            },
        );
        support_matches_expected(
            support_for(TargetPlatform::SteamOs),
            TargetPlatform::SteamOs,
            SupportTier::Secondary,
            PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: false,
                voice_input: true,
                handwriting_input: true,
                permission_bridge: false,
            },
        );
    }
}
