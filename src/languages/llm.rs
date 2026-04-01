use std::sync::Arc;

use crate::ime::{Candidate, LanguagePlugin, build_sentence_candidates_for_variants};

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCompletionRequest {
    pub language_id: String,
    pub seed_text: String,
    pub normalized_phrase: String,
    pub confidence: f32,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCompletion {
    pub text: String,
    pub score_bias: f32,
}

pub trait LlmCompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion>;
}

#[derive(Clone)]
pub struct LlmLanguagePlugin {
    language_id: String,
    display_name: String,
    provider: Arc<dyn LlmCompletionProvider>,
    fallback_variants: fn(&str) -> Vec<String>,
}

impl LlmLanguagePlugin {
    pub fn new(
        language_id: impl Into<String>,
        display_name: impl Into<String>,
        provider: Arc<dyn LlmCompletionProvider>,
        fallback_variants: fn(&str) -> Vec<String>,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            display_name: display_name.into(),
            provider,
            fallback_variants,
        }
    }
}

impl LanguagePlugin for LlmLanguagePlugin {
    fn id(&self) -> &str {
        &self.language_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn expand_token(&self, token: &str, degraded: bool) -> Vec<String> {
        if degraded {
            vec![token.to_string()]
        } else {
            vec![token.to_string(), token.to_lowercase()]
        }
    }

    fn build_candidates(
        &self,
        parts: &[String],
        seed_text: &str,
        confidence: f32,
    ) -> Vec<Candidate> {
        let phrase = parts.join(" ").replace("  ", " ").trim().to_string();
        let request = LlmCompletionRequest {
            language_id: self.language_id.clone(),
            seed_text: seed_text.to_string(),
            normalized_phrase: phrase.clone(),
            confidence,
            degraded: confidence < 0.45,
        };
        let llm_variants = self.provider.generate(&request);

        if !llm_variants.is_empty() {
            return llm_variants
                .into_iter()
                .map(|completion| {
                    let word_count = completion.text.split_whitespace().count();
                    let sentence_bonus = if word_count >= 4 { 0.22 } else { 0.12 };
                    Candidate {
                        label: completion.text.clone(),
                        text: completion.text,
                        score: confidence + sentence_bonus + completion.score_bias,
                    }
                })
                .collect();
        }

        build_sentence_candidates_for_variants(parts, seed_text, confidence, self.fallback_variants)
    }
}
