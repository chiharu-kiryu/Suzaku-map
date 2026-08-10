use super::{CommitAttempt, PanelState};
use crate::render::create_font_atlas;
use std::time::{Duration, Instant};
use suzaku_map::ime::SignalState;
use suzaku_map::ime::InputSource;
use suzaku_map::ime::gpu::InteractionKind;
use suzaku_map::platform::ime_host_adapter::shared_session_bridge;
use suzaku_map::platform::ime_host_adapter::ImeHostSessionBridge;
use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
use suzaku_map::languages::llama::{LlamaProviderConfig, llama_english_plugin_with_config};
use suzaku_map::panel_support::{
    derive_next_token_candidates, derive_sentence_candidates_with_indices,
};

const COMMIT_INPUT_REPEAT_WINDOW: Duration = Duration::from_millis(260);
const ACTION_REPEAT_WINDOW: Duration = Duration::from_millis(260);

fn is_action_repeat(
    target: InteractionKind,
    last: Option<(InteractionKind, Instant)>,
    now: Instant,
) -> bool {
    last.is_some_and(|(last_action, started_at)| {
        last_action == target && now.duration_since(started_at) < ACTION_REPEAT_WINDOW
    })
}

fn is_commit_input_repeat(
    selected_index: usize,
    text: &str,
    last: &CommitAttempt,
    now: Instant,
) -> bool {
    last.selected_index == selected_index
        && last.text == text
        && now.duration_since(last.timestamp) < COMMIT_INPUT_REPEAT_WINDOW
}

impl PanelState {
    pub(super) fn is_repeating_interaction(&self, target: InteractionKind) -> bool {
        is_action_repeat(target, self.last_interaction_action, Instant::now())
    }

    pub(super) fn note_interaction_action(&mut self, action: InteractionKind) {
        self.last_interaction_action = Some((action, Instant::now()));
    }

    pub(super) fn reset_signal(&mut self) {
        self.engine.update_signal(SignalState {
            pointer_precision: 0.42,
            gaze_stability: 0.50,
            host_intent_weight: 0.80,
            source_confidence: 0.70,
        });
        self.refresh_seed();
    }

    fn current_llama_config(&self) -> LlamaProviderConfig {
        LlamaProviderConfig {
            endpoint: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
            model: match self.chrome.llm_model {
                suzaku_map::ime::gpu::LlmModelPreset::Llama32_3b => "llama3.2:3b".to_string(),
            },
            system_prompt: "You are a sentence-completion engine for an XR and tablet IME. Expand the user's seed into 3 short, tap-friendly English sentence candidates. Return plain text only, one candidate per line, no numbering.".to_string(),
            max_tokens: 96,
            temperature_tenths: match self.chrome.llm_temperature {
                suzaku_map::ime::gpu::LlmTemperaturePreset::Focused => 2,
                suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced => 4,
                suzaku_map::ime::gpu::LlmTemperaturePreset::Expressive => 7,
            },
            timeout_ms: 1200,
            handwriting_hint: self.last_handwriting_summary.clone(),
        }
    }

    pub(super) fn reconfigure_llama_plugin(&mut self) {
        if self.chrome.llm_enabled {
            self.engine
                .register_language_plugin(llama_english_plugin_with_config(
                    self.current_llama_config(),
                ));
            self.engine.set_language("llama-en");
        } else {
            self.engine.set_language("en");
        }
        self.refresh_seed();
    }

    pub(super) fn is_duplicate_commit(&self, selected_index: usize, text: &str) -> bool {
        let Some(last) = self.last_commit_attempt.as_ref() else {
            return false;
        };

        is_commit_input_repeat(selected_index, text, last, Instant::now())
    }

    pub(super) fn note_commit_attempt(&mut self, selected_index: usize, text: &str) {
        self.last_commit_attempt = Some(super::CommitAttempt {
            selected_index,
            text: text.to_string(),
            timestamp: Instant::now(),
        });
    }

    fn sync_marked_text_with_host(&mut self, text: &str) {
        if !current_ime_host_dispatch().marked_text_roundtrip {
            return;
        }

        let normalized = text.trim();
        let bridge = shared_session_bridge();
        let _ = bridge.activate_session();
        if normalized.is_empty() {
            bridge.clear_marked_text();
            return;
        }
        let _ = bridge.replace_marked_text(normalized, InputSource::OnScreenPanel);
    }

