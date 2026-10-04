//! Curated offline vocabulary, not a replacement for a full Pinyin dictionary.
//! Unknown input is always preserved; an optional LLM can enrich these candidates.

use super::ranked_typed_candidates;
use crate::ime::candidate_mix::CandidateKind;
use crate::ime::{Candidate, LanguagePlugin};
use crate::lexicon::{EntryKind, Lexicon};
use std::sync::OnceLock;

#[derive(Default)]
pub struct ChineseLanguagePlugin;

impl LanguagePlugin for ChineseLanguagePlugin {
    fn id(&self) -> &str {
        "zh-Hans"
    }
    fn display_name(&self) -> &str {
        "简体中文"
    }
    fn normalize_seed(&self, input: &str) -> String {
        // Separators belong to the preedit and the lossless literal candidate.
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        ranked_typed_candidates(pinyin_choices(seed), confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        Some(self.build_candidates(&[], seed, confidence))
    }
    fn commit_separator(&self) -> &str {
        ""
    }
}

// Authored bootstrap vocabulary, with explicit syllables rather than inferred
// splits. Optional typed separators may join these syllables, never split one.
struct PinyinEntry {
    reading: &'static str,
    text: &'static str,
    require_separators: bool,
    kind: EntryKind,
}

impl PinyinEntry {
    #[cfg(test)]
    const fn new(reading: &'static str, text: &'static str) -> Self {
        Self {
            reading,
            text,
            require_separators: false,
            kind: EntryKind::Word,
        }
    }

    fn match_input(&self, input: &str) -> Option<ReadingMatch> {
        let mut consumed = 0;
        for expected in self.reading.bytes() {
            if consumed == input.len() {
                return Some(ReadingMatch::Prefix);
            }
            if expected == b'\'' {
                let separators = separator_bytes(&input[consumed..]);
                if separators > 0 {
                    consumed += separators;
                } else if self.require_separators {
                    return None;
                }
            } else if input.as_bytes()[consumed] == expected {
                consumed += 1;
            } else {
                return None;
            }
        }
        Some(ReadingMatch::Exact(consumed))
    }
}

fn vocabulary() -> &'static Lexicon {
    crate::lexicon::active("zh-Hans").expect("Chinese vocabulary is registered")
}

fn pinyin() -> &'static [PinyinEntry] {
    static ENTRIES: OnceLock<Vec<PinyinEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        vocabulary()
            .readings()
            .iter()
            .map(|entry| PinyinEntry {
                reading: &entry.reading,
                text: &entry.text,
                require_separators: entry.require_separators,
                kind: entry.kind,
            })
            .collect()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadingMatch {
    Exact(usize),
    Prefix,
}

// Bucket by the first ASCII letter, retaining the authored order within each
// bucket. Expanding the lexicon must not make every beam path scan all entries
// or change how exact readings and homophones are ranked.
fn pinyin_starting_with(input: &str) -> &'static [&'static PinyinEntry] {
    static INDEX: OnceLock<[Vec<&'static PinyinEntry>; 26]> = OnceLock::new();
    let Some(initial) = input.bytes().next().filter(u8::is_ascii_lowercase) else {
        return &[];
    };
    let index = INDEX.get_or_init(|| {
        let mut buckets: [Vec<&'static PinyinEntry>; 26] = std::array::from_fn(|_| Vec::new());
        for entry in pinyin() {
            buckets[usize::from(entry.reading.as_bytes()[0] - b'a')].push(entry);
        }
        buckets
    });
    &index[usize::from(initial - b'a')]
}

fn is_pinyin_spacing(ch: char) -> bool {
    // Horizontal separators can join syllables; line/paragraph boundaries are
    // literal text, even when both neighbouring runs happen to be Pinyin.
    ch == '\t' || (ch.is_whitespace() && !ch.is_control() && !matches!(ch, '\u{2028}' | '\u{2029}'))
}

fn is_pinyin_separator(ch: char) -> bool {
    ch == '\'' || is_pinyin_spacing(ch)
}

fn separator_bytes(text: &str) -> usize {
    text.len() - text.trim_start_matches(is_pinyin_separator).len()
}

fn is_tone_terminator(tail: &[char]) -> bool {
    let Some((&next, rest)) = tail.split_first() else {
        return true;
    };
    if next.is_ascii_alphabetic() || next.is_whitespace() || next == '\'' {
        return true;
    }
    match next {
        // A decimal, clock value, dotted identifier or path is not a sentence
        // boundary. Commas immediately before digits may be numeric grouping.
        '.' | ':' => rest
            .first()
            .is_none_or(|ch| !ch.is_alphanumeric() && !matches!(ch, '/' | '\\' | '_' | '-')),
        ',' => !rest.first().is_some_and(|ch| ch.is_numeric()),
        // An opening aside/quote terminates the preceding syllable just like
        // its closing partner. Chinese em dashes also delimit clauses; keep
        // this explicit so hyphens, en-dash ranges and arithmetic stay literal.
        '!' | '?' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '“' | '”' | '‘' | '’'
        | '，' | '。' | '！' | '？' | '；' | '：' | '、' | '…' | '—' | '（' | '）' | '【'
        | '】' | '〔' | '〕' | '〈' | '〉' | '《' | '》' | '「' | '」' | '『' | '』' => {
            true
        }
        _ => false,
    }
}

fn normalize_pinyin(seed: &str) -> String {
    // Normalize only letters used by this reading system. Unicode lowercasing
    // the entire draft corrupts literal Greek/Cyrillic/accented text. Preserve
    // separators here so the decoder can distinguish readings from literal spans.
    let lowered = seed
        .to_ascii_lowercase()
        .replace("u:", "v")
        .replace(['ü', 'Ü'], "v");
    let chars: Vec<_> = lowered.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(index, &ch)| {
            let tone = matches!(ch, '1'..='5')
                && index > 0
                && chars[index - 1].is_ascii_alphabetic()
                && is_tone_terminator(&chars[index + 1..]);
            (!tone).then_some(ch)
        })
        .collect()
}

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    pinyin()
        .iter()
        .any(|word| word.text == text && word.kind == EntryKind::Word)
}

