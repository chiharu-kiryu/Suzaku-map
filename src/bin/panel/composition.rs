use super::{CommitAttempt, PanelState};
use crate::render::create_font_atlas;
use std::time::{Duration, Instant};
use suzaku_map::ime::InputSource;
use suzaku_map::ime::SignalState;
use suzaku_map::ime::gpu::InteractionKind;
use suzaku_map::ime::settings::ImeSettings;
use suzaku_map::languages::model::{HttpModelProvider, ModelProviderConfig};
use suzaku_map::panel_support::composition_candidate_previews;
use suzaku_map::platform::ime_host_adapter::ImeHostSessionBridge;
use suzaku_map::platform::ime_host_adapter::shared_session_bridge;
use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;

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

    fn current_model_config(&self) -> ModelProviderConfig {
        ModelProviderConfig {
            temperature_tenths: self.chrome.llm_temperature.tenths(),
            handwriting_hint: self.last_handwriting_summary.clone(),
            ..ImeSettings::load().unwrap_or_default().provider
        }
    }

    pub(super) fn reconfigure_model_provider(&mut self) {
        if self.native.showing {
            self.engine.configure_prediction(None);
            return;
        }
        if self.chrome.llm_enabled {
            self.engine
                .configure_prediction(Some(std::sync::Arc::new(HttpModelProvider::new(
                    self.current_model_config(),
                ))));
        } else {
            self.engine.configure_prediction(None);
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

        let normalized = text;
        let bridge = shared_session_bridge();
        let _ = bridge.activate_session();
        if normalized.trim().is_empty() {
            bridge.clear_marked_text();
            return;
        }
        let _ = bridge.replace_marked_text(normalized, InputSource::OnScreenPanel);
    }

    fn refresh_seed_with_text(&mut self, text: &str) {
        if self.native_action(suzaku_map::ime::companion::NativeOperation::Replace(
            text.to_string(),
        )) {
            self.refresh_native_view();
            return;
        }
        self.engine.seed(text);
        self.refresh_composition_candidates();
        self.sync_marked_text_with_host(text);
    }

    pub(super) fn reset_after_commit(&mut self) {
        self.completion_history.clear();
        self.next_token_completions.clear();
        self.chrome.composed_tokens.clear();
        self.chrome.next_token_candidates.clear();
        self.chrome.sentence_candidates.clear();
        self.chrome.sentence_candidate_source_indices.clear();
        self.chrome.set_seed_text(String::new());
        self.chrome.move_caret_to_end();
        self.refresh_seed();
        self.chrome.focus_input();
    }

    pub(super) fn sync_manual_seed_base(&mut self) {
        if self.native.showing {
            return;
        }
        self.completion_history.clear();
    }

    pub(super) fn refresh_composition_candidates(&mut self) {
        if self.native.showing {
            self.refresh_native_view();
            return;
        }
        let snapshot = self.engine.snapshot();
        self.chrome.composed_tokens = self.completion_history.labels();
        let previews = composition_candidate_previews(
            &snapshot.seed_text,
            &snapshot.active_language,
            self.engine.candidates(),
            6,
            4,
        );
        let (source_indices, sentences): (Vec<_>, Vec<_>) = previews.sentences.into_iter().unzip();
        let candidates_changed = self.next_token_completions != previews.next_tokens
            || self.chrome.sentence_candidate_source_indices != source_indices
            || self.chrome.sentence_candidates != sentences;
        if candidates_changed
            && matches!(
                self.interaction.pressed_interaction,
                Some(InteractionKind::Candidate(_) | InteractionKind::SelectNextToken(_))
            )
        {
            // An index is not a candidate identity: an async result can replace
            // the pressed item before release, for both mouse and touch input.
            self.clear_pressed_interaction();
            self.interaction.touch_tap_pending = false;
        }
        self.chrome.next_token_candidates = previews
            .next_tokens
            .iter()
            .map(|edit| edit.label.clone())
            .collect();
        self.next_token_completions = previews.next_tokens;
        self.chrome.sentence_candidate_source_indices = source_indices;
        self.chrome.sentence_candidates = sentences;
        self.last_scene = None;

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
        if !next_token_scroll_index
            .is_none_or(|index| index < self.chrome.next_token_candidates.len())
        {
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
        let Some(edit) = self.next_token_completions.get(index).cloned() else {
            return;
        };
        if self.native.showing {
            let mut history = suzaku_map::panel_support::CompletionHistory::default();
            if let Some(seed) = history.apply(&self.chrome.seed_text, &edit) {
                self.native_action(suzaku_map::ime::companion::NativeOperation::Replace(seed));
            }
            return;
        }
        let action = InteractionKind::SelectNextToken(index);
        if self.is_repeating_interaction(action) {
            return;
        }
        self.note_interaction_action(action);
        let Some(full_seed) = self.completion_history.apply(&self.chrome.seed_text, &edit) else {
            return;
        };
        self.chrome.set_seed_text(full_seed.clone());
        self.chrome.move_caret_to_end();
        self.refresh_seed_with_text(&full_seed);
    }

    /// Number-key choices are editable replacements; only an explicit send commits.
    pub(super) fn continue_sentence_candidate(&mut self, index: usize) {
        let text = if self.native.showing {
            self.native.frame.as_ref().and_then(|frame| {
                frame
                    .candidates
                    .get(index)
                    .map(|candidate| candidate.text.clone())
            })
        } else {
            self.engine
                .candidates()
                .get(index)
                .map(|candidate| candidate.text.clone())
        };
        if let Some(text) = text {
            self.replace_continuing_draft(text);
        }
    }

    pub(super) fn continue_composition_with_space(&mut self) {
        if self.chrome.seed_text.is_empty() {
            return;
        }
        let text = if self.native.showing {
            self.chrome.seed_text.as_str()
        } else {
            self.engine
                .selected_completion_text(true)
                .unwrap_or(&self.chrome.seed_text)
        };
        self.replace_continuing_draft(format!("{text} "));
    }

    fn replace_continuing_draft(&mut self, text: String) {
        if !self.native.showing {
            self.sync_manual_seed_base();
            self.chrome.set_seed_text(text.clone());
            self.chrome.move_caret_to_end();
        }
        self.refresh_seed_with_text(&text);
    }

    pub(super) fn rewind_next_token(&mut self) {
        if self.native.showing {
            return;
        }
        if self.is_repeating_interaction(InteractionKind::RewindNextToken) {
            return;
        }
        self.note_interaction_action(InteractionKind::RewindNextToken);
        let Some(full_seed) = self.completion_history.undo(&self.chrome.seed_text) else {
            return;
        };
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
        if !can_process_text_input(self.chrome.input_focused) {
            return;
        }

        let accepted = sanitize_text_input(text);

        if !accepted.is_empty() {
            self.chrome.insert_text(&accepted);
            self.sync_manual_seed_base();
            self.refresh_seed();
        }
    }

    pub(super) fn handle_settings_search_text(&mut self, text: &str) {
        let accepted = sanitize_text_input(text);
        if accepted.is_empty() {
            return;
        }

        self.chrome.settings_search_query.push_str(&accepted);
        self.chrome.settings_search_focused = true;
    }

    pub(super) fn backspace_settings_search_text(&mut self) {
        if self.chrome.settings_search_query.pop().is_some() {
            self.chrome.settings_search_focused = true;
        }
    }

    pub(super) fn clear_settings_search_text(&mut self) {
        self.chrome.settings_search_query.clear();
        self.chrome.settings_search_focused = true;
    }

    pub(super) fn backspace_seed(&mut self) {
        self.chrome.backspace();
        self.sync_manual_seed_base();
        self.refresh_seed();
    }

    pub(super) fn refresh_seed(&mut self) {
        self.refresh_seed_with_text(&self.chrome.seed_text.clone());
    }
}

fn should_accept_input_char(ch: char) -> bool {
    !ch.is_control()
}

fn sanitize_text_input(text: &str) -> String {
    text.chars()
        .filter(|ch| should_accept_input_char(*ch))
        .collect()
}

fn can_process_text_input(input_focused: bool) -> bool {
    input_focused
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
        assert_eq!(sanitize_text_input("👍🏽 hello 🇨🇦"), "👍🏽 hello 🇨🇦");
    }

    #[test]
    fn focused_text_input_does_not_depend_on_the_selected_input_tab() {
        assert!(can_process_text_input(true));
    }

    #[test]
    fn text_input_rejected_when_input_blurred() {
        assert!(!can_process_text_input(false));
    }

    #[test]
    fn text_input_allowed_when_virtual_keyboard_focused() {
        assert!(can_process_text_input(true));
    }

    #[test]
    fn text_input_inserts_emoji_when_allowed() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hi".to_string();
        chrome.caret_index = 2;
        chrome.active_input_mode = suzaku_map::ime::gpu::InputMode::VirtualKeyboard;
        chrome.input_focused = true;

        if can_process_text_input(chrome.input_focused) {
            let accepted = sanitize_text_input(" 😀");
            if !accepted.is_empty() {
                chrome.insert_text(&accepted);
            }
        }

        assert_eq!(chrome.seed_text, "hi 😀");
        assert_eq!(chrome.caret_index, 3 + "😀".chars().count());
    }

    #[test]
    fn handwriting_text_field_accepts_keyboard_input() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hello".to_string();
        chrome.caret_index = 5;
        chrome.active_input_mode = suzaku_map::ime::gpu::InputMode::Handwriting;
        chrome.input_focused = true;
        if can_process_text_input(chrome.input_focused) {
            let accepted = sanitize_text_input(" 🐶");
            if !accepted.is_empty() {
                chrome.insert_text(&accepted);
            }
        }

        assert_eq!(chrome.seed_text, "hello 🐶");
        assert_eq!(chrome.caret_index, 7);
    }

    #[test]
    fn text_input_does_not_mutate_seed_when_rejected_by_blur() {
        let mut chrome = PanelChromeState::default();
        chrome.seed_text = "hello".to_string();
        chrome.input_focused = false;
        chrome.caret_index = 5;

        let expected = chrome.seed_text.clone();
        let snapshot = chrome.caret_index;

        if can_process_text_input(chrome.input_focused) {
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

        assert!(is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_text_changes() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(!is_commit_input_repeat(2, "world", &last, now,));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_text_case_changes_only() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(120),
        };

        assert!(!is_commit_input_repeat(2, "Hello", &last, now,));
    }

    #[test]
    fn commit_repeat_expires_at_window_boundary() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - COMMIT_INPUT_REPEAT_WINDOW,
        };

        assert!(!is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_expires_just_outside_window() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(261),
        };

        assert!(!is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_hits_when_timestamp_is_current() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now,
        };

        assert!(is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_is_not_detected_when_candidate_index_changes() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 1,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(10),
        };

        assert!(!is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_treats_emoji_candidates_as_text_identity() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 3,
            text: "😀 emoji".to_string(),
            timestamp: now - Duration::from_millis(80),
        };

        assert!(is_commit_input_repeat(3, "😀 emoji", &last, now,));
    }

    #[test]
    fn commit_repeat_hits_same_index_and_text_within_window_edge_minus_one_ms() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 0,
            text: "hello".to_string(),
            timestamp: now - Duration::from_millis(259),
        };

        assert!(is_commit_input_repeat(0, "hello", &last, now,));
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
        let last = Some((suzaku_map::ime::gpu::InteractionKind::RewindNextToken, now));

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

        assert!(!is_commit_input_repeat(2, "hello", &last, now,));
    }

    #[test]
    fn commit_repeat_is_true_when_elapsed_is_zero() {
        let now = Instant::now();
        let last = CommitAttempt {
            selected_index: 2,
            text: "hello".to_string(),
            timestamp: now,
        };

        assert!(is_commit_input_repeat(2, "hello", &last, now,));
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

        assert!(!is_commit_input_repeat(1, " hello", &last, now,));
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