    fn refresh_seed_with_text(&mut self, text: &str) {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.engine.seed(&normalized);
        self.refresh_composition_candidates();
        self.sync_marked_text_with_host(&normalized);
    }

    pub(super) fn reset_after_commit(&mut self, committed_seed: &str) {
        self.selected_next_tokens.clear();
        self.composition_base_seed.clear();
        self.chrome.composed_tokens.clear();
        self.chrome.next_token_candidates.clear();
        self.chrome.sentence_candidates.clear();
        self.chrome.sentence_candidate_source_indices.clear();
        self.chrome.set_seed_text(committed_seed.to_string());
        self.chrome.move_caret_to_end();
        self.refresh_seed();
        self.chrome.focus_input();
    }

    pub(super) fn sync_manual_seed_base(&mut self) {
        self.selected_next_tokens.clear();
        self.composition_base_seed = self
            .chrome
            .seed_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
    }

    fn full_composed_seed(&self) -> String {
        let mut parts = Vec::new();
        if !self.composition_base_seed.is_empty() {
            parts.push(self.composition_base_seed.clone());
        }
        if !self.selected_next_tokens.is_empty() {
            parts.push(self.selected_next_tokens.join(" "));
        }
        parts.join(" ").trim().to_string()
    }

    fn refresh_composition_candidates(&mut self) {
        let snapshot = self.engine.snapshot();
        let normalized_seed = snapshot.seed_text.trim().to_string();
        self.chrome.composed_tokens = self.selected_next_tokens.clone();
        self.chrome.next_token_candidates =
            derive_next_token_candidates(&normalized_seed, &snapshot.candidate_labels, 6);
        if normalized_seed.split_whitespace().count() >= 2 {
            let sentence_candidates = derive_sentence_candidates_with_indices(
                &normalized_seed,
                &snapshot.candidate_labels,
                4,
            );
            self.chrome.sentence_candidate_source_indices = sentence_candidates
                .iter()
                .map(|(index, _)| *index)
                .collect();
            self.chrome.sentence_candidates = sentence_candidates
                .into_iter()
                .map(|(_, sentence)| sentence)
                .collect();
        } else {
            self.chrome.sentence_candidate_source_indices.clear();
            self.chrome.sentence_candidates.clear();
        }

        let scroll_index = self.interaction.sentence_candidate_scroll_index;
        if !scroll_index.is_none_or(|index| {
            self.chrome
                .sentence_candidate_source_indices
                .contains(&index)
        }) {
            self.interaction.sentence_candidate_scroll_index = None;
            self.interaction.sentence_candidate_scroll_started_at = None;
        }

        let next_token_scroll_index = self.interaction.next_token_candidate_scroll_index;
        if !next_token_scroll_index.is_none_or(|index| index < self.chrome.next_token_candidates.len()) {
            self.interaction.next_token_candidate_scroll_index = None;
            self.interaction.next_token_candidate_scroll_started_at = None;
        }

        let handwriting_scroll_index = self.interaction.handwriting_candidate_scroll_index;
        if !handwriting_scroll_index
            .is_none_or(|index| index < self.chrome.handwriting_candidates.len())
        {
            self.interaction.handwriting_candidate_scroll_index = None;
            self.interaction.handwriting_candidate_scroll_started_at = None;
        }
    }

    pub(super) fn select_next_token(&mut self, index: usize) {
        let Some(token) = self.chrome.next_token_candidates.get(index).cloned() else {
            return;
        };
        let action = InteractionKind::SelectNextToken(index);
        if self.is_repeating_interaction(action) {
            return;
        }
        self.note_interaction_action(action);
        if self.selected_next_tokens.is_empty() {
            self.composition_base_seed = self
                .chrome
                .seed_text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        }
        self.selected_next_tokens.push(token);
        let full_seed = self.full_composed_seed();
        self.chrome.set_seed_text(full_seed.clone());
        self.chrome.move_caret_to_end();
        self.refresh_seed_with_text(&full_seed);
    }

    pub(super) fn rewind_next_token(&mut self) {
        if self.is_repeating_interaction(InteractionKind::RewindNextToken) {
            return;
        }
        self.note_interaction_action(InteractionKind::RewindNextToken);
        if self.selected_next_tokens.pop().is_none() {
            return;
        }
        let full_seed = self.full_composed_seed();
        self.chrome.set_seed_text(full_seed.clone());
        self.chrome.move_caret_to_end();
        self.refresh_seed_with_text(&full_seed);
    }

