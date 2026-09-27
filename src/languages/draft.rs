//! A bounded local editing window. The prefix is never normalized or sent to a
//! provider: model requests retain their independent whole-draft limit.
use super::BuiltinLanguage;

const LOCAL_TAIL_CHARS: usize = 256;

pub(crate) fn long_local_tail<'a>(language: &str, seed: &'a str) -> Option<(&'a str, &'a str)> {
    seed.chars().nth(LOCAL_TAIL_CHARS)?;
    let language = BuiltinLanguage::resolve(language)?;
    let mut boundaries = seed.char_indices().rev().take(LOCAL_TAIL_CHARS);
    let start = match language {
        // Keep the longest complete-word window, including recent collocations.
        // Never split a long URL, identifier or natural-language word midway.
        BuiltinLanguage::English => boundaries
            .filter(|(_, ch)| ch.is_whitespace())
            .map(|(index, ch)| index + ch.len_utf8())
            .last()?,
        // Only an explicit Han / CJK punctuation boundary can freeze a prefix.
        // Spaces and apostrophes may still join unconverted Pinyin syllables;
        // ASCII punctuation can be part of an identifier, path or URL.
        BuiltinLanguage::ChineseSimplified => {
            let start = boundaries
                .find(|(_, ch)| {
                    matches!(ch,
                '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' |
                '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}' |
                '。' | '，' | '！' | '？' | '；' | '：' | '、' | '「' | '」' | '『' | '』')
                })
                .map(|(index, ch)| index + ch.len_utf8())?;
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
