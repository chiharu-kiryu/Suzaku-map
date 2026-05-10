pub fn derive_next_token_candidates(
    seed_text: &str,
    sentence_candidates: &[String],
    limit: usize,
) -> Vec<String> {
    let seed_tokens: Vec<String> = seed_text
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect();
    let mut next = Vec::new();
    let mut later = Vec::new();

    for sentence in sentence_candidates {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        let mut prefix_len = 0;
        while prefix_len < seed_tokens.len()
            && prefix_len < words.len()
            && seed_tokens[prefix_len].eq_ignore_ascii_case(words[prefix_len])
        {
            prefix_len += 1;
        }

        if prefix_len >= words.len() {
            continue;
        }

        let token = words[prefix_len]
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '\'')
            .to_ascii_lowercase();
        if !token.is_empty()
            && !seed_tokens.iter().any(|existing| existing == &token)
            && !next.iter().any(|existing| existing == &token)
        {
            next.push(token);
        }
        if next.len() >= limit {
            return next;
        }

        for candidate in words.iter().skip(prefix_len + 1) {
            let token = candidate
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '\'')
                .to_ascii_lowercase();
            if token.is_empty()
                || seed_tokens.iter().any(|existing| existing == &token)
                || next.iter().any(|existing| existing == &token)
                || later.iter().any(|existing| existing == &token)
            {
                continue;
            }
            later.push(token);
        }
    }

    for token in later {
        next.push(token);
        if next.len() >= limit {
            return next;
        }
    }

    for fallback in ["is", "can", "will", "for", "with", "next"] {
        if !seed_tokens.iter().any(|existing| existing == fallback)
            && !next.iter().any(|existing| existing == fallback)
        {
            next.push(fallback.to_string());
        }
        if next.len() >= limit {
            break;
        }
    }
    next
}

pub fn derive_sentence_candidates_with_indices(
    seed_text: &str,
    raw_candidates: &[String],
    limit: usize,
) -> Vec<(usize, String)> {
    let normalized_seed = seed_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut derived = Vec::new();

    for (index, candidate) in raw_candidates.iter().enumerate() {
        let cleaned = clean_sentence_candidate(&normalized_seed, candidate);
        if cleaned.is_empty() || derived.iter().any(|(_, existing)| existing == &cleaned) {
            continue;
        }
        derived.push((index, cleaned));
    }

    derived.sort_by(|(left_index, left), (right_index, right)| {
        sentence_candidate_rank(&normalized_seed, left, *left_index).cmp(&sentence_candidate_rank(
            &normalized_seed,
            right,
            *right_index,
        ))
    });
    derived.truncate(limit);
    derived
}

pub fn derive_sentence_candidates(
    seed_text: &str,
    raw_candidates: &[String],
    limit: usize,
) -> Vec<String> {
    derive_sentence_candidates_with_indices(seed_text, raw_candidates, limit)
        .into_iter()
        .map(|(_, sentence)| sentence)
        .collect()
}

pub fn sentence_candidate_style_label(sentence: &str, primary: bool) -> &'static str {
    if primary {
        return "Best";
    }

    let normalized = sentence.to_ascii_lowercase();
    let token_count = normalized.split_whitespace().count();
    if token_count <= 5 {
        "Short"
    } else if normalized.contains("next suggestion") {
        "Guided"
    } else if normalized.contains("complete sentence") || normalized.contains("full sentence") {
        "Expanded"
    } else if normalized.contains(" is ready.") {
        "Natural"
    } else if normalized.contains("please") || normalized.contains("could") {
        "Polite"
    } else if token_count >= 11 {
        "Detailed"
    } else {
        "Alternate"
    }
}

