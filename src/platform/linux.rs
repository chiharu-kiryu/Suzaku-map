use super::{PlatformCapabilities, PlatformSupport, SupportTier, TargetPlatform};

pub fn ubuntu_support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::Ubuntu,
        tier: SupportTier::Secondary,
        capabilities: PlatformCapabilities {
            window_host: true,
            gpu_panel: true,
            voice_input: false,
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
            voice_input: false,
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
            voice_input: false,
            handwriting_input: true,
            permission_bridge: false,
        },
    }
}
