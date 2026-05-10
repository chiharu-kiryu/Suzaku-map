use crate::ime::gpu::VoicePermissionState;

#[derive(Debug, Default)]
pub struct MacOsSpeechBridge;

impl MacOsSpeechBridge {
    pub fn new() -> Option<Self> {
        if !native_voice_enabled() {
            return None;
        }
        if !unsafe { suzaku_speech_is_bundled_host() } {
            return None;
        }
        if unsafe { suzaku_speech_is_supported() } {
            Some(Self)
        } else {
            None
        }
    }

    pub fn start(&self) -> bool {
        unsafe { suzaku_speech_start() }
    }

    pub fn request_permissions(&self) {
        unsafe { suzaku_speech_request_permissions() }
    }

    pub fn stop(&self) {
        unsafe { suzaku_speech_stop() }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        unsafe {
            match suzaku_speech_state() {
                1 | 2 => VoicePermissionState::Ready,
                3 => VoicePermissionState::Pending,
                4 => VoicePermissionState::Denied,
                5 => VoicePermissionState::Error,
                0 => VoicePermissionState::Unavailable,
                _ => VoicePermissionState::Unknown,
            }
        }
    }

    pub fn poll_transcript(&self) -> Option<String> {
        let mut buf = vec![0u8; 2048];
        let ok = unsafe { suzaku_speech_consume_transcript(buf.as_mut_ptr().cast(), buf.len()) };
        if !ok {
            return None;
        }
        let nul = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..nul].to_vec()).ok()
    }

    pub fn source_label(&self) -> &'static str {
        "Apple Speech"
    }

    pub fn supports_live_capture(&self) -> bool {
        true
    }
}

fn native_voice_enabled() -> bool {
    std::env::var("SUZAKU_MACOS_VOICE_NATIVE")
        .map(|value| !matches!(value.trim(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(true)
}

unsafe extern "C" {
    fn suzaku_speech_is_bundled_host() -> bool;
    fn suzaku_speech_is_supported() -> bool;
    fn suzaku_speech_state() -> i32;
    fn suzaku_speech_request_permissions();
    fn suzaku_speech_start() -> bool;
    fn suzaku_speech_stop();
    fn suzaku_speech_consume_transcript(buffer: *mut std::ffi::c_char, capacity: usize) -> bool;
}
