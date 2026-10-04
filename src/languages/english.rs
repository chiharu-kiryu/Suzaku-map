use super::ranked_candidates;
use crate::ime::{Candidate, LanguagePlugin};
use crate::lexicon::{Lexicon, WordLayer};
use std::{collections::HashSet, sync::OnceLock};

fn vocabulary() -> &'static Lexicon {
    crate::lexicon::active("en").expect("English vocabulary is registered")
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’')
}

// Indexing is an English implementation concern; the data layer only supplies
// ordered words, contexts and sentences, with no knowledge of ranking rules.
fn indexed_words(layers: &[WordLayer]) -> impl Iterator<Item = &str> {
    layers.iter().flat_map(|layer| {
        layer
            .words
            .iter()
            .map(String::as_str)
            .chain(
                layer
                    .next_words
                    .iter()
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(layer.sentences.iter().flat_map(|sentence| {
                sentence
                    .split(|ch: char| !is_word_char(ch))
                    .filter(|word| !word.is_empty())
            }))
    })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnglishLanguagePlugin;

impl LanguagePlugin for EnglishLanguagePlugin {
    fn id(&self) -> &str {
        "en"
    }
    fn display_name(&self) -> &str {
        "English"
    }
    fn normalize_seed(&self, input: &str) -> String {
        // A trailing space means "next word", not "finish the previous word".
        // Preserve literal input and whitespace in the replacement range.
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        ranked_candidates(english_sentence_variants(seed), confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        self.direct_candidates_with_context(seed, "", confidence)
    }
    fn direct_candidates_with_context(
        &self,
        seed: &str,
        context: &str,
        confidence: f32,
    ) -> Option<Vec<Candidate>> {
        Some(ranked_candidates(
            english_variants(seed, context),
            confidence,
        ))
    }
}

struct Word {
    text: String,
    rank: usize,
}

// Match apostrophe variants in one index, while retaining the user's original
// prefix in every replacement. Curly punctuation must not need duplicate words.
fn canonical_word(text: &str) -> String {
    text.replace('’', "'").to_ascii_lowercase()
}

fn dictionary() -> &'static [Word] {
    static WORDS: OnceLock<Vec<Word>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut words: Vec<_> = indexed_words(vocabulary().word_layers())
            .enumerate()
            .map(|(rank, text)| Word {
                text: canonical_word(text),
                rank,
            })
            .collect();
        words.sort_by(|a, b| a.text.cmp(&b.text).then(a.rank.cmp(&b.rank)));
        words.dedup_by(|a, b| a.text == b.text);
        words
    })
}

/// Explicit prose separators can start an English word without an ASCII space.
/// Scan forwards once: punctuation inside a URL/path/identifier must not hide
/// its syntax and expose a completion-eligible suffix, even in a long draft.
fn word_boundaries(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut protected_token = false;
    text.char_indices().filter(move |(_, ch)| {
        if ch.is_whitespace() {
            protected_token = false;
            return true;
        }
        protected_token |= matches!(ch, '/' | '\\' | '_' | '@' | '.' | ':' | '-');
        !protected_token && matches!(ch, '—' | '，' | '。' | '！' | '？' | '；' | '：' | '、')
    })
}

// Begin at the nearest preceding token boundary, not at session-history start.
// A token without whitespace still needs its whole prefix: dropping it could
// hide a URL/path marker and incorrectly reinterpret its tail as prose.
fn boundary_scan_window(text: &str, start: usize) -> (usize, &str) {
    let start = if text[start..].starts_with(char::is_whitespace) {
        start
    } else {
        text[..start]
            .char_indices()
            .rev()
            .find(|(_, ch)| ch.is_whitespace())
            .map_or(0, |(index, _)| index)
    };
    (start, &text[start..])
}

pub(crate) fn word_boundaries_from(
    text: &str,
    start: usize,
) -> impl Iterator<Item = (usize, char)> + '_ {
    let (base, window) = boundary_scan_window(text, start);
    word_boundaries(window).map(move |(index, ch)| (base + index, ch))
}

/// Only natural-language word tails are eligible. Do not rewrite URLs, paths,
/// identifiers, numbers, hyphenated expressions or mixed-script input.
pub fn english_word_prefix(seed: &str) -> Option<&str> {
    let start = word_boundaries_from(seed, seed.len())
        .last()
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let token = &seed[start..];
    let word = token.trim_start_matches(['(', '[', '{', '"', '“', '‘', '\'']);
    if word.is_empty()
        || word.len() > 64
        || !word.starts_with(|c: char| c.is_ascii_alphabetic())
        || !word.chars().all(is_word_char)
    {
        return None;
    }
    let letters: Vec<_> = word.chars().filter(char::is_ascii_alphabetic).collect();
    let all_upper = letters.iter().all(char::is_ascii_uppercase);
    if !all_upper && letters.iter().skip(1).any(char::is_ascii_uppercase) {
        return None;
    }
    Some(word)
}

pub fn is_known_english_word(word: &str) -> bool {
    let lower = canonical_word(word);
    dictionary()
        .binary_search_by(|entry| entry.text.as_str().cmp(&lower))
        .is_ok()
}

pub(crate) fn suggestion_mode(seed: &str) -> &'static str {
    let Some(prefix) = english_word_prefix(seed) else {
        return "next_word";
    };
    if !is_known_english_word(prefix) {
        return "complete_word";
    }
    let lower = canonical_word(prefix);
    let words = dictionary();
    let next = words.partition_point(|word| word.text.as_str() <= lower.as_str());
    if words
        .get(next)
        .is_some_and(|word| word.text.starts_with(&lower))
    {
        // "a", "can", "in", ... can be a complete word OR the beginning of a
        // longer one. Only a typed separator unambiguously asks for the next word.
        "complete_or_continue"
    } else {
        "next_word"
    }
}

