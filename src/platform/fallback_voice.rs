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
