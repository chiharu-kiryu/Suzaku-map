//! Deterministic Romaji/Kana baseline; Kanji vocabulary is intentionally a bootstrap set.

use super::ranked_candidates;
use crate::ime::{Candidate, LanguagePlugin};
use crate::lexicon::{EntryKind, Lexicon};

fn vocabulary() -> &'static Lexicon {
    crate::lexicon::active("ja").expect("Japanese vocabulary is registered")
}

fn kanji() -> impl DoubleEndedIterator<Item = (&'static str, &'static str)> {
    vocabulary()
        .readings()
        .iter()
        .map(|entry| (entry.reading.as_str(), entry.text.as_str()))
}

#[derive(Default)]
pub struct JapaneseLanguagePlugin;

impl LanguagePlugin for JapaneseLanguagePlugin {
    fn id(&self) -> &str {
        "ja"
    }
    fn display_name(&self) -> &str {
        "日本語"
    }
    fn normalize_seed(&self, input: &str) -> String {
        input.to_owned()
    }
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        if seed.trim().is_empty() {
            return Vec::new();
        }
        let kana = composition_kana(seed);
        let mut values: Vec<String> = kanji()
            .filter(|(reading, _)| *reading == kana)
            .map(|(_, text)| text.to_string())
            .collect();
        if values.is_empty() {
            let converted = convert_segments(&kana);
            if converted != kana {
                values.push(converted);
            }
        }
        values.extend([kana.clone(), hiragana_to_katakana(&kana), seed.to_string()]);
        ranked_candidates(values, confidence)
    }
    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        Some(self.build_candidates(&[], seed, confidence))
    }
    fn commit_separator(&self) -> &str {
        ""
    }
}

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    vocabulary().readings().iter().any(|entry| {
        entry.kind == EntryKind::Word
            && (entry.text == text
                || entry.reading == text
                || hiragana_to_katakana(&entry.reading) == text)
    })
}

