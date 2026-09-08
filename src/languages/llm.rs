use std::sync::Arc;

use crate::ime::{Candidate, LanguagePlugin, build_sentence_candidates_for_variants};

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

#[derive(Debug, Clone, PartialEq)]
pub struct LlmCompletion {
    pub text: String,
    pub score_bias: f32,
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
}

impl std::fmt::Display for LlmProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidEndpoint => "模型地址无效，只允许本机 HTTP 服务",
            Self::Unavailable => "本机模型服务未运行或连接已断开",
            Self::Timeout => "模型请求超时，本地候选仍可使用",
            Self::HttpStatus(404) => "模型或接口不存在，请检查已安装的 Llama 模型",
            Self::HttpStatus(_) => "模型服务返回错误，请检查本机服务状态",
            Self::InvalidResponse => "模型响应格式无效，已保留本地候选",
            Self::ResponseTooLarge => "模型响应过大，已拒绝处理",
            Self::NoCandidates => "模型未返回有效候选，已保留本地候选",
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
                    }
                })
                .collect();
        }

        build_sentence_candidates_for_variants(parts, seed_text, confidence, self.fallback_variants)
    }
}