/// Match already converted text without erasing its literal horizontal padding.
/// The optional reading tail is only a routing hint for the long-draft window:
/// emitted sentences must match an actual decoder result, never this hint.
fn authored_sentence_suffix<'a>(
    input: &str,
    sentence: &'a str,
    allow_reading_tail: bool,
) -> Option<&'a str> {
    let mut expected = sentence.char_indices().peekable();
    let mut consumed = 0;
    for (offset, ch) in input.char_indices() {
        if is_pinyin_spacing(ch) {
            // Respect authored spaces when present, but preserve the user's
            // exact spacing in the prefix used to construct the candidate.
            while let Some(&(index, next)) = expected.peek()
                && is_pinyin_spacing(next)
            {
                consumed = index + next.len_utf8();
                expected.next();
            }
            continue;
        }
        let (index, next) = expected.next()?;
        if ch != next {
            if allow_reading_tail && (ch.is_ascii_alphabetic() || matches!(ch, 'ü' | 'Ü')) {
                let reading = normalize_pinyin(&input[offset..]);
                if reading
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || is_pinyin_separator(ch))
                    && pinyin_candidates(&input[offset..])
                        .iter()
                        .take(5)
                        .any(|converted| {
                            authored_sentence_suffix(converted, &sentence[index..], false).is_some()
                        })
                {
                    return Some(&sentence[index..]);
                }
            }
            return None;
        }
        consumed = index + ch.len_utf8();
    }
    (consumed < sentence.len()).then_some(&sentence[consumed..])
}

/// Prefer the longest matching sentence progress, not merely the last word.
/// Both converted text and raw long-draft routing search the same bounded tail.
fn authored_continuation(
    seed: &str,
    allow_reading_tail: bool,
) -> Option<(&str, &'static [String])> {
    let window_start = seed
        .char_indices()
        .rev()
        .nth(super::draft::LOCAL_TAIL_CHARS - 1)
        .map_or(0, |(index, _)| index);
    let mut best = None;
    let mut best_key = (0, 0);
    for (phrase, values) in vocabulary().continuations() {
        for (offset, _) in seed[window_start..].match_indices(phrase) {
            let start = window_start + offset;
            let tail = &seed[start..];
            let key = (tail.len(), phrase.len());
            if key <= best_key {
                continue;
            }
            let token = seed[..start]
                .rsplit(char::is_whitespace)
                .next()
                .unwrap_or("");
            if token.contains(['/', '\\', '_', '@']) {
                continue;
            }
            if values
                .iter()
                .any(|text| authored_sentence_suffix(tail, text, allow_reading_tail).is_some())
            {
                best_key = key;
                best = Some((tail, values));
            }
        }
    }
    best
}

/// Retain authored Han sentence progress, including an unfinished Pinyin tail.
/// Horizontal padding belongs to the draft; line boundaries and punctuation
/// are not discarded to force a continuation.
pub(crate) fn adopted_continuation_tail(seed: &str) -> Option<&str> {
    authored_continuation(seed, true).map(|(tail, _)| tail)
}