fn sentence_candidate_rank(
    seed_text: &str,
    sentence: &str,
    source_index: usize,
) -> (usize, usize, usize, usize, usize, usize) {
    let normalized_seed = seed_text.to_ascii_lowercase();
    let normalized_sentence = sentence.to_ascii_lowercase();
    let token_count = normalized_sentence.split_whitespace().count();
    let ideal_length_delta = token_count.abs_diff(8);
    let template_rank = if normalized_sentence.contains(" is ready.") {
        0
    } else if normalized_sentence.contains("next suggestion") {
        1
    } else if normalized_sentence.contains("complete sentence") {
        2
    } else if normalized_sentence.contains("full sentence") {
        3
    } else {
        0
    };
    let repeat_penalty = usize::from(has_adjacent_repeated_word(&normalized_sentence));
    let seed_repetition_penalty = usize::from(
        normalized_sentence
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|window| window[0] == window[1])
            || normalized_sentence.contains(&format!("{normalized_seed} {normalized_seed}")),
    );
    let punctuation_penalty =
        usize::from(!matches!(sentence.chars().last(), Some('.' | '!' | '?')));

    (
        template_rank,
        repeat_penalty,
        seed_repetition_penalty,
        ideal_length_delta,
        punctuation_penalty,
        source_index,
    )
}

fn has_adjacent_repeated_word(sentence: &str) -> bool {
    let mut previous = None;
    for token in sentence
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '\''))
        .filter(|token| !token.is_empty())
    {
        if previous == Some(token) {
            return true;
        }
        previous = Some(token);
    }
    false
}

fn clean_sentence_candidate(seed_text: &str, candidate: &str) -> String {
    let seed = seed_text.trim();
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return String::new();
    }

    let normalized_seed = seed.to_ascii_lowercase();
    let normalized_candidate = candidate.to_ascii_lowercase();
    let normalized_template_candidate =
        normalized_candidate.trim_end_matches(|ch: char| matches!(ch, '.' | '!' | '?'));

    let reduced_seed = trim_repeated_suffix(seed, &normalized_candidate);
    let templated = if normalized_template_candidate.ends_with("is ready as the next full sentence")
    {
        Some(format!("{seed} is ready."))
    } else if normalized_template_candidate.ends_with("can continue by tapping the next suggestion")
    {
        Some(format!(
            "{reduced_seed} can continue with the next suggestion."
        ))
    } else if normalized_template_candidate.ends_with("now expands into a complete candidate") {
        Some(format!("{seed} now expands into a complete sentence."))
    } else {
        None
    };

    let sentence = templated.unwrap_or_else(|| candidate.to_string());
    finalize_sentence(&sentence, &normalized_seed)
}

fn trim_repeated_suffix<'a>(seed: &'a str, normalized_candidate: &str) -> &'a str {
    if normalized_candidate.contains(" can continue by tapping the next suggestion")
        && seed.to_ascii_lowercase().ends_with(" can")
    {
        return seed
            .strip_suffix(" can")
            .or_else(|| seed.strip_suffix(" Can"))
            .unwrap_or(seed);
    }
    seed
}

fn finalize_sentence(sentence: &str, normalized_seed: &str) -> String {
    let mut text = sentence.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return text;
    }
    if !normalized_seed.is_empty() && text.to_ascii_lowercase() == normalized_seed {
        text.push('.');
    } else if !matches!(text.chars().last(), Some('.' | '!' | '?')) {
        text.push('.');
    }

    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut capitalized = String::new();
    capitalized.extend(first.to_uppercase());
    capitalized.push_str(chars.as_str());
    capitalized
}

pub fn recognize_handwriting_candidates(strokes: &[Vec<[f32; 2]>]) -> Vec<String> {
    if let Some(grouped) = recognize_grouped_handwriting_candidates(strokes) {
        return grouped;
    }
    recognize_handwriting_candidates_base(strokes)
}