fn case_completion(word: &str, prefix: &str) -> String {
    // Canonical apostrophes have a different UTF-8 width from a typed ’. Split
    // by characters, not prefix bytes, before applying case/punctuation style.
    let split = word
        .char_indices()
        .nth(prefix.chars().count())
        .map_or(word.len(), |(i, _)| i);
    let suffix = &word[split..];
    let suffix = if prefix.contains('’') {
        suffix.replace('\'', "’")
    } else {
        suffix.to_owned()
    };
    let uppercase = prefix.chars().filter(char::is_ascii_alphabetic).count() > 1
        && prefix
            .chars()
            .all(|c| !c.is_ascii_alphabetic() || c.is_ascii_uppercase());
    format!(
        "{prefix}{}",
        if uppercase {
            suffix.to_uppercase()
        } else {
            suffix
        }
    )
}

// Horizontal padding can join authored words; a new line or paragraph cannot.
// Keep byte boundaries correct for NEL / Unicode line and paragraph separators.
fn current_line(text: &str) -> &str {
    text.char_indices()
        .rfind(|(_, ch)| {
            matches!(
                ch,
                '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
            )
        })
        .map_or(text, |(index, ch)| &text[index + ch.len_utf8()..])
}

fn current_clause(text: &str) -> &str {
    let line = current_line(text);
    let start = word_boundaries(line)
        .filter(|(_, ch)| !ch.is_whitespace())
        .last()
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    &line[start..]
}

fn canonical_context(context: &str) -> String {
    context
        .split_whitespace()
        .map(|word| {
            // Only ordinary phrase wrappers/separators may be ignored. Keeping
            // digits, symbols and sentence punctuation also prevents a period
            // inside `example.hello` from exposing a false `hello` collocation.
            canonical_word(
                word.trim_matches(['(', ')', '[', ']', '{', '}', '"', '“', '”', '‘', ',', ':']),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

struct WordContext {
    prefix: String,
    strength: usize,
    words: &'static [String],
}

fn index_contexts(
    contexts: impl Iterator<Item = (&'static str, &'static [String])>,
) -> Vec<WordContext> {
    let mut seen = HashSet::new();
    contexts
        .filter_map(|(prefix, words)| {
            let prefix = canonical_context(prefix);
            // Appended packs may use equivalent case, spacing or apostrophes.
            // Their spelling must not displace an earlier authored context.
            (!prefix.is_empty() && seen.insert(prefix.clone())).then(|| WordContext {
                strength: prefix.chars().count(),
                prefix,
                words,
            })
        })
        .collect()
}

fn next_word_contexts() -> &'static [WordContext] {
    static CONTEXTS: OnceLock<Vec<WordContext>> = OnceLock::new();
    CONTEXTS.get_or_init(|| index_contexts(vocabulary().next_words()))
}

fn phrase_ending_contexts() -> &'static [WordContext] {
    static CONTEXTS: OnceLock<Vec<WordContext>> = OnceLock::new();
    CONTEXTS.get_or_init(|| index_contexts(vocabulary().phrase_endings()))
}

fn context_word_match(context: &str) -> (usize, &'static [String]) {
    let normalized = canonical_context(current_clause(context));
    next_word_contexts()
        .iter()
        .filter(|entry| {
            normalized == entry.prefix
                || normalized
                    .strip_suffix(&entry.prefix)
                    .is_some_and(|left| left.ends_with(' '))
        })
        .max_by_key(|entry| entry.strength)
        .map_or((0, &[]), |entry| (entry.strength, entry.words))
}

fn bounded_context(context: &str) -> &str {
    let start = context
        .char_indices()
        .rev()
        .nth(159)
        .map_or(0, |(index, _)| index);
    if start == 0 {
        return context;
    }
    // Discard a token cut by the 160-scalar budget instead of treating its
    // suffix as a fresh word. Inspect only this bounded tail and its containing
    // token, preserving the protection state without scanning old history.
    word_boundaries_from(context, start)
        .find(|(index, ch)| index + ch.len_utf8() >= start)
        .map_or("", |(index, ch)| {
            let after = index + ch.len_utf8();
            let boundary = if after == start {
                start
            } else if ch.is_whitespace() {
                index
            } else {
                after
            };
            &context[boundary..]
        })
}

fn english_variants(seed: &str, committed_context: &str) -> Vec<String> {
    if seed.trim().is_empty() {
        return Vec::new();
    }
    let mut output = vec![seed.to_owned()];
    if seed.len() > 4096 {
        return output;
    }
    let context = format!("{} {seed}", bounded_context(committed_context));
    let phrases = sentence_remainders(seed, committed_context);
    let phrase_strength = phrases.first().map_or(0, |(matched, _)| *matched);
    let mut add = |text: String| {
        if output.len() < 12 && !output.contains(&text) {
            output.push(text);
        }
    };
    if seed.ends_with(char::is_whitespace) {
        let (strength, words) = context_word_match(&context);
        // "how can I -> help" must beat the shorter "I -> am". Equal-strength
        // authored collocations retain their priority and useful alternatives.
        if !seed.trim_end().ends_with(',') && phrase_strength <= strength {
            for next in words {
                add(format!("{seed}{next}"));
            }
        }
        for (_, remaining) in &phrases {
            let next = remaining
                .split(|ch: char| !is_word_char(ch))
                .next()
                .unwrap_or("");
            if !next.is_empty() {
                add(format!("{seed}{next}"));
            }
        }
        return output;
    }
    let Some(prefix) = english_word_prefix(seed) else {
        return output;
    };
    let left = &seed[..seed.len() - prefix.len()];
    let lower = canonical_word(prefix);
    let previous = format!("{} {left}", bounded_context(committed_context));
    // Preceding words outrank general vocabulary for incomplete words.
    let mut matched_context = false;
    let (strength, words) = context_word_match(&previous);
    let phrase_context = phrase_strength.saturating_sub(prefix.chars().count() + 1);
    for word in words.iter().filter(|_| phrase_context <= strength) {
        let word = canonical_word(word);
        if word.starts_with(&lower) && word != lower {
            matched_context = true;
            add(format!("{left}{}", case_completion(&word, prefix)));
        }
    }
    for (matched, remaining) in phrases.iter().copied() {
        // A known phrase can finish its next word even when that word has no
        // dedicated NEXT_WORDS entry. A bare prefix still uses vocabulary ranks.
        if matched <= prefix.chars().count() {
            continue;
        }
        let suffix = remaining
            .split(|ch: char| !is_word_char(ch))
            .next()
            .unwrap_or("");
        if !suffix.is_empty() {
            matched_context = true;
            add(format!(
                "{left}{}",
                case_completion(&format!("{lower}{suffix}"), prefix)
            ));
        }
    }
    // Do not dilute "good m -> morning" with "my/me", or "please sen -> send"
    // with "sent/sentence". Unmatched prefixes still use the general index.
    if matched_context {
        return output;
    }
    // Exact words continue naturally without being rewritten (hello -> hello world).
    let (strength, words) = context_word_match(&context);
    if phrase_strength > strength && phrase_strength > prefix.chars().count() {
        let mut continued = false;
        for (_, remaining) in &phrases {
            if remaining.starts_with(char::is_whitespace) {
                let next = remaining
                    .trim_start()
                    .split(|ch: char| !is_word_char(ch))
                    .next()
                    .unwrap_or("");
                if !next.is_empty() {
                    add(format!("{seed} {next}"));
                    continued = true;
                }
            }
        }
        if continued {
            return output;
        }
    } else {
        for word in words {
            add(format!("{seed} {word}"));
        }
    }
    let ending_context = canonical_context(current_clause(seed));
    if let Some(entry) = phrase_ending_contexts()
        .iter()
        .find(|entry| entry.prefix == ending_context)
    {
        for ending in entry.words {
            add(format!("{seed} {ending}"));
        }
    }
    let words = dictionary();
    let first = words.partition_point(|word| word.text.as_str() < lower.as_str());
    let mut matches: Vec<_> = words[first..]
        .iter()
        .take_while(|word| word.text.starts_with(&lower))
        .filter(|word| word.text != lower)
        .collect();
    matches.sort_by_key(|word| word.rank);
    for word in matches.into_iter().take(5) {
        add(format!("{left}{}", case_completion(&word.text, prefix)));
    }
    output
}

/// Offline replacements shared by the IME and the legacy provider fallback.
pub fn english_sentence_variants(seed: &str) -> Vec<String> {
    english_variants(seed, "")
}

/// Match a typed prefix without changing its spelling, case or spacing. Return
/// only new characters from the authored phrase, never committed context.
fn phrase_remainder<'a>(typed: &str, phrase: &'a str) -> Option<&'a str> {
    let mut typed = typed.chars().peekable();
    let mut phrase_chars = phrase.char_indices().peekable();
    while let Some(ch) = typed.next() {
        let (_, expected) = phrase_chars.next()?;
        if ch.is_whitespace() && expected.is_whitespace() {
            while typed.peek().is_some_and(|ch| ch.is_whitespace()) {
                typed.next();
            }
            while phrase_chars
                .peek()
                .is_some_and(|(_, ch)| ch.is_whitespace())
            {
                phrase_chars.next();
            }
        } else if !(ch.eq_ignore_ascii_case(&expected)
            || matches!(ch, '\'' | '’') && matches!(expected, '\'' | '’'))
        {
            return None;
        }
    }
    let start = phrase_chars
        .peek()
        .map_or(phrase.len(), |(index, _)| *index);
    (start < phrase.len()).then_some(&phrase[start..])
}

