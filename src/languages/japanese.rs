//! Deterministic Romaji/Kana baseline; Kanji vocabulary is intentionally a bootstrap set.

use super::ranked_candidates;
use crate::ime::{Candidate, LanguagePlugin};

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
        let mut values: Vec<String> = KANJI
            .iter()
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

const KANJI: &[(&str, &str)] = &[
    ("にほん", "日本"),
    ("にほんご", "日本語"),
    ("とうきょう", "東京"),
    ("わたし", "私"),
    ("きょう", "今日"),
    ("あした", "明日"),
    ("きのう", "昨日"),
    ("ありがとう", "ありがとう"),
    ("おはよう", "おはよう"),
    ("こんにちは", "こんにちは"),
    ("こんばんは", "こんばんは"),
    ("せかい", "世界"),
    ("にゅうりょく", "入力"),
    ("へんかん", "変換"),
    ("こうほ", "候補"),
    ("げんご", "言語"),
    ("べんきょう", "勉強"),
    ("がっこう", "学校"),
    ("しごと", "仕事"),
    ("てんき", "天気"),
    ("じかん", "時間"),
    ("なまえ", "名前"),
    ("すき", "好き"),
    ("かんじ", "漢字"),
    ("かんじ", "感じ"),
    ("すずめ", "雀"),
];

pub(crate) fn is_dictionary_word(text: &str) -> bool {
    KANJI.iter().any(|(reading, word)| {
        *word == text || *reading == text || hiragana_to_katakana(reading) == text
    })
}

/// Greedy dictionary segments plus untouched particles. This is a bounded
/// bootstrap conversion, not a morphological analyzer or a full Japanese IME.
fn convert_segments(kana: &str) -> String {
    let mut rest = kana;
    let mut output = String::new();
    while !rest.is_empty() {
        if let Some((reading, word)) = KANJI
            .iter()
            .rev() // Keep the dictionary's first variant when readings have equal lengths.
            .filter(|(reading, _)| rest.starts_with(reading))
            .max_by_key(|(reading, _)| reading.len())
        {
            output.push_str(word);
            rest = &rest[reading.len()..];
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    output
}

fn composition_kana(seed: &str) -> String {
    // Shift+Space separates romaji words, not words in the converted Japanese text.
    seed.split_whitespace().map(romaji_to_hiragana).collect()
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
    if seed.ends_with(|ch: char| {
        ch.is_ascii_alphabetic() && !"aiueo".contains(ch.to_ascii_lowercase())
    }) {
        queries.extend(
            ['a', 'i', 'u', 'e', 'o']
                .into_iter()
                .map(|vowel| composition_kana(&format!("{seed}{vowel}"))),
        );
    }
    let mut offset = 0;
    let mut prefix = String::new();
    let mut output = Vec::new();
    while offset < kana.len() {
        let rest = &kana[offset..];
        for (reading, word) in KANJI {
            if *reading != rest
                && queries.iter().any(|query| {
                    query
                        .strip_prefix(&kana[..offset])
                        .is_some_and(|tail| !tail.is_empty() && reading.starts_with(tail))
                })
            {
                let text = format!("{prefix}{word}");
                if !output.iter().any(|(value, _)| value == &text) {
                    output.push((text, CandidateKind::Word));
                    if output.len() == 4 {
                        return output;
                    }
                }
            }
        }
        if let Some((reading, word)) = KANJI
            .iter()
            .rev()
            .filter(|(reading, _)| rest.starts_with(reading))
            .max_by_key(|(reading, _)| reading.len())
        {
            prefix.push_str(word);
            offset += reading.len();
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

#[cfg(test)]
mod completion_tests {
    use super::*;
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
    const CONTINUATIONS: &[(&str, &[&str])] = &[
        (
            "日本語",
            &["日本語を勉強しています。", "日本語で入力できます。"],
        ),
        ("日本語入力", &["日本語入力を試しています。"]),
        (
            "私は日本語",
            &[
                "私は日本語を勉強しています。",
                "私は日本語で入力しています。",
            ],
        ),
        (
            "日本語を勉強",
            &["日本語を勉強しています。", "日本語を勉強したいです。"],
        ),
        (
            "私",
            &["私は日本語を勉強しています。", "私はそう思います。"],
        ),
        ("今日", &["今日はいい天気ですね。", "今日は何をしますか？"]),
        (
            "明日",
            &["明日また会いましょう。", "明日よろしくお願いします。"],
        ),
        (
            "ありがとう",
            &["ありがとうございます。", "ありがとう、助かりました。"],
        ),
        (
            "こんにちは",
            &[
                "こんにちは、お元気ですか？",
                "こんにちは、よろしくお願いします。",
            ],
        ),
        (
            "おはよう",
            &[
                "おはようございます。",
                "おはよう、今日もよろしくお願いします。",
            ],
        ),
        (
            "入力",
            &["入力方法を変更できます。", "入力を確認してください。"],
        ),
        ("勉強", &["勉強を続けたいです。", "勉強になりました。"]),
        (
            "仕事",
            &["仕事が終わりました。", "仕事について相談したいです。"],
        ),
        ("天気", &["天気がいいですね。", "天気はどうですか？"]),
    ];
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
        if let Some((_, values)) = CONTINUATIONS.iter().find(|(prefix, _)| *prefix == word) {
            for text in *values {
                if !output.iter().any(|(existing, _)| existing == text) {
                    output.push(((*text).into(), CandidateKind::Sentence));
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
    let input = input.to_lowercase();
    let mut rest = input.as_str();
    let mut output = String::new();
    while !rest.is_empty() {
        let bytes = rest.as_bytes();
        if rest.starts_with("n'") {
            output.push('ん');
            rest = &rest[2..];
            continue;
        }
        if rest.starts_with('n')
            && (bytes.len() == 1 || (bytes.get(1).is_some_and(|ch| !b"aiueoy".contains(ch))))
        {
            output.push('ん');
            let consume = if rest == "nn" { 2 } else { 1 };
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

fn hiragana_to_katakana(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ('ぁ'..='ゖ').contains(&ch) {
                char::from_u32(ch as u32 + 0x60).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
