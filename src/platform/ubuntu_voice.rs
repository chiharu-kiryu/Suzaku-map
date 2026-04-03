use crate::ime::gpu::VoicePermissionState;
use crate::platform::linux;

#[derive(Debug, Clone, Default)]
pub struct UbuntuSpeechBridge {
    active: std::sync::Arc<std::sync::Mutex<bool>>,
    queued_transcript: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl UbuntuSpeechBridge {
    pub fn new() -> Self {
        Self {
            active: std::sync::Arc::new(std::sync::Mutex::new(false)),
            queued_transcript: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn start(&self) -> bool {
        *self.active.lock().expect("ubuntu voice active") = true;
        self.seed_debug_transcript_from_env();
        true
    }

    pub fn request_permissions(&self) {}

    pub fn stop(&self) {
        *self.active.lock().expect("ubuntu voice active") = false;
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        if std::env::var("SUZAKU_LINUX_VOICE_FORCE_DENIED")
            .map(|value| value == "1")
            .unwrap_or(false)
        {
            return VoicePermissionState::Denied;
        }
        if std::env::var("SUZAKU_LINUX_VOICE_FORCE_ERROR")
            .map(|value| value == "1")
            .unwrap_or(false)
        {
            return VoicePermissionState::Error;
        }
        let portal = linux::linux_voice_portal_available();
        let pipewire = linux::linux_voice_pipewire_available();
        if portal && pipewire {
            VoicePermissionState::Ready
        } else {
            VoicePermissionState::Pending
        }
    }

    pub fn poll_transcript(&self) -> Option<String> {
        self.queued_transcript
            .lock()
            .expect("ubuntu voice transcript")
            .take()
    }

    pub fn seed_debug_transcript_from_env(&self) {
        let mut transcript = self
            .queued_transcript
            .lock()
            .expect("ubuntu voice transcript");
        if transcript.is_none() {
            for key in linux::linux_voice_sample_env_keys() {
                if let Ok(value) = std::env::var(key) {
                    *transcript = Some(value);
                    break;
                }
            }
        }
    }

    pub fn source_label(&self) -> &'static str {
        linux::linux_voice_backend_label()
    }

    pub fn supports_live_capture(&self) -> bool {
        linux::linux_voice_portal_available() && linux::linux_voice_pipewire_available()
    }
}

#[cfg(test)]
mod tests {
    use super::UbuntuSpeechBridge;
    use crate::ime::gpu::VoicePermissionState;

    #[test]
    fn ubuntu_bridge_defaults_to_ready_permission() {
        let bridge = UbuntuSpeechBridge::new();
        unsafe {
            std::env::set_var("SUZAKU_LINUX_PORTAL_AVAILABLE", "1");
            std::env::set_var("SUZAKU_LINUX_PIPEWIRE_AVAILABLE", "1");
        }

        assert_eq!(bridge.permission_state(), VoicePermissionState::Ready);
        assert!(bridge.supports_live_capture());

        unsafe {
            std::env::remove_var("SUZAKU_LINUX_PORTAL_AVAILABLE");
            std::env::remove_var("SUZAKU_LINUX_PIPEWIRE_AVAILABLE");
        }
    }

    #[test]
    fn ubuntu_bridge_can_seed_debug_transcript_from_env() {
        let bridge = UbuntuSpeechBridge::new();
        unsafe {
            std::env::set_var("SUZAKU_LINUX_VOICE_SAMPLE", "hello from ubuntu");
        }

        bridge.seed_debug_transcript_from_env();

        assert_eq!(
            bridge.poll_transcript().as_deref(),
            Some("hello from ubuntu")
        );
        unsafe {
            std::env::remove_var("SUZAKU_LINUX_VOICE_SAMPLE");
        }
    }

    #[test]
    fn ubuntu_bridge_reports_pending_without_portal_and_pipewire() {
        let bridge = UbuntuSpeechBridge::new();
        unsafe {
            std::env::remove_var("SUZAKU_LINUX_PORTAL_AVAILABLE");
            std::env::remove_var("SUZAKU_LINUX_PIPEWIRE_AVAILABLE");
        }

        assert_eq!(bridge.permission_state(), VoicePermissionState::Pending);
        assert!(!bridge.supports_live_capture());
    }
}
