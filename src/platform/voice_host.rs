use crate::ime::gpu::{VoiceCaptureState, VoicePermissionState};

use crate::platform::fallback_voice::FallbackSpeechBridge;
#[cfg(target_os = "macos")]
use crate::platform::macos_voice::MacOsSpeechBridge;
#[cfg(target_os = "windows")]
use crate::platform::windows_voice::WindowsSpeechBridge;

pub fn voice_transcript_placeholder(
    permission: VoicePermissionState,
    backend_label: &str,
    supports_live_capture: bool,
) -> String {
    #[cfg(target_os = "windows")]
    if permission == VoicePermissionState::Pending {
        return format!("{backend_label} is preparing permission and recognition state.");
    }

    match permission {
        VoicePermissionState::Pending => "Waiting for microphone and speech permission".to_string(),
        VoicePermissionState::Denied => {
            "Microphone or speech permission denied on this host".to_string()
        }
        VoicePermissionState::Error => {
            "Speech recognition hit an error. Stop and try again.".to_string()
        }
        VoicePermissionState::Unavailable => {
            "Speech framework unavailable. Using local fallback samples.".to_string()
        }
        _ => {
            if supports_live_capture {
                format!("Tap Listen to capture a voice seed with {backend_label}.")
            } else {
                format!("{backend_label} is connected, but live capture is not available yet.")
            }
        }
    }
}

pub fn voice_status_text(
    voice_state: VoiceCaptureState,
    voice_permission: VoicePermissionState,
    has_transcript: bool,
    backend_label: &str,
) -> String {
    if voice_state == VoiceCaptureState::Listening {
        return format!("Voice listening · {backend_label}");
    }

    match voice_permission {
        VoicePermissionState::Pending => format!("Voice permission pending · {backend_label}"),
        VoicePermissionState::Denied => format!("Voice permission denied · {backend_label}"),
        VoicePermissionState::Error => format!("Voice recognition error · {backend_label}"),
        VoicePermissionState::Unavailable => format!("Voice fallback mode · {backend_label}"),
        VoicePermissionState::Ready => {
            if has_transcript {
                format!("Transcript ready · {backend_label}")
            } else {
                format!("Voice ready · {backend_label}")
            }
        }
        VoicePermissionState::Unknown => format!("Voice setup · {backend_label}"),
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum VoiceBackend {
    #[cfg(target_os = "macos")]
    MacOs(MacOsSpeechBridge),
    #[cfg(target_os = "windows")]
    Windows(WindowsSpeechBridge),
    Fallback(FallbackSpeechBridge),
}

pub struct HostSpeechRecognizer {
    backend: VoiceBackend,
}

impl HostSpeechRecognizer {
    pub fn new() -> Option<Self> {
        #[cfg(target_os = "macos")]
        {
            if let Some(bridge) = MacOsSpeechBridge::new() {
                return Some(Self {
                    backend: VoiceBackend::MacOs(bridge),
                });
            }
            return None;
        }

        #[cfg(target_os = "windows")]
        {
            return Some(Self {
                backend: VoiceBackend::Windows(WindowsSpeechBridge::new()),
            });
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return Some(Self {
                backend: VoiceBackend::Fallback(FallbackSpeechBridge::new()),
            });
        }
    }

    pub fn start(&self) -> bool {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.start(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.start(),
            VoiceBackend::Fallback(bridge) => bridge.start(),
        }
    }

    pub fn request_permissions(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.request_permissions(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.request_permissions(),
            VoiceBackend::Fallback(bridge) => bridge.request_permissions(),
        }
    }

    pub fn stop(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.stop(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.stop(),
            VoiceBackend::Fallback(bridge) => bridge.stop(),
        }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.permission_state(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.permission_state(),
            VoiceBackend::Fallback(bridge) => bridge.permission_state(),
        }
    }

    pub fn poll_transcript(&self) -> Option<String> {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.poll_transcript(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.poll_transcript(),
            VoiceBackend::Fallback(bridge) => bridge.poll_transcript(),
        }
    }

    pub fn seed_debug_transcript_from_env(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(_) => {}
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.seed_debug_transcript_from_env(),
            VoiceBackend::Fallback(_) => {}
        }
    }

    pub fn source_label(&self) -> &'static str {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.source_label(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.source_label(),
            VoiceBackend::Fallback(bridge) => bridge.source_label(),
        }
    }

    pub fn supports_live_capture(&self) -> bool {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.supports_live_capture(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.supports_live_capture(),
            VoiceBackend::Fallback(bridge) => bridge.supports_live_capture(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HostSpeechRecognizer, VoiceBackend, voice_status_text, voice_transcript_placeholder,
    };
    use crate::ime::gpu::{VoiceCaptureState, VoicePermissionState};

    #[test]
    fn fallback_backend_reports_no_live_capture() {
        let host = HostSpeechRecognizer {
            backend: VoiceBackend::Fallback(
                crate::platform::fallback_voice::FallbackSpeechBridge::new(),
            ),
        };

        assert!(!host.supports_live_capture());
        assert_eq!(host.source_label(), "Fallback Samples");
    }

    #[test]
    fn placeholder_mentions_backend_when_live_capture_is_missing() {
        let text = voice_transcript_placeholder(
            VoicePermissionState::Ready,
            "Windows Native Speech",
            false,
        );

        assert!(text.contains("Windows Native Speech"));
        assert!(text.contains("not available yet"));
    }

    #[test]
    fn status_text_includes_backend_label() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Ready,
            false,
            "Apple Speech",
        );

        assert!(text.contains("Voice ready"));
        assert!(text.contains("Apple Speech"));
    }
}
