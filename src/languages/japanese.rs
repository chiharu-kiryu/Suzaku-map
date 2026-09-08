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
    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }
    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        if seed.trim().is_empty() {
            return Vec::new();
        }
        let kana = romaji_to_hiragana(seed);
        let mut values: Vec<String> = KANJI
            .iter()
            .filter(|(reading, _)| *reading == kana)
            .map(|(_, text)| text.to_string())
            .collect();
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
