use std::collections::HashMap;

use crate::ime::{
    Candidate, LanguagePlugin, build_sentence_candidates_for_variants, contains_all,
    expand_token_with_lexicon,
};

#[derive(Debug, Clone, PartialEq)]
pub struct EnglishLanguagePlugin {
    lexicon: HashMap<String, Vec<String>>,
}

impl Default for EnglishLanguagePlugin {
    fn default() -> Self {
        Self {
            lexicon: default_english_lexicon(),
        }
    }
}

impl LanguagePlugin for EnglishLanguagePlugin {
    fn id(&self) -> &str {
        "en"
    }

    fn display_name(&self) -> &str {
        "English"
    }

    fn expand_token(&self, token: &str, degraded: bool) -> Vec<String> {
        expand_token_with_lexicon(token, &self.lexicon, degraded)
    }

    fn build_candidates(
        &self,
        parts: &[String],
        seed_text: &str,
        confidence: f32,
    ) -> Vec<Candidate> {
        build_sentence_candidates_for_variants(
            parts,
            seed_text,
            confidence,
            english_sentence_variants,
        )
    }
}

fn default_english_lexicon() -> HashMap<String, Vec<String>> {
    HashMap::from([
        (
            "ni".into(),
            vec!["ni".into(), "you".into(), "need".into(), "new".into()],
        ),
        (
            "hao".into(),
            vec!["hao".into(), "how".into(), "hello".into()],
        ),
        (
            "nihao".into(),
            vec!["ni hao".into(), "hello".into(), "hi there".into()],
        ),
        (
            "xr".into(),
            vec!["XR".into(), "extended reality".into(), "spatial".into()],
        ),
        (
            "tablet".into(),
            vec!["tablet".into(), "pad".into(), "slate".into()],
        ),
        ("map".into(), vec!["map".into(), "mapping".into()]),
        ("ime".into(), vec!["IME".into(), "input method".into()]),
        ("keyboard".into(), vec!["keyboard".into(), "keypad".into()]),
        ("zen".into(), vec!["zen".into(), "calm".into()]),
        ("wo".into(), vec!["wo".into(), "I".into()]),
        ("ai".into(), vec!["ai".into(), "love".into()]),
        ("women".into(), vec!["women".into(), "we".into()]),
        ("shi".into(), vec!["shi".into(), "is".into(), "are".into()]),
        ("de".into(), vec!["de".into(), "of".into()]),
        ("zhong".into(), vec!["zhong".into(), "middle".into()]),
        ("guo".into(), vec!["guo".into(), "country".into()]),
        ("zhongguo".into(), vec!["zhong guo".into(), "China".into()]),
    ])
}

fn english_sentence_variants(phrase: &str) -> Vec<String> {
    let normalized = phrase.to_lowercase();
    if contains_all(&normalized, &["ni", "hao", "xr"])
        || contains_all(&normalized, &["hello", "xr"])
    {
        return vec![
            "hello XR input is ready for quick selection".to_string(),
            "hello XR users can tap the next sentence".to_string(),
            "hello XR typing now continues by selection".to_string(),
        ];
    }

    if contains_all(&normalized, &["ni", "hao"]) || normalized.contains("hello") {
        return vec![
            "hello how can I help today".to_string(),
            "hello welcome to the input panel".to_string(),
            "hello this sentence is ready to commit".to_string(),
        ];
    }

    if contains_all(&normalized, &["tablet", "ime"]) || normalized.contains("input method") {
        return vec![
            "tablet ime works well for quick sentence entry".to_string(),
            "tablet ime lets you tap to complete the sentence".to_string(),
            "tablet ime can keep suggesting the next phrase".to_string(),
        ];
    }

    if normalized == "xr"
        || normalized.contains("extended reality")
        || normalized.contains("spatial")
    {
        return vec![
            "XR input keeps the next phrase ready to tap".to_string(),
            "XR typing can continue without more key presses".to_string(),
            "XR sentence completion stays easy to select".to_string(),
        ];
    }

    if contains_all(&normalized, &["zhong", "guo"]) || normalized.contains("china") {
        return vec![
            "China appears here as a complete sentence candidate".to_string(),
            "China can be selected as the next full sentence".to_string(),
        ];
    }

    let seed = phrase.trim();
    if seed.is_empty() {
        return Vec::new();
    }

    vec![
        format!("{seed} is ready as the next full sentence"),
        format!("{seed} can continue by tapping the next suggestion"),
        format!("{seed} now expands into a complete candidate"),
    ]
}
