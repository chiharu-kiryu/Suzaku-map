//! IBus ranking and presentation. Scores are bounded ranking weights, not probabilities.
//! Decoration is display-only: a commit always uses Candidate::text.
use super::Candidate;
use crate::languages::{
    BuiltinLanguage, chinese, english, japanese,
    llm::{
        LlmCompletion, MAX_PREDICTION_SEED_CHARS, completion_fits_budget, normalize_completion_text,
    },
};
use std::collections::HashSet;

pub const PAGE_SIZE: usize = 6;
pub const PREVIEW_CHARS: usize = 42;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CandidateKind {
    #[default]
    Unspecified,
    Literal,
    Word,
    Sentence,
}
impl CandidateKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Unspecified => "unknown",
            Self::Literal => "literal",
            Self::Word => "word",
            Self::Sentence => "sentence",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unknown" => Some(Self::Unspecified),
            "literal" => Some(Self::Literal),
            "word" => Some(Self::Word),
            "sentence" => Some(Self::Sentence),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CandidateSource {
    #[default]
    Local,
    Model,
}
impl CandidateSource {
    pub fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Model => "model",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "local" => Some(Self::Local),
            "model" => Some(Self::Model),
            _ => None,
        }
    }
}

/// Legacy providers without typed output use conservative language-specific classification.
pub fn classify(language: &str, seed: &str, text: &str) -> CandidateKind {
    if text == seed {
        return if language == "en"
            && english::english_word_prefix(seed).is_some_and(english::is_known_english_word)
        {
            CandidateKind::Word
        } else {
            CandidateKind::Literal
        };
    }
    match BuiltinLanguage::resolve(language) {
        Some(BuiltinLanguage::English) => {
            let left = english::english_word_prefix(seed)
                .map_or(seed, |prefix| &seed[..seed.len() - prefix.len()]);
            if text.strip_prefix(left).is_some_and(|tail| {
                !tail.is_empty()
                    && tail
                        .chars()
                        .all(|c| c.is_ascii_alphabetic() || matches!(c, '\'' | '’'))
            }) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            }
        }
        Some(BuiltinLanguage::ChineseSimplified) => {
            if chinese::is_dictionary_word(text) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            }
        }
        Some(BuiltinLanguage::Japanese) => {
            if japanese::is_dictionary_word(text) {
                CandidateKind::Word
            } else if text.chars().any(|ch| ch.is_ascii_alphabetic())
                || text.chars().all(|ch| matches!(ch, '\u{3040}'..='\u{30ff}'))
            {
                // An unfinished phonetic reading is not a sentence. Leave it untyped
                // until the dictionary or a typed model result identifies its role.
                CandidateKind::Unspecified
            } else {
                CandidateKind::Sentence
            }
        }
        None => CandidateKind::Word,
    }
}

fn candidate(text: String, kind: CandidateKind, weight: f32) -> Candidate {
    Candidate {
        label: text.clone(),
        text,
        score: weight,
        kind,
        source: CandidateSource::Local,
    }
}

pub fn offline(
    language: &str,
    seed: &str,
    context: &str,
    base: Vec<Candidate>,
    limit: usize,
) -> Vec<Candidate> {
    if seed.chars().count() > MAX_PREDICTION_SEED_CHARS {
        return vec![candidate(seed.into(), CandidateKind::Literal, 100.0)];
    }
    let mut pool: Vec<_> = base
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let kind = if item.kind == CandidateKind::Unspecified {
                classify(language, seed, &item.text)
            } else {
                item.kind
            };
            let weight = if index == 0 {
                100.0
            } else if kind == CandidateKind::Literal {
                55.0
            } else {
                94.0 - index as f32 * 2.0
            };
            candidate(item.text, kind, weight)
        })
        .collect();
    let additions = match BuiltinLanguage::resolve(language) {
        Some(BuiltinLanguage::English) => english::mixed_candidates(seed, context),
        Some(BuiltinLanguage::ChineseSimplified) => chinese::mixed_candidates(seed),
        Some(BuiltinLanguage::Japanese) => japanese::mixed_candidates(seed),
        None => Vec::new(),
    };
    for (index, (text, kind)) in additions.into_iter().enumerate() {
        if text != seed && !pool.iter().any(|c| c.text == text) {
            let weight = if kind == CandidateKind::Word {
                91.0
            } else {
                79.0
            } - index as f32 * 0.5;
            pool.push(candidate(text, kind, weight));
        }
    }
    // Preserve literal input as an explicit choice, even when a converter has many alternatives.
    if !pool.iter().any(|c| c.text == seed) {
        pool.push(candidate(seed.into(), CandidateKind::Literal, 55.0));
    }
    let pinned = pinned_count(language, &pool);
    balance(pool, pinned, limit)
}

