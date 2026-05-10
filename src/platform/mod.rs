#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetPlatform {
    MacOs,
    Windows,
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
    #[cfg(all(target_os = "linux", not(any())))]
    {
        return TargetPlatform::Ubuntu;
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
        TargetPlatform::Ubuntu => linux::ubuntu_support_profile(),
        TargetPlatform::ArchLinux => linux::arch_support_profile(),
        TargetPlatform::SteamOs => linux::steamos_support_profile(),
    }
}

pub mod fallback_voice;
pub mod gpu_host;
pub mod ime_host_dispatch;
pub mod linux;
pub mod linux_ime;
pub mod linux_voice;
pub mod macos;
pub mod macos_ime;
#[cfg(target_os = "macos")]
pub mod macos_voice;
pub mod settings_host;
pub mod text_output_host;
pub mod ubuntu_voice;
pub mod voice_host;
pub mod windows;
pub mod windows_ime;
pub mod windows_voice;

pub const PLATFORM_SUPPORT_ROADMAP: [PlatformSupport; 5] = [
    support_for_const(TargetPlatform::MacOs),
    support_for_const(TargetPlatform::Windows),
    support_for_const(TargetPlatform::Ubuntu),
    support_for_const(TargetPlatform::ArchLinux),
    support_for_const(TargetPlatform::SteamOs),
];

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
        TargetPlatform::Ubuntu => PlatformSupport {
            platform: TargetPlatform::Ubuntu,
            tier: SupportTier::Secondary,
            capabilities: PlatformCapabilities {
                window_host: true,
                gpu_panel: true,
                system_ime_host: false,
                voice_input: false,
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
                voice_input: false,
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
                voice_input: false,
                handwriting_input: true,
                permission_bridge: false,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PLATFORM_SUPPORT_ROADMAP, SupportTier, TargetPlatform, host_platform, support_for,
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
}
