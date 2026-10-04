//! A bounded local editing window. The prefix is never normalized or sent to a
//! provider: model requests retain their independent whole-draft limit.
use super::{BuiltinLanguage, chinese, english};

pub(crate) const LOCAL_TAIL_CHARS: usize = 256;

pub(crate) fn long_local_tail<'a>(language: &str, seed: &'a str) -> Option<(&'a str, &'a str)> {
    seed.chars().nth(LOCAL_TAIL_CHARS)?;
    let language = BuiltinLanguage::resolve(language)?;
    // The separating scalar belongs to the frozen prefix, not the local tail.
    // Inspect it as well so a complete 256-scalar tail can use the full budget.
    let mut boundaries = seed.char_indices().rev().take(LOCAL_TAIL_CHARS + 1);
    let start = match language {
        // Keep the longest complete-word window, including recent collocations.
        // Never split a long URL, identifier or natural-language word midway.
        BuiltinLanguage::English => {
            let earliest_boundary = boundaries.last()?.0;
            english::word_boundaries_from(seed, earliest_boundary)
                .find(|(index, _)| *index >= earliest_boundary)
                .map(|(index, ch)| index + ch.len_utf8())?
        }
        // Keep an adopted authored phrase and its in-progress continuation;
        // otherwise only a Han / CJK punctuation boundary can freeze a prefix.
        // Spaces and apostrophes may still join unconverted Pinyin syllables.
        BuiltinLanguage::ChineseSimplified => {
            let start = if let Some(tail) = chinese::adopted_continuation_tail(seed)
                .filter(|tail| tail.chars().count() <= LOCAL_TAIL_CHARS)
            {
                seed.len() - tail.len()
            } else {
                boundaries
                    .find(|(_, ch)| {
                        matches!(ch,
                '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' |
                '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}' |
                '。' | '，' | '！' | '？' | '；' | '：' | '、' | '「' | '」' | '『' | '』')
                    })
                    .map(|(index, ch)| index + ch.len_utf8())?
            };
            // Han can also occur inside URLs / identifiers. Do not hide their
            // syntax to make a protected suffix look like plain Pinyin.
            let token = seed[..start]
                .rsplit(char::is_whitespace)
                .next()
                .unwrap_or("");
            if token.contains(['/', '\\', '_', '@']) {
                return None;
            }
            start
        }
        BuiltinLanguage::Japanese => return None,
    };
    let (prefix, tail) = seed.split_at(start);
    (!tail.trim().is_empty()).then_some((prefix, tail))
}
