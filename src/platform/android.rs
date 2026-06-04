use crate::ime::gpu::FontFaceChoice;

use super::{PlatformCapabilities, PlatformSupport, SupportTier, TargetPlatform};

pub fn support_profile() -> PlatformSupport {
    PlatformSupport {
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
    }
}

pub fn preferred_font_paths(font_face: FontFaceChoice) -> Vec<(&'static str, &'static str)> {
    match font_face {
        FontFaceChoice::Auto => vec![
            ("/system/fonts/Roboto-Regular.ttf", "Roboto"),
            ("/system/fonts/NotoSans-Regular.ttf", "Noto Sans"),
            ("/system/fonts/NotoSansCJK-Regular.ttc", "Noto Sans CJK"),
        ],
        FontFaceChoice::Monaco | FontFaceChoice::Menlo => {
            vec![("/system/fonts/RobotoMono-Regular.ttf", "Roboto Mono")]
        }
        FontFaceChoice::Geneva | FontFaceChoice::Helvetica => {
            vec![("/system/fonts/Roboto-Regular.ttf", "Roboto")]
        }
        FontFaceChoice::PingFang | FontFaceChoice::ArialUnicode => {
            vec![("/system/fonts/NotoSansCJK-Regular.ttc", "Noto Sans CJK")]
        }
    }
}

pub fn settings_directory_name() -> &'static str {
    "SuzakuPanel"
}

#[cfg(test)]
mod tests {
    use super::{preferred_font_paths, settings_directory_name, support_profile};
    use crate::ime::gpu::FontFaceChoice;
    use crate::platform::TargetPlatform;

    #[test]
    fn android_support_profile_exposes_system_ime_host() {
        let support = support_profile();
        assert_eq!(support.platform, TargetPlatform::Android);
        assert!(support.capabilities.system_ime_host);
    }

    #[test]
    fn android_auto_fonts_prefer_roboto() {
        let fonts = preferred_font_paths(FontFaceChoice::Auto);
        assert_eq!(fonts[0].1, "Roboto");
    }

    #[test]
    fn android_settings_dir_is_stable() {
        assert_eq!(settings_directory_name(), "SuzakuPanel");
    }
}