/// Greedy dictionary segments plus untouched particles. This is a bounded
/// bootstrap conversion, not a morphological analyzer or a full Japanese IME.
fn convert_segments(kana: &str) -> String {
    let mut rest = kana;
    let mut output = String::new();
    while !rest.is_empty() {
        if let Some((length, word)) = conversion_prefix(rest) {
            output.push_str(word);
            rest = &rest[length..];
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    output
}

fn is_line_boundary(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

fn composition_kana(seed: &str) -> String {
    // Shift+Space separates romaji words, not words in the converted Japanese text.
    // Line boundaries are different: never join two lines to manufacture a
    // dictionary word or an authored sentence. Preserve CRLF as two scalars too.
    let mut output = String::new();
    let mut start = 0;
    for (offset, ch) in seed.char_indices().filter(|(_, ch)| ch.is_whitespace()) {
        output.push_str(&romaji_to_hiragana(&seed[start..offset]));
        if is_line_boundary(ch) {
            output.push(ch);
        }
        start = offset + ch.len_utf8();
    }
    output.push_str(&romaji_to_hiragana(&seed[start..]));
    output
}

/// Complete the final reading after known dictionary words and particles. Never
/// scan through an arbitrary unknown prefix looking for an unrelated dictionary suffix.
fn word_completions(seed: &str) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    use crate::ime::candidate_mix::CandidateKind;
    if seed.len() < 2 || seed.chars().count() > 256 {
        return Vec::new();
    }
    let kana = composition_kana(seed);
    let mut queries = vec![kana.clone()];
    // Trailing reading separators do not end a draft or its unfinished
    // syllable's completions. Probe before them, never across a line boundary.
    let reading = seed.trim_end_matches(|ch: char| ch.is_whitespace() && !is_line_boundary(ch));
    if reading.ends_with(|ch: char| {
        ch.is_ascii_alphabetic() && !"aiueo".contains(ch.to_ascii_lowercase())
    }) {
        queries.extend(
            ['a', 'i', 'u', 'e', 'o']
                .into_iter()
                .map(|vowel| composition_kana(&format!("{reading}{vowel}"))),
        );
    }
    let mut offset = 0;
    let mut prefix = String::new();
    let mut output = Vec::new();
    while offset < kana.len() {
        let rest = &kana[offset..];
        for (reading, word) in kanji() {
            if (reading != rest || offset > 0)
                && queries.iter().any(|query| {
                    query
                        .strip_prefix(&kana[..offset])
                        .is_some_and(|tail| !tail.is_empty() && reading.starts_with(tail))
                })
            {
                let text = format!("{prefix}{word}");
                if !output.iter().any(|(value, _)| value == &text) {
                    output.push((
                        text,
                        if is_dictionary_word(word) {
                            CandidateKind::Word
                        } else {
                            CandidateKind::Sentence
                        },
                    ));
                    if output.len() == 4 {
                        return output;
                    }
                }
            }
        }
        if let Some((length, word)) = conversion_prefix(rest) {
            prefix.push_str(word);
            offset += length;
        } else {
            let ch = rest.chars().next().unwrap();
            if !matches!(
                ch,
                'は' | 'を' | 'が' | 'に' | 'で' | 'と' | 'も' | 'へ' | 'の' | '\u{3400}'
                    ..='\u{9fff}'
            ) {
                break;
            }
            prefix.push(ch);
            offset += ch.len_utf8();
        }
    }
    output
}

/// Keep a recognized written word whole before converting the next reading.
/// A longer phonetic match still wins, and ties preserve the dictionary's
/// existing first variant (including pack homophones and pure Kana words).
fn conversion_prefix(rest: &str) -> Option<(usize, &str)> {
    let phonetic = kanji()
        .rev()
        .filter(|(reading, _)| rest.starts_with(reading))
        .max_by_key(|(reading, _)| reading.len());
    if let Some(length) = known_written_prefix_len(rest)
        && phonetic.is_none_or(|(reading, _)| length > reading.len())
    {
        return Some((length, &rest[..length]));
    }
    phonetic.map(|(reading, word)| (reading.len(), word))
}

fn known_written_prefix_len(text: &str) -> Option<usize> {
    vocabulary()
        .readings()
        .iter()
        .filter(|entry| entry.kind == EntryKind::Word)
        .flat_map(|entry| {
            [
                text.starts_with(&entry.text).then_some(entry.text.len()),
                text.get(..entry.reading.len())
                    .filter(|prefix| prefix.chars().eq(entry.reading.chars().map(katakana_char)))
                    .map(str::len),
            ]
        })
        .flatten()
        .max()
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    #[test]
    fn authored_sentences_follow_the_whole_draft_not_only_exact_triggers() {
        for (seed, expected) in [
            ("予定ga", "予定が決まったら連絡します。"),
            ("nihongo wo", "日本語を勉強しています。"),
            ("JYUNBI GADEKI", "準備ができたら連絡します。"),
            ("日本語を勉強して", "日本語を勉強しています。"),
        ] {
            let choices = mixed_candidates(seed);
            assert!(
                choices.iter().any(|(text, _)| text == expected),
                "{seed}: {choices:?}"
            );
        }
        let choices = mixed_candidates("日本語を勉強");
        let study: Vec<_> = choices
            .iter()
            .filter(|(_, kind)| *kind == crate::ime::candidate_mix::CandidateKind::Sentence)
            .map(|(text, _)| text.as_str())
            .collect();
        assert_eq!(
            study,
            ["日本語を勉強しています。", "日本語を勉強したいです。"]
        );
    }

    #[test]
    fn conversion_keeps_line_boundaries_but_joins_reading_separators() {
        assert_eq!(
            composition_kana("nihongo wo benkyou"),
            "にほんごをべんきょう"
        );
        for boundary in [
            "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("yotei{boundary}ga");
            assert_eq!(composition_kana(&seed), format!("よてい{boundary}が"));
            assert_eq!(
                convert_segments(&composition_kana(&seed)),
                format!("予定{boundary}が")
            );
        }
    }

    #[test]
    fn complete_tail_readings_keep_homophones_after_known_prefixes() {
        for seed in ["私はkanji", "watashihakanji", "私はかんじ"] {
            let choices = word_completions(seed);
            for word in ["私は漢字", "私は感じ"] {
                assert!(
                    choices.iter().any(|(text, kind)| text == word
                        && *kind == crate::ime::candidate_mix::CandidateKind::Word),
                    "{seed}: {choices:?}"
                );
            }
            assert!(choices.len() <= 4);
        }
    }
    #[test]
    fn known_katakana_prefixes_keep_their_spelling_during_completion() {
        for seed in ["ニホンゴwobenky", "ニホンゴ wo benky", "ニホンゴをべんき"] {
            let choices = word_completions(seed);
            assert!(
                choices.iter().any(|(text, _)| text == "ニホンゴを勉強"),
                "{seed}: {choices:?}"
            );
        }
        for seed in [
            "ワカラナイniho",
            "テストniho",
            "ニホンゴxyzniho",
            "ニホンゴ🙂niho",
            "ニホンゴ123niho",
        ] {
            assert!(word_completions(seed).is_empty(), "{seed}");
        }
        assert!(word_completions(&format!("{}benky", "ニ".repeat(252))).is_empty());
    }
    #[test]
    fn incomplete_consonants_and_terminal_n_can_finish_the_last_known_word() {
        for (seed, word) in [
            ("niho", "日本語"),
            ("nihong", "日本語"),
            ("nihon", "日本語"),
            ("watashihanihong", "私は日本語"),
            ("私はniho", "私は日本語"),
            ("nihongo wo benky", "日本語を勉強"),
        ] {
            let choices = word_completions(seed);
            assert!(
                choices.iter().any(|(text, _)| text == word),
                "{seed}: {choices:?}"
            );
            assert!(choices.len() <= 4);
        }
    }
    #[test]
    fn unknown_prefixes_are_not_scanned_for_unrelated_suffix_completions() {
        for seed in ["xyzniho", "https://niho", "foobarniho", "🙂niho"] {
            assert!(word_completions(seed).is_empty(), "{seed}");
        }
        assert!(word_completions(&"あ".repeat(300)).is_empty());
    }
}

pub(crate) fn mixed_candidates(
    seed: &str,
) -> Vec<(String, crate::ime::candidate_mix::CandidateKind)> {
    use crate::ime::candidate_mix::CandidateKind;

    let kana = composition_kana(seed);
    let converted = convert_segments(&kana);
    let mut output = Vec::new();
    // Segmented conversion should be available before the unchanged kana forms.
    if converted != kana {
        output.push((
            converted.clone(),
            if is_dictionary_word(&converted) {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            },
        ));
    }
    output.extend(word_completions(seed));
    let words: Vec<_> = std::iter::once(converted)
        .chain(
            output
                .iter()
                .filter(|(_, kind)| *kind == CandidateKind::Word)
                .map(|(text, _)| text.clone()),
        )
        .collect();
    for word in words {
        let mut contexts: Vec<_> = vocabulary()
            .continuations()
            .filter(|(prefix, values)| {
                *prefix == word
                    || (word.starts_with(prefix)
                        && values
                            .iter()
                            .any(|text| text.len() > word.len() && text.starts_with(&word)))
            })
            .collect();
        // Exact triggers retain their priority. Otherwise prefer the longest
        // applicable context; ties preserve the lexicon's authored layer order.
        contexts.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        for (prefix, values) in contexts {
            for text in values {
                // Match the *whole* progress, never discard an unknown prefix,
                // conflicting tail or a line break to recover a trigger. Exact
                // trigger behavior remains compatible with existing packs.
                if prefix != word && (text.len() <= word.len() || !text.starts_with(&word)) {
                    continue;
                }
                if !output.iter().any(|(existing, _)| existing == text) {
                    if output
                        .iter()
                        .filter(|(_, kind)| *kind == CandidateKind::Sentence)
                        .count()
                        >= 4
                    {
                        return output;
                    }
                    output.push((text.clone(), CandidateKind::Sentence));
                }
            }
        }
        if output
            .iter()
            .filter(|(_, kind)| *kind == CandidateKind::Sentence)
            .count()
            >= 4
        {
            break;
        }
    }
    output
}

// Standard five-vowel rows plus common alternative spellings. The conversion algorithm,
// including terminal n and doubled consonants, is separate from language/LLM dispatch.
const ROWS: &[(&str, &str)] = &[
    ("", "あいうえお"),
    ("k", "かきくけこ"),
    ("g", "がぎぐげご"),
    ("s", "さしすせそ"),
    ("z", "ざじずぜぞ"),
    ("t", "たちつてと"),
    ("d", "だぢづでど"),
    ("n", "なにぬねの"),
    ("h", "はひふへほ"),
    ("b", "ばびぶべぼ"),
    ("p", "ぱぴぷぺぽ"),
    ("m", "まみむめも"),
    ("r", "らりるれろ"),
    ("x", "ぁぃぅぇぉ"),
    ("l", "ぁぃぅぇぉ"),
];
const SPECIAL: &[(&str, &str)] = &[
    ("ltsu", "っ"),
    ("xtsu", "っ"),
    ("ltu", "っ"),
    ("xtu", "っ"),
    ("shi", "し"),
    ("chi", "ち"),
    ("tsu", "つ"),
    ("fu", "ふ"),
    ("ji", "じ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("sho", "しょ"),
    ("she", "しぇ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("che", "ちぇ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("jo", "じょ"),
    ("je", "じぇ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("ye", "いぇ"),
    ("xn", "ん"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
];

pub fn romaji_to_hiragana(input: &str) -> String {
    let input = input.to_ascii_lowercase();
    let mut rest = input.as_str();
    let mut output = String::new();
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        if let Some(after) = rest.strip_prefix("n'").or_else(|| rest.strip_prefix("n’")) {
            output.push('ん');
            rest = after;
            continue;
        }
        if rest.starts_with('n')
            && (bytes.len() == 1 || (bytes.get(1).is_some_and(|ch| !b"aiueoy".contains(ch))))
        {
            output.push('ん');
            // A completed nn stays one ん when a following consonant starts a
            // new syllable, just as at a text boundary. Reserve the second n
            // for live na/ni/nya/etc. syllables, repeated n, and explicit n'.
            let consume = if rest.strip_prefix("nn").is_some_and(|tail| {
                tail.chars().next().is_none_or(|ch| {
                    !matches!(ch, 'a' | 'i' | 'u' | 'e' | 'o' | 'n' | 'y' | '\'' | '’')
                })
            }) {
                2
            } else {
                1
            };
            rest = &rest[consume..];
            continue;
        }
        if bytes.len() >= 2
            && bytes[0].is_ascii_lowercase()
            && !b"aiueon".contains(&bytes[0])
            && (bytes[0] == bytes[1] || rest.starts_with("tch"))
        {
            output.push('っ');
            rest = &rest[1..];
            continue;
        }
        if let Some((roman, kana)) = SPECIAL
            .iter()
            .filter(|(roman, _)| rest.starts_with(roman))
            .max_by_key(|(roman, _)| roman.len())
        {
            output.push_str(kana);
            rest = &rest[roman.len()..];
            continue;
        }
        if bytes.len() >= 3
            && bytes[1] == b'y'
            && b"auo".contains(&bytes[2])
            && let Some((_, row)) = ROWS
                .iter()
                .find(|(prefix, _)| prefix.as_bytes() == &bytes[..1])
        {
            output.push(row.chars().nth(1).unwrap());
            output.push(match bytes[2] {
                b'a' => 'ゃ',
                b'u' => 'ゅ',
                _ => 'ょ',
            });
            rest = &rest[3..];
            continue;
        }
        let mut matched = false;
        for (prefix, kana) in ROWS {
            if let Some(after) = rest.strip_prefix(prefix)
                && let Some(index) = after
                    .as_bytes()
                    .first()
                    .and_then(|vowel| b"aiueo".iter().position(|ch| ch == vowel))
            {
                output.push(kana.chars().nth(index).unwrap());
                rest = &rest[prefix.len() + 1..];
                matched = true;
                break;
            }
        }
        if !matched {
            let ch = rest.chars().next().unwrap();
            output.push(match ch {
                '-' => 'ー',
                _ => ch,
            });
            rest = &rest[ch.len_utf8()..];
        }
    }
    output
}

fn katakana_char(ch: char) -> char {
    if ('ぁ'..='ゖ').contains(&ch) {
        char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
    } else {
        ch
    }
}

fn hiragana_to_katakana(input: &str) -> String {
    input.chars().map(katakana_char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_n_boundaries_aliases_and_non_roman_text_are_preserved() {
        for (input, expected) in [
            ("nn。", "ん。"),
            ("konn!", "こん!"),
            ("shuudennha", "しゅうでんは"),
            ("shuudenn", "しゅうでん"),
            ("shuudennh", "しゅうでんh"),
            ("dennwawo", "でんわを"),
            ("konnbanha", "こんばんは"),
            ("konnkaino", "こんかいの"),
            ("nn🙂", "ん🙂"),
            ("nn\u{3000}", "ん\u{3000}"),
            ("shin’you", "しんよう"),
            ("nn'ya", "んんや"),
            ("nn’ya", "んんや"),
            ("nna", "んな"),
            ("nni", "んに"),
            ("nnya", "んにゃ"),
            ("nnna", "んんな"),
            ("shinnyuu", "しんにゅう"),
            ("nnn", "んん"),
            ("nnk", "んk"),
            ("konnichiha", "こんにちは"),
            ("nya", "にゃ"),
            ("jya", "じゃ"),
            ("jyu", "じゅ"),
            ("jyo", "じょ"),
            ("jyuu", "じゅう"),
            ("jyunbi", "じゅんび"),
            ("Ωテスト", "Ωテスト"),
            ("Σ123", "Σ123"),
            ("İテスト", "İテスト"),
            ("NIHONGO", "にほんご"),
        ] {
            assert_eq!(romaji_to_hiragana(input), expected, "{input}");
        }
    }
    #[test]
    fn handles_romanization_small_kana_geminates_and_syllabic_n() {
        for (input, expected) in [
            ("konnichiha", "こんにちは"),
            ("gakkou", "がっこう"),
            ("shin'you", "しんよう"),
            ("nya", "にゃ"),
            ("kan", "かん"),
            ("nn", "ん"),
            ("kya", "きゃ"),
            ("tcha", "っちゃ"),
            ("nihongo", "にほんご"),
            ("ky", "ky"),
            ("テスト123", "テスト123"),
        ] {
            assert_eq!(romaji_to_hiragana(input), expected, "{input}");
        }
    }
    #[test]
    fn offers_kanji_kana_katakana_and_original_input() {
        let candidates = JapaneseLanguagePlugin.build_candidates(&[], "nihongo", 1.0);
        let texts: Vec<_> = candidates.iter().map(|item| item.text.as_str()).collect();
        assert_eq!(texts, ["日本語", "にほんご", "ニホンゴ", "nihongo"]);
    }
}
