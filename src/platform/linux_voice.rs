use crate::ime::gpu::VoicePermissionState;
use crate::platform::linux;

#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct LinuxSpeechBridge;

#[cfg(target_os = "linux")]
impl LinuxSpeechBridge {
    pub fn new() -> Self {
        Self
    }

    pub fn start(&self) -> bool {
        unsafe { suzaku_linux_speech_start() }
    }

    pub fn request_permissions(&self) {
        unsafe { suzaku_linux_speech_request_permissions() }
    }

    pub fn stop(&self) {
        unsafe { suzaku_linux_speech_stop() }
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        match unsafe { suzaku_linux_speech_state() } {
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
            unsafe { suzaku_linux_speech_consume_transcript(buf.as_mut_ptr().cast(), buf.len()) };
        if !ok {
            return None;
        }
        let nul = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..nul].to_vec()).ok()
    }

    pub fn seed_debug_transcript_from_env(&self) {
        for key in linux::linux_voice_sample_env_keys() {
            if let Ok(sample) = std::env::var(key) {
                let mut bytes = sample.into_bytes();
                bytes.retain(|byte| *byte != 0);
                bytes.push(0);
                unsafe {
                    suzaku_linux_speech_debug_seed_transcript(bytes.as_ptr().cast());
                }
                break;
            }
        }
    }

    pub fn source_label(&self) -> &'static str {
        linux::linux_voice_backend_label()
    }

    pub fn supports_live_capture(&self) -> bool {
        unsafe { suzaku_linux_speech_supports_live_capture() }
    }

    pub fn portal_available(&self) -> bool {
        unsafe { suzaku_linux_speech_portal_available() }
    }

    pub fn pipewire_available(&self) -> bool {
        unsafe { suzaku_linux_speech_pipewire_available() }
    }
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn suzaku_linux_speech_supports_live_capture() -> bool;
    fn suzaku_linux_speech_portal_available() -> bool;
    fn suzaku_linux_speech_pipewire_available() -> bool;
    fn suzaku_linux_speech_state() -> i32;
    fn suzaku_linux_speech_request_permissions();
    fn suzaku_linux_speech_start() -> bool;
    fn suzaku_linux_speech_stop();
    fn suzaku_linux_speech_consume_transcript(
        buffer: *mut std::ffi::c_char,
        capacity: usize,
    ) -> bool;
    fn suzaku_linux_speech_debug_seed_transcript(text: *const std::ffi::c_char);
}

#[cfg(not(target_os = "linux"))]
use std::sync::{Arc, Mutex};

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone)]
pub struct LinuxSpeechBridge {
    state: Arc<Mutex<LinuxSpeechState>>,
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
struct LinuxSpeechState {
    permission: VoicePermissionState,
    active: bool,
    queued_transcript: Option<String>,
}

#[cfg(not(target_os = "linux"))]
impl LinuxSpeechBridge {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(LinuxSpeechState {
                permission: VoicePermissionState::Pending,
                active: false,
                queued_transcript: None,
            })),
        }
    }

    pub fn start(&self) -> bool {
        if self.permission_state() != VoicePermissionState::Ready {
            return false;
        }

        let mut state = self.state.lock().expect("linux speech state");
        state.active = true;
        if state.queued_transcript.is_none() {
            for key in linux::linux_voice_sample_env_keys() {
                if let Ok(sample) = std::env::var(key) {
                    state.queued_transcript = Some(sample);
                    break;
                }
            }
        }
        true
    }

    pub fn request_permissions(&self) {
        let mut state = self.state.lock().expect("linux speech state");
        state.permission = computed_linux_permission_state();
    }

    pub fn stop(&self) {
        let mut state = self.state.lock().expect("linux speech state");
        state.active = false;
    }

    pub fn permission_state(&self) -> VoicePermissionState {
        let computed = computed_linux_permission_state();
        let mut state = self.state.lock().expect("linux speech state");
        state.permission = computed;
        state.permission
    }

    pub fn poll_transcript(&self) -> Option<String> {
        let mut state = self.state.lock().expect("linux speech state");
        state.queued_transcript.take()
    }

    pub fn seed_debug_transcript_from_env(&self) {
        let mut state = self.state.lock().expect("linux speech state");
        if state.queued_transcript.is_some() {
            return;
        }
        for key in linux::linux_voice_sample_env_keys() {
            if let Ok(sample) = std::env::var(key) {
                state.queued_transcript = Some(sample);
                break;
            }
        }
    }

    pub fn source_label(&self) -> &'static str {
        linux::linux_voice_backend_label()
    }

    pub fn supports_live_capture(&self) -> bool {
        self.portal_available() && self.pipewire_available()
    }

    pub fn portal_available(&self) -> bool {
        linux::linux_voice_portal_available()
    }

    pub fn pipewire_available(&self) -> bool {
        linux::linux_voice_pipewire_available()
    }
}

#[cfg(not(target_os = "linux"))]
fn computed_linux_permission_state() -> VoicePermissionState {
    if std::env::var("SUZAKU_LINUX_VOICE_FORCE_READY")
        .map(|value| value == "1")
        .unwrap_or(false)
    {
        return VoicePermissionState::Ready;
    }
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
    if linux::linux_voice_portal_available() && linux::linux_voice_pipewire_available() {
        VoicePermissionState::Ready
    } else {
        VoicePermissionState::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::LinuxSpeechBridge;
    use crate::ime::gpu::VoicePermissionState;
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;

    #[test]
    fn linux_bridge_can_seed_debug_transcript_from_env() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            let bridge = LinuxSpeechBridge::new();
            env.set_var("SUZAKU_LINUX_VOICE_SAMPLE", "hello from linux");

            bridge.seed_debug_transcript_from_env();

            assert_eq!(
                bridge.poll_transcript().as_deref(),
                Some("hello from linux")
            );
        });
    }

    #[test]
    fn linux_bridge_reports_ready_when_portal_and_pipewire_are_available() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            let bridge = LinuxSpeechBridge::new();
            env.set_var("SUZAKU_LINUX_VOICE_FORCE_READY", "1");

            bridge.request_permissions();

            assert_eq!(bridge.permission_state(), VoicePermissionState::Ready);
            assert_eq!(
                bridge.supports_live_capture(),
                bridge.portal_available() && bridge.pipewire_available()
            );
        });
    }

    #[test]
    fn linux_bridge_requires_ready_permission_before_starting() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            let bridge = LinuxSpeechBridge::new();
            env.set_var("SUZAKU_LINUX_VOICE_FORCE_DENIED", "1");
            assert!(!bridge.start());

            env.remove_var("SUZAKU_LINUX_VOICE_FORCE_DENIED");
            env.set_var("SUZAKU_LINUX_VOICE_FORCE_READY", "1");
            bridge.request_permissions();
            assert!(bridge.start());
        });
    }

    #[test]
    fn linux_bridge_exposes_probe_details() {
        let bridge = LinuxSpeechBridge::new();
        assert_eq!(
            bridge.supports_live_capture(),
            bridge.portal_available() && bridge.pipewire_available()
        );
    }
}
