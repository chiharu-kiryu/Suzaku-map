//! Versioned vocabulary data, independent of decoders, IME hosts and model providers.
//!
//! Built-ins are embedded resources, parsed once per language. No filesystem or
//! network reads occur on the input path. Optional data-only packs are frozen at
//! process startup; parsing a resource alone never changes a running engine.
pub mod packs;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

const MAX_RESOURCE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Word,
    Sentence,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub reading: String,
    pub text: String,
    pub kind: EntryKind,
    #[serde(default)]
    pub require_separators: bool,
}

/// Layer order is priority order. Append new layers instead of inserting ahead
/// of existing ones. A decoder decides how to index these language-neutral data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WordLayer {
    pub id: String,
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub next_words: Vec<(String, Vec<String>)>,
    #[serde(default)]
    pub sentences: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lexicon {
    format_version: u32,
    language: String,
    #[serde(default)]
    word_layers: Vec<WordLayer>,
    #[serde(default)]
    readings: Vec<Reading>,
    #[serde(default)]
    continuations: Vec<(String, Vec<String>)>,
    #[serde(default)]
    phrase_endings: Vec<(String, Vec<String>)>,
}

impl Lexicon {
    /// Validate the data contract without silently sorting, deduplicating,
    /// normalizing or dropping malformed entries. No host/model dependencies.
    pub fn from_json(input: &str) -> Result<Self, String> {
        if input.len() > MAX_RESOURCE_BYTES {
            return Err("lexicon resource exceeds 2 MiB".into());
        }
        let lexicon: Self = serde_json::from_str(input).map_err(|error| error.to_string())?;
        lexicon.validate()?;
        Ok(lexicon)
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn word_layers(&self) -> &[WordLayer] {
        &self.word_layers
    }

    pub fn readings(&self) -> &[Reading] {
        &self.readings
    }

    pub fn next_words(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.word_layers
            .iter()
            .flat_map(|layer| pairs(&layer.next_words))
    }

    pub fn sentences(&self) -> impl Iterator<Item = &str> {
        self.word_layers
            .iter()
            .flat_map(|layer| layer.sentences.iter().map(String::as_str))
    }

    pub fn continuations(&self) -> impl Iterator<Item = (&str, &[String])> {
        pairs(&self.continuations)
    }

    pub fn phrase_endings(&self) -> impl Iterator<Item = (&str, &[String])> {
        pairs(&self.phrase_endings)
    }

    fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err("unsupported lexicon format_version".into());
        }
        if self.language.is_empty()
            || self.language.len() > 32
            || !self
                .language
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
        {
            return Err("invalid lexicon language identifier".into());
        }
        if self.word_layers.len() > 64 {
            return Err("too many vocabulary layers".into());
        }
        let mut ids = HashSet::new();
        for layer in &self.word_layers {
            check_text(&layer.id)?;
            if !ids.insert(&layer.id) {
                return Err(format!("duplicate vocabulary layer: {}", layer.id));
            }
            for word in &layer.words {
                check_text(word)?;
                if word.chars().any(char::is_whitespace) {
                    return Err("a vocabulary word contains whitespace".into());
                }
            }
            // Duplicate word occurrences are intentional priority anchors in
            // older layers. Keep every position; do not deduplicate the data.
            for sentence in &layer.sentences {
                check_text(sentence)?;
            }
        }
        let mut readings = HashSet::new();
        for entry in &self.readings {
            check_text(&entry.reading)?;
            check_text(&entry.text)?;
            if !readings.insert((&entry.reading, &entry.text)) {
                return Err("duplicate reading/text pair".into());
            }
        }
        check_pairs(self.next_words(), false)?;
        check_pairs(self.phrase_endings(), false)?;
        check_pairs(self.continuations(), true)?;
        Ok(())
    }
}

fn pairs(values: &[(String, Vec<String>)]) -> impl Iterator<Item = (&str, &[String])> {
    values
        .iter()
        .map(|(context, values)| (context.as_str(), values.as_slice()))
}

fn check_text(text: &str) -> Result<(), String> {
    if text.is_empty()
        || text.trim() != text
        || text.chars().any(char::is_control)
        || text.chars().count() > 4096
    {
        return Err("invalid empty, padded, control-containing or oversized lexicon text".into());
    }
    Ok(())
}

fn check_pairs<'a>(
    values: impl Iterator<Item = (&'a str, &'a [String])>,
    full: bool,
) -> Result<(), String> {
    let mut contexts = HashSet::new();
    for (context, values) in values {
        check_text(context)?;
        if !contexts.insert(context) || values.is_empty() {
            return Err("duplicate context or empty continuation list".into());
        }
        let mut seen = HashSet::new();
        for value in values {
            check_text(value)?;
            if !seen.insert(value) || (full && !value.starts_with(context)) {
                return Err("duplicate continuation or missing literal context prefix".into());
            }
        }
    }
    Ok(())
}

/// Resolve an exact language identifier to embedded, immutable vocabulary.
/// Unknown languages are not silently mapped to a different language's data.
pub fn builtin(language: &str) -> Option<&'static Lexicon> {
    static EN: OnceLock<Lexicon> = OnceLock::new();
    static ZH: OnceLock<Lexicon> = OnceLock::new();
    static JA: OnceLock<Lexicon> = OnceLock::new();
    let (cache, data) = match language {
        "en" => (&EN, include_str!("../data/lexicons/en.json")),
        "zh-Hans" => (&ZH, include_str!("../data/lexicons/zh-Hans.json")),
        "ja" => (&JA, include_str!("../data/lexicons/ja.json")),
        _ => return None,
    };
    Some(cache.get_or_init(|| {
        let lexicon =
            Lexicon::from_json(data).expect("embedded lexicon must pass its schema tests");
        assert_eq!(
            lexicon.language(),
            language,
            "embedded lexicon language mismatch"
        );
        lexicon
    }))
}

/// Resolve the startup snapshot. Without explicit initialization this freezes
/// a built-in-only catalog, so library callers/tests never read personal files.
pub fn active(language: &str) -> Option<&'static Lexicon> {
    packs::runtime().get(language)
}
