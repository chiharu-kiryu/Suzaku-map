use crate::ime::gpu::VoicePermissionState;

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub struct WindowsSpeechBridge;

#[cfg(target_os = "windows")]
impl WindowsSpeechBridge {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self) -> bool {
        unsafe { suzaku_windows_speech_start() }
    }

    pub fn request_permissions(&self) {
        unsafe { suzaku_windows_speech_request_permissions() }
    }

    pub fn stop(&self) {
        unsafe { suzaku_windows_speech_stop() }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        match unsafe { suzaku_windows_speech_state() } {
            1 | 2 => VoicePermissionState::Ready,
            3 => VoicePermissionState::Pending,
            4 => VoicePermissionState::Denied,
            5 => VoicePermissionState::Error,
            0 => VoicePermissionState::Unavailable,
            _ => VoicePermissionState::Unknown,
        }
    }

    pub fn poll_transcript(&self) -> Option<String> {
        let mut buf = vec![0u8; 2048];
        let ok =
            unsafe { suzaku_windows_speech_consume_transcript(buf.as_mut_ptr().cast(), buf.len()) };
        if !ok {
            return None;
        }
        let nul = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..nul].to_vec()).ok()
    }

    pub fn seed_debug_transcript_from_env(&self) {
        if let Ok(sample) = std::env::var("SUZAKU_WINDOWS_VOICE_SAMPLE") {
            let mut bytes = sample.into_bytes();
            bytes.retain(|byte| *byte != 0);
            bytes.push(0);
            unsafe {
                suzaku_windows_speech_debug_seed_transcript(bytes.as_ptr().cast());
            }
        }
    }

    pub fn debug_set_state(&self, state: VoicePermissionState) {
        let raw = match state {
            VoicePermissionState::Unavailable => 0,
            VoicePermissionState::Ready => 1,
            VoicePermissionState::Pending => 3,
            VoicePermissionState::Denied => 4,
            VoicePermissionState::Error => 5,
            VoicePermissionState::Unknown => 5,
        };
        unsafe {
            suzaku_windows_speech_debug_set_state(raw);
        }
    }

    pub fn source_label(&self) -> &'static str {
        "Windows Native Speech"
    }

    pub fn supports_live_capture(&self) -> bool {
        unsafe { suzaku_windows_speech_supports_live_capture() }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn suzaku_windows_speech_is_supported() -> bool;
    fn suzaku_windows_speech_supports_live_capture() -> bool;
    fn suzaku_windows_speech_state() -> i32;
    fn suzaku_windows_speech_request_permissions();
    fn suzaku_windows_speech_start() -> bool;
    fn suzaku_windows_speech_stop();
    fn suzaku_windows_speech_consume_transcript(
        buffer: *mut std::ffi::c_char,
        capacity: usize,
    ) -> bool;
    fn suzaku_windows_speech_debug_seed_transcript(text: *const std::ffi::c_char);
    fn suzaku_windows_speech_debug_set_state(state: i32);
}

#[cfg(not(target_os = "windows"))]
use std::sync::{Arc, Mutex};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone)]
pub struct WindowsSpeechBridge {
    state: Arc<Mutex<WindowsSpeechState>>,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug)]
struct WindowsSpeechState {
    permission: VoicePermissionState,
    active: bool,
    queued_transcript: Option<String>,
}

impl Default for WindowsSpeechBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "windows"))]
impl WindowsSpeechBridge {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowsSpeechState {
                permission: VoicePermissionState::Pending,
                active: false,
                queued_transcript: None,
            })),
        }
    }

    pub fn start(&self) -> bool {
        let mut state = self.state.lock().expect("windows speech state");
        if state.permission != VoicePermissionState::Ready {
            return false;
        }
        state.active = true;
        if state.queued_transcript.is_none() {
            state.queued_transcript = std::env::var("SUZAKU_WINDOWS_VOICE_SAMPLE").ok();
        }
        true
    }

    pub fn request_permissions(&self) {
        let mut state = self.state.lock().expect("windows speech state");
        if state.permission == VoicePermissionState::Pending {
            state.permission = VoicePermissionState::Ready;
        }
    }

    pub fn stop(&self) {
        let mut state = self.state.lock().expect("windows speech state");
        state.active = false;
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        self.state.lock().expect("windows speech state").permission
    }

    pub fn poll_transcript(&self) -> Option<String> {
        let mut state = self.state.lock().expect("windows speech state");
        state.queued_transcript.take()
    }

    pub fn seed_debug_transcript_from_env(&self) {
        let mut state = self.state.lock().expect("windows speech state");
        if state.queued_transcript.is_none() {
            state.queued_transcript = std::env::var("SUZAKU_WINDOWS_VOICE_SAMPLE").ok();
        }
    }

    pub fn debug_set_state(&self, state: VoicePermissionState) {
        let mut inner = self.state.lock().expect("windows speech state");
        inner.permission = state;
    }

    pub fn source_label(&self) -> &'static str {
        "Windows Native Speech"
    }

    pub fn supports_live_capture(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::WindowsSpeechBridge;
    use crate::ime::gpu::VoicePermissionState;
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;

    #[test]
    fn windows_bridge_supports_debug_formatting() {
        let bridge = WindowsSpeechBridge::new();

        assert!(format!("{bridge:?}").starts_with("WindowsSpeechBridge"));
    }

    #[test]
    fn windows_bridge_starts_in_permission_pending_state() {
        let bridge = WindowsSpeechBridge::new();

        assert_eq!(bridge.permission_state(), VoicePermissionState::Pending);
    }

    #[test]
    fn windows_bridge_becomes_ready_after_permission_request() {
        let bridge = WindowsSpeechBridge::new();

        bridge.request_permissions();

        assert_eq!(bridge.permission_state(), VoicePermissionState::Ready);
    }

    #[test]
    fn windows_bridge_requires_ready_permission_before_starting() {
        let bridge = WindowsSpeechBridge::new();

        assert!(!bridge.start());
        bridge.request_permissions();
        assert!(bridge.start());
    }

    #[test]
    fn windows_bridge_can_seed_debug_transcript_from_env() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            let bridge = WindowsSpeechBridge::new();
            env.set_var("SUZAKU_WINDOWS_VOICE_SAMPLE", "hello from windows");

            bridge.seed_debug_transcript_from_env();

            assert_eq!(
                bridge.poll_transcript().as_deref(),
                Some("hello from windows")
            );
        });
    }

    #[test]
    fn windows_bridge_can_switch_into_denied_state_for_debugging() {
        let bridge = WindowsSpeechBridge::new();

        bridge.debug_set_state(VoicePermissionState::Denied);

        assert_eq!(bridge.permission_state(), VoicePermissionState::Denied);
    }
}
