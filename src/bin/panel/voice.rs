use super::PanelState;
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState, VoicePermissionState};

impl PanelState {
    pub(super) fn advance_voice_sample(&mut self) {
        self.chrome.voice_transcript = self.voice.next_sample();
    }

    pub(super) fn voice_fallback_allowed(
        permission: VoicePermissionState,
        bridge_available: bool,
    ) -> bool {
        !bridge_available || permission == VoicePermissionState::Unavailable
    }

    pub(super) fn poll_voice_bridge(&mut self) {
        if let Some(bridge) = self.voice.bridge.as_ref() {
            self.chrome.voice_permission = bridge.permission_state();
            self.chrome.voice_backend_label = bridge.source_label().to_string();
            self.chrome.voice_supports_live_capture = bridge.supports_live_capture();
            if self.chrome.voice_permission != VoicePermissionState::Ready
                && self.chrome.voice_state == VoiceCaptureState::Listening
            {
                self.chrome.voice_state = VoiceCaptureState::Idle;
            }
            if let Some(transcript) = bridge.poll_transcript() {
                self.chrome.voice_transcript = transcript;
            }
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_backend_label = "Fallback Samples".to_string();
            self.chrome.voice_supports_live_capture = false;
        }
    }

    pub(super) fn start_voice_capture(&mut self) {
        let Some(bridge) = self.voice.bridge.as_ref() else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_state = VoiceCaptureState::Idle;
            self.advance_voice_sample();
            return;
        };

        bridge.request_permissions();
        let permission = bridge.permission_state();
        self.chrome.voice_permission = permission;

        if permission != VoicePermissionState::Ready {
            self.chrome.voice_state = VoiceCaptureState::Idle;
            if Self::voice_fallback_allowed(permission, true) {
                self.advance_voice_sample();
            }
            return;
        }

        if bridge.start() {
            bridge.seed_debug_transcript_from_env();
            self.chrome.voice_state = VoiceCaptureState::Listening;
        } else {
            let permission = bridge.permission_state();
            self.chrome.voice_permission = permission;
            self.chrome.voice_state = VoiceCaptureState::Idle;
            if Self::voice_fallback_allowed(permission, true) {
                self.advance_voice_sample();
            }
        }
    }

    pub(super) fn stop_voice_capture(&mut self) {
        if let Some(bridge) = self.voice.bridge.as_ref() {
            bridge.stop();
            self.chrome.voice_permission = bridge.permission_state();
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
        }
        self.chrome.voice_state = VoiceCaptureState::Idle;
    }

    pub(super) fn insert_voice_transcript(&mut self) {
        if self.chrome.voice_transcript.is_empty() {
            return;
        }

        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        let transcript = self.chrome.voice_transcript.clone();
        self.chrome.insert_text(&transcript);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_focused = true;
        self.chrome.move_caret_to_end();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }
}