fn recognize_grouped_handwriting_candidates(strokes: &[Vec<[f32; 2]>]) -> Option<Vec<String>> {
    if strokes.len() < 2 {
        return None;
    }

    let mut groups: Vec<Vec<Vec<[f32; 2]>>> = Vec::new();
    for stroke in strokes {
        let min_x = stroke.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = stroke
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = stroke.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = stroke
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let stroke_width = (max_x - min_x).max(1.0);
        let stroke_height = (max_y - min_y).max(1.0);
        if let Some(last_group) = groups.last_mut() {
            let last_group_min_x = last_group
                .iter()
                .flat_map(|existing| existing.iter().map(|p| p[0]))
                .fold(f32::INFINITY, f32::min);
            let last_group_max_x = last_group
                .iter()
                .flat_map(|existing| existing.iter().map(|p| p[0]))
                .fold(f32::NEG_INFINITY, f32::max);
            let last_group_min_y = last_group
                .iter()
                .flat_map(|existing| existing.iter().map(|p| p[1]))
                .fold(f32::INFINITY, f32::min);
            let last_group_max_y = last_group
                .iter()
                .flat_map(|existing| existing.iter().map(|p| p[1]))
                .fold(f32::NEG_INFINITY, f32::max);
            let horizontal_gap = if min_x > last_group_max_x {
                min_x - last_group_max_x
            } else if last_group_min_x > max_x {
                last_group_min_x - max_x
            } else {
                0.0
            };
            let vertical_gap = if min_y > last_group_max_y {
                min_y - last_group_max_y
            } else if last_group_min_y > max_y {
                last_group_min_y - max_y
            } else {
                0.0
            };
            let same_group =
                horizontal_gap <= stroke_width.max(18.0) && vertical_gap <= stroke_height.max(20.0);
            if same_group {
                last_group.push(stroke.clone());
                continue;
            }
        }
        groups.push(vec![stroke.clone()]);
    }

    if groups.len() < 2 || groups.len() > 3 {
        return None;
    }

    let mut seeds = Vec::new();
    for group in &groups {
        let first = recognize_handwriting_candidates_single(group);
        if first.is_empty() {
            return None;
        }
        seeds.push(first);
    }

    let combined = seeds.join(" ");
    let compact = seeds.join("");
    let joined = seeds.join("-");
    Some(vec![combined, compact, joined])
}

fn recognize_handwriting_candidates_single(strokes: &[Vec<[f32; 2]>]) -> String {
    recognize_handwriting_candidates_base(strokes)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn recognize_handwriting_candidates_base(strokes: &[Vec<[f32; 2]>]) -> Vec<String> {
    let points: Vec<[f32; 2]> = strokes
        .iter()
        .flat_map(|stroke| stroke.iter().copied())
        .collect();
    if points.len() < 2 {
        return Vec::new();
    }

    let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let aspect = width / height;
    let start = points[0];
    let end = *points.last().unwrap_or(&start);
    let end_distance = (end[0] - start[0]).hypot(end[1] - start[1]);
    let path_length: f32 = points
        .windows(2)
        .map(|window| (window[1][0] - window[0][0]).hypot(window[1][1] - window[0][1]))
        .sum();
    let straightness = end_distance / path_length.max(1.0);
    let closure = end_distance / width.max(height);
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];

    if strokes.len() >= 2 && strokes.iter().all(|stroke| stroke.len() >= 2) {
        return vec!["t".into(), "tap".into(), "text".into()];
    }

    if closure < 0.35 && path_length > (width + height) * 1.2 {
        return vec!["o".into(), "open".into(), "okay".into()];
    }

    if straightness > 0.9 {
        if aspect < 0.55 {
            return vec!["i".into(), "line".into(), "input".into()];
        }
        if aspect > 1.8 {
            return vec!["to".into(), "go".into(), "next".into()];
        }
        if dx.abs() > dy.abs() {
            return vec!["hi".into(), "hello".into(), "hand".into()];
        }
    }

    if dx.abs() > dy.abs() && aspect > 1.25 && path_length > width * 1.4 {
        return vec!["wave".into(), "write".into(), "word".into()];
    }

    vec!["apple".into(), "hello".into(), "input".into()]
}