pub(crate) fn mixed_candidates(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    let code = normalize_pinyin(seed);
    let mut output = Vec::new();
    if code.bytes().filter(u8::is_ascii_lowercase).count() >= 2 {
        for word in pinyin_starting_with(&code)
            .iter()
            .filter(|word| word.match_input(&code) == Some(ReadingMatch::Prefix))
            .take(5)
        {
            output.push((
                word.text.to_owned(),
                if is_dictionary_word(word.text) {
                    CandidateKind::Word
                } else {
                    CandidateKind::Sentence
                },
            ));
        }
    }
    // A valid homophone may continue the authored sentence even if the primary
    // conversion does not. Keep that primary untouched; only add suggestions
    // from the bounded decoder paths, never rewrite literal adopted Han text.
    for converted in pinyin_candidates(seed).iter().take(5) {
        if let Some((tail, values)) = authored_continuation(converted, false) {
            for text in values {
                if let Some(suffix) = authored_sentence_suffix(tail, text, false) {
                    let continuation = format!("{converted}{suffix}");
                    if !output.iter().any(|(text, _)| *text == continuation) {
                        output.push((continuation, CandidateKind::Sentence));
                    }
                }
            }
        }
    }
    output
}

pub fn pinyin_candidates(seed: &str) -> Vec<String> {
    pinyin_choices(seed)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

fn pinyin_choices(seed: &str) -> Vec<(String, CandidateKind)> {
    if seed.trim().is_empty() {
        return Vec::new();
    }
    let input_chars = seed.chars().count();
    if input_chars > super::draft::LOCAL_TAIL_CHARS {
        return vec![(seed.into(), CandidateKind::Literal)];
    }
    let normalized = normalize_pinyin(seed);
    let mut paths = vec![(0usize, String::new(), 0usize)];
    // Bounded beam search: never exponential in the number of ambiguous syllables.
    let mut finished = Vec::new();
    let mut completions = Vec::new();
    // Every live path consumes at least one Unicode scalar per round. Allow
    // the already bounded input to finish, plus one round to collect its end;
    // a fixed 128 rounds loses Pinyin after long adopted/literal prefixes.
    // Normalization never increases the input's scalar count, and the beam
    // width remains capped below, independently of the number of syllables.
    for _ in 0..=input_chars {
        let mut next = Vec::new();
        for (offset, text, segments) in paths {
            let rest = &normalized[offset..];
            if rest.is_empty() {
                finished.push((text, segments));
                continue;
            }
            let separators = separator_bytes(rest);
            if separators > 0 {
                // Only separators within a decoded phonetic span may disappear.
                // A trailing space can still end a reading; a closing quote or
                // spacing before literal Han text must not disappear with it.
                let after_reading = normalized[..offset]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_lowercase());
                let tail = &rest[separators..];
                let within_readings = tail.starts_with(|ch: char| ch.is_ascii_lowercase());
                let trailing_spacing = tail.is_empty() && rest.chars().all(is_pinyin_spacing);
                let text = if after_reading && (within_readings || trailing_spacing) {
                    text
                } else {
                    format!("{text}{}", &rest[..separators])
                };
                next.push((offset + separators, text, segments));
                continue;
            }
            if let Some(ch) = rest.chars().next().filter(|ch| !ch.is_ascii_alphabetic()) {
                next.push((offset + ch.len_utf8(), format!("{text}{ch}"), segments));
                continue;
            }
            let can_complete = rest.bytes().filter(u8::is_ascii_lowercase).count() >= 2;
            let mut completed = 0;
            for word in pinyin_starting_with(rest) {
                match word.match_input(rest) {
                    Some(ReadingMatch::Exact(bytes)) => {
                        next.push((offset + bytes, format!("{text}{}", word.text), segments + 1))
                    }
                    // Complete only the final reading, including explicitly
                    // separated syllables. A separator inside a syllable fails
                    // the match; it is never blindly stripped to make a word.
                    Some(ReadingMatch::Prefix) if can_complete && completed < 4 => {
                        completed += 1;
                        completions.push((
                            format!("{text}{}", word.text),
                            segments + 1,
                            if is_dictionary_word(word.text) {
                                CandidateKind::Word
                            } else {
                                CandidateKind::Sentence
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }
        next.sort_by(|left, right| right.0.cmp(&left.0).then(left.2.cmp(&right.2)));
        next.truncate(24);
        completions.sort_by_key(|(_, segments, _)| *segments);
        completions.truncate(64);
        if next.is_empty() {
            break;
        }
        paths = next;
    }
    finished.sort_by_key(|(_, segments)| *segments);
    let mut seen = std::collections::HashSet::new();
    let mut output: Vec<_> = finished
        .into_iter()
        .filter(|(text, _)| seen.insert(text.clone()))
        .take(5)
        .map(|(text, _)| {
            let kind = if text == seed {
                CandidateKind::Literal
            } else if is_dictionary_word(text.trim_matches(is_pinyin_spacing)) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            };
            (text, kind)
        })
        .collect();
    output.extend(
        completions
            .into_iter()
            .filter(|(text, _, _)| seen.insert(text.clone()))
            .take(4)
            .map(|(text, _, kind)| (text, kind)),
    );
    if seen.insert(seed.to_owned()) {
        output.push((seed.into(), CandidateKind::Literal));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_separators_use_utf8_boundaries_without_consuming_newlines() {
        let entry = PinyinEntry::new("shu'ru'fa", "输入法");
        for input in [
            "shu ru fa",
            "shu\tru\tfa",
            "shu\u{3000}ru\u{a0}fa",
            "shu''ru'fa",
        ] {
            assert!(entry.match_input(input) == Some(ReadingMatch::Exact(input.len())));
        }
        for input in [
            "shu\nru fa",
            "shu\r\nru fa",
            "shu\u{2028}ru fa",
            "sh u ru fa",
        ] {
            assert!(entry.match_input(input).is_none(), "{input:?}");
        }
        assert_eq!(pinyin_candidates("ni hao ")[0], "你好");
        assert_eq!(pinyin_candidates("nÜ")[0], "女");
        assert_eq!(pinyin_candidates("NU:")[0], "女");
    }

    #[test]
    fn authored_syllables_match_joined_and_separated_readings_without_inferred_splits() {
        let mut entries = std::collections::HashSet::new();
        for entry in pinyin() {
            assert!(entries.insert((entry.reading, entry.text)));
            assert_eq!(
                entry.reading.split('\'').count(),
                entry.text.chars().count()
            );
            assert!(
                entry
                    .reading
                    .split('\'')
                    .all(|part| !part.is_empty() && part.bytes().all(|ch| ch.is_ascii_lowercase()))
            );
            assert!(
                entry.match_input(entry.reading) == Some(ReadingMatch::Exact(entry.reading.len()))
            );
            if !entry.require_separators {
                let joined = entry.reading.replace('\'', "");
                assert!(entry.match_input(&joined) == Some(ReadingMatch::Exact(joined.len())));
            }
            let repeated = entry.reading.replace('\'', "''");
            assert!(entry.match_input(&repeated) == Some(ReadingMatch::Exact(repeated.len())));
        }
        for (seed, expected) in [
            ("shu1 ru4 fa3", "输入法"),
            ("shu1ru4fa3", "输入法"),
            ("WO3  XI3  HUAN1  BEI3  JING1", "我喜欢北京"),
            ("我想xue2 xi2 zhong1 wen2", "我想学习中文"),
            ("xi'an zai", "西安在"),
            ("xian zai", "现在"),
        ] {
            assert_eq!(pinyin_candidates(seed)[0], expected, "{seed}");
            assert!(pinyin_candidates(seed).iter().any(|value| value == seed));
        }
    }

    #[test]
    fn prefix_index_retains_every_entry_and_its_authored_rank() {
        let mut count = 0;
        for initial in b'a'..=b'z' {
            let prefix = char::from(initial).to_string();
            let indexed = pinyin_starting_with(&prefix);
            let exhaustive: Vec<_> = pinyin()
                .iter()
                .filter(|entry| entry.reading.starts_with(char::from(initial)))
                .collect();
            assert_eq!(indexed.len(), exhaustive.len());
            for (actual, expected) in indexed.iter().zip(exhaustive) {
                // Ordering concerns entry contents, not storage addresses.
                assert_eq!(
                    (actual.reading, actual.text, actual.require_separators),
                    (expected.reading, expected.text, expected.require_separators)
                );
            }
            count += indexed.len();
        }
        assert_eq!(count, pinyin().len());
        for input in ["", "A", "1", "'", " ", "中", "ü"] {
            assert!(pinyin_starting_with(input).is_empty(), "{input}");
        }
    }

    #[test]
    fn every_authored_entry_reaches_the_decoder_with_valid_syllable_boundaries() {
        for entry in pinyin() {
            let mut spellings = vec![entry.reading.to_owned(), entry.reading.replace('\'', " ")];
            if !entry.require_separators {
                spellings.push(entry.reading.replace('\'', ""));
            }
            for seed in spellings {
                let candidates = pinyin_candidates(&seed);
                assert!(
                    candidates.iter().any(|text| text == entry.text),
                    "{seed} -> {}: {candidates:?}",
                    entry.text
                );
                assert!(candidates.contains(&seed));
            }
        }
    }

    #[test]
    fn authored_continuations_are_unique_complete_and_preserve_padding() {
        assert_eq!(pinyin().len(), 2367);
        let mut phrases = std::collections::HashSet::new();
        let mut sentences = std::collections::HashSet::new();
        for (phrase, values) in vocabulary().continuations() {
            assert!(phrases.insert(phrase), "duplicate phrase: {phrase}");
            assert!(!values.is_empty());
            for sentence in values {
                assert!(sentences.insert(sentence), "duplicate sentence: {sentence}");
                assert!(sentence.starts_with(phrase));
                assert!(sentence.ends_with(['。', '？', '！']));
            }
            for padding in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{phrase}{padding}");
                let candidates = mixed_candidates(&seed);
                for sentence in values {
                    let expected = format!("{seed}{}", &sentence[phrase.len()..]);
                    assert!(
                        candidates.iter().any(|(text, kind)| {
                            *text == expected && *kind == CandidateKind::Sentence
                        }),
                        "missing {expected:?}: {candidates:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn converts_common_pinyin_and_tone_numbers() {
        for seed in ["nihao", "ni hao", "ni3 hao3", "NIHAO"] {
            assert_eq!(pinyin_candidates(seed)[0], "你好");
        }
        assert_eq!(pinyin_candidates("woaizhongguo")[0], "我爱中国");
    }

    #[test]
    fn tone_terminators_preserve_punctuation_and_numeric_literal_boundaries() {
        for punctuation in [
            ".", ":", ",", "!", "?", ";", ")", "]", "}", "\"", "”", "’", "，", "。", "！", "？",
            "；", "：", "、", "…", "）", "】", "》", "」", "』", "?!", "...", ".)",
        ] {
            let seed = format!("ni3hao3{punctuation}");
            assert_eq!(normalize_pinyin(&seed), format!("nihao{punctuation}"));
            assert_eq!(pinyin_candidates(&seed)[0], format!("你好{punctuation}"));
            assert!(pinyin_candidates(&seed).contains(&seed));
        }
        for suffix in [
            "3.5",
            "3:30",
            "3,000",
            "3,４００",
            "30",
            "36",
            "0!",
            "6!",
            "3/4",
            "3-4",
            "3_4",
            "3.rs",
            "3://",
            "3@home",
            "3%",
            "3+4",
            "3=4",
        ] {
            let seed = format!("hao{suffix}");
            assert_eq!(normalize_pinyin(&seed), seed, "{seed:?}");
        }
    }
    #[test]
    fn apostrophe_keeps_syllable_boundaries_and_unknown_input_is_lossless() {
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("Rust2026"), ["Rust2026"]);
        assert!(pinyin_candidates("nihao").contains(&"nihao".into()));
        assert_eq!(pinyin_candidates("nü")[0], "女");
    }

    #[test]
    fn spaces_and_apostrophes_keep_pinyin_syllable_boundaries() {
        for seed in ["xi an", "xi  an", "XI AN", "xi1 an1", "xi'an"] {
            let candidates = pinyin_candidates(seed);
            assert_eq!(candidates[0], "西安", "{seed}: {candidates:?}");
            assert!(candidates.iter().any(|text| text == seed));
        }
        assert_eq!(pinyin_candidates("xian")[0], "先");
        assert_eq!(pinyin_candidates("xi'an")[0], "西安");
    }

    #[test]
    fn partial_last_syllables_and_typed_han_prefixes_have_bounded_lossless_completions() {
        for (seed, expected) in [
            ("woxihuanbeij", "我喜欢北京"),
            ("我喜欢bei", "我喜欢北京"),
            ("wo xihuan bei", "我喜欢北京"),
            ("我想学习zhongw", "我想学习中文"),
        ] {
            let candidates = pinyin_choices(seed);
            assert!(
                candidates
                    .iter()
                    .any(|(text, kind)| text == expected && *kind == CandidateKind::Word),
                "{seed}: {candidates:?}"
            );
            assert!(candidates.iter().any(|(text, _)| text == seed));
            assert!(candidates.len() <= 10);
        }
        for seed in [
            "unknownbei",
            "https://bei",
            "Rust2026",
            "字".repeat(300).as_str(),
        ] {
            assert_eq!(pinyin_candidates(seed), [seed]);
        }
    }
}
