use crate::ime::gpu::{FontFaceChoice, VoiceCaptureState, VoicePermissionState};
use winit::event_loop::EventLoopBuilder;
use winit::keyboard::ModifiersState;
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
        return match font_face {
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
        };
    }

    #[cfg(target_os = "linux")]
    {
        return match font_face {
            FontFaceChoice::Auto => vec![
                ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", "DejaVu Sans Mono"),
                ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "DejaVu Sans"),
                ("/usr/share/fonts/noto/NotoSansCJK-Regular.ttc", "Noto Sans CJK"),
            ],
            FontFaceChoice::Monaco => {
                vec![("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", "DejaVu Sans Mono")]
            }
            FontFaceChoice::Geneva => {
                vec![("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "DejaVu Sans")]
            }
            FontFaceChoice::ArialUnicode => {
                vec![("/usr/share/fonts/noto/NotoSansCJK-Regular.ttc", "Noto Sans CJK")]
            }
        };
    }

    #[allow(unreachable_code)]
    Vec::new()
}

pub fn voice_transcript_placeholder(permission: VoicePermissionState) -> &'static str {
    match permission {
        VoicePermissionState::Pending => "Waiting for microphone and speech permission",
        VoicePermissionState::Denied => "Microphone or speech permission denied on this host",
        VoicePermissionState::Error => "Speech recognition hit an error. Stop and try again.",
        VoicePermissionState::Unavailable => {
            "Speech framework unavailable. Using local fallback samples."
        }
        _ => "Tap Listen to capture a voice seed",
    }
}

pub fn voice_status_text(
    voice_state: VoiceCaptureState,
    voice_permission: VoicePermissionState,
    has_transcript: bool,
) -> &'static str {
    if voice_state == VoiceCaptureState::Listening {
        return "Voice listening";
    }

    match voice_permission {
        VoicePermissionState::Pending => "Voice permission pending",
        VoicePermissionState::Denied => "Voice permission denied",
        VoicePermissionState::Error => "Voice recognition error",
        VoicePermissionState::Unavailable => "Voice fallback mode",
        VoicePermissionState::Ready => {
            if has_transcript {
                "Transcript ready"
            } else {
                "Voice ready"
            }
        }
        VoicePermissionState::Unknown => "Voice setup",
    }
}

pub struct HostSpeechRecognizer;

impl HostSpeechRecognizer {
    pub fn new() -> Option<Self> {
        #[cfg(target_os = "macos")]
        {
            if unsafe { suzaku_speech_is_supported() } {
                return Some(Self);
            }
        }

        None
    }

    pub fn start(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            unsafe { return suzaku_speech_start() };
        }

        #[cfg(not(target_os = "macos"))]
        false
    }

    pub fn request_permissions(&self) {
        #[cfg(target_os = "macos")]
        unsafe {
            suzaku_speech_request_permissions()
        }
    }

    pub fn stop(&self) {
        #[cfg(target_os = "macos")]
        unsafe {
            suzaku_speech_stop()
        }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                return match suzaku_speech_state() {
                1 | 2 => VoicePermissionState::Ready,
                3 => VoicePermissionState::Pending,
                4 => VoicePermissionState::Denied,
                5 => VoicePermissionState::Error,
                0 => VoicePermissionState::Unavailable,
                _ => VoicePermissionState::Unknown,
                };
            }
        }

        #[cfg(not(target_os = "macos"))]
        VoicePermissionState::Unavailable
    }

    pub fn poll_transcript(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            let mut buf = vec![0u8; 2048];
            let ok =
                unsafe { suzaku_speech_consume_transcript(buf.as_mut_ptr().cast(), buf.len()) };
            if !ok {
                return None;
            }
            let nul = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
            return String::from_utf8(buf[..nul].to_vec()).ok();
        }

        #[cfg(not(target_os = "macos"))]
        None
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn suzaku_speech_is_supported() -> bool;
    fn suzaku_speech_state() -> i32;
    fn suzaku_speech_request_permissions();
    fn suzaku_speech_start() -> bool;
    fn suzaku_speech_stop();
    fn suzaku_speech_consume_transcript(buffer: *mut std::ffi::c_char, capacity: usize) -> bool;
}