fn sentence_remainders(seed: &str, context: &str) -> Vec<(usize, &'static str)> {
    if seed.len() > 4096 || english_word_prefix(seed.trim_end().trim_end_matches(',')).is_none() {
        return Vec::new();
    }
    let combined = format!("{} {seed}", bounded_context(context));
    let combined = current_line(&combined);
    let mut separators = word_boundaries(combined).peekable();
    let mut boundary = true;
    let starts: Vec<_> = combined
        .char_indices()
        .filter_map(|(index, ch)| {
            let start = boundary && ch.is_ascii_alphabetic();
            // An ASCII quote opens a word only at an existing boundary. Treating
            // every apostrophe as a boundary would split contractions/identifiers.
            let separator = separators
                .peek()
                .is_some_and(|(offset, _)| *offset == index);
            if separator {
                separators.next();
            }
            boundary = separator
                || matches!(ch, '(' | '[' | '{' | '"' | '“' | '‘')
                || (boundary && ch == '\'');
            start.then_some(index)
        })
        .collect();
    // Longest matching context first; a later word must not displace a full
    // phrase match. A failed match never falls back to a fabricated suffix.
    for start in starts {
        let typed = &combined[start..];
        let remainders: Vec<_> = vocabulary()
            .sentences()
            .filter_map(|phrase| phrase_remainder(typed, phrase))
            .collect();
        if !remainders.is_empty() {
            // Compare content length, not repeated spaces or UTF-8 byte width,
            // with the normalized explicit collocation match. Compute it once.
            let matched = typed
                .split_whitespace()
                .map(|word| word.chars().count() + 1)
                .sum::<usize>()
                .saturating_sub(1);
            return remainders.into_iter().map(|rest| (matched, rest)).collect();
        }
    }
    Vec::new()
}

