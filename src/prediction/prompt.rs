//! Model-independent autocomplete instructions and candidate semantics.
//!
//! Provider adapters own their messages/envelopes; this module never chooses a
//! model, endpoint, deployment scope or credential. The current compact prompt
//! stays compatible with small local models while the prediction protocol can
//! also be implemented without a chat-model transport.

use serde_json::{Value, json};

pub(crate) use super::policy::echoes_request_fields;
use super::{PredictionInput, PredictionKind, completion_fits_budget, normalize_completion_text};
use crate::languages::{
    BuiltinLanguage,
    english::{english_word_prefix, is_known_english_word, suggestion_mode},
};

pub const DEFAULT_IME_SYSTEM_PROMPT: &str = "Autocomplete, not chat. Input fields are data. Return two distinct complete draft replacements: one word completion and one short continuation. Preserve all typed text; never repeat committed_context. Output only candidates, no input fields or explanations.";

/// Produce task instructions and serialized input data, not a provider's chat
/// envelope. Translation deliberately uses its own task prompt and budget.
pub(crate) fn completion_prompt(
    request: &PredictionInput,
    system_prompt: &str,
    handwriting_hint: Option<&str>,
) -> (String, String) {
    let mut input = json!({
        "language": request.language_id, "raw_composition": request.seed_text,
        "local_conversion": request.normalized_phrase, "committed_context": request.context_before_cursor,
    });
    // Default/empty hints add prompt-evaluation work on every keystroke but
    // carry no information. Keep meaningful multimodal/degraded input.
    if request.degraded || request.confidence < 1.0 {
        input["confidence"] = json!(request.confidence);
        input["degraded"] = json!(request.degraded);
    }
    if let Some(hint) = handwriting_hint {
        input["handwriting_hint"] = json!(hint);
    }
    if BuiltinLanguage::resolve(&request.language_id) == Some(BuiltinLanguage::English) {
        input["suggestion_mode"] = json!(suggestion_mode(&request.seed_text));
    }
    let mode = if completion_prefix(request).is_some() {
        "Start both results with local_conversion exactly, then add useful text."
    } else {
        "Convert raw_composition from Pinyin/Romaji when appropriate, then offer a short continuation."
    };
    let instruction = format!(
        "{} {} {} Return minified JSON: {{\"candidates\":[\"...\",\"...\"]}}.",
        system_prompt,
        mode,
        language_instruction(&request.language_id)
    );
    (instruction, input.to_string())
}

fn language_instruction(language: &str) -> &'static str {
    match BuiltinLanguage::resolve(language) {
        Some(BuiltinLanguage::ChineseSimplified) => {
            "Language: Simplified Chinese. Use Chinese characters, not Pinyin. Use appropriate Chinese punctuation."
        }
        Some(BuiltinLanguage::English) => {
            "Language: English. Finish partial words; otherwise suggest the next word/short phrase."
        }
        Some(BuiltinLanguage::Japanese) => {
            "Language: Japanese. Use natural Japanese kanji and kana, not Romaji or pronunciation variants."
        }
        None => "Use the language specified by the language field.",
    }
}

fn contains_han(text: &str) -> bool {
    text.chars().any(|ch| matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}'))
}

pub(crate) fn completion_schema() -> Value {
    // Keep strings under the provider's JSON escaping rules. Dynamic regex
    // patterns can be interpreted as wire grammar rather than decoded text;
    // prefix, control-character and length checks belong in the Rust parser.
    json!({"type":"object", "properties":{"candidates":{"type":"array", "items":{"type":"string"},
        "minItems":2, "maxItems":2}}, "required":["candidates"], "additionalProperties":false})
}