pub fn summarize_handwriting_strokes(strokes: &[Vec<[f32; 2]>]) -> String {
    let points: Vec<[f32; 2]> = strokes
        .iter()
        .flat_map(|stroke| stroke.iter().copied())
        .collect();
    if points.len() < 2 {
        return "very short trace".to_string();
    }

    let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let width = (max_x - min_x).max(1.0);
    let height = (max_y - min_y).max(1.0);
    let aspect = width / height;
    let start = points[0];
    let end = *points.last().unwrap_or(&start);
    let closure = (end[0] - start[0]).hypot(end[1] - start[1]) / width.max(height);

    let shape = if closure < 0.35 {
        "looped"
    } else if aspect > 1.6 {
        "wide"
    } else if aspect < 0.65 {
        "tall"
    } else {
        "balanced"
    };

    format!(
        "{} handwriting trace with {} stroke(s) and {} points",
        shape,
        strokes.len(),
        points.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{
        derive_next_token_candidates, derive_sentence_candidates,
        derive_sentence_candidates_with_indices, recognize_handwriting_candidates,
        sentence_candidate_style_label, summarize_handwriting_strokes,
    };

    #[test]
    fn next_token_derivation_advances_after_selected_token() {
        let tokens = derive_next_token_candidates(
            "apple can",
            &[
                "apple can is ready as the next full sentence".into(),
                "apple can continue by tapping the next suggestion".into(),
                "apple can now expands into a complete candidate".into(),
            ],
            6,
        );

        assert!(
            tokens.starts_with(&["is".to_string(), "continue".to_string(), "now".to_string(),])
        );
        assert!(!tokens.iter().any(|token| token == "can"));
    }

    #[test]
    fn sentence_derivation_cleans_generic_engine_templates() {
        let sentences = derive_sentence_candidates(
            "apple can",
            &[
                "apple can is ready as the next full sentence".into(),
                "apple can continue by tapping the next suggestion".into(),
                "apple can now expands into a complete candidate".into(),
            ],
            4,
        );

        assert_eq!(sentences[0], "Apple can is ready.");
        assert_eq!(sentences[1], "Apple can continue with the next suggestion.");
        assert_eq!(
            sentences[2],
            "Apple can now expands into a complete sentence."
        );
    }

    #[test]
    fn sentence_derivation_prefers_more_natural_commit_candidate() {
        let sentences = derive_sentence_candidates_with_indices(
            "now can as",
            &[
                "now can as can continue by tapping the next suggestion".into(),
                "Now can as is ready as the next full sentence.".into(),
                "now can as now expands into a complete candidate".into(),
            ],
            4,
        );

        assert_eq!(
            sentences.first().map(|(_, sentence)| sentence.as_str()),
            Some("Now can as is ready.")
        );
    }

    #[test]
    fn sentence_style_labels_distinguish_primary_and_alternates() {
        assert_eq!(
            sentence_candidate_style_label("Now can as is ready.", true),
            "Best"
        );
        assert_eq!(
            sentence_candidate_style_label("Now can continue with the next suggestion.", false),
            "Guided"
        );
        assert_eq!(
            sentence_candidate_style_label(
                "This is a more detailed alternate phrasing for review in the final candidate area.",
                false
            ),
            "Detailed"
        );
        assert_eq!(sentence_candidate_style_label("Okay then.", false), "Short");
    }

    #[test]
    fn handwriting_recognizer_returns_loop_candidates() {
        let stroke = vec![
            [10.0, 10.0],
            [20.0, 8.0],
            [28.0, 16.0],
            [26.0, 28.0],
            [16.0, 32.0],
            [8.0, 22.0],
            [10.0, 10.0],
        ];

        let candidates = recognize_handwriting_candidates(&[stroke]);

        assert!(candidates.iter().any(|candidate| candidate == "o"));
    }

    #[test]
    fn handwriting_recognizer_returns_cross_candidates_for_multi_stroke_input() {
        let candidates = recognize_handwriting_candidates(&[
            vec![[10.0, 10.0], [24.0, 24.0]],
            vec![[24.0, 10.0], [10.0, 24.0]],
        ]);

        assert_eq!(candidates.first().map(String::as_str), Some("t"));
    }

    #[test]
    fn handwriting_summary_mentions_shape() {
        let summary = summarize_handwriting_strokes(&[vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0]]]);

        assert!(summary.contains("wide") || summary.contains("balanced"));
    }

    #[test]
    fn handwriting_recognizer_groups_separated_multi_part_input() {
        let candidates = recognize_handwriting_candidates(&[
            vec![[10.0, 10.0], [10.0, 34.0]],
            vec![[48.0, 10.0], [76.0, 10.0]],
        ]);

        assert!(candidates.iter().any(|candidate| candidate.contains(' ')));
    }
}
