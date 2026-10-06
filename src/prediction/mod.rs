//! Provider-neutral, versioned input prediction contract.
//!
//! A candidate replaces the complete editable draft. It is never a token to
//! append, a commit command, a UI label or a tool call. Endpoint, model identity,
//! credentials and wire envelopes belong to adapters, not this contract.
//! Version 1 is bounded, non-streaming and uses only this focus session's own
//! committed context. Serde support permits other transports without requiring
//! a chat model to emit this envelope or to echo request IDs itself.

pub(crate) mod policy;
pub(crate) mod prompt;

use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_PREDICTION_SEED_CHARS: usize = 256;
pub const MAX_GENERATED_CHARS: usize = 160;
pub const MAX_CONTEXT_CHARS: usize = 160;
pub const MAX_CANDIDATES: usize = 6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionInput {
    pub language_id: String,
    pub seed_text: String,
    pub normalized_phrase: String,
    /// Only this focused session's own recent commits; never surrounding desktop text.
    pub context_before_cursor: String,
    pub confidence: f32,
    pub degraded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionOutput {
    CompleteDraftReplacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictionLimits {
    pub max_candidates: usize,
    /// Unicode scalar values added after an exact bounded conversion prefix.
    /// Without that prefix, this is the limit for the entire replacement.
    pub max_generated_chars: usize,
}

impl Default for PredictionLimits {
    fn default() -> Self {
        Self {
            max_candidates: MAX_CANDIDATES,
            max_generated_chars: MAX_GENERATED_CHARS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionRequest {
    pub version: u32,
    /// Assigned by the caller; adapters echo it, not the language model.
    pub request_id: u64,
    pub input: PredictionInput,
    pub output: PredictionOutput,
    pub limits: PredictionLimits,
}

impl PredictionRequest {
    pub fn new(request_id: u64, input: PredictionInput) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            request_id,
            input,
            output: PredictionOutput::CompleteDraftReplacement,
            limits: PredictionLimits::default(),
        }
    }

    pub fn validate(&self) -> Result<(), PredictionError> {
        if self.version != PROTOCOL_VERSION {
            return Err(PredictionError::UnsupportedProtocol);
        }
        if self.input.language_id.trim().is_empty()
            || self.input.language_id.len() > 64
            || self.input.language_id.chars().any(char::is_control)
            || self.input.seed_text.trim().is_empty()
            || self.input.seed_text.chars().count() > MAX_PREDICTION_SEED_CHARS
            || self.input.normalized_phrase.chars().count()
                > MAX_PREDICTION_SEED_CHARS + MAX_GENERATED_CHARS
            || self.input.context_before_cursor.chars().count() > MAX_CONTEXT_CHARS
            || !self.input.confidence.is_finite()
            || !(0.0..=1.0).contains(&self.input.confidence)
            || !(1..=MAX_CANDIDATES).contains(&self.limits.max_candidates)
            || !(1..=MAX_GENERATED_CHARS).contains(&self.limits.max_generated_chars)
        {
            return Err(PredictionError::InvalidRequest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionKind {
    Word,
    Sentence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionCandidate {
    pub text: String,
    /// Relative ranking hint, not a probability. The engine clamps its influence.
    pub score_bias: f32,
    /// None permits conservative language classification for untyped backends.
    pub kind: Option<PredictionKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionResponse {
    pub version: u32,
    pub request_id: u64,
    pub candidates: Vec<PredictionCandidate>,
}

impl PredictionResponse {
    pub fn new(request: &PredictionRequest, candidates: Vec<PredictionCandidate>) -> Self {
        Self {
            version: request.version,
            request_id: request.request_id,
            candidates,
        }
    }

    /// Structural/budget validation, not a claim of linguistic quality. The
    /// adapter's task policy and the engine's language ranking remain separate.
    pub fn validate_for(&self, request: &PredictionRequest) -> Result<(), PredictionError> {
        request.validate()?;
        if self.version != PROTOCOL_VERSION {
            return Err(PredictionError::UnsupportedProtocol);
        }
        if self.request_id != request.request_id
            || self.candidates.len() > request.limits.max_candidates
            || self.candidates.iter().any(|candidate| {
                candidate.text.trim().is_empty()
                    || candidate.text.chars().any(char::is_control)
                    || !candidate.score_bias.is_finite()
                    || !fits_budget(
                        &candidate.text,
                        Some(&request.input.normalized_phrase),
                        request.limits.max_generated_chars,
                    )
            })
        {
            return Err(PredictionError::InvalidResponse);
        }
        if self.candidates.is_empty() {
            return Err(PredictionError::NoCandidates);
        }
        Ok(())
    }
}

pub trait PredictionProvider: Send + Sync {
    /// Called only on a worker. Implementations should honour cancellation and
    /// bound their I/O; a noncooperative provider cannot be forcibly interrupted.
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredictionError {
    Cancelled,
    InvalidRequest,
    UnsupportedProtocol,
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

impl std::fmt::Display for PredictionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "模型请求已取消，本地输入不受影响",
            Self::InvalidRequest => "预测请求无效或超出预算，已保留本地候选",
            Self::UnsupportedProtocol => "预测协议版本不受支持，已保留本地候选",
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
impl std::error::Error for PredictionError {}

/// One-way cancellation; signalling never waits for a provider on the input thread.
#[derive(Clone, Debug, Default)]
pub struct PredictionCancellation(Arc<AtomicBool>);

impl PredictionCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn check(&self) -> Result<(), PredictionError> {
        if self.is_cancelled() {
            Err(PredictionError::Cancelled)
        } else {
            Ok(())
        }
    }
}

pub(crate) fn fits_budget(text: &str, prefix: Option<&str>, generated_limit: usize) -> bool {
    text.chars().count() <= generated_limit
        || prefix
            .filter(|prefix| prefix.chars().count() <= MAX_PREDICTION_SEED_CHARS)
            .and_then(|prefix| text.strip_prefix(prefix))
            .is_some_and(|suffix| suffix.chars().count() <= generated_limit)
}

pub(crate) fn completion_fits_budget(text: &str, prefix: Option<&str>) -> bool {
    fits_budget(text, prefix, MAX_GENERATED_CHARS)
}

/// Trim provider padding, never a prefix already owned by the composition.
pub(crate) fn normalize_completion_text<'a>(text: &'a str, prefix: Option<&str>) -> &'a str {
    if let Some(prefix) = prefix.filter(|prefix| !prefix.is_empty() && text.starts_with(prefix)) {
        &text[..text.trim_end().len().max(prefix.len())]
    } else {
        text.trim()
    }
}