/// Explicit collocations only, including a partly typed continuation. Unknown
/// words, identifiers and URLs still have no made-up sentence fallback.
pub(crate) fn mixed_candidates(
    seed: &str,
    context: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    use crate::ime::candidate_mix::CandidateKind;
    let mut output = Vec::new();
    for (_, remaining) in sentence_remainders(seed, context) {
        let text = if let Some(prefix) = english_word_prefix(seed) {
            let split = remaining
                .find(|ch: char| !is_word_char(ch))
                .unwrap_or(remaining.len());
            let word = case_completion(
                &format!(
                    "{}{suffix}",
                    canonical_word(prefix),
                    suffix = &remaining[..split]
                ),
                prefix,
            );
            format!(
                "{}{word}{}",
                &seed[..seed.len() - prefix.len()],
                &remaining[split..]
            )
        } else {
            format!("{seed}{remaining}")
        };
        if output.len() < 6 && !output.iter().any(|(value, _)| value == &text) {
            output.push((text, CandidateKind::Sentence));
        }
    }
    output
}

/// A known, complete last word may continue before the user types Space.
/// Return only the new word's tail; keep every separator in the full payload.
pub(crate) fn known_word_continuation<'a>(seed: &str, text: &'a str) -> Option<&'a str> {
    let prefix = english_word_prefix(seed)?;
    if !is_known_english_word(prefix) {
        return None;
    }
    text.strip_prefix(seed)?
        .strip_prefix(' ')
        .map(|tail| tail.trim_start_matches(' '))
}