fn pinned_count(language: &str, pool: &[Candidate]) -> usize {
    if language == "en" && pool.get(1).is_some_and(|c| c.kind == CandidateKind::Word) {
        2
    } else {
        pool.len().min(1)
    }
}

pub fn merge_model(
    language: &str,
    seed: &str,
    mut local: Vec<Candidate>,
    completions: Vec<LlmCompletion>,
    limit: usize,
) -> (Vec<Candidate>, bool) {
    let pinned = pinned_count(language, &local);
    // This is also the normalized prefix that request_prediction sends.
    let prefix = local.first().map(|candidate| candidate.text.clone());
    let mut accepted = false;
    for completion in completions.into_iter().take(6) {
        let text = normalize_completion_text(&completion.text, (language == "en").then_some(seed))
            .to_owned();
        if text.is_empty()
            || text == seed
            || !completion_fits_budget(&text, prefix.as_deref())
            || text.chars().any(char::is_control)
            || !completion.score_bias.is_finite()
        {
            continue;
        }
        let kind = match completion.kind {
            // A provider's type hint cannot turn an English sentence into a word.
            Some(CandidateKind::Word) if language == "en" => classify(language, seed, &text),
            Some(CandidateKind::Word | CandidateKind::Sentence) => completion.kind.unwrap(),
            _ => classify(language, seed, &text),
        };
        let weight = (if kind == CandidateKind::Word {
            90.0
        } else {
            82.0
        }) + completion.score_bias.clamp(0.0, 1.0) * 6.0;
        // Some providers return only sentences despite the requested groups.
        // Offer their first actual word completion too, without inventing text,
        // hiding the original sentence or reclassifying local corroboration as AI.
        let word = (language == "en" && kind == CandidateKind::Sentence)
            .then(|| english::word_from_continuation(seed, &text))
            .flatten()
            .map(str::to_owned);
        accepted = true;
        let derived_weight = 89.0 + completion.score_bias.clamp(0.0, 1.0) * 6.0;
        for (text, kind, score) in std::iter::once((text, kind, weight))
            .chain(word.map(|text| (text, CandidateKind::Word, derived_weight)))
        {
            if let Some(existing) = local.iter_mut().find(|c| c.text == text) {
                // Corroboration raises weight, never moves/relabels a local anchor.
                existing.score = existing.score.max(score);
            } else {
                local.push(Candidate {
                    label: format!("{text} · AI"),
                    text,
                    score,
                    kind,
                    source: CandidateSource::Model,
                });
            }
        }
    }
    (balance(local, pinned, limit), accepted)
}

fn balance(mut pool: Vec<Candidate>, pinned: usize, limit: usize) -> Vec<Candidate> {
    let limit = limit.max(1);
    let mut seen = HashSet::new();
    pool.retain(|c| !c.text.is_empty() && c.score.is_finite() && seen.insert(c.text.clone()));
    let count = pinned.min(pool.len()).min(limit);
    let mut rest = pool.split_off(count);
    rest.sort_by(|a, b| b.score.total_cmp(&a.score));
    let page = PAGE_SIZE.min(limit);
    let quota: usize = if page >= 5 { 2 } else { 1 };
    let mut reserved = Vec::new();
    for kind in [CandidateKind::Word, CandidateKind::Sentence] {
        let mut needed = quota.saturating_sub(pool.iter().filter(|c| c.kind == kind).count());
        let mut index = 0;
        while needed > 0 && pool.len() + reserved.len() < page && index < rest.len() {
            if rest[index].kind == kind {
                reserved.push(rest.remove(index));
                needed -= 1;
            } else {
                index += 1;
            }
        }
    }
    while pool.len() + reserved.len() < page && !rest.is_empty() {
        reserved.push(rest.remove(0));
    }
    reserved.sort_by(|a, b| b.score.total_cmp(&a.score));
    pool.extend(reserved);
    pool.extend(rest);
    let literal = pool
        .iter()
        .find(|c| c.kind == CandidateKind::Literal)
        .cloned();
    pool.truncate(limit);
    if let Some(literal) = literal
        && pool.len() > 1
        && !pool.iter().any(|c| c.text == literal.text)
    {
        *pool.last_mut().unwrap() = literal;
    }
    pool
}

