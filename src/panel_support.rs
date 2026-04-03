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

pub fn recognize_handwriting_candidates(strokes: &[Vec<[f32; 2]>]) -> Vec<String> {
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
        derive_next_token_candidates, recognize_handwriting_candidates,
        summarize_handwriting_strokes,
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
}