/// Extract only a complete next word actually present in a valid continuation.
/// This never rewrites the typed prefix or splits URLs, hyphenations/identifiers.
pub(crate) fn word_from_continuation<'a>(seed: &str, text: &'a str) -> Option<&'a str> {
    if seed.is_empty() {
        return None;
    }
    let suffix = text.strip_prefix(seed)?;
    if !seed.ends_with(char::is_whitespace) {
        english_word_prefix(seed)?;
    }
    // Before a separator, complete the current word; after one, finish the next.
    // A model may supply the separator after a known complete word. An unknown
    // spelling such as "hel there" still cannot become a word completion.
    let suffix = if suffix.starts_with(' ') {
        known_word_continuation(seed, text)?
    } else {
        suffix
    };
    let split = suffix.find(|ch: char| !is_word_char(ch))?;
    if split == 0 {
        return None;
    }
    let end = text.len() - suffix.len() + split;
    let word = &text[..end];
    let prefix = english_word_prefix(word)?;
    if !prefix.ends_with(|ch: char| ch.is_ascii_alphabetic()) {
        return None;
    }
    let mut rest = text[end..].chars();
    let boundary = rest.next()?;
    let separated = boundary.is_whitespace()
        || (matches!(
            boundary,
            ',' | '.' | '?' | '!' | ';' | ':' | ')' | ']' | '}' | '"'
        ) && rest.next().is_none_or(char::is_whitespace));
    separated.then_some(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Current total, independent of the immutable per-layer historical baselines.
    const EXPECTED_DICTIONARY_WORDS: usize = 6124;

    fn layer_words(id: &str) -> impl Iterator<Item = &'static str> {
        vocabulary()
            .word_layers()
            .iter()
            .find(|layer| layer.id == id)
            .unwrap()
            .words
            .iter()
            .map(String::as_str)
    }

    #[test]
    fn raw_input_is_first_and_completions_preserve_case_and_prefix() {
        for (seed, expected) in [
            ("hel", "hello"),
            ("Say Hel", "Say Hello"),
            ("HEL", "HELLO"),
            ("(hel", "(hello"),
            ("  please  sen", "  please  send"),
            ("don'", "don't"),
            ("don’", "don’t"),
        ] {
            let words = english_sentence_variants(seed);
            assert_eq!(words[0], seed);
            assert!(words.contains(&expected.into()), "{seed}: {words:?}");
            assert!(words.iter().all(|word| word.starts_with(seed)));
        }
    }

    #[test]
    fn boundary_scan_window_excludes_finished_history_without_hiding_token_syntax() {
        let history = format!("{}\u{3000}", "Completed note. ".repeat(4096));
        for token in [
            "说明—hel".to_owned(),
            "前文，hel".to_owned(),
            format!("https://{}，hel", "字".repeat(400)),
            "src/说明—hel".to_owned(),
            "user_name，hel".to_owned(),
        ] {
            let text = format!("{history}{token}");
            let tail_start = text.len() - "hel".len();
            let (base, window) = boundary_scan_window(&text, tail_start);
            assert_eq!(base, history.len() - '\u{3000}'.len_utf8());
            assert_eq!(window, format!("\u{3000}{token}"));
            assert_eq!(
                word_boundaries_from(&text, tail_start)
                    .filter(|(index, _)| *index >= tail_start)
                    .collect::<Vec<_>>(),
                word_boundaries(&text)
                    .filter(|(index, _)| *index >= tail_start)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                word_boundaries_from(&text, text.len()).last(),
                word_boundaries(&text).last()
            );
        }
        let unsplit = format!("https://{}—hel", "字".repeat(400));
        assert_eq!(
            boundary_scan_window(&unsplit, unsplit.len() - 3),
            (0, unsplit.as_str())
        );
        assert!(
            word_boundaries_from(&unsplit, unsplit.len() - 3)
                .next()
                .is_none()
        );
        let whitespace_start = history.len() - '\u{3000}'.len_utf8();
        assert_eq!(
            boundary_scan_window(&history, whitespace_start),
            (whitespace_start, "\u{3000}")
        );
    }

    #[test]
    fn bounded_context_token_scan_preserves_full_scan_semantics() {
        fn full_scan(context: &str) -> &str {
            let start = context
                .char_indices()
                .rev()
                .nth(159)
                .map_or(0, |(index, _)| index);
            if start == 0
                || word_boundaries(context).any(|(index, ch)| index + ch.len_utf8() == start)
            {
                return &context[start..];
            }
            word_boundaries(context)
                .find(|(index, _)| *index >= start)
                .map_or("", |(index, ch)| {
                    &context[if ch.is_whitespace() {
                        index
                    } else {
                        index + ch.len_utf8()
                    }..]
                })
        }
        let history = "Already finished. ".repeat(1024);
        for separator in [" ", "  ", "\u{3000}", "\n", "—", "，", "。"] {
            for padding in [0, 150, 154, 155, 156, 157, 158, 159, 160, 161] {
                for token in ["plain", "https://host/", "user_name", "example.com", "你好"] {
                    let context =
                        format!("{history}{token}{}{separator}good", "字".repeat(padding));
                    assert_eq!(
                        bounded_context(&context),
                        full_scan(&context),
                        "{token:?}/{separator:?}/{padding}"
                    );
                }
            }
        }
    }

    #[test]
    fn contractions_complete_equally_with_straight_and_smart_apostrophes() {
        for (prefix, expected) in [
            ("we'", "we're"),
            ("they'", "they're"),
            ("isn'", "isn't"),
            ("couldn'", "couldn't"),
            ("haven'", "haven't"),
            ("I'V", "I'VE"),
        ] {
            let straight = english_sentence_variants(prefix);
            assert!(straight.contains(&expected.to_owned()), "{straight:?}");
            let smart_prefix = prefix.replace('\'', "’");
            let smart = english_sentence_variants(&smart_prefix);
            assert_eq!(
                smart,
                straight
                    .iter()
                    .map(|word| word.replace('\'', "’"))
                    .collect::<Vec<_>>()
            );
            assert!(is_known_english_word(&expected.replace('\'', "’")));
        }
    }

    #[test]
    fn projected_model_words_are_exact_complete_natural_language_tails() {
        for (seed, text, expected) in [
            ("don", "don't worry.", "don't"),
            ("don’", "don’t worry.", "don’t"),
            ("let's ", "let's try again.", "let's try"),
            ("hel", "hello, how are you?", "hello"),
            ("appre", "appreciate.", "appreciate"),
            ("Please RE", "Please REVIEW the changes.", "Please REVIEW"),
            ("hello", "hello sunshine today.", "hello sunshine"),
            ("hello", "hello  sunshine today.", "hello  sunshine"),
            ("Hello", "Hello don't worry.", "Hello don't"),
            ("你好 hello", "你好 hello don't worry.", "你好 hello don't"),
            ("I’m", "I’m ready for it.", "I’m ready"),
            (
                "please send",
                "please send reliable backups.",
                "please send reliable",
            ),
        ] {
            assert_eq!(word_from_continuation(seed, text), Some(expected));
        }
        for (seed, text) in [
            ("hel", "hel there"),
            ("hel", "hello-world event"),
            ("hel", "hello.com site"),
            ("hel", "hello@example.com"),
            ("user_na", "user_name is ready"),
            ("src/he", "src/hello.rs"),
            ("hel", "helloЖ test"),
            ("hel", "hello123 test"),
            ("hel", "hello' word"),
            ("hel", "goodbye, hello"),
            ("hel", "hello"),
            ("hello", "hello world-wide event"),
            ("hello", "hello example.com site"),
            ("hello", "hello user_name today"),
            ("hello", "hello 世界 today"),
            ("iPh", "iPh model today"),
        ] {
            assert_eq!(word_from_continuation(seed, text), None, "{seed} -> {text}");
        }
    }

    #[test]
    fn opening_quotes_allow_completion_without_splitting_contractions() {
        for (seed, word, sentence) in [
            ("'hel", "'hello", "'hello, how are you?"),
            (
                "He said 'please sen",
                "He said 'please send",
                "He said 'please send me the details.",
            ),
            (
                "('Please  sen",
                "('Please  send",
                "('Please  send me the details.",
            ),
        ] {
            assert!(
                english_sentence_variants(seed).contains(&word.to_owned()),
                "{seed}"
            );
            assert!(
                mixed_candidates(seed, "")
                    .iter()
                    .any(|(text, _)| text == sentence),
                "{seed}"
            );
        }
        for seed in ["'hello'", "can't'hel", "don't'please", "user'hel"] {
            assert_eq!(english_sentence_variants(seed), [seed]);
            assert!(mixed_candidates(seed, "").is_empty(), "{seed}");
        }
        assert_eq!(
            phrase_remainder("We’re  wor", "we're working on it."),
            Some("king on it.")
        );
        assert_eq!(case_completion("wouldn't've", "wouldn’t'v"), "wouldn’t've");
    }

    #[test]
    fn context_ranks_completions_and_only_suggests_immediate_next_words() {
        assert_eq!(english_variants("w", "hello")[1], "world");
        assert_eq!(english_variants("m", "good")[1], "morning");
        assert_eq!(english_sentence_variants("please sen")[1], "please send");
        assert_eq!(
            english_sentence_variants("please sen"),
            ["please sen", "please send"]
        );
        assert!(!english_sentence_variants("good m").contains(&"good my".into()));
        assert_eq!(english_sentence_variants("thank you ")[1], "thank you for");
        assert_eq!(english_sentence_variants("let me ")[1], "let me know");
        assert_eq!(english_sentence_variants("hello  ")[1], "hello  world");
        assert_eq!(
            english_variants("m", "good.")[1],
            english_variants("m", "")[1]
        );
        assert_eq!(english_sentence_variants("unknownword "), ["unknownword "]);
    }

    #[test]
    fn writing_flow_keeps_words_and_sentences_through_spaces_and_partial_words() {
        for (seed, word, sentence) in [
            (
                "please send ",
                "please send me",
                "please send me the details.",
            ),
            (
                "please send m",
                "please send me",
                "please send me the details.",
            ),
            (
                "please send me the d",
                "please send me the details",
                "please send me the details.",
            ),
            (
                "let me know ",
                "let me know what",
                "let me know what you think.",
            ),
            (
                "let me know w",
                "let me know what",
                "let me know what you think.",
            ),
            (
                "  Please  send  ",
                "  Please  send  me",
                "  Please  send  me the details.",
            ),
            ("hello, ", "hello, how", "hello, how are you?"),
            ("hello, h", "hello, how", "hello, how are you?"),
            ("HEL", "HELLO", "HELLO, how are you?"),
            ("Please SEN", "Please SEND", "Please SEND me the details."),
        ] {
            let words = english_variants(seed, "");
            let sentences = mixed_candidates(seed, "");
            assert_eq!(words[0], seed);
            assert!(words.contains(&word.into()), "{seed}: {words:?}");
            assert!(
                sentences.iter().any(|(text, _)| text == sentence),
                "{seed}: {sentences:?}"
            );
            assert!(sentences.iter().all(|(text, _)| text.starts_with(seed)));
        }
    }

    #[test]
    fn writing_flow_extends_only_the_draft_and_stops_at_unknown_or_finished_text() {
        assert!(
            mixed_candidates("me the d", "please send")
                .iter()
                .any(|(text, _)| text == "me the details.")
        );
        assert!(
            mixed_candidates("know ", "let me")
                .iter()
                .any(|(text, _)| text == "know what you think.")
        );
        for seed in [
            "let me know zzz",
            "please send!",
            "please send me the details.",
            "https://please",
            "src/please",
            "user_please",
            "please-send",
            "don't qzxv",
        ] {
            assert!(mixed_candidates(seed, "").is_empty(), "{seed}");
        }
        // The daily-chat layer now deliberately covers this ordinary contraction.
        // Keep the unknown suffix negative control above rather than forbidding
        // all authored continuations that start with don't.
        assert!(mixed_candidates("don't", "").iter().any(|(text, kind)| {
            text == "don't worry about it."
                && *kind == crate::ime::candidate_mix::CandidateKind::Sentence
        }));
        assert!(mixed_candidates("x".repeat(5000).as_str(), "please").is_empty());
    }

    #[test]
    fn prose_boundaries_keep_bilingual_word_and_sentence_completion() {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let prefix = format!("前文{separator}");
            for (tail, word, sentence) in [
                ("hel", "hello", "hello, how are you?"),
                ("Please SEN", "Please SEND", "Please SEND me the details."),
                (
                    "please send ",
                    "please send me",
                    "please send me the details.",
                ),
            ] {
                let seed = format!("{prefix}{tail}");
                let words = english_sentence_variants(&seed);
                assert_eq!(words[0], seed);
                assert!(
                    words.contains(&format!("{prefix}{word}")),
                    "{seed}: {words:?}"
                );
                let sentences = mixed_candidates(&seed, "");
                assert!(
                    sentences
                        .iter()
                        .any(|(text, _)| text == &format!("{prefix}{sentence}")),
                    "{seed}: {sentences:?}"
                );
                assert!(words.iter().all(|text| text.starts_with(&seed)));
                assert!(sentences.iter().all(|(text, _)| text.starts_with(&seed)));
            }
            assert_eq!(
                english_variants("m", &format!("{prefix}good"))[1],
                "morning"
            );
            assert!(
                mixed_candidates("me the d", &format!("{prefix}please send"))
                    .iter()
                    .any(|(text, _)| text == "me the details.")
            );
        }
    }

    #[test]
    fn prose_boundaries_do_not_expose_suffixes_inside_opaque_tokens() {
        for seed in [
            "hello-world",
            "note.hel",
            "note:hel",
            "note,hel",
            "user_hel",
            "test@hel",
            "src/hel",
            "src\\hel",
            "hello–hel",
            "你好hel",
            "https://host/前文，hel",
            "src/前文—hel",
            "user_前文，hel",
            "person@前文。hel",
            "example.com—hel",
            "user-name，hel",
        ] {
            assert_eq!(english_sentence_variants(seed), [seed], "{seed}");
            assert!(mixed_candidates(seed, "").is_empty(), "{seed}");
        }
    }

    #[test]
    fn literal_input_is_not_autocorrected_or_split_inside_non_words() {
        for seed in [
            "unknown-token",
            "https://exa",
            "test@exam",
            "src/mai",
            "user_nam",
            "v0.4",
            "iPh",
            "你好hel",
            "foo.rs",
            "hello!",
            "hello-world",
        ] {
            assert_eq!(english_sentence_variants(seed), [seed], "{seed}");
        }
        assert!(english_sentence_variants("   ").is_empty());
        assert_eq!(
            EnglishLanguagePlugin.normalize_seed("  hello  "),
            "  hello  "
        );
    }

    #[test]
    fn vocabulary_is_bounded_indexed_and_covers_every_prefix() {
        assert!(dictionary().len() >= 5900, "{}", dictionary().len());
        assert!(dictionary().len() < 6144, "{}", dictionary().len());
        for pair in dictionary().windows(2) {
            assert!(pair[0].text < pair[1].text);
        }
        for prefix in [
            "a", "com", "proj", "sched", "compati", "develop", "en", "keyb",
        ] {
            let words = english_sentence_variants(prefix);
            assert!(words.len() >= 2 && words.len() <= 12, "{prefix}: {words:?}");
            assert!(words.iter().all(|word| word.starts_with(prefix)));
        }
    }

    #[test]
    fn authored_additions_are_unique_explicit_word_forms() {
        let baseline: std::collections::HashSet<_> =
            layer_words("core").map(canonical_word).collect();
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("extended") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(!baseline.contains(word), "duplicate baseline word: {word}");
            assert!(additions.insert(word), "duplicate addition: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        for invented in ["runned", "buyed", "goed", "choosed", "bringed", "writed"] {
            assert!(!is_known_english_word(invented), "{invented}");
        }
    }

    #[test]
    fn writing_tier_preserves_every_existing_word_rank() {
        let baseline = indexed_words(&vocabulary().word_layers()[..2]);
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in baseline.enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 3906);
        for (word, rank) in &old_ranks {
            let index = dictionary()
                .binary_search_by(|entry| entry.text.cmp(word))
                .unwrap();
            assert_eq!(dictionary()[index].rank, *rank, "rank changed for {word}");
        }
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("writing") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate writing word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 912);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
    }

    #[test]
    fn daily_tier_preserves_the_whole_writing_index_and_real_word_forms() {
        let previous = indexed_words(&vocabulary().word_layers()[..3]);
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in previous.enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 4827);
        for (word, rank) in &old_ranks {
            let index = dictionary()
                .binary_search_by(|entry| entry.text.cmp(word))
                .unwrap();
            assert_eq!(dictionary()[index].rank, *rank, "rank changed for {word}");
        }
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("daily") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate daily word: {word}");
            assert!(is_known_english_word(word), "missing word: {word}");
            assert!(
                is_known_english_word(&word.replace('\'', "’")),
                "missing curly form: {word}"
            );
        }
        assert_eq!(additions.len(), 419);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        for invented in [
            "sweeped",
            "oversleeped",
            "hangged",
            "picnicing",
            "rechargeing",
        ] {
            assert!(!is_known_english_word(invented), "{invented}");
        }
    }

    #[test]
    fn fallback_tier_preserves_all_previous_ranks_and_uses_explicit_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..4]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 5251);
        for (word, rank) in &old_ranks {
            let index = dictionary()
                .binary_search_by(|entry| entry.text.cmp(word))
                .unwrap();
            assert_eq!(dictionary()[index].rank, *rank, "rank changed for {word}");
        }
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("fallback") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate fallback word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 153);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[4];
        assert_eq!(layer.id, "fallback");
        assert_eq!(layer.next_words.len(), 40);
        assert_eq!(layer.sentences.len(), 80);
    }

    #[test]
    fn conversation_tier_preserves_the_previous_fallback_and_real_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..5]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 5410);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before this layer was appended. Keep the original migration
        // fingerprint below too: neither old data nor the first fallback may drift.
        assert_eq!(count, 5410);
        assert_eq!(hash, 0x15a8c224b0150864);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("conversation") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(
                additions.insert(word),
                "duplicate conversation word: {word}"
            );
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 184);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[5];
        assert_eq!(layer.id, "conversation");
        assert_eq!(layer.next_words.len(), 48);
        assert_eq!(layer.sentences.len(), 96);
    }

    #[test]
    fn essentials_tier_preserves_conversation_ranks_and_explicit_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..6]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 5607);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before essentials was appended. Keep the older snapshots too,
        // so each expansion protects its predecessors rather than rebaselining them.
        assert_eq!(count, 5607);
        assert_eq!(hash, 0x94bde893829bcfb0);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("essentials") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate essentials word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 321);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[6];
        assert_eq!(layer.id, "essentials");
        assert_eq!(layer.next_words.len(), 48);
        assert_eq!(layer.sentences.len(), 96);
    }

    #[test]
    fn clarity_tier_preserves_essentials_ranks_and_explicit_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..7]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 5934);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured from the complete 0.7.4 index before this append-only layer.
        // Keep all earlier snapshots rather than replacing their baselines.
        assert_eq!(count, 5934);
        assert_eq!(hash, 0x9155820a5fb26b42);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("clarity") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate clarity word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 58);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[7];
        assert_eq!(layer.id, "clarity");
        assert_eq!(layer.next_words.len(), 40);
        assert_eq!(layer.sentences.len(), 80);
    }

    #[test]
    fn digital_tier_preserves_clarity_ranks_and_explicit_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..8]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6004);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured from the complete 0.7.5 index before adding the digital layer.
        // Earlier rank snapshots stay intact, not rerecorded after expansion.
        assert_eq!(count, 6004);
        assert_eq!(hash, 0xa8ce6407237cac70);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("digital") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate digital word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 30);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[8];
        assert_eq!(layer.id, "digital");
        assert_eq!(layer.next_words.len(), 32);
        assert_eq!(layer.sentences.len(), 64);
    }

    #[test]
    fn home_tier_preserves_digital_ranks_and_explicit_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..9]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6041);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before appending home vocabulary. Keep all earlier snapshots
        // and the complete digital layer, not just the originally migrated words.
        assert_eq!(count, 6041);
        assert_eq!(hash, 0x3321c9cc81a2e17e);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("home") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate home word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 20);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[9];
        assert_eq!(layer.id, "home");
        assert_eq!(layer.next_words.len(), 16);
        assert_eq!(layer.sentences.len(), 32);
    }

    #[test]
    fn errands_tier_preserves_home_ranks_and_explicit_word_forms() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..10]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6074);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Taken before adding errands; never regenerate old snapshots to hide reordering.
        assert_eq!(count, 6074);
        assert_eq!(hash, 0xd2b2bfcaf0dc2a6c);
        let mut additions = std::collections::HashSet::new();
        for word in layer_words("errands") {
            assert!(
                word.bytes()
                    .all(|ch| ch.is_ascii_lowercase() || ch == b'\'')
            );
            assert!(
                !old_ranks.contains_key(word),
                "duplicate earlier word: {word}"
            );
            assert!(additions.insert(word), "duplicate errands word: {word}");
            assert!(is_known_english_word(word), "missing indexed word: {word}");
        }
        assert_eq!(additions.len(), 22);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
        let layer = &vocabulary().word_layers()[10];
        assert_eq!(layer.id, "errands");
        assert_eq!(layer.next_words.len(), 16);
        assert_eq!(layer.sentences.len(), 32);
    }

    #[test]
    fn daily_chat_tier_preserves_all_previous_ranks() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..11]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6102);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured from all eleven layers before daily_chat was appended.
        assert_eq!(count, 6102);
        assert_eq!(hash, 0xeb278317cd907ded);
        let layer = &vocabulary().word_layers()[11];
        assert_eq!(layer.id, "daily_chat");
        assert_eq!(layer.next_words.len(), 24);
        assert_eq!(layer.sentences.len(), 48);
        assert_eq!(dictionary().len(), EXPECTED_DICTIONARY_WORDS);
    }

    #[test]
    fn daily_needs_tier_preserves_daily_chat_ranks() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..12]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6108);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before this batch, retaining all earlier snapshots unchanged.
        assert_eq!(count, 6108);
        assert_eq!(hash, 0xe7ff45518e731e05);
        let layer = &vocabulary().word_layers()[12];
        assert_eq!(layer.id, "daily_needs");
        assert_eq!(layer.next_words.len(), 24);
        assert_eq!(layer.sentences.len(), 48);
    }

    #[test]
    fn daily_coordination_preserves_all_released_word_ranks() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..13]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6111);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured from the clean 0.8.1 source, before this batch was appended.
        assert_eq!(count, 6111);
        assert_eq!(hash, 0x99a6271dff7f0b9f);
        let layer = &vocabulary().word_layers()[13];
        assert_eq!(layer.id, "daily_coordination");
        assert_eq!(layer.next_words.len(), 24);
        assert_eq!(layer.sentences.len(), 48);
    }

    #[test]
    fn daily_social_preserves_coordination_word_ranks() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..14]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6113);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before appending social data; retain every older snapshot.
        assert_eq!(count, 6113);
        assert_eq!(hash, 0xdf1bddaff4c3ecea);
        let layer = &vocabulary().word_layers()[14];
        assert_eq!(layer.id, "daily_social");
        assert_eq!(layer.next_words.len(), 24);
        assert_eq!(layer.sentences.len(), 48);
    }

    #[test]
    fn daily_objects_preserves_social_word_ranks() {
        let mut old_ranks = std::collections::HashMap::new();
        for (rank, word) in indexed_words(&vocabulary().word_layers()[..15]).enumerate() {
            old_ranks.entry(canonical_word(word)).or_insert(rank);
        }
        assert_eq!(old_ranks.len(), 6117);
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| old_ranks.contains_key(&word.text))
        {
            count += 1;
            assert_eq!(
                word.rank, old_ranks[&word.text],
                "rank changed for {}",
                word.text
            );
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before appending daily_objects; all earlier snapshots remain.
        assert_eq!(count, 6117);
        assert_eq!(hash, 0x0e93ef5b25176f5a);
        let layer = &vocabulary().word_layers()[15];
        assert_eq!(layer.id, "daily_objects");
        assert_eq!(layer.next_words.len(), 24);
        assert_eq!(layer.sentences.len(), 48);
    }

    #[test]
    fn authored_collocations_and_sentences_are_unique_and_reachable() {
        use crate::ime::candidate_mix::CandidateKind;
        let mut keys = std::collections::HashSet::new();
        for (key, words) in vocabulary().next_words() {
            assert!(keys.insert(key), "duplicate collocation: {key}");
            assert_eq!(key.trim(), key);
            assert!(!words.is_empty());
            let mut unique = std::collections::HashSet::new();
            for word in words {
                assert!(
                    word.chars()
                        .all(|ch| ch.is_ascii_alphabetic() || ch == '\'')
                );
                assert!(
                    unique.insert(canonical_word(word)),
                    "duplicate next word: {key} {word}"
                );
                assert!(is_known_english_word(word), "{key} {word}");
            }
        }
        let mut sentences = std::collections::HashSet::new();
        for sentence in vocabulary().sentences() {
            assert!(
                sentences.insert(canonical_word(sentence)),
                "duplicate sentence: {sentence}"
            );
            assert!(
                sentence.ends_with(['.', '?', '!']),
                "unfinished sentence: {sentence}"
            );
            let seed = sentence.trim_end_matches(['.', '?', '!']);
            assert!(
                mixed_candidates(seed, "")
                    .iter()
                    .any(|(text, kind)| { text == sentence && *kind == CandidateKind::Sentence }),
                "unreachable sentence: {sentence}"
            );
        }
        assert_eq!(keys.len(), 497);
        assert_eq!(sentences.len(), 958);
    }

    #[test]
    fn resource_migration_preserves_all_5251_word_ranks() {
        // Captured from the pre-migration Rust tables, including duplicate
        // priority positions and words projected from explicit sentences.
        // Later layers may add words, but must not change this old fingerprint.
        let original: std::collections::HashSet<_> =
            indexed_words(&vocabulary().word_layers()[..4])
                .map(canonical_word)
                .collect();
        let mut hash = 0xcbf29ce484222325_u64;
        let mut count = 0;
        for word in dictionary()
            .iter()
            .filter(|word| original.contains(&word.text))
        {
            count += 1;
            for byte in word
                .text
                .bytes()
                .chain([0])
                .chain((word.rank as u64).to_le_bytes())
            {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(count, 5251);
        assert_eq!(hash, 0x9db0c1a22060829a);
    }

    #[test]
    fn indexing_accepts_data_layers_without_knowing_their_names() {
        let data = Lexicon::from_json(r#"{"format_version":1,"language":"en","word_layers":[
            {"id":"unrelated-name","words":["first"],"next_words":[["a context",["second"]]],"sentences":["third fourth."]},
            {"id":"one-more-layer","words":["fifth"]}
        ]}"#).unwrap();
        assert_eq!(
            indexed_words(data.word_layers()).collect::<Vec<_>>(),
            ["first", "second", "third", "fourth", "fifth"]
        );
    }
}
