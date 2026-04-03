use crate::ime::gpu::{FontFaceChoice, VoicePermissionState};

use super::{PlatformCapabilities, PlatformSupport, SupportTier, TargetPlatform};

pub fn support_profile() -> PlatformSupport {
    PlatformSupport {
        platform: TargetPlatform::Windows,
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

pub fn preferred_font_paths(font_face: FontFaceChoice) -> Vec<(&'static str, &'static str)> {
    match font_face {
        FontFaceChoice::Auto => vec![
            ("C:\\Windows\\Fonts\\consola.ttf", "Consolas"),
            ("C:\\Windows\\Fonts\\segoeui.ttf", "Segoe UI"),
            ("C:\\Windows\\Fonts\\arialuni.ttf", "Arial Unicode"),
        ],
        FontFaceChoice::Monaco => vec![("C:\\Windows\\Fonts\\consola.ttf", "Consolas")],
        FontFaceChoice::Geneva => vec![("C:\\Windows\\Fonts\\segoeui.ttf", "Segoe UI")],
        FontFaceChoice::ArialUnicode => {
            vec![("C:\\Windows\\Fonts\\arialuni.ttf", "Arial Unicode")]
        }
    }
}

pub fn settings_directory_name() -> &'static str {
    "SuzakuPanel"
}

pub fn voice_permission_stub_state() -> VoicePermissionState {
    VoicePermissionState::Pending
}

pub fn voice_bridge_label() -> &'static str {
    "Windows Speech API"
}

#[cfg(test)]
mod tests {
    use super::{
        preferred_font_paths, settings_directory_name, voice_bridge_label,
        voice_permission_stub_state,
    };
    use crate::ime::gpu::{FontFaceChoice, VoicePermissionState};

    #[test]
    fn windows_auto_fonts_prioritize_consolas_then_segoe() {
        let fonts = preferred_font_paths(FontFaceChoice::Auto);

        assert_eq!(fonts[0].1, "Consolas");
        assert_eq!(fonts[1].1, "Segoe UI");
    }

    #[test]
    fn windows_settings_directory_name_is_stable() {
        assert_eq!(settings_directory_name(), "SuzakuPanel");
    }

    #[test]
    fn windows_voice_bridge_is_marked_pending_for_now() {
        assert_eq!(voice_permission_stub_state(), VoicePermissionState::Pending);
        assert_eq!(voice_bridge_label(), "Windows Speech API");
    }
}
