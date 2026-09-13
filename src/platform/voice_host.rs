use crate::ime::gpu::{VoiceCaptureState, VoicePermissionState};

use crate::platform::fallback_voice::FallbackSpeechBridge;
#[cfg(target_os = "linux")]
use crate::platform::linux_voice::LinuxSpeechBridge;
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
        VoicePermissionState::Pending => {
            "Enable Speech Recognition and Microphone access in System Settings first.".to_string()
        }
        VoicePermissionState::Denied => "Microphone or speech access was denied.".to_string(),
        VoicePermissionState::Error => {
            "Speech recognition hit an error. Try Listen again.".to_string()
        }
        VoicePermissionState::Unavailable => {
            "Live voice capture is unavailable. Use sample voice text instead.".to_string()
        }
        _ => {
            if supports_live_capture {
                format!("Tap Listen, speak, then Use Seed with {backend_label}.")
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
        VoicePermissionState::Pending => {
            format!("Voice permission required in System Settings · {backend_label}")
        }
        VoicePermissionState::Denied => "denied".to_string(),
        VoicePermissionState::Error => format!("Voice recognition error · {backend_label}"),
        VoicePermissionState::Unavailable => format!("Voice fallback mode · {backend_label}"),
        VoicePermissionState::Ready => {
            if has_transcript {
                format!("Transcript captured · tap Use Seed · {backend_label}")
            } else {
                format!("Voice ready · {backend_label}")
            }
        }
        VoicePermissionState::Unknown => format!("Voice setup · {backend_label}"),
    }
}

pub fn open_voice_permission_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        return Command::new("open")
            .arg(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition",
            )
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        return Command::new("cmd")
            .args(["/C", "start", "ms-settings:privacy-microphone"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }

    #[cfg(target_os = "linux")]
    {
        return false;
    }

    #[allow(unreachable_code)]
    false
}

#[derive(Debug)]
#[allow(dead_code)]
enum VoiceBackend {
    #[cfg(target_os = "macos")]
    MacOs(MacOsSpeechBridge),
    #[cfg(target_os = "windows")]
    Windows(WindowsSpeechBridge),
    #[cfg(target_os = "linux")]
    Linux(LinuxSpeechBridge),
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

