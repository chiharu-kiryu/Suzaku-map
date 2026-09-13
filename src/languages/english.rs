use super::ranked_candidates;
use crate::ime::{Candidate, LanguagePlugin};
use std::sync::OnceLock;

#[path = "english_lexicon.rs"]
mod lexicon;

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
        let mut words: Vec<_> = lexicon::WORDS
            .split_whitespace()
            .chain(
                lexicon::NEXT_WORDS
                    .iter()
                    .flat_map(|(_, words)| words.iter().copied()),
            )
            .chain(lexicon::SENTENCES.iter().flat_map(|sentence| {
                sentence
                    .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
                    .filter(|word| !word.is_empty())
            }))
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

/// Only natural-language word tails are eligible. Do not rewrite URLs, paths,
/// identifiers, numbers, hyphenated expressions or mixed-script input.
pub fn english_word_prefix(seed: &str) -> Option<&str> {
    let token = seed.rsplit(char::is_whitespace).next()?;
    let word = token.trim_start_matches(['(', '[', '{', '"', '“', '‘', '\'']);
    if word.is_empty()
        || word.len() > 64
        || !word.starts_with(|c: char| c.is_ascii_alphabetic())
        || !word
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '’')
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

fn context_word_match(context: &str) -> (usize, &'static [&'static str]) {
    let boundary = context
        .rfind(['.', '?', '!', ';', '\n', '\r'])
        .map_or(0, |i| i + 1);
    let normalized = context[boundary..]
        .split_whitespace()
        .map(|word| {
            canonical_word(word.trim_matches(|c: char| !c.is_alphabetic() && c != '\'' && c != '’'))
        })
        .collect::<Vec<_>>()
        .join(" ");
    lexicon::NEXT_WORDS
        .iter()
        .filter(|(prefix, _)| {
            normalized == *prefix
                || normalized
                    .strip_suffix(prefix)
                    .is_some_and(|left| left.ends_with(' '))
        })
        .max_by_key(|(prefix, _)| prefix.len())
        .map_or((0, &[]), |(prefix, words)| (prefix.chars().count(), *words))
}

fn bounded_context(context: &str) -> &str {
    let start = context
        .char_indices()
        .rev()
        .nth(159)
        .map_or(0, |(index, _)| index);
    &context[start..]
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
                .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
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
            .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
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
                    .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
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
    if let Some((_, endings)) = lexicon::PHRASE_ENDINGS
        .iter()
        .find(|(prefix, _)| *prefix == seed.to_lowercase())
    {
        for ending in *endings {
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
    let mut boundary = true;
    let starts: Vec<_> = combined
        .char_indices()
        .filter_map(|(index, ch)| {
            let start = boundary && ch.is_ascii_alphabetic();
            // An ASCII quote opens a word only at an existing boundary. Treating
            // every apostrophe as a boundary would split contractions/identifiers.
            boundary = ch.is_whitespace()
                || matches!(ch, '(' | '[' | '{' | '"' | '“' | '‘')
                || (boundary && ch == '\'');
            start.then_some(index)
        })
        .collect();
    // Longest matching context first; a later word must not displace a full
    // phrase match. A failed match never falls back to a fabricated suffix.
    for start in starts {
        let typed = &combined[start..];
        let remainders: Vec<_> = lexicon::SENTENCES
            .iter()
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
                .find(|ch: char| !ch.is_ascii_alphabetic() && ch != '\'')
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
    // Do not turn "hel there" into a purported word completion.
    let split = suffix.find(|ch: char| !ch.is_ascii_alphabetic() && !matches!(ch, '\'' | '’'))?;
    if split == 0 {
        return None;
    }
    let end = seed.len() + split;
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
            "don't",
        ] {
            assert!(mixed_candidates(seed, "").is_empty(), "{seed}");
        }
        assert!(mixed_candidates("x".repeat(5000).as_str(), "please").is_empty());
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
        assert!(dictionary().len() >= 900, "{}", dictionary().len());
        assert!(dictionary().len() < 2000);
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
}
