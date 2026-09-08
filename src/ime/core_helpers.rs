use super::Candidate;

pub(crate) fn tokenize_seed(seed: &str) -> Vec<String> {
    seed.split_whitespace().map(ToString::to_string).collect()
}

pub(crate) fn build_combinations(
    expansions: &[Vec<String>],
    index: usize,
    current: &mut Vec<String>,
    output: &mut Vec<Vec<String>>,
    max_pool: usize,
) {
    if output.len() >= max_pool {
        return;
    }

    if index == expansions.len() {
        output.push(current.clone());
        return;
    }

    for option in &expansions[index] {
        current.push(option.clone());
        build_combinations(expansions, index + 1, current, output, max_pool);
        current.pop();

        if output.len() >= max_pool {
            return;
        }
    }
}

pub(crate) fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

pub(crate) fn build_sentence_candidates_for_variants<F>(
    parts: &[String],
    seed_text: &str,
    confidence: f32,
    variant_builder: F,
) -> Vec<Candidate>
where
    F: Fn(&str) -> Vec<String>,
{
    let phrase = parts.join(" ").replace("  ", " ").trim().to_string();
    let mut variants = variant_builder(&phrase);

    if variants.is_empty() {
        variants.push(phrase.clone());
    }

    variants
        .into_iter()
        .map(|text| {
            let word_count = text.split_whitespace().count();
            let exact_bonus = if text == seed_text { 0.20 } else { 0.0 };
            let sentence_bonus = if word_count >= 4 {
                0.22
            } else if word_count >= 2 {
                0.12
            } else {
                0.02
            };
            let continuation_bonus = if text.starts_with(&phrase) && text.len() > phrase.len() {
                0.18
            } else {
                0.0
            };
            Candidate {
                label: text.clone(),
                text,
                score: confidence + exact_bonus + sentence_bonus + continuation_bonus,
            }
        })
        .collect()
}

pub(crate) fn measure_text_prefix_width(
    text: &str,
    char_count: usize,
    pixel_size: f32,
    letter_spacing: f32,
) -> f32 {
    text.chars().take(char_count).fold(0.0, |acc, ch| {
        acc + text_char_advance(ch, pixel_size, letter_spacing)
    })
}

pub(crate) fn text_char_width(ch: char, pixel_size: f32) -> f32 {
    if unicode_width::UnicodeWidthChar::width(ch) == Some(2) {
        pixel_size * 7.0
    } else {
        pixel_size * 4.4
    }
}

pub(crate) fn text_char_advance(ch: char, pixel_size: f32, letter_spacing: f32) -> f32 {
    if ch == ' ' {
        text_space_advance(pixel_size)
    } else {
        text_glyph_advance(pixel_size, letter_spacing)
            + (text_char_width(ch, pixel_size) - pixel_size * 4.4)
    }
}

pub(crate) fn text_glyph_advance(pixel_size: f32, letter_spacing: f32) -> f32 {
    (pixel_size * 4.65 + letter_spacing).max(pixel_size * 4.05)
}

pub(crate) fn text_space_advance(pixel_size: f32) -> f32 {
    pixel_size * 2.8
}
