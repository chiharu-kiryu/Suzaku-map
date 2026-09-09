use super::PanelState;
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState, VoicePermissionState};

const VOICE_AUTO_INSERT_QUIET_TIME: Duration = Duration::from_millis(900);

#[derive(Default)]
pub(super) struct VoiceTranscriptProgress {
    transcript: String,
    changed_at: Option<Instant>,
    generation: u64,
}

impl VoiceTranscriptProgress {
    pub(super) fn reset(&mut self) {
        self.transcript.clear();
        self.changed_at = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    fn observe(&mut self, transcript: &str, now: Instant) -> Duration {
        if self.transcript != transcript || self.changed_at.is_none() {
            self.transcript = transcript.to_string();
            self.changed_at = Some(now);
            self.generation = self.generation.wrapping_add(1);
        }
        now.saturating_duration_since(self.changed_at.unwrap())
    }
}

impl PanelState {
    pub(super) fn normalize_voice_transcript(raw: &str) -> String {
        raw.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub(super) fn should_auto_insert_voice_transcript(
        transcript: &str,
        stable_for: Duration,
        voice_state: VoiceCaptureState,
        permission: VoicePermissionState,
    ) -> bool {
        if voice_state != VoiceCaptureState::Listening || permission != VoicePermissionState::Ready
        {
            return false;
        }
        let normalized = Self::normalize_voice_transcript(transcript);
        if normalized.is_empty() || stable_for < VOICE_AUTO_INSERT_QUIET_TIME {
            return false;
        }
        normalized.len() >= 6 || normalized.split_whitespace().count() >= 2
    }

    pub(super) fn advance_voice_sample(&mut self) {
        self.voice_progress.reset();
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
        self.poll_voice_bridge_at(Instant::now());
    }

    pub(super) fn poll_voice_bridge_at(&mut self, now: Instant) {
        self.refresh_voice_permission_state();
        if self.chrome.voice_state != VoiceCaptureState::Listening {
            return;
        }
        if self.chrome.voice_permission != VoicePermissionState::Ready {
            self.stop_voice_capture();
            return;
        }
        if let Some(transcript) = self
            .voice
            .bridge
            .as_ref()
            .and_then(|bridge| bridge.poll_transcript())
        {
            self.chrome.voice_transcript = Self::normalize_voice_transcript(&transcript);
        }
        // None means no update, not an empty transcript. This works for both
        // consume-once backends and those repeatedly returning the latest partial.
        let stable_for = self
            .voice_progress
            .observe(&self.chrome.voice_transcript, now);
        if self.chrome.voice_auto_insert
            && self.chrome.active_input_mode == InputMode::Dictation
            && Self::should_auto_insert_voice_transcript(
                &self.chrome.voice_transcript,
                stable_for,
                self.chrome.voice_state,
                self.chrome.voice_permission,
            )
        {
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
        self.voice_progress.reset();
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
        self.voice_progress.reset();
    }

    pub(super) fn clear_voice_transcript(&mut self) {
        self.chrome.voice_transcript.clear();
        self.voice_progress.reset();
    }

    pub(super) fn insert_voice_transcript(&mut self) {
        let transcript = Self::normalize_voice_transcript(&self.chrome.voice_transcript);
        if transcript.is_empty() {
            return;
        }

        // Stop on the first attempt, including rejection: retain the source but
        // never auto-replay it into a different native context on the next poll.
        if self.chrome.voice_state == VoiceCaptureState::Listening {
            self.stop_voice_capture();
        }
        if self.native.showing {
            self.native_insert_text(
                &transcript,
                super::native_sync::NativeInsertion::Voice {
                    transcript: self.chrome.voice_transcript.clone(),
                    generation: self.voice_progress.generation(),
                },
            );
            return;
        }

        if !self.chrome.seed_text.is_empty() && !self.chrome.seed_text.ends_with(' ') {
            self.chrome.insert_text(" ");
        }
        self.chrome.insert_text(&transcript);
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_modes_expanded = false;
        self.chrome.input_focused = true;
        self.chrome.move_caret_to_end();
        self.clear_voice_transcript();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_stability_uses_elapsed_time_not_poll_count() {
        let mut progress = VoiceTranscriptProgress::default();
        let now = Instant::now();
        for ms in 0..900 {
            let elapsed = progress.observe("hello world", now + Duration::from_millis(ms));
            assert!(!PanelState::should_auto_insert_voice_transcript(
                "hello world",
                elapsed,
                VoiceCaptureState::Listening,
                VoicePermissionState::Ready,
            ));
        }
        assert_eq!(
            progress.observe("hello world", now + VOICE_AUTO_INSERT_QUIET_TIME),
            VOICE_AUTO_INSERT_QUIET_TIME
        );
    }

    #[test]
    fn voice_partial_changes_restart_the_quiet_period() {
        let mut progress = VoiceTranscriptProgress::default();
        let now = Instant::now();
        progress.observe("hello", now);
        assert_eq!(
            progress.observe("hello world", now + Duration::from_millis(800)),
            Duration::ZERO
        );
        assert_eq!(
            progress.observe("hello world", now + Duration::from_millis(900)),
            Duration::from_millis(100)
        );
        assert_eq!(
            progress.observe("hello world", now + Duration::from_millis(1700)),
            VOICE_AUTO_INSERT_QUIET_TIME
        );
    }

    #[test]
    fn voice_restart_distinguishes_an_identical_new_transcript() {
        let mut progress = VoiceTranscriptProgress::default();
        let now = Instant::now();
        progress.observe("hello world", now);
        let generation = progress.generation();
        progress.reset();
        assert_ne!(progress.generation(), generation);
        assert_eq!(
            progress.observe("hello world", now + Duration::from_secs(2)),
            Duration::ZERO
        );
        assert_ne!(progress.generation(), generation);
    }

    #[test]
    fn voice_empty_update_cancels_previous_stability() {
        let mut progress = VoiceTranscriptProgress::default();
        let now = Instant::now();
        progress.observe("hello world", now);
        progress.observe("", now + Duration::from_secs(2));
        assert_eq!(
            progress.observe("hello world", now + Duration::from_secs(3)),
            Duration::ZERO
        );
    }
}