    pub(super) fn rebuild_font_atlas(&mut self) {
        let text_bind_group_layout = self.text_pipeline.get_bind_group_layout(0);
        self.font_atlas = create_font_atlas(
            &self.device,
            &self.queue,
            &text_bind_group_layout,
            self.chrome.font_face,
            self.chrome.text_smoothing,
            self.window_scale,
        );
    }

    pub(super) fn handle_text_input(&mut self, text: &str) {
        if !can_process_text_input(self.chrome.active_input_mode, self.chrome.input_focused) {
            return;
        }

        let accepted = sanitize_text_input(text);

        if !accepted.is_empty() {
            self.chrome.insert_text(&accepted);
            self.sync_manual_seed_base();
            self.refresh_seed();
        }
    }

    pub(super) fn backspace_seed(&mut self) {
        self.chrome.backspace();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }

    pub(super) fn refresh_seed(&mut self) {
        let normalized = self
            .chrome
            .seed_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.refresh_seed_with_text(&normalized);
    }
}

fn should_accept_input_char(ch: char) -> bool {
    !ch.is_control()
}

fn sanitize_text_input(text: &str) -> String {
    text.chars().filter(|ch| should_accept_input_char(*ch)).collect()
}

fn can_process_text_input(
    active_input_mode: suzaku_map::ime::gpu::InputMode,
    input_focused: bool,
) -> bool {
    active_input_mode == suzaku_map::ime::gpu::InputMode::VirtualKeyboard && input_focused
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzaku_map::ime::gpu::PanelChromeState;

    #[test]
    fn text_input_keeps_emoji_characters() {
        assert_eq!(sanitize_text_input("hello 😀 world"), "hello 😀 world");
    }

    #[test]
    fn text_input_removes_control_characters() {
        let input = "hello\t😀\n";
        let output = sanitize_text_input(input);

        assert_eq!(output, "hello😀");
    }

    #[test]
    fn text_input_keeps_multi_codepoint_emoji() {
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(sanitize_text_input(family), family);
    }

    #[test]
    fn text_input_keeps_skin_tone_and_flag_emojis() {
        assert_eq!(
            sanitize_text_input("👍🏽 hello 🇨🇦"),
            "👍🏽 hello 🇨🇦"
        );
    }

    #[test]
    fn text_input_rejected_when_not_virtual_keyboard() {
        assert!(!can_process_text_input(
            suzaku_map::ime::gpu::InputMode::Handwriting,
            true,
        ));
    }

    #[test]
    fn text_input_rejected_when_input_blurred() {
        assert!(!can_process_text_input(
            suzaku_map::ime::gpu::InputMode::VirtualKeyboard,
            false,
        ));
    }

    #[test]
    fn text_input_allowed_when_virtual_keyboard_focused() {
        assert!(can_process_text_input(
            suzaku_map::ime::gpu::InputMode::VirtualKeyboard,
            true,
        ));
    }

    #[test]
    fn text_input_inserts_emoji_when_allowed() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hi".to_string();
        chrome.caret_index = 2;
        chrome.active_input_mode = suzaku_map::ime::gpu::InputMode::VirtualKeyboard;
        chrome.input_focused = true;

        if can_process_text_input(chrome.active_input_mode, chrome.input_focused) {
            let accepted = sanitize_text_input(" 😀");
            if !accepted.is_empty() {
                chrome.insert_text(&accepted);
            }
        }

        assert_eq!(chrome.seed_text, "hi 😀");
        assert_eq!(chrome.caret_index, 3 + "😀".chars().count());
    }

    #[test]
    fn text_input_does_not_mutate_seed_when_rejected_by_mode() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hello".to_string();
        chrome.caret_index = 5;

        let expected = chrome.seed_text.clone();
        let snapshot = chrome.caret_index;

        if can_process_text_input(suzaku_map::ime::gpu::InputMode::Handwriting, chrome.input_focused) {
            let accepted = sanitize_text_input(" 🐶");
            if !accepted.is_empty() {
                chrome.insert_text(&accepted);
            }
        }

        assert_eq!(chrome.seed_text, expected);
        assert_eq!(chrome.caret_index, snapshot);
    }

    #[test]
    fn text_input_does_not_mutate_seed_when_rejected_by_blur() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hello".to_string();
        chrome.input_focused = false;
        chrome.caret_index = 5;

        let expected = chrome.seed_text.clone();
        let snapshot = chrome.caret_index;

        if can_process_text_input(
            suzaku_map::ime::gpu::InputMode::VirtualKeyboard,
            chrome.input_focused,
        ) {
            let accepted = sanitize_text_input(" 🐶");
            if !accepted.is_empty() {
                chrome.insert_text(&accepted);
            }
        }

        assert_eq!(chrome.seed_text, expected);
        assert_eq!(chrome.caret_index, snapshot);
    }

    #[test]
    fn commit_repeat_is_detected_for_same_index_text_within_window() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_text_changes() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(!is_commit_input_repeat(
            2,
            "world",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_text_case_changes_only() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(!is_commit_input_repeat(
            2,
            "Hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_expires_at_window_boundary() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - COMMIT_INPUT_REPEAT_WINDOW,
        };

        assert!(!is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_expires_just_outside_window() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(261),
        };

        assert!(!is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_hits_when_timestamp_is_current() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now,
        };

        assert!(is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_candidate_index_changes() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 1,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(10),
        };

        assert!(!is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_treats_emoji_candidates_as_text_identity() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 3,
            text: "😀 emoji".to_string(),
            timestamp: now - Duration::from_millis(80),
        };

        assert!(is_commit_input_repeat(
            3,
            "😀 emoji",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_hits_same_index_and_text_within_window_edge_minus_one_ms() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 0,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(259),
        };

        assert!(is_commit_input_repeat(
            0,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_within_window_and_same_target() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now - Duration::from_millis(120),
        ));

        assert!(is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_requires_exact_same_target() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now - Duration::from_millis(120),
        ));

        assert!(!is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::SelectNextToken(2),
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_is_false_when_no_previous_action() {
        let now = Instant::now();

        assert!(!is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            None,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_is_false_when_at_window_boundary() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now - ACTION_REPEAT_WINDOW,
        ));

        assert!(!is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_is_true_at_zero_elapsed() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now,
        ));

        assert!(is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_hits_within_window_edge_minus_one_ms() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now - Duration::from_millis(259),
        ));

        assert!(is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_expires_outside_window() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            now - Duration::from_millis(261),
        ));

        assert!(!is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::RewindNextToken,
            last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_false_at_repeat_window_boundary() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(260),
        };

        assert!(!is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_true_when_elapsed_is_zero() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now,
        };

        assert!(is_commit_input_repeat(
            2,
            "hello",
            &last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_is_false_when_same_kind_but_different_payload() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::SelectNextToken(2),
            now - Duration::from_millis(120),
        ));

        assert!(!is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::SelectNextToken(4),
            last,
            now,
        ));
    }

    #[test]
    fn interaction_repeat_is_true_when_same_indexed_select_token_repeats_within_window() {
        let now = Instant::now();
        let last = Some((
            suzaku_map::ime::gpu::InteractionKind::SelectNextToken(7),
            now - Duration::from_millis(120),
        ));

        assert!(is_action_repeat(
            suzaku_map::ime::gpu::InteractionKind::SelectNextToken(7),
            last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_is_false_when_text_trim_difference_only() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 1,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(!is_commit_input_repeat(
            1,
            " hello",
            &last,
            now,
        ));
    }

    #[test]
    fn commit_repeat_sequence_treats_second_as_repeat_and_third_after_window_as_new() {
        let first = Instant::now();
        let second = first + Duration::from_millis(120);
        let third = second + Duration::from_millis(280);

        let previous = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: first,
        };

        assert!(is_commit_input_repeat(2, "hello", &previous, second));

        let previous = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: second,
        };

        assert!(!is_commit_input_repeat(2, "hello", &previous, third));
    }

    #[test]
    fn interaction_repeat_sequence_within_window_then_after_window() {
        let kind = suzaku_map::ime::gpu::InteractionKind::SelectNextToken(7);
        let first = Instant::now();
        let second = first + Duration::from_millis(120);
        let third = second + Duration::from_millis(300);

        let previous = Some((kind, first));
        assert!(is_action_repeat(kind, previous, second));

        let previous = Some((kind, second));
        assert!(!is_action_repeat(kind, previous, third));
    }
}
