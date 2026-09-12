//! Explicit text translation, independent of the three composition language engines.
use super::llm::LlmProviderError;

mod worker;
pub use worker::TranslationWorker;

pub const MAX_TRANSLATION_INPUT_CHARS: usize = 1000;
pub const MAX_TRANSLATION_OUTPUT_CHARS: usize = 2000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TranslationLanguage {
    English,
    #[default]
    ChineseSimplified,
    Japanese,
    Korean,
    Spanish,
    French,
    German,
    Portuguese,
}

impl TranslationLanguage {
    pub const ALL: [Self; 8] = [
        Self::English,
        Self::ChineseSimplified,
        Self::Japanese,
        Self::Korean,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::Portuguese,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::ChineseSimplified => "zh-Hans",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Portuguese => "pt",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::ChineseSimplified => "Simplified Chinese",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::Spanish => "Spanish",
            Self::French => "French",
            Self::German => "German",
            Self::Portuguese => "Portuguese",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::English => "EN",
            Self::ChineseSimplified => "中",
            Self::Japanese => "日",
            Self::Korean => "한",
            Self::Spanish => "ES",
            Self::French => "FR",
            Self::German => "DE",
            Self::Portuguese => "PT",
        }
    }

    pub const fn script_instruction(self) -> &'static str {
        match self {
            Self::ChineseSimplified => {
                "Write natural Simplified Chinese (简体中文), using Chinese characters, not English or Pinyin."
            }
            Self::Japanese => {
                "Write natural Japanese (日本語), using Kanji and Kana, not English or Romaji."
            }
            Self::Korean => {
                "Write natural Korean (한국어) using Hangul, not English or romanization."
            }
            _ => "Use natural wording in the target language.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationRequest {
    pub text: String,
    /// None asks the model to detect the source language, not a device-wide locale.
    pub source: Option<TranslationLanguage>,
    pub target: TranslationLanguage,
}

impl TranslationRequest {
    pub fn validate(&self) -> Result<(), TranslationError> {
        if self.text.trim().is_empty() || self.text.chars().any(invalid_text_character) {
            return Err(TranslationError::InvalidText);
        }
        if self.text.chars().count() > MAX_TRANSLATION_INPUT_CHARS {
            return Err(TranslationError::InputTooLong);
        }
        Ok(())
    }

    /// A conservative script guard, not a claim of semantic language detection.
    /// Leave identifiers/names alone; catch untranslated Latin sentences for CJK targets.
    pub fn validate_target_script(&self, text: &str) -> Result<(), TranslationError> {
        if self.source == Some(self.target) || self.text.split_whitespace().count() < 2 {
            return Ok(());
        }
        let has_script = |language| {
            text.chars().any(|ch| match language {
            TranslationLanguage::ChineseSimplified => matches!(ch, '\u{3400}'..='\u{9fff}' | '\u{20000}'..='\u{3134f}'),
            TranslationLanguage::Japanese => matches!(ch, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ff66}'..='\u{ff9f}'),
            TranslationLanguage::Korean => matches!(ch, '\u{1100}'..='\u{11ff}' | '\u{3130}'..='\u{318f}' | '\u{ac00}'..='\u{d7af}'),
            _ => true,
        })
        };
        if text.chars().any(|ch| ch.is_ascii_alphabetic()) && !has_script(self.target) {
            return Err(TranslationError::WrongLanguage);
        }
        Ok(())
    }
}

pub(crate) fn invalid_text_character(ch: char) -> bool {
    ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')
}

pub(crate) fn validate_translation_output(text: String) -> Result<String, TranslationError> {
    if text.trim().is_empty() || text.chars().any(invalid_text_character) {
        return Err(LlmProviderError::InvalidResponse.into());
    }
    if text.chars().count() > MAX_TRANSLATION_OUTPUT_CHARS {
        return Err(LlmProviderError::ResponseTooLarge.into());
    }
    Ok(text)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationError {
    InvalidText,
    InputTooLong,
    WrongLanguage,
    Provider(LlmProviderError),
}

impl From<LlmProviderError> for TranslationError {
    fn from(value: LlmProviderError) -> Self {
        Self::Provider(value)
    }
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use LlmProviderError::*;
        f.write_str(match self {
            Self::InvalidText => {
                "Enter text to translate; unsupported control characters are not allowed."
            }
            Self::InputTooLong => {
                "Translate up to 1,000 characters at a time; your draft is unchanged."
            }
            Self::WrongLanguage => "The model did not use the target language. Try a model with stronger multilingual support.",
            Self::Provider(Timeout) => {
                "Translation timed out. Retry explicitly when the model is ready."
            }
            Self::Provider(CloudConsentRequired) => {
                "Cloud access is not authorized. No text was sent."
            }
            Self::Provider(MissingCredentials | HttpStatus(401 | 403)) => {
                "Model authorization failed. Check the configured credentials."
            }
            Self::Provider(NoLocalModel | Unavailable) => {
                "Local model or configured service unavailable. Check model settings."
            }
            Self::Provider(InvalidEndpoint | HttpStatus(404)) => {
                "Check the configured model and endpoint."
            }
            Self::Provider(_) => "No complete translation received. Your draft is unchanged.",
        })
    }
}
impl std::error::Error for TranslationError {}

pub trait TranslationProvider: Send + Sync {
    fn translate(&self, request: &TranslationRequest) -> Result<String, TranslationError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_guard_rejects_untranslated_latin_sentences_but_is_not_a_quality_score() {
        for (target, valid) in [
            (TranslationLanguage::ChineseSimplified, "你好，世界！"),
            (TranslationLanguage::Japanese, "こんにちは、世界！"),
            (TranslationLanguage::Korean, "안녕하세요 세계!"),
        ] {
            let request = TranslationRequest {
                text: "Hello, world!".into(),
                source: Some(TranslationLanguage::English),
                target,
            };
            assert!(request.validate_target_script(valid).is_ok());
            assert_eq!(
                request.validate_target_script("hello, world!"),
                Err(TranslationError::WrongLanguage)
            );
            let name = TranslationRequest {
                text: "Suzaku".into(),
                ..request
            };
            assert!(name.validate_target_script("Suzaku").is_ok());
        }
    }
    #[test]
    fn eight_translation_languages_are_distinct_from_composition_languages() {
        let ids: std::collections::HashSet<_> = TranslationLanguage::ALL.map(|l| l.id()).into();
        assert_eq!(ids.len(), 8);
        assert_eq!(crate::languages::BuiltinLanguage::ALL.len(), 3);
        for source in TranslationLanguage::ALL {
            for target in TranslationLanguage::ALL {
                assert!(
                    TranslationRequest {
                        text: "Hello 世界\n안녕하세요".into(),
                        source: Some(source),
                        target
                    }
                    .validate()
                    .is_ok()
                );
            }
        }
    }
    #[test]
    fn translation_limits_count_unicode_and_never_truncate_input() {
        let mut request = TranslationRequest {
            text: "中".repeat(1000),
            source: None,
            target: TranslationLanguage::English,
        };
        assert!(request.validate().is_ok());
        request.text.push('文');
        assert_eq!(request.validate(), Err(TranslationError::InputTooLong));
        request.text = "private\0text".into();
        assert_eq!(request.validate(), Err(TranslationError::InvalidText));
        request.text = " \n\t".into();
        assert_eq!(request.validate(), Err(TranslationError::InvalidText));
    }
}