pub fn display_label(
    text: &str,
    kind: CandidateKind,
    source: CandidateSource,
    weight: u8,
) -> String {
    display_label_for_seed("", "", text, kind, source, weight)
}

/// English long drafts elide only text shared with the preedit, keeping the
/// changing word/continuation visible. Replacement and commit text stay untouched.
pub fn display_label_for_seed(
    language: &str,
    seed: &str,
    text: &str,
    kind: CandidateKind,
    source: CandidateSource,
    weight: u8,
) -> String {
    let marker = match kind {
        CandidateKind::Word => "ᵂ",
        CandidateKind::Sentence => "ˢ",
        CandidateKind::Literal => "ᴿ",
        CandidateKind::Unspecified => "",
    };
    let subscript: String = weight
        .min(100)
        .to_string()
        .chars()
        .map(|ch| char::from_u32('₀' as u32 + ch as u32 - '0' as u32).unwrap())
        .collect();
    let suffix = format!(
        " {marker}{}{subscript}",
        if source == CandidateSource::Model {
            "ᴬᴵ"
        } else {
            ""
        }
    );
    let room = PREVIEW_CHARS.saturating_sub(suffix.chars().count());
    let preview = if language == "en" {
        english_preview(seed, text, room)
    } else {
        bounded_preview(text, room)
    };
    format!("{preview}{suffix}")
}

