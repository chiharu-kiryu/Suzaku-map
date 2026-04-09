use super::PanelState;
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState, VoicePermissionState};

impl PanelState {
    pub(super) fn normalize_voice_transcript(raw: &str) -> String {
        raw.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub(super) fn should_auto_insert_voice_transcript(
        transcript: &str,
        stability_ticks: u8,
        voice_state: VoiceCaptureState,
        permission: VoicePermissionState,
    ) -> bool {
        if voice_state != VoiceCaptureState::Listening || permission != VoicePermissionState::Ready
        {
            return false;
        }
        let normalized = Self::normalize_voice_transcript(transcript);
        if normalized.is_empty() || stability_ticks < 3 {
            return false;
        }
        normalized.len() >= 6 || normalized.split_whitespace().count() >= 2
    }

    pub(super) fn advance_voice_sample(&mut self) {
        self.chrome.voice_transcript = self.voice.next_sample();
    }

    pub(super) fn enter_voice_mode(&mut self) {
        self.chrome.blur_input();
        self.chrome.active_input_mode = InputMode::Dictation;
        self.refresh_voice_permission_state();
    }

    pub(super) fn refresh_voice_permission_state(&mut self) {
        if let Some(bridge) = self.voice.bridge.as_ref() {
            self.chrome.voice_permission = bridge.permission_state();
            self.chrome.voice_backend_label = bridge.source_label().to_string();
            self.chrome.voice_supports_live_capture = bridge.supports_live_capture();
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_backend_label = "Fallback Samples".to_string();
            self.chrome.voice_supports_live_capture = false;
        }
    }

    pub(super) fn voice_fallback_allowed(
        permission: VoicePermissionState,
        bridge_available: bool,
    ) -> bool {
        !bridge_available || permission == VoicePermissionState::Unavailable
    }

    pub(super) fn poll_voice_bridge(&mut self) {
        let mut should_auto_insert = false;
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
                let normalized = Self::normalize_voice_transcript(&transcript);
                if !normalized.is_empty() {
                    if normalized == self.last_polled_voice_transcript {
                        self.voice_stability_ticks = self.voice_stability_ticks.saturating_add(1);
                    } else {
                        self.last_polled_voice_transcript = normalized.clone();
                        self.voice_stability_ticks = 1;
                    }
                    self.chrome.voice_transcript = normalized;
                    should_auto_insert = self.chrome.voice_auto_insert
                        && Self::should_auto_insert_voice_transcript(
                            &self.chrome.voice_transcript,
                            self.voice_stability_ticks,
                            self.chrome.voice_state,
                            self.chrome.voice_permission,
                        )
                        && self.chrome.active_input_mode == InputMode::Dictation;
                }
            }
        } else {
            self.chrome.voice_permission = VoicePermissionState::Unavailable;
            self.chrome.voice_backend_label = "Fallback Samples".to_string();
            self.chrome.voice_supports_live_capture = false;
        }
        if should_auto_insert {
            self.insert_voice_transcript();
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

        self.chrome.voice_transcript.clear();
        self.last_polled_voice_transcript.clear();
        self.voice_stability_ticks = 0;
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
        self.voice_stability_ticks = 0;
    }

    pub(super) fn insert_voice_transcript(&mut self) {
        let transcript = Self::normalize_voice_transcript(&self.chrome.voice_transcript);
        if transcript.is_empty() {
            return;
        }

        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        self.chrome.insert_text(&transcript);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_focused = true;
        self.chrome.move_caret_to_end();
        self.chrome.voice_transcript.clear();
        self.last_polled_voice_transcript.clear();
        self.voice_stability_ticks = 0;
        if self.chrome.voice_state == VoiceCaptureState::Listening {
            self.stop_voice_capture();
        }
        self.sync_manual_seed_base();
        self.refresh_seed();
    }
}