        #[cfg(target_os = "linux")]
        {
            Some(Self {
                backend: VoiceBackend::Linux(LinuxSpeechBridge::new()),
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
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
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.start(),
            VoiceBackend::Fallback(bridge) => bridge.start(),
        }
    }

    pub fn request_permissions(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.request_permissions(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.request_permissions(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.request_permissions(),
            VoiceBackend::Fallback(bridge) => bridge.request_permissions(),
        }
    }

    pub fn stop(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.stop(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.stop(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.stop(),
            VoiceBackend::Fallback(bridge) => bridge.stop(),
        }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.permission_state(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.permission_state(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.permission_state(),
            VoiceBackend::Fallback(bridge) => bridge.permission_state(),
        }
    }

    pub fn poll_transcript(&self) -> Option<String> {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.poll_transcript(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.poll_transcript(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.poll_transcript(),
            VoiceBackend::Fallback(bridge) => bridge.poll_transcript(),
        }
    }

    pub fn seed_debug_transcript_from_env(&self) {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(_) => {}
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.seed_debug_transcript_from_env(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.seed_debug_transcript_from_env(),
            VoiceBackend::Fallback(_) => {}
        }
    }

    pub fn source_label(&self) -> &'static str {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.source_label(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.source_label(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.source_label(),
            VoiceBackend::Fallback(bridge) => bridge.source_label(),
        }
    }

    pub fn supports_live_capture(&self) -> bool {
        match &self.backend {
            #[cfg(target_os = "macos")]
            VoiceBackend::MacOs(bridge) => bridge.supports_live_capture(),
            #[cfg(target_os = "windows")]
            VoiceBackend::Windows(bridge) => bridge.supports_live_capture(),
            #[cfg(target_os = "linux")]
            VoiceBackend::Linux(bridge) => bridge.supports_live_capture(),
            VoiceBackend::Fallback(bridge) => bridge.supports_live_capture(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn macos_speech_bridge_has_no_transcript_logging_sinks() {
        let source = include_str!("../macos/speech_bridge.m");
        // Source-level privacy guard, runnable on every host (not a macOS runtime test).
        for sink in [
            "suzaku_voice_log",
            "NSLog",
            "os_log",
            "printf(",
            "writeToFile:",
            "writeData:",
            "fileHandleForWritingAtPath:",
        ] {
            assert!(
                !source.contains(sink),
                "speech bridge must not log private input: {sink}"
            );
        }
    }

    use super::{
        HostSpeechRecognizer, VoiceBackend, open_voice_permission_settings, voice_status_text,
        voice_transcript_placeholder,
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
    fn fallback_backend_wrapper_methods_reflect_fallback_state() {
        let host = HostSpeechRecognizer {
            backend: VoiceBackend::Fallback(
                crate::platform::fallback_voice::FallbackSpeechBridge::new(),
            ),
        };

        assert!(!host.start());
        host.request_permissions();
        host.stop();
        host.seed_debug_transcript_from_env();
        assert_eq!(host.permission_state(), VoicePermissionState::Unavailable);
        assert_eq!(host.source_label(), "Fallback Samples");
        assert!(host.poll_transcript().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn open_voice_permission_settings_is_disabled_in_linux_host() {
        assert!(!open_voice_permission_settings());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn open_voice_permission_settings_is_callable_on_macos_without_panicking() {
        let _ = open_voice_permission_settings();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn open_voice_permission_settings_is_callable_on_windows_without_panicking() {
        let _ = open_voice_permission_settings();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn placeholder_mentions_windows_permission_preparation_state() {
        let text = voice_transcript_placeholder(
            VoicePermissionState::Pending,
            "Windows Native Speech",
            true,
        );

        assert!(text.contains("preparing permission and recognition state"));
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
    fn placeholder_mentions_system_settings_for_pending_access() {
        let text =
            voice_transcript_placeholder(VoicePermissionState::Pending, "Apple Speech", true);

        assert!(text.contains("System Settings"));
        assert!(text.contains("Speech Recognition"));
    }

    #[test]
    fn placeholder_mentions_denied_access() {
        let text =
            voice_transcript_placeholder(VoicePermissionState::Denied, "Apple Speech", false);

        assert!(text.contains("denied"));
    }

    #[test]
    fn placeholder_mentions_error_state() {
        let text = voice_transcript_placeholder(VoicePermissionState::Error, "Apple Speech", false);

        assert!(text.contains("Try Listen"));
    }

    #[test]
    fn placeholder_mentions_unavailable_capture() {
        let text =
            voice_transcript_placeholder(VoicePermissionState::Unavailable, "Apple Speech", false);

        assert!(text.contains("Live voice capture is unavailable"));
    }

    #[test]
    fn placeholder_mentions_listen_flow_when_capture_is_available() {
        let text = voice_transcript_placeholder(VoicePermissionState::Ready, "Apple Speech", true);

        assert!(text.contains("Tap Listen"));
        assert!(text.contains("Use Seed"));
        assert!(text.contains("Apple Speech"));
    }

    #[test]
    fn status_for_listening_is_live_hint() {
        let text = voice_status_text(
            VoiceCaptureState::Listening,
            VoicePermissionState::Ready,
            false,
            "Apple Speech",
        );
        assert!(text.contains("listening"));
        assert!(text.contains("Apple Speech"));
    }

    #[test]
    fn status_for_listening_ignores_permission_and_transcript_state() {
        let text = voice_status_text(
            VoiceCaptureState::Listening,
            VoicePermissionState::Denied,
            true,
            "Apple Speech",
        );
        assert_eq!(text, "Voice listening · Apple Speech");
    }

    #[test]
    fn status_for_unknown_permission_mentions_setup_hint() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Unknown,
            false,
            "Apple Speech",
        );
        assert!(text.contains("setup"));
        assert!(text.contains("Apple Speech"));
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

    #[test]
    fn status_text_ready_without_transcript_uses_ready_copy() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Ready,
            false,
            "Fallback Samples",
        );
        assert_eq!(text, "Voice ready · Fallback Samples");
    }

    #[test]
    fn status_text_requires_permission() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Pending,
            false,
            "Apple Speech",
        );

        assert!(text.contains("permission required"));
        assert!(text.contains("Apple Speech"));
    }

    #[test]
    fn status_text_handles_denied_state() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Denied,
            false,
            "Apple Speech",
        );

        assert_eq!(text, "denied");
    }

    #[test]
    fn status_text_handles_error_state() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Error,
            false,
            "Apple Speech",
        );

        assert!(text.contains("error"));
        assert!(text.contains("Apple Speech"));
    }

    #[test]
    fn status_text_handles_unavailable_state() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Unavailable,
            false,
            "Apple Speech",
        );

        assert!(text.contains("fallback"));
        assert!(text.contains("Apple Speech"));
    }

    #[test]
    fn status_text_highlights_use_seed_when_transcript_exists() {
        let text = voice_status_text(
            VoiceCaptureState::Idle,
            VoicePermissionState::Ready,
            true,
            "Apple Speech",
        );

        assert!(text.contains("Use Seed"));
    }
}
