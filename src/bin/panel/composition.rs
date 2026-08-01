use super::PanelState;
use crate::render::create_font_atlas;
use std::time::{Duration, Instant};
use suzaku_map::ime::SignalState;
use suzaku_map::ime::gpu::InteractionKind;
use suzaku_map::languages::llama::{LlamaProviderConfig, llama_english_plugin_with_config};
use suzaku_map::panel_support::{
    derive_next_token_candidates, derive_sentence_candidates_with_indices,
};

const COMMIT_INPUT_REPEAT_WINDOW: Duration = Duration::from_millis(260);
const ACTION_REPEAT_WINDOW: Duration = Duration::from_millis(260);

impl PanelState {
    pub(super) fn is_repeating_interaction(&self, target: InteractionKind) -> bool {
        self.last_interaction_action
            .is_some_and(|(last_action, started_at)| {
                last_action == target && started_at.elapsed() < ACTION_REPEAT_WINDOW
            })
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
        last.selected_index == selected_index
            && last.text == text
            && last.timestamp.elapsed() < COMMIT_INPUT_REPEAT_WINDOW
    }

    pub(super) fn note_commit_attempt(&mut self, selected_index: usize, text: &str) {
        self.last_commit_attempt = Some(super::CommitAttempt {
            selected_index,
            text: text.to_string(),
            timestamp: Instant::now(),
        });
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
        self.engine.seed(&full_seed);
        self.refresh_composition_candidates();
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
        self.engine.seed(&full_seed);
        self.refresh_composition_candidates();
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
        self.engine.seed(&normalized);
        self.refresh_composition_candidates();
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
}
