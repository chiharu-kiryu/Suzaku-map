use std::sync::Arc;

use crate::ime::{Candidate, LanguagePlugin, build_sentence_candidates_for_variants};
use crate::languages::BuiltinLanguage;

// Source-compatible legacy API. New backends implement PredictionProvider.
#[cfg(test)]
use crate::prediction::MAX_GENERATED_CHARS;
pub(crate) use crate::prediction::{
    MAX_PREDICTION_SEED_CHARS, completion_fits_budget, normalize_completion_text,
};
pub use crate::prediction::{
    PredictionCancellation as LlmCancellation, PredictionError as LlmProviderError,
    PredictionInput as LlmCompletionRequest,
};
use crate::prediction::{
    PredictionCandidate, PredictionKind, PredictionProvider, PredictionRequest, PredictionResponse,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LlmCompletion {
    pub text: String,
    pub score_bias: f32,
    /// Older/plain-text providers may omit this; the language profile then classifies it.
    pub kind: Option<crate::ime::candidate_mix::CandidateKind>,
}

pub trait LlmCompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion>;

    fn generate_checked(
        &self,
        request: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Ok(self.generate(request))
    }

    /// Existing providers remain source compatible. Override to interrupt an
    /// in-flight call cooperatively; the default only guards entry and return.
    fn generate_cancellable(
        &self,
        request: &LlmCompletionRequest,
        cancellation: &LlmCancellation,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        cancellation.check()?;
        let result = self.generate_checked(request);
        cancellation.check()?;
        result
    }
}

impl From<LlmCompletion> for PredictionCandidate {
    fn from(completion: LlmCompletion) -> Self {
        use crate::ime::candidate_mix::CandidateKind;
        Self {
            text: completion.text,
            score_bias: completion.score_bias,
            kind: match completion.kind {
                Some(CandidateKind::Word) => Some(PredictionKind::Word),
                Some(CandidateKind::Sentence) => Some(PredictionKind::Sentence),
                _ => None,
            },
        }
    }
}

impl From<PredictionCandidate> for LlmCompletion {
    fn from(candidate: PredictionCandidate) -> Self {
        use crate::ime::candidate_mix::CandidateKind;
        Self {
            text: candidate.text,
            score_bias: candidate.score_bias,
            kind: candidate.kind.map(|kind| match kind {
                PredictionKind::Word => CandidateKind::Word,
                PredictionKind::Sentence => CandidateKind::Sentence,
            }),
        }
    }
}

/// Compatibility bridge for existing custom providers; no new methods required.
pub struct LegacyPredictionAdapter(pub Arc<dyn LlmCompletionProvider>);

impl PredictionProvider for LegacyPredictionAdapter {
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &LlmCancellation,
    ) -> Result<PredictionResponse, LlmProviderError> {
        cancellation.check()?;
        request.validate()?;
        let result = self.0.generate_cancellable(&request.input, cancellation);
        cancellation.check()?;
        let completions = result?;
        // Legacy lists were filtered individually by the engine. Keep that
        // behaviour while ensuring only bounded, well-formed protocol rows cross
        // the new boundary. Do not let a bad row erase other valid suggestions.
        let prefix = Some(
            if BuiltinLanguage::resolve(&request.input.language_id)
                == Some(BuiltinLanguage::English)
            {
                request.input.seed_text.as_str()
            } else {
                request.input.normalized_phrase.as_str()
            },
        );
        let candidates = completions
            .into_iter()
            .take(crate::prediction::MAX_CANDIDATES)
            .filter_map(|completion| {
                let mut candidate: PredictionCandidate = completion.into();
                candidate.text = normalize_completion_text(&candidate.text, prefix).to_owned();
                PredictionResponse::new(request, vec![candidate.clone()])
                    .validate_for(request)
                    .is_ok()
                    .then_some(candidate)
            })
            .take(request.limits.max_candidates)
            .collect();
        let response = PredictionResponse::new(request, candidates);
        response.validate_for(request)?;
        cancellation.check()?;
        Ok(response)
    }
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
            context_before_cursor: String::new(),
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
                        kind: completion.kind.unwrap_or_default(),
                        source: crate::ime::candidate_mix::CandidateSource::Model,
                    }
                })
                .collect();
        }

        build_sentence_candidates_for_variants(parts, seed_text, confidence, self.fallback_variants)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_adapter_keeps_raw_english_spacing_when_conversion_differs() {
        struct Provider;
        impl LlmCompletionProvider for Provider {
            fn provider_id(&self) -> &str {
                "synthetic"
            }

            fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
                vec![
                    LlmCompletion::default(),
                    LlmCompletion {
                        text: "  help  ".into(),
                        score_bias: 0.5,
                        kind: None,
                    },
                ]
            }
        }
        let mut request = PredictionRequest::new(
            1,
            LlmCompletionRequest {
                language_id: "en".into(),
                seed_text: "  hel".into(),
                normalized_phrase: "  helmet".into(),
                context_before_cursor: String::new(),
                confidence: 1.0,
                degraded: false,
            },
        );
        request.limits.max_candidates = 1;
        for language in ["en", "en-US", "en_GB"] {
            request.input.language_id = language.into();
            let response = LegacyPredictionAdapter(Arc::new(Provider))
                .predict(&request, &LlmCancellation::default())
                .unwrap();
            assert_eq!(response.candidates.len(), 1, "{language}");
            assert_eq!(response.candidates[0].text, "  help", "{language}");
            response.validate_for(&request).unwrap();
        }
    }

    #[test]
    fn completion_budget_bounds_generated_text_without_charging_for_the_typed_prefix() {
        for character in ['a', '字', '😀'] {
            let prefix = character.to_string().repeat(MAX_PREDICTION_SEED_CHARS);
            let suffix = "新".repeat(MAX_GENERATED_CHARS);
            let full = format!("{prefix}{suffix}");
            assert!(completion_fits_budget(&full, Some(&prefix)));
            assert!(!completion_fits_budget(&format!("{full}x"), Some(&prefix)));
            assert!(!completion_fits_budget(&full, None));
            assert!(!completion_fits_budget(&full, Some("mismatched")));
            let oversized_prefix = format!("{prefix}{character}");
            assert!(!completion_fits_budget(
                &format!("{oversized_prefix}{suffix}"),
                Some(&oversized_prefix)
            ));
        }
        assert!(completion_fits_budget(
            &"字".repeat(MAX_GENERATED_CHARS),
            None
        ));
        assert!(!completion_fits_budget(
            &"字".repeat(MAX_GENERATED_CHARS + 1),
            None
        ));
    }

    #[test]
    fn response_padding_never_removes_an_exact_typed_prefix() {
        for (text, prefix, expected) in [
            ("  hello  ", Some("  hel"), "  hello"),
            ("hello   ", Some("hello  "), "hello  "),
            (
                "\u{3000}hello\u{3000}",
                Some("\u{3000}hel"),
                "\u{3000}hello",
            ),
            ("hello\u{3000} ", Some("hello\u{3000}"), "hello\u{3000}"),
            ("   ", Some("  "), "  "),
            (" hello ", Some("  hel"), "hello"),
            ("  hello  ", Some(""), "hello"),
            ("  你好  ", None, "你好"),
            ("", Some("hel"), ""),
        ] {
            assert_eq!(normalize_completion_text(text, prefix), expected);
        }
    }
}
