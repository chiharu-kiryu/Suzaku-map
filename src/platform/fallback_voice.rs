use crate::ime::gpu::VoicePermissionState;

#[derive(Debug, Default)]
pub struct FallbackSpeechBridge;

impl FallbackSpeechBridge {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self) -> bool {
        false
    }

    pub fn request_permissions(&self) {}

    pub fn stop(&self) {}

    pub fn permission_state(&self) -> VoicePermissionState {
        VoicePermissionState::Unavailable
    }

    pub fn poll_transcript(&self) -> Option<String> {
        None
    }

    pub fn source_label(&self) -> &'static str {
        "Fallback Samples"
    }

    pub fn supports_live_capture(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{FallbackSpeechBridge, VoicePermissionState};

    #[test]
    fn fallback_bridge_is_always_unavailable() {
        let bridge = FallbackSpeechBridge::new();

        assert!(!bridge.start());
        assert_eq!(bridge.permission_state(), VoicePermissionState::Unavailable);
        assert_eq!(bridge.source_label(), "Fallback Samples");
        assert!(!bridge.supports_live_capture());
    }

    #[test]
    fn fallback_bridge_polling_is_safe_and_empty() {
        let bridge = FallbackSpeechBridge::new();

        bridge.request_permissions();
        bridge.stop();
        assert!(bridge.poll_transcript().is_none());
    }
}
