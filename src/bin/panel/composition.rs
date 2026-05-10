use super::PanelState;
use crate::render::create_font_atlas;
use suzaku_map::ime::SignalState;
use suzaku_map::languages::llama::{LlamaProviderConfig, llama_english_plugin_with_config};
use suzaku_map::panel_support::{
    derive_next_token_candidates, derive_sentence_candidates_with_indices,
};

impl PanelState {
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

    pub(super) fn reset_after_commit(&mut self) {
        let committed_snapshot = self.engine.snapshot();
        self.selected_next_tokens.clear();
        self.composition_base_seed.clear();
        self.chrome.composed_tokens.clear();
        self.chrome.next_token_candidates.clear();
        self.chrome.sentence_candidates.clear();
        self.chrome.sentence_candidate_source_indices.clear();
        self.chrome.set_seed_text(committed_snapshot.seed_text);
        self.chrome.move_caret_to_end();
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
    }

    pub(super) fn select_next_token(&mut self, index: usize) {
        let Some(token) = self.chrome.next_token_candidates.get(index).cloned() else {
            return;
        };
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
        );
    }

    pub(super) fn handle_text_input(&mut self, text: &str) {
        if self.chrome.active_input_mode != suzaku_map::ime::gpu::InputMode::VirtualKeyboard
            || !self.chrome.input_focused
        {
            return;
        }

        let mut accepted = String::new();
        for ch in text.chars() {
            if ch.is_ascii_alphanumeric()
                || ch == ' '
                || matches!(
                    ch,
                    '.' | ','
                        | '?'
                        | '!'
                        | '\''
                        | '-'
                        | '/'
                        | ':'
                        | ';'
                        | '('
                        | ')'
                        | '$'
                        | '&'
                        | '@'
                )
            {
                accepted.push(ch);
            }
        }

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
