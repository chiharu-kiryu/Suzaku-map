//! Input-preservation rules shared by every prediction backend.
//! These protect draft semantics; they are not a general language-quality test.
use super::PredictionInput;
use crate::languages::{
    BuiltinLanguage,
    english::{english_word_prefix, has_word_completion, is_known_english_word},
};

pub(crate) fn preserves_input(text: &str, input: &PredictionInput) -> bool {
    if text.trim() == input.seed_text.trim() {
        return false;
    }
    let generated = if BuiltinLanguage::resolve(&input.language_id)
        == Some(BuiltinLanguage::English)
    {
        // The raw draft, not a longer local completion, belongs to the user.
        // Alternate completions may differ after that exact prefix.
        let Some(suffix) = text.strip_prefix(&input.seed_text) else {
            return false;
        };
        if english_word_prefix(&input.seed_text)
            .is_some_and(|word| !is_known_english_word(word) && has_word_completion(word))
            && !suffix.starts_with(|ch: char| ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’'))
        {
            return false;
        }
        suffix
    } else {
        // CJK phonetic conversion and homophones legitimately replace Roman
        // input. Do not pin these backends to the primary local conversion.
        // An alternative conversion can retain a literal prefix without
        // matching the entire raw reading or the primary local conversion.
        // Exclude only the exact shared prefix, at a Unicode boundary.
        let shared_bytes: usize = text
            .chars()
            .zip(input.seed_text.chars())
            .take_while(|(actual, typed)| actual == typed)
            .map(|(ch, _)| ch.len_utf8())
            .sum();
        let preserved = if text.starts_with(&input.normalized_phrase) {
            shared_bytes.max(input.normalized_phrase.len())
        } else {
            shared_bytes
        };
        &text[preserved..]
    };
    !echoes_request_fields(generated)
}

pub(crate) fn echoes_request_fields(text: &str) -> bool {
    // Two distinct quoted protocol fields with colons indicate request data
    // echoed inside a candidate. The caller excludes already typed text.
    [
        "raw_composition",
        "local_conversion",
        "committed_context",
        "suggestion_mode",
        "handwriting_hint",
        "seed_text",
        "normalized_phrase",
        "context_before_cursor",
    ]
    .iter()
    .filter(|field| {
        ['\'', '"'].iter().any(|quote| {
            let key = format!("{quote}{field}{quote}");
            text.match_indices(&key)
                .any(|(index, _)| text[index + key.len()..].trim_start().starts_with(':'))
        })
    })
    .take(2)
    .count()
        == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(language: &str, raw: &str, local: &str) -> PredictionInput {
        PredictionInput {
            language_id: language.into(),
            seed_text: raw.into(),
            normalized_phrase: local.into(),
            context_before_cursor: String::new(),
            confidence: 1.0,
            degraded: false,
        }
    }

    #[test]
    fn local_vocabulary_does_not_limit_independent_prediction_coverage() {
        let request = input("en-US", "  hel", "  hello");
        assert!(preserves_input("  heliography", &request));
        assert!(!preserves_input("  hel there", &request));
        assert!(!preserves_input("heliography", &request));
        for seed in ["newest", "qzx"] {
            assert!(preserves_input(
                &format!("{seed} useful result."),
                &input("en", seed, seed)
            ));
        }
    }

    #[test]
    fn alternative_cjk_conversions_preserve_typed_metadata_and_unicode_prefixes() {
        let prefix = "😀 café {\"raw_composition\":\"typed\",\"local_conversion\":\"data\"} ";
        for (language, reading, local, alternative) in [
            ("zh-Hans", "shijian", "时间", "实践让学习更扎实。"),
            ("ja", "hashi", "橋", "箸を使って食べます。"),
        ] {
            for conversion in [format!("{prefix}{local}"), String::new()] {
                let request = input(language, &format!("{prefix}{reading}"), &conversion);
                assert!(preserves_input(&format!("{prefix}{alternative}"), &request));
                assert!(!preserves_input(
                    &format!("{prefix}{alternative} {{\"seed_text\":1,\"normalized_phrase\":2}}"),
                    &request
                ));
            }
        }
    }

    #[test]
    fn protocol_field_echo_checks_only_generated_keys_not_typed_text() {
        let prefix = "{\"seed_text\":\"typed\",\"normalized_phrase\":\"data\"} hello ";
        let request = input("en", prefix, prefix);
        assert!(preserves_input(&format!("{prefix}reader."), &request));
        assert!(!preserves_input(
            &format!("{prefix}{{\"seed_text\":1,\"normalized_phrase\":2}}"),
            &request
        ));
        assert!(preserves_input(
            &format!("{prefix}mentions seed_text and normalized_phrase."),
            &request
        ));
    }
}