fn bounded_preview(text: &str, room: usize) -> String {
    if text.chars().count() > room {
        format!(
            "{}…",
            text.chars()
                .take(room.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        text.into()
    }
}

fn english_preview(seed: &str, text: &str, room: usize) -> String {
    let characters: Vec<_> = text.char_indices().collect();
    let shared = seed
        .chars()
        .zip(text.chars())
        .take_while(|(a, b)| a == b)
        .count();
    if characters.len() <= room || shared <= room / 2 {
        return bounded_preview(text, room);
    }
    let starts: Vec<_> = characters
        .iter()
        .enumerate()
        .filter_map(|(index, (_, ch))| {
            (index > 0
                && index <= shared
                && !ch.is_whitespace()
                && characters[index - 1].1.is_whitespace())
            .then_some(index)
        })
        .collect();
    let start = starts
        .iter()
        .find(|&&index| characters.len() - index < room)
        .or_else(|| starts.last());
    match start {
        Some(&index) => format!(
            "…{}",
            bounded_preview(&text[characters[index].0..], room.saturating_sub(1))
        ),
        None => bounded_preview(text, room),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

    #[test]
    fn english_long_previews_show_the_changed_tail_without_changing_payloads() {
        let seed = "For the next release please review the current docu";
        let one = format!("{seed}ment");
        let two = format!("{seed}mentation");
        let label = |text: &str| {
            display_label_for_seed(
                "en",
                seed,
                text,
                CandidateKind::Word,
                CandidateSource::Local,
                92,
            )
        };
        assert_ne!(label(&one), label(&two));
        assert!(label(&one).starts_with('…') && label(&one).contains("document"));
        assert!(label(&two).contains("documentation"));
        for text in [
            &one,
            &two,
            &format!("{seed}ment before we publish the next stable version tomorrow."),
        ] {
            assert!(label(text).chars().count() <= PREVIEW_CHARS);
        }
        let rewritten = format!("Instead {}", "a different opening ".repeat(6));
        assert!(label(&rewritten).starts_with("Instead "));
        let unicode_seed = format!("日本語 café 😀 {seed}");
        let unicode_text = format!("{unicode_seed}mentation");
        let unicode_label = display_label_for_seed(
            "en",
            &unicode_seed,
            &unicode_text,
            CandidateKind::Word,
            CandidateSource::Local,
            92,
        );
        assert!(unicode_label.starts_with('…') && unicode_label.contains("documentation"));
        assert!(unicode_label.chars().count() <= PREVIEW_CHARS);
        assert_eq!(
            display_label_for_seed(
                "ja",
                seed,
                &one,
                CandidateKind::Word,
                CandidateSource::Local,
                92
            ),
            display_label(&one, CandidateKind::Word, CandidateSource::Local, 92)
        );
        let mut engine = engine("en", seed);
        let index = engine
            .candidates()
            .iter()
            .position(|c| c.text == one)
            .unwrap();
        assert_eq!(engine.ibus_candidate_label(index), Some(label(&one)));
        engine.select_candidate(index);
        assert_eq!(engine.selected_completion_text(true), Some(one.as_str()));
        assert_eq!(
            engine.commit(CommitOptions { force: true }).text.as_deref(),
            Some(one.as_str())
        );
    }

    fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: language.into(),
            ..Default::default()
        });
        engine.enable_ibus_candidate_mix();
        engine.seed(seed);
        engine
    }

    #[test]
    fn all_three_languages_offer_words_and_sentences_on_the_first_page_offline() {
        for (language, seed, word, sentence) in [
            ("en", "hel", "hello", "hello, how are you?"),
            (
                "en",
                "please sen",
                "please send",
                "please send me the details.",
            ),
            (
                "en",
                "thank you ",
                "thank you for",
                "thank you for your help.",
            ),
            ("zh-Hans", "nihao", "你好", "你好，很高兴认识你。"),
            ("zh-Hans", "niha", "你好", "你好，很高兴认识你。"),
            ("ja", "nihongo", "日本語", "日本語を勉強しています。"),
            ("ja", "niho", "日本語", "日本語を勉強しています。"),
        ] {
            let engine = engine(language, seed);
            let all = engine.candidates();
            let page = &all[..all.len().min(PAGE_SIZE)];
            assert!(
                page.iter().any(|c| c.kind == CandidateKind::Word),
                "{seed}: {page:?}"
            );
            assert!(
                page.iter().any(|c| c.kind == CandidateKind::Sentence),
                "{seed}: {page:?}"
            );
            assert!(all.iter().any(|c| c.text == word), "{seed}: {all:?}");
            assert!(all.iter().any(|c| c.text == sentence), "{seed}: {all:?}");
            assert!(all.iter().any(|c| c.text == seed));
            assert!(all.len() <= 12);
            assert!(all.iter().all(|c| (0.0..=100.0).contains(&c.score)));
        }
    }

    #[test]
    fn spelling_separators_and_japanese_segment_conversion_preserve_literal_input() {
        for (language, seed, expected) in [
            ("zh-Hans", "ni  hao ", "你好"),
            ("zh-Hans", "xi'an", "西安"),
            ("ja", "nihongo wo benkyoushitai", "日本語を勉強したい"),
            ("ja", "kanjiwo", "漢字を"),
        ] {
            let engine = engine(language, seed);
            assert_eq!(engine.snapshot().seed_text, seed);
            assert_eq!(engine.candidates()[0].text, expected);
            assert!(engine.candidates().iter().any(|c| c.text == seed));
        }
    }

    #[test]
    fn unfinished_japanese_readings_are_not_mislabeled_as_sentences() {
        assert_eq!(classify("ja", "niho", "にほ"), CandidateKind::Unspecified);
        assert_eq!(classify("ja", "niho", "ニホ"), CandidateKind::Unspecified);
        assert_eq!(
            classify("ja", "nihong", "日本g"),
            CandidateKind::Unspecified
        );
        assert_eq!(classify("ja", "nihongo", "にほんご"), CandidateKind::Word);
    }

    #[test]
    fn continuous_cjk_input_completes_the_last_word_without_losing_the_prefix() {
        for (language, seed, word, sentence) in [
            ("zh-Hans", "woxihuanbei", "我喜欢北京", "我喜欢北京的文化。"),
            (
                "zh-Hans",
                "wo xihuan bei",
                "我喜欢北京",
                "我喜欢北京的文化。",
            ),
            ("zh-Hans", "我喜欢bei", "我喜欢北京", "我喜欢北京的文化。"),
            (
                "ja",
                "watashihanihong",
                "私は日本語",
                "私は日本語を勉強しています。",
            ),
            (
                "ja",
                "私はniho",
                "私は日本語",
                "私は日本語を勉強しています。",
            ),
            (
                "ja",
                "nihongowobenky",
                "日本語を勉強",
                "日本語を勉強しています。",
            ),
        ] {
            let engine = engine(language, seed);
            let candidates = engine.candidates();
            let first = &candidates[..candidates.len().min(PAGE_SIZE)];
            assert!(
                first
                    .iter()
                    .any(|c| c.text == word && c.kind == CandidateKind::Word),
                "{seed}: {candidates:?}"
            );
            assert!(
                first
                    .iter()
                    .any(|c| c.text == sentence && c.kind == CandidateKind::Sentence),
                "{seed}: {candidates:?}"
            );
            assert!(candidates.iter().any(|c| c.text == seed));
        }
    }

    #[test]
    fn completion_text_is_read_only_full_text_and_can_require_an_explicit_selection() {
        let mut engine = engine("en", "before");
        engine.commit(CommitOptions { force: true });
        engine.seed("hel");
        assert_eq!(engine.selected_completion_text(false), Some("hel"));
        assert_eq!(engine.selected_completion_text(true), None);
        let index = engine
            .candidates()
            .iter()
            .position(|c| c.text == "hello, how are you?")
            .unwrap();
        engine.select_candidate(index);
        let before = engine.snapshot();
        assert_eq!(
            engine.selected_completion_text(true),
            Some("hello, how are you?")
        );
        assert_eq!(engine.snapshot(), before);
        engine.seed("hello ");
        assert_eq!(engine.selected_completion_text(true), None);
        engine.clear_session_context();
        assert_eq!(engine.selected_completion_text(false), None);
    }

    #[test]
    fn unknown_identifiers_do_not_receive_fabricated_local_sentences() {
        for seed in ["https://exa", "user_nam", "Rust2026", "unknownword"] {
            let engine = engine("en", seed);
            assert_eq!(engine.candidates().len(), 1, "{seed}");
            assert_eq!(engine.candidates()[0].kind, CandidateKind::Literal);
            assert_eq!(engine.candidates()[0].text, seed);
        }
    }

    #[test]
    fn weight_ranking_reserves_both_types_without_moving_typing_anchors() {
        let mut pool = vec![
            candidate("hel".into(), CandidateKind::Literal, 100.0),
            candidate("hello".into(), CandidateKind::Word, 92.0),
        ];
        for index in 0..10 {
            pool.push(candidate(
                format!("word{index}"),
                CandidateKind::Word,
                91.0 - index as f32,
            ));
        }
        pool.push(candidate(
            "hello there".into(),
            CandidateKind::Sentence,
            79.0,
        ));
        pool.push(candidate(
            "hello everyone".into(),
            CandidateKind::Sentence,
            78.0,
        ));
        let mixed = balance(pool, 2, 12);
        assert_eq!(mixed[0].text, "hel");
        assert_eq!(mixed[1].text, "hello");
        assert_eq!(
            mixed[..6]
                .iter()
                .filter(|c| c.kind == CandidateKind::Sentence)
                .count(),
            2
        );
        assert!(
            mixed[2..6]
                .windows(2)
                .all(|pair| pair[0].score >= pair[1].score)
        );
        assert!(
            mixed[6..]
                .windows(2)
                .all(|pair| pair[0].score >= pair[1].score)
        );
    }

    #[test]
    fn merging_model_candidates_deduplicates_bounds_weights_and_keeps_the_literal() {
        let local = engine("ja", "nihongo").candidates().to_vec();
        let mut replies: Vec<_> = (0..5)
            .map(|i| LlmCompletion {
                text: format!("日本語の文章{i}です。"),
                score_bias: 1000.0,
                kind: Some(CandidateKind::Sentence),
            })
            .collect();
        replies.push(LlmCompletion {
            text: "日本語".into(),
            score_bias: 1.0,
            kind: Some(CandidateKind::Word),
        });
        let (mixed, accepted) = merge_model("ja", "nihongo", local, replies, 8);
        assert!(accepted);
        assert_eq!(mixed[0].text, "日本語");
        assert_eq!(mixed[0].source, CandidateSource::Local);
        assert_eq!(mixed.iter().filter(|c| c.text == "日本語").count(), 1);
        assert!(mixed.iter().any(|c| c.text == "nihongo"));
        assert!(
            mixed
                .iter()
                .filter(|c| c.source == CandidateSource::Model)
                .all(|c| c.score == 88.0)
        );
        assert_eq!(mixed.len(), 8);
    }

    #[test]
    fn unsafe_model_output_leaves_local_candidates_unchanged() {
        let local = engine("en", "hel").candidates().to_vec();
        let invalid = [
            "hel".into(),
            "bad\0text".into(),
            "字".repeat(161),
            String::new(),
        ];
        let mut replies: Vec<_> = invalid
            .into_iter()
            .map(|text| LlmCompletion {
                text,
                score_bias: 1.0,
                kind: None,
            })
            .collect();
        replies.push(LlmCompletion {
            text: "hello new sentence".into(),
            score_bias: f32::NAN,
            kind: None,
        });
        let (mixed, accepted) = merge_model("en", "hel", local.clone(), replies, 12);
        assert!(!accepted);
        assert_eq!(mixed, local);
    }

    #[test]
    fn legacy_and_incorrectly_typed_english_sentences_are_still_classified_as_sentences() {
        for kind in [None, Some(CandidateKind::Word)] {
            let (mixed, _) = merge_model(
                "en",
                "hel",
                engine("en", "hel").candidates().to_vec(),
                vec![LlmCompletion {
                    text: "hello from a model".into(),
                    score_bias: 0.5,
                    kind,
                }],
                12,
            );
            assert_eq!(
                mixed
                    .iter()
                    .find(|c| c.text == "hello from a model")
                    .unwrap()
                    .kind,
                CandidateKind::Sentence
            );
        }
    }

    #[test]
    fn annotations_and_preview_are_bounded_but_never_become_commit_text() {
        assert_eq!(
            display_label("hello", CandidateKind::Word, CandidateSource::Local, 92),
            "hello ᵂ₉₂"
        );
        assert_eq!(
            display_label(
                "hello world",
                CandidateKind::Sentence,
                CandidateSource::Model,
                85
            ),
            "hello world ˢᴬᴵ₈₅"
        );
        for text in ["界".repeat(160), "🙂".repeat(100), "e\u{301}".repeat(80)] {
            let label = display_label(&text, CandidateKind::Sentence, CandidateSource::Model, 255);
            assert_eq!(label.chars().count(), PREVIEW_CHARS);
            assert!(label.ends_with("… ˢᴬᴵ₁₀₀"));
        }
        let mut engine = engine("en", "hel");
        let index = engine
            .candidates()
            .iter()
            .position(|c| c.text == "hello, how are you?")
            .unwrap();
        engine.select_candidate(index);
        assert_eq!(
            engine.commit(CommitOptions { force: true }).text.as_deref(),
            Some("hello, how are you?")
        );
    }

    #[test]
    fn large_inputs_use_a_lossless_bounded_fallback() {
        let seed = "abc".repeat(100);
        let engine = engine("en", &seed);
        assert_eq!(engine.candidates().len(), 1);
        assert_eq!(engine.candidates()[0].text, seed);
        assert_eq!(engine.candidates()[0].score, 100.0);
        assert_eq!(engine.candidates()[0].kind, CandidateKind::Literal);
    }
}