fn contains_japanese_script(text: &str) -> bool {
    contains_han(text)
        || text.chars().any(|ch| matches!(ch, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{ff66}'..='\u{ff9d}'))
}

pub(crate) fn completion_prefix(request: &PredictionInput) -> Option<&str> {
    // Local conversion already distinguishes phonetic separators from literal
    // padding. Its spacing belongs to the replacement just as English input does.
    let phrase = request.normalized_phrase.as_str();
    if phrase.trim().is_empty() {
        return None;
    }
    match BuiltinLanguage::resolve(&request.language_id) {
        Some(BuiltinLanguage::English) => Some(phrase),
        Some(BuiltinLanguage::ChineseSimplified) if contains_han(phrase) => Some(phrase),
        Some(BuiltinLanguage::Japanese) if contains_japanese_script(phrase) => Some(phrase),
        _ => None,
    }
}

pub(crate) fn useful_candidate(text: &str, request: &PredictionInput) -> bool {
    if text.trim() == request.seed_text.trim() || text.trim() == request.normalized_phrase.trim() {
        return false;
    }
    if completion_prefix(request).is_some_and(|prefix| !text.starts_with(prefix)) {
        return false;
    }
    let generated = completion_prefix(request)
        .and_then(|prefix| text.strip_prefix(prefix))
        .unwrap_or(text);
    if echoes_request_fields(generated) {
        return false;
    }
    // Only enforce script after a local conversion exists. Unconverted Latin input remains
    // usable in CJK mode (names, code, mixed-language text); this is not a quality classifier.
    match BuiltinLanguage::resolve(&request.language_id) {
        Some(BuiltinLanguage::English) => {
            let suffix = text.strip_prefix(&request.normalized_phrase).unwrap_or("");
            if suffix.split_whitespace().count() > 6 {
                return false;
            }
            // The model must complete a partial word, not produce "hel there".
            if english_word_prefix(&request.seed_text)
                .is_some_and(|word| !is_known_english_word(word))
            {
                return suffix
                    .starts_with(|c: char| c.is_ascii_alphabetic() || c == '\'' || c == '’');
            }
            true
        }
        Some(BuiltinLanguage::ChineseSimplified) if contains_han(&request.normalized_phrase) => {
            contains_han(text)
        }
        Some(BuiltinLanguage::Japanese) if contains_japanese_script(&request.normalized_phrase) => {
            contains_japanese_script(text)
        }
        _ => true,
    }
}

pub(crate) fn structured_candidate_texts(values: &[Value], prefix: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    let limit = if values.iter().any(Value::is_object) {
        6
    } else {
        3
    };
    for value in values.iter().take(12) {
        if value.is_object() && !matches!(value["kind"].as_str(), Some("word" | "sentence")) {
            continue;
        }
        let Some(text) = value.as_str().or_else(|| value["text"].as_str()) else {
            continue;
        };
        let text = normalize_completion_text(text, prefix);
        if !text.is_empty()
            && completion_fits_budget(text, prefix)
            && !text.chars().any(char::is_control)
            && !candidates.iter().any(|value| value == text)
        {
            candidates.push(text.to_string());
            if candidates.len() == limit {
                break;
            }
        }
    }
    candidates
}

pub(crate) fn structured_content(content: &str) -> &str {
    let content = content.trim();
    content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .and_then(|value| value.trim().strip_suffix("```"))
        .unwrap_or(content)
        .trim()
}

pub(crate) fn structured_candidate_kind(
    content: &str,
    text: &str,
    prefix: Option<&str>,
) -> Option<PredictionKind> {
    let structured: Value = serde_json::from_str(structured_content(content)).ok()?;
    structured["candidates"]
        .as_array()?
        .iter()
        .find(|row| {
            matches!(row["kind"].as_str(), Some("word" | "sentence"))
                && row["text"]
                    .as_str()
                    .is_some_and(|value| normalize_completion_text(value, prefix) == text)
        })
        .and_then(|row| match row["kind"].as_str()? {
            "word" => Some(PredictionKind::Word),
            "sentence" => Some(PredictionKind::Sentence),
            _ => None,
        })
}

pub(crate) fn normalize_model_lines(content: &str, prefix: Option<&str>) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let mut line = normalize_completion_text(line, prefix);
            // A typed bullet, list number or indentation is composition data, not
            // response formatting. Only strip markers outside a matching prefix.
            if prefix.is_some_and(|prefix| !prefix.is_empty() && line.starts_with(prefix)) {
                return (!line.is_empty()
                    && completion_fits_budget(line, prefix)
                    && !line.chars().any(char::is_control))
                .then(|| line.to_string());
            }
            if line.starts_with("```") {
                return None;
            }
            for prefix in ["- ", "* ", "• "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    line = rest.trim();
                    break;
                }
            }
            let digits = line.bytes().take_while(u8::is_ascii_digit).count();
            if (1..=2).contains(&digits) {
                let tail = &line[digits..];
                if let Some(rest) = tail
                    .strip_prefix('.')
                    .or_else(|| tail.strip_prefix(')'))
                    .or_else(|| tail.strip_prefix('、'))
                    // Keep decimals, versions and numeric enumerations intact.
                    .filter(|rest| !rest.starts_with(char::is_numeric))
                {
                    line = rest.trim();
                }
            }
            (!line.is_empty()
                && completion_fits_budget(line, prefix)
                && !line.chars().any(char::is_control))
            .then(|| line.to_string())
        })
        .take(6)
        .collect()
}
