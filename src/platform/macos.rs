use super::{PlatformCapabilities, PlatformSupport, SupportTier, TargetPlatform};

pub fn support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::MacOs,
        tier: SupportTier::Primary,
        capabilities: PlatformCapabilities {
            window_host: true,
            gpu_panel: true,
            voice_input: true,
            handwriting_input: true,
            permission_bridge: true,
        },
    }
}
