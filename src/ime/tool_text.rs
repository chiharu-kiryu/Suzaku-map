//! Shared word/phrase handoff rules for local caret insertion and native append.

fn needs_separator(left: Option<char>, right: Option<char>) -> bool {
    left.zip(right).is_some_and(|(left, right)| {
        !left.is_whitespace()
            && !right.is_whitespace()
            && !"([{（［｛「『【《〈".contains(left)
            && !",.!?;:)]}，。！？；：、）］｝」』】》〉".contains(right)
    })
}

/// Insert a recognized result without merging it into adjacent words. Indices
/// count Unicode scalar values, matching the panel caret (not UTF-8 bytes).
/// Existing text/whitespace is never removed. The returned caret follows the
/// adopted text, before any automatically added separator for the old suffix.
pub fn insert_tool_text(draft: &str, caret: usize, text: &str) -> (String, usize) {
    let caret = caret.min(draft.chars().count());
    if text.is_empty() {
        return (draft.to_owned(), caret);
    }
    let byte = draft
        .char_indices()
        .nth(caret)
        .map_or(draft.len(), |(byte, _)| byte);
    let (prefix, suffix) = draft.split_at(byte);
    let leading = needs_separator(prefix.chars().next_back(), text.chars().next());
    let trailing = needs_separator(text.chars().next_back(), suffix.chars().next());
    let mut result = String::with_capacity(draft.len() + text.len() + 2);
    result.push_str(prefix);
    if leading {
        result.push(' ');
    }
    result.push_str(text);
    if trailing {
        result.push(' ');
    }
    result.push_str(suffix);
    (result, caret + usize::from(leading) + text.chars().count())
}

#[cfg(test)]
mod tests {
    use super::insert_tool_text;

    #[test]
    fn inserts_at_the_caret_without_changing_the_old_prefix_or_suffix() {
        for (draft, caret, expected, next_caret) in [
            ("", 0, "world", 5),
            ("hello", 5, "hello world", 11),
            ("hello ", 6, "hello world", 11),
            ("hello there ", 5, "hello world there ", 11),
            ("hello there", 6, "hello world there", 11),
            (" there", 0, "world there", 5),
            ("there", 0, "world there", 5),
            ("hellothere", 5, "hello world there", 11),
            ("hello", usize::MAX, "hello world", 11),
        ] {
            assert_eq!(
                insert_tool_text(draft, caret, "world"),
                (expected.into(), next_caret),
                "draft={draft:?}, caret={caret}"
            );
        }
    }

    #[test]
    fn preserves_unicode_whitespace_on_both_sides() {
        for separator in [" ", "\t", "\n", "\u{a0}", "\u{2003}", "\u{3000}"] {
            assert_eq!(
                insert_tool_text(&format!("hello{separator}there"), 6, "world"),
                (format!("hello{separator}world there"), 11)
            );
            assert_eq!(
                insert_tool_text(&format!("hello{separator}there"), 5, "world"),
                (format!("hello world{separator}there"), 11)
            );
            assert_eq!(
                insert_tool_text(&format!("hello{separator}"), 6, "world"),
                (format!("hello{separator}world"), 11)
            );
        }
    }

    #[test]
    fn preserves_result_whitespace_and_punctuation_adjacency() {
        for (draft, caret, text, expected, next_caret) in [
            ("hello there", 5, " world ", "hello world  there", 12),
            ("hello, there", 5, "world", "hello world, there", 11),
            ("hello () there", 7, "world", "hello (world) there", 12),
            ("「」", 1, "世界", "「世界」", 3),
            ("hello there", 5, "!", "hello! there", 6),
        ] {
            assert_eq!(
                insert_tool_text(draft, caret, text),
                (expected.into(), next_caret)
            );
        }
    }

    #[test]
    fn unicode_carets_and_empty_results_are_safe() {
        assert_eq!(insert_tool_text("日😀本", 2, "é"), ("日😀 é 本".into(), 4));
        assert_eq!(insert_tool_text("hello", 2, ""), ("hello".into(), 2));
        assert_eq!(insert_tool_text("日本", usize::MAX, ""), ("日本".into(), 2));
    }
}
