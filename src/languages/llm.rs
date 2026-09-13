use std::sync::Arc;

use crate::ime::{Candidate, LanguagePlugin, build_sentence_candidates_for_variants};

pub(crate) const MAX_PREDICTION_SEED_CHARS: usize = 256;
pub(crate) const MAX_GENERATED_CHARS: usize = 160;

/// A complete replacement may repeat the bounded input prefix plus a bounded
/// addition. Without that exact prefix, retain the small conversion-text cap.
/// This only checks size; callers still validate prefix semantics and controls.
pub(crate) fn completion_fits_budget(text: &str, prefix: Option<&str>) -> bool {
    if text.chars().count() <= MAX_GENERATED_CHARS {
        return true;
    }
    prefix
        .filter(|prefix| prefix.chars().count() <= MAX_PREDICTION_SEED_CHARS)
        .and_then(|prefix| text.strip_prefix(prefix))
        .is_some_and(|suffix| suffix.chars().count() <= MAX_GENERATED_CHARS)
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCompletionRequest {
    pub language_id: String,
    pub seed_text: String,
    pub normalized_phrase: String,
    /// Only this focused session's own recent commits; never desktop-wide surrounding text.
    pub context_before_cursor: String,
    pub confidence: f32,
    pub degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LlmCompletion {
    pub text: String,
    pub score_bias: f32,
    /// Older/plain-text providers may omit this; the language profile then classifies it.
    pub kind: Option<crate::ime::candidate_mix::CandidateKind>,
}

/// Trim response padding, never a prefix that already belongs to the user's composition.
/// This preserves exact indentation/spacing without inventing text the provider omitted.
pub(crate) fn normalize_completion_text<'a>(text: &'a str, prefix: Option<&str>) -> &'a str {
    if let Some(prefix) = prefix.filter(|prefix| !prefix.is_empty() && text.starts_with(prefix)) {
        &text[..text.trim_end().len().max(prefix.len())]
    } else {
        text.trim()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProviderError {
    InvalidEndpoint,
    Unavailable,
    Timeout,
    HttpStatus(u16),
    InvalidResponse,
    ResponseTooLarge,
    NoCandidates,
    NoLocalModel,
    CloudConsentRequired,
    MissingCredentials,
}

impl std::fmt::Display for LlmProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "模型配置无效：本地需回环 HTTP，云端需 HTTPS 和明确模型",
            Self::Unavailable => "模型服务不可用或安全连接失败，本地候选仍可使用",
            Self::Timeout => "模型请求超时，本地候选仍可使用",
            Self::HttpStatus(401 | 403) => "模型服务拒绝授权，请检查密钥与访问权限",
            Self::HttpStatus(404) => "模型或接口不存在，请检查模型配置",
            Self::HttpStatus(_) => "模型服务返回错误，请检查服务状态",
            Self::InvalidResponse => "模型响应格式无效，已保留本地候选",
            Self::ResponseTooLarge => "模型响应过大，已拒绝处理",
            Self::NoCandidates => "模型未返回有效候选，已保留本地候选",
            Self::NoLocalModel => "未发现可用本机模型，请启动本地服务或指定模型；未访问云端",
            Self::CloudConsentRequired => "云端联想尚未授权，输入内容不会发送到云端",
            Self::MissingCredentials => "模型密钥环境变量未设置或格式无效",
        })
    }
}
impl std::error::Error for LlmProviderError {}

pub trait LlmCompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion>;

    fn generate_checked(
        &self,
        request: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Ok(self.generate(request))
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
