const HANDWRITING_DENOISE_DISTANCE: f32 = 0.9;
const HANDWRITING_RESAMPLE_DISTANCE: f32 = 3.6;
const HANDWRITING_PREPROCESS_SMOOTH_ALPHA: f32 = 0.2;
const HANDWRITING_GROUP_GAP_MIN: f32 = 12.0;
const HANDWRITING_GROUP_GAP_MAX: f32 = 42.0;
const HANDWRITING_SPEED_REFERENCE: f32 = 16.0;
const HANDWRITING_SPEED_MIN: f32 = 0.55;
const HANDWRITING_SPEED_MAX: f32 = 2.0;
const HANDWRITING_RESAMPLE_DISTANCE_MIN: f32 = 2.2;
const HANDWRITING_RESAMPLE_DISTANCE_MAX: f32 = 6.5;
const HANDWRITING_DENOISE_DISTANCE_MIN: f32 = 0.78;
const HANDWRITING_DENOISE_DISTANCE_MAX: f32 = 2.0;
const HANDWRITING_PREPROCESS_ALPHA_MIN: f32 = 0.14;
const HANDWRITING_PREPROCESS_ALPHA_MAX: f32 = 0.32;
const KAO_MOJI_MAX_LENGTH: usize = 32;
const KAO_MOJI_COMMON: &[&str] = &[
    "XD",
    "xD",
    ":D",
    ":d",
    ":P",
    ":p",
    ":3",
    ":)",
    ":-)",
    ":(",
    ";)",
    ";_;",
    "^_^",
    "^.^",
    "(^_^)",
    "(^_^;)",
    "(>_<)",
    "><",
    ">_<",
    ">.<",
    "ಠ_ಠ",
    "ಠ_ಥ",
    "◉_◉",
    "(◕‿◕)",
    "(◕ᴗ◕)",
    "(¬_¬)",
    "(╥﹏╥)",
    "(╯°□°)╯",
    "(╯°□°)╯︵",
    "(¬◡¬)",
    "(╯°o°)╯",
    "(ノ^_^)ノ",
    "¯\\_(ツ)_/¯",
    "_(⌒▽⌒)_",
    "T_T",
    "(-_-)",
    "(ง'̀-́'̀)ง",
    "^_^",
    "T.T",
    "O_O",
    "o.o",
    "(¬‿¬)",
    "(＾▽＾)",
    "(ノへ￣)",
    "(╬ಠ益ಠ)",
    "(ノಠ益ಠ)ノ",
    "(╭￣3￣)╭",
    "(￣﹏￣)",
    "(^_-)",
    "(^◡^)",
    "(◕︿◕)",
    "(^_−)",
    "(╰_╯)",
];

fn handwriting_speed_profile(
    distance: f32,
    previous_distance: f32,
) -> (f32, f32, f32) {
    let speed_hint = (distance.max(0.0) + previous_distance.max(0.0) * 0.65) * 0.5;
    let speed_ratio = (speed_hint / HANDWRITING_SPEED_REFERENCE)
        .clamp(HANDWRITING_SPEED_MIN, HANDWRITING_SPEED_MAX);

    let resample_distance = (HANDWRITING_RESAMPLE_DISTANCE / speed_ratio)
        .clamp(HANDWRITING_RESAMPLE_DISTANCE_MIN, HANDWRITING_RESAMPLE_DISTANCE_MAX);
    let denoise_distance = (HANDWRITING_DENOISE_DISTANCE / speed_ratio)
        .clamp(HANDWRITING_DENOISE_DISTANCE_MIN, HANDWRITING_DENOISE_DISTANCE_MAX);
    let smooth_alpha = (HANDWRITING_PREPROCESS_SMOOTH_ALPHA * speed_ratio.sqrt())
        .clamp(HANDWRITING_PREPROCESS_ALPHA_MIN, HANDWRITING_PREPROCESS_ALPHA_MAX);

    (denoise_distance, resample_distance, smooth_alpha)
}

fn normalize_handwriting_strokes(strokes: &[Vec<[f32; 2]>]) -> Vec<Vec<[f32; 2]>> {
    let mut normalized = Vec::new();

    for stroke in strokes {
        if stroke.is_empty() {
            continue;
        }

        let mut normalized_stroke = vec![stroke[0]];
        let mut anchor = stroke[0];
        let mut previous_distance = 0.0;

        for point in stroke.iter().copied().skip(1) {
            let dx = point[0] - anchor[0];
            let dy = point[1] - anchor[1];
            let distance = dx.hypot(dy);
            let (denoise_distance, resample_distance, smooth_alpha) =
                handwriting_speed_profile(distance, previous_distance);
            if distance < denoise_distance {
                continue;
            }

            previous_distance = distance;
            let segment_count = (distance / resample_distance).ceil() as usize;
            let segment_count = segment_count.max(1);
            for segment in 1..=segment_count {
                let t = segment as f32 / segment_count as f32;
                let sampled = [anchor[0] + dx * t, anchor[1] + dy * t];
                let candidate = if segment == segment_count {
                    sampled
                } else {
                    let [prev_x, prev_y] = *normalized_stroke.last().unwrap_or(&sampled);
                    [
                        prev_x + (sampled[0] - prev_x) * smooth_alpha,
                        prev_y + (sampled[1] - prev_y) * smooth_alpha,
                    ]
                };
                let [prev_x, prev_y] = *normalized_stroke.last().unwrap();
                if (candidate[0] - prev_x).hypot(candidate[1] - prev_y)
                    >= denoise_distance * 0.8
                {
                    normalized_stroke.push(candidate);
                }
            }
            anchor = point;
        }

        if !normalized_stroke.is_empty() {
            normalized.push(normalized_stroke);
        }
    }

    normalized
}

fn flatten_handwriting_points(strokes: &[Vec<[f32; 2]>]) -> Vec<[f32; 2]> {
    strokes
        .iter()
        .flat_map(|stroke| stroke.iter().copied())
        .collect()
}

pub fn derive_next_token_candidates(
    seed_text: &str,
    sentence_candidates: &[String],
    limit: usize,
) -> Vec<String> {
    let seed_tokens = tokenize_seed_words(seed_text);
    let seed_token_refs = seed_tokens.iter().map(String::as_str).collect::<Vec<_>>();
    let mut ranked = Vec::<(String, i32, usize)>::new();

    for (source_index, sentence) in sentence_candidates.iter().enumerate() {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        let prefix_len = matching_prefix_len_str(&seed_token_refs, &words);

        if prefix_len >= words.len() {
            continue;
        }

        let immediate_token = words
            .get(prefix_len)
            .and_then(|raw| normalize_candidate_token(raw))
            .filter(|token| !seed_tokens.iter().any(|existing| existing == token))
            .map(|token| {
                (
                    token,
                    score_next_token_candidate(source_index, true, 0),
                    prefix_len,
                )
            });

        if let Some((token, score, source_position)) = immediate_token {
            push_ranked_token(&mut ranked, token, score, source_position);
        }

        for (candidate_index, candidate) in words.iter().enumerate().skip(prefix_len + 1) {
            let Some(token) = normalize_candidate_token(candidate) else {
                continue;
            };
            if token.is_empty() || seed_tokens.iter().any(|existing| existing == &token) {
                continue;
            }
            let distance = candidate_index.saturating_sub(prefix_len + 1);
            let score = score_next_token_candidate(source_index, false, distance);
            push_ranked_token(&mut ranked, token, score, candidate_index);
        }
    }

    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut next = ranked
        .into_iter()
        .map(|(token, _, _)| token)
        .take(limit)
        .collect::<Vec<_>>();

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

fn tokenize_seed_words(seed_text: &str) -> Vec<String> {
    seed_text
        .split_whitespace()
        .map(|token| token.to_lowercase())
        .collect()
}

fn matching_prefix_len_str(seed_tokens: &[&str], words: &[&str]) -> usize {
    let mut prefix_len = 0;
    while prefix_len < seed_tokens.len()
        && prefix_len < words.len()
        && seed_tokens[prefix_len].to_lowercase() == words[prefix_len].to_lowercase()
    {
        prefix_len += 1;
    }
    prefix_len
}

fn is_kaomoji_face_like_char(ch: char) -> bool {
    matches!(
        ch,
        '^' | 'o' | 'O' | 'T' | 'x' | 'X' | 'V' | 'v' | 'w' | 'W' | 'ω' | '°' | '•'
            | '◉' | '◕' | '◔' | '◯' | '◠' | '◡' | 'ツ' | 'ಠ' | 'ಥ' | 'ʖ' | 'ᴗ' | '0'
            | '3' | '7' | '9' | '_' | '¬' | '₍' | '╯' | '╰' | '╭' | '╮' | '□' | '・' | '⊂'
    )
}

fn is_kaomoji_connector(ch: char) -> bool {
    matches!(
        ch,
        ':' | ';' | '=' | '-' | '_' | '.' | '/' | '\\' | '(' | ')' | '[' | ']' | '<' | '>' | '{'
            | '}' | 'ノ' | '◡' | '︿' | '╲' | '╱' | '┐' | '┘' | '└' | '┌'
    )
}

fn has_kaomoji_enclosure(token: &str) -> bool {
    (token.starts_with('(') && token.ends_with(')'))
        || (token.starts_with('[') && token.ends_with(']'))
        || (token.starts_with('{') && token.ends_with('}'))
        || (token.starts_with('（') && token.ends_with('）'))
        || (token.starts_with('【') && token.ends_with('】'))
        || (token.starts_with('<') && token.ends_with('>'))
        || (token.starts_with('「') && token.ends_with('」'))
}

fn looks_like_kaomoji_token(raw: &str) -> bool {
    let token = raw.trim();
    if token.is_empty()
        || token.len() > KAO_MOJI_MAX_LENGTH
        || token.chars().any(char::is_whitespace)
    {
        return false;
    }

    if KAO_MOJI_COMMON
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(token))
    {
        return true;
    }

    let mut face_like_count = 0usize;
    let mut connector_count = 0usize;
    let mut non_alpha_count = 0usize;
    let mut ascii_alpha_count = 0usize;

    for ch in token.chars() {
        if is_kaomoji_face_like_char(ch) {
            face_like_count += 1;
        }
        if is_kaomoji_connector(ch) {
            connector_count += 1;
        }
        if !ch.is_ascii_alphanumeric() {
            non_alpha_count += 1;
        }
        if ch.is_ascii_alphabetic() {
            ascii_alpha_count += 1;
        }
    }

    let has_face_like_char = face_like_count > 0;
    let has_kaomoji_bridge = connector_count > 0;
    let has_repeated_marks = face_like_count >= 2;
    let has_pairing_delimiter = has_kaomoji_enclosure(token);
    let has_table_flip_marks = token.contains('┬') || token.contains('┴') || token.contains('┌');

    let structural_match =
        has_face_like_char && (has_kaomoji_bridge || has_pairing_delimiter) && non_alpha_count > 0;
    let short_ascii_filter = structural_match && ascii_alpha_count <= 4;
    let repeated_match = has_repeated_marks && has_pairing_delimiter && token.len() <= 16;

    short_ascii_filter || has_table_flip_marks || repeated_match
}

fn normalize_candidate_token(raw: &str) -> Option<String> {
    if looks_like_kaomoji_token(raw) {
        return Some(raw.trim().to_string());
    }

    let token = raw
        .trim_matches(|ch: char| ch.is_ascii() && !ch.is_ascii_alphanumeric() && ch != '\'')
        .to_lowercase();

    if token.is_empty() { None } else { Some(token) }
}

fn score_next_token_candidate(source_index: usize, immediate: bool, offset: usize) -> i32 {
    let source_credit = (1600_i32).saturating_sub(source_index as i32 * 35);
    let immediate_bonus = if immediate { 260 } else { 130 };
    let distance_penalty = (offset as i32).saturating_mul(26);
    source_credit + immediate_bonus - distance_penalty
}

fn push_ranked_token(
    ranked: &mut Vec<(String, i32, usize)>,
    token: String,
    score: i32,
    source_position: usize,
) {
    if let Some(existing) = ranked.iter_mut().find(|entry| entry.0 == token) {
        if score > existing.1 {
            *existing = (token, score, source_position);
        }
        return;
    }
    ranked.push((token, score, source_position));
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

    let normalized = sentence.to_lowercase();
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
) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let normalized_seed = seed_text.to_lowercase();
    let normalized_sentence = sentence.to_lowercase();
    let seed_tokens = normalized_seed.split_whitespace().collect::<Vec<_>>();
    let sentence_tokens = normalized_sentence.split_whitespace().collect::<Vec<_>>();
    let token_count = normalized_sentence.split_whitespace().count();
    let ideal_length_delta = token_count.abs_diff((seed_tokens.len() + 6).clamp(6, 14));
    let continuation_penalty = seed_tokens
        .len()
        .abs_diff(matching_prefix_len_str(&seed_tokens, &sentence_tokens));
    let template_rank = if normalized_sentence.contains(" is ready.") {
        0
    } else if normalized_sentence.contains("next suggestion") {
        1
    } else if normalized_sentence.contains("complete sentence") {
        2
    } else if normalized_sentence.contains("full sentence") {
        3
    } else {
        4
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
    let seed_term_penalty = if seed_tokens.is_empty() {
        0
    } else if sentence_tokens.len() <= seed_tokens.len() {
        3
    } else {
        0
    };

    (
        template_rank,
        continuation_penalty,
        repeat_penalty,
        seed_repetition_penalty,
        ideal_length_delta,
        seed_term_penalty,
        punctuation_penalty,
        source_index,
    )
}

fn has_adjacent_repeated_word(sentence: &str) -> bool {
    let mut previous = None;
    for token in sentence
        .split_whitespace()
        .filter_map(normalize_candidate_token)
        .filter(|token| !token.is_empty())
    {
        if previous.as_deref() == Some(token.as_str()) {
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

    let normalized_seed = seed.to_lowercase();
    let normalized_candidate = candidate.to_lowercase();
    let normalized_template_candidate =
        normalized_candidate.trim_end_matches(|ch: char| matches!(ch, '.' | '!' | '?'));

    let template_prefix = |template: &str| -> Option<String> {
        if !normalized_template_candidate.ends_with(template) {
            return None;
        }
        let normalized_template = normalized_template_candidate.strip_suffix(template)?;
        let keep_chars = normalized_template.chars().count();
        let split_idx = candidate
            .char_indices()
            .nth(keep_chars)
            .map(|(index, _)| index)
            .unwrap_or(candidate.len());

        Some(candidate[..split_idx].trim_end().to_string())
    };

    let reduced_seed = trim_repeated_suffix(seed, &normalized_candidate);
    let templated = if normalized_template_candidate.ends_with("is ready as the next full sentence")
    {
        let prefix = template_prefix("is ready as the next full sentence")
            .unwrap_or_else(|| seed.to_string());
        Some(format!("{prefix} is ready."))
    } else if normalized_template_candidate.ends_with("can continue by tapping the next suggestion")
    {
        Some(format!(
            "{} can continue with the next suggestion.",
            template_prefix("can continue by tapping the next suggestion")
                .unwrap_or_else(|| reduced_seed.to_string())
        ))
    } else if normalized_template_candidate.ends_with("now expands into a complete candidate") {
        let prefix =
            template_prefix("now expands into a complete candidate").unwrap_or_else(|| seed.to_string());
        Some(format!("{prefix} now expands into a complete sentence."))
    } else {
        None
    };

    let sentence = templated.unwrap_or_else(|| candidate.to_string());
    finalize_sentence(&sentence, &normalized_seed)
}

fn trim_repeated_suffix<'a>(seed: &'a str, normalized_candidate: &str) -> &'a str {
    if normalized_candidate.contains(" can continue by tapping the next suggestion")
        && seed.to_lowercase().ends_with(" can")
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
    if !normalized_seed.is_empty() && text.to_lowercase() == normalized_seed.to_lowercase() {
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
    let normalized_strokes = normalize_handwriting_strokes(strokes);
    if normalized_strokes.is_empty() {
        return Vec::new();
    }

    if let Some(grouped) = recognize_grouped_handwriting_candidates(&normalized_strokes) {
        return grouped;
    }
    recognize_handwriting_candidates_base(&normalized_strokes)
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
            let last_group_width = (last_group_max_x - last_group_min_x).max(1.0);
            let last_group_height = (last_group_max_y - last_group_min_y).max(1.0);
            let group_gap_x = (last_group_width.max(stroke_width))
                .clamp(HANDWRITING_GROUP_GAP_MIN, HANDWRITING_GROUP_GAP_MAX);
            let group_gap_y = (last_group_height.max(stroke_height))
                .clamp(HANDWRITING_GROUP_GAP_MIN, HANDWRITING_GROUP_GAP_MAX);
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
            let same_group = horizontal_gap <= group_gap_x && vertical_gap <= group_gap_y;
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
    let points: Vec<[f32; 2]> = flatten_handwriting_points(strokes);
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
        return vec![
            "t".into(),
            "XD".into(),
            "¯\\_(ツ)_/¯".into(),
            "tap".into(),
            "text".into(),
        ];
    }

    if closure < 0.35 && path_length > (width + height) * 1.2 {
        return vec![
            "^_^".into(),
            ":)".into(),
            "(>_<)".into(),
            "o_o".into(),
            "o".into(),
            "open".into(),
            "okay".into(),
        ];
    }

    if straightness > 0.9 {
        if aspect < 0.55 {
            return vec!["T_T".into(), "-_-".into(), "ಠ_ಠ".into(), "i".into(), "line".into(), "input".into()];
        }
        if aspect > 1.8 {
            return vec![
                "^_^".into(),
                "(>_<)".into(),
                "to".into(),
                "go".into(),
                "next".into(),
            ];
        }
        if dx.abs() > dy.abs() {
            return vec![
                "(^_^)".into(),
                "(◕ᴗ◕)".into(),
                "hi".into(),
                "hello".into(),
                "hand".into(),
            ];
        }
    }

    if dx.abs() > dy.abs() && aspect > 1.25 && path_length > width * 1.4 {
        return vec![
            "XD".into(),
            "(◕‿◕)".into(),
            "wave".into(),
            "write".into(),
            "word".into(),
        ];
    }

    vec![
        "^_^".into(),
        "◉_◉".into(),
        "apple".into(),
        "hello".into(),
        "input".into(),
    ]
}

pub fn summarize_handwriting_strokes(strokes: &[Vec<[f32; 2]>]) -> String {
    let normalized = normalize_handwriting_strokes(strokes);
    let points: Vec<[f32; 2]> = flatten_handwriting_points(&normalized);
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
        derive_sentence_candidates_with_indices, normalize_candidate_token,
        matching_prefix_len_str, normalize_handwriting_strokes, finalize_sentence,
        tokenize_seed_words,
        recognize_handwriting_candidates, sentence_candidate_style_label,
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
    fn next_and_sentence_candidates_change_after_token_selection() {
        let base_candidates = vec![
            "apple can is ready as the next full sentence".into(),
            "apple can continue by tapping the next suggestion".into(),
            "apple can now expands into a complete candidate".into(),
            "apple can with this app".into(),
        ];
        let before_seed = "apple";
        let before_next = derive_next_token_candidates(before_seed, &base_candidates, 6);
        let _before_sentence = derive_sentence_candidates(before_seed, &base_candidates, 4);

        let selected = before_next
            .iter()
            .find(|token| token.as_str() == "can")
            .cloned()
            .unwrap_or_else(|| "can".to_string());
        let selected_seed = format!("{before_seed} {}", selected);
        let after_next = derive_next_token_candidates(&selected_seed, &base_candidates, 6);
        let after_sentence = derive_sentence_candidates(&selected_seed, &base_candidates, 4);

        assert!(
            !selected_seed
                .split_whitespace()
                .collect::<Vec<_>>()
                .is_empty()
                && selected_seed.split_whitespace().count() >= 2
        );
        assert!(!before_next.is_empty());
        assert!(!after_next.is_empty());
        assert!(
            !after_next.iter().any(|token| token == "can"),
            "seed extension should consume `can` into context"
        );
        assert!(
            after_sentence
                .iter()
                .any(|candidate| candidate.to_lowercase().starts_with("apple can")),
            "extended seed should surface sentence candidates"
        );
        assert_ne!(before_next.first(), after_next.first());
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
    fn next_token_derivation_prefers_immediate_continuation() {
        let tokens = derive_next_token_candidates(
            "apple",
            &[
                "apple can continue with confidence".into(),
                "apple and then fly".into(),
            ],
            6,
        );

        assert!(!tokens.is_empty());
        assert_eq!(tokens.first(), Some(&"can".to_string()));
    }

    #[test]
    fn sentence_derivation_prefers_highest_continuation_signal() {
        let sentences = derive_sentence_candidates_with_indices(
            "let us",
            &[
                "let us now can continue".into(),
                "let us is ready as the next full sentence".into(),
            ],
            2,
        );

        assert_eq!(
            sentences.first().map(|(_, sentence)| sentence.as_str()),
            Some("Let us is ready.")
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
        assert!(
            candidates.iter().any(|candidate| candidate == "^_^"),
            "loop-like strokes should include a kaomoji candidate"
        );
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
    fn handwriting_normalization_reduces_dense_jitter() {
        let normalized = normalize_handwriting_strokes(&[vec![
            [0.0, 0.0],
            [0.2, 0.1],
            [0.5, 0.0],
            [1.0, 0.1],
            [8.0, 0.2],
            [16.0, 0.0],
        ]]);

        assert_eq!(normalized.len(), 1);
        assert!(
            (2..=6).contains(&normalized[0].len()),
            "normalized stroke should stay compact while keeping the shape"
        );
        assert_eq!(normalized[0][0], [0.0, 0.0]);
        assert_eq!(normalized[0].last().copied(), Some([16.0, 0.0]));
    }

    #[test]
    fn handwriting_recognizer_groups_separated_multi_part_input() {
        let candidates = recognize_handwriting_candidates(&[
            vec![[10.0, 10.0], [10.0, 34.0]],
            vec![[48.0, 10.0], [76.0, 10.0]],
        ]);

        assert!(candidates.iter().any(|candidate| candidate.contains(' ')));
    }

    #[test]
    fn seed_tokenization_keeps_emoji() {
        assert_eq!(
            tokenize_seed_words("hello 😀 world"),
            vec!["hello".to_string(), "😀".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn seed_tokenization_keeps_multi_codepoint_emoji() {
        assert_eq!(
            tokenize_seed_words("hello 👨‍👩‍👧‍👦 world"),
            vec!["hello".to_string(), "👨‍👩‍👧‍👦".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn matching_prefix_len_respects_emoji_seed_tokens() {
        let seed_tokens = ["hello", "👨‍👩‍👧‍👦", "world"];
        let words = ["hello", "👨‍👩‍👧‍👦", "earth"];

        assert_eq!(matching_prefix_len_str(&seed_tokens, &words), 2);
    }

    #[test]
    fn next_token_candidates_keep_emoji() {
        let tokens = derive_next_token_candidates(
            "hello",
            &["hello 😀".into(), "hello world".into()],
            6,
        );

        assert!(tokens.iter().any(|token| token == "😀"));
    }

    #[test]
    fn next_token_candidates_keep_multi_codepoint_emoji() {
        let tokens = derive_next_token_candidates(
            "hello",
            &["hello 👨‍👩‍👧‍👦".into(), "hello there".into()],
            6,
        );

        assert!(tokens.iter().any(|token| token == "👨‍👩‍👧‍👦"));
    }

    #[test]
    fn candidate_token_normalization_keeps_emoji() {
        assert_eq!(normalize_candidate_token("😀").as_deref(), Some("😀"));
        assert_eq!(
            normalize_candidate_token("Hello😀").as_deref(),
            Some("hello😀")
        );
    }

    #[test]
    fn candidate_normalization_trims_ascii_punctuation_around_emoji() {
        assert_eq!(normalize_candidate_token("😀!").as_deref(), Some("😀"));
    }

    #[test]
    fn candidate_normalization_keeps_kaomoji() {
        assert_eq!(normalize_candidate_token("(^_^)").as_deref(), Some("(^_^)"));
        assert_eq!(normalize_candidate_token(":-)").as_deref(), Some(":-)"));
        assert_eq!(normalize_candidate_token(":)").as_deref(), Some(":)"));
        assert_eq!(normalize_candidate_token("ಠ_ಠ").as_deref(), Some("ಠ_ಠ"));
        assert_eq!(normalize_candidate_token("(>_<)").as_deref(), Some("(>_<)"));
        assert_eq!(normalize_candidate_token("(╯°□°)╯").as_deref(), Some("(╯°□°)╯"));
        assert_eq!(normalize_candidate_token("(╯°□°)╯︵").as_deref(), Some("(╯°□°)╯︵"));
        assert_eq!(normalize_candidate_token("ಠ_ಥ").as_deref(), Some("ಠ_ಥ"));
        assert_eq!(normalize_candidate_token("◉_◉").as_deref(), Some("◉_◉"));
        assert_eq!(normalize_candidate_token("(ノಠ_ಠ)ノ").as_deref(), Some("(ノಠ_ಠ)ノ"));
        assert_eq!(normalize_candidate_token("(╬ಠ益ಠ)").as_deref(), Some("(╬ಠ益ಠ)"));
        assert_eq!(normalize_candidate_token("¯\\_(ツ)_/¯").as_deref(), Some("¯\\_(ツ)_/¯"));
        assert_eq!(normalize_candidate_token("(ノಠ益ಠ)ノ").as_deref(), Some("(ノಠ益ಠ)ノ"));
        assert_eq!(normalize_candidate_token("(￣﹏￣)").as_deref(), Some("(￣﹏￣)"));
        assert_eq!(normalize_candidate_token("(^◡^)").as_deref(), Some("(^◡^)"));
        assert_eq!(normalize_candidate_token("(^_^)!!").as_deref(), Some("(^_^)!!"));
    }

    #[test]
    fn candidate_normalization_keeps_family_emoji_with_trailing_punctuation() {
        assert_eq!(
            normalize_candidate_token("👨‍👩‍👧‍👦,").as_deref(),
            Some("👨‍👩‍👧‍👦")
        );
    }

    #[test]
    fn finalize_sentence_keeps_multi_codepoint_emoji() {
        let sentence = finalize_sentence("hello 👨‍👩‍👧‍👦", "hello");

        assert!(sentence.contains('👨'));
        assert!(sentence.ends_with('.'));
        assert!(sentence.starts_with('H'));
    }

    #[test]
    fn emoji_seed_survives_next_and_sentence_generation() {
        let raw_candidates = vec![
            "hello there 😀 can continue by tapping the next suggestion".into(),
            "hello there 😀 is ready as the next full sentence".into(),
            "hello there 😀 with the next emoji".into(),
        ];

        let seed = "hello there 😀";
        let next_tokens = derive_next_token_candidates(seed, &raw_candidates, 6);

        assert!(
            next_tokens.iter().any(|token| token == "can"),
            "continuation token should be preserved for emoji seed"
        );
        assert!(
            next_tokens.iter().any(|token| token == "is"),
            "alternative next token should be preserved for emoji seed"
        );

        let sentences =
            derive_sentence_candidates_with_indices(seed, &raw_candidates, 4);

        assert!(
            sentences
                .iter()
                .any(|(_, sentence)| sentence == "Hello there 😀 can continue with the next suggestion."),
            "template cleaning should keep emoji while reformatting continuation candidates"
        );
        assert!(
            sentences
                .iter()
                .any(|(_, sentence)| sentence == "Hello there 😀 is ready."),
            "template cleaning should keep emoji while reformatting ready candidates"
        );
    }

    #[test]
    fn emoji_template_chain_keeps_skin_tone_and_flag() {
        let raw_candidates = vec![
            "hello world 👍🏽 is ready as the next full sentence".into(),
            "hello world 🇨🇦 can continue by tapping the next suggestion".into(),
        ];
        let seed = "hello world";

        let next_tokens = derive_next_token_candidates(seed, &raw_candidates, 6);
        assert!(next_tokens.iter().any(|token| token == "👍🏽"));
        assert!(next_tokens.iter().any(|token| token == "🇨🇦"));

        let sentences = derive_sentence_candidates_with_indices(seed, &raw_candidates, 4);
        assert!(sentences.iter().any(|(_, sentence)| sentence == "Hello world 👍🏽 is ready."));
        assert!(
            sentences
                .iter()
                .any(|(_, sentence)| sentence.contains("🇨🇦 can continue with the next suggestion"))
        );
    }

    #[test]
    fn emoji_family_chain_keeps_emoji_in_candidates_and_sentences() {
        let raw_candidates = vec![
            "hello family 👨‍👩‍👧‍👦 can continue by tapping the next suggestion".into(),
            "hello family 👨‍👩‍👧‍👦 is ready as the next full sentence".into(),
        ];
        let seed = "hello family";

        let next_tokens = derive_next_token_candidates(seed, &raw_candidates, 6);
        assert!(
            next_tokens.iter().any(|token| token == "👨‍👩‍👧‍👦"),
            "family emoji token should remain in next token candidates"
        );

        let sentences = derive_sentence_candidates_with_indices(seed, &raw_candidates, 4);
        assert!(
            sentences
                .iter()
                .any(|(_, sentence)| sentence == "Hello family 👨‍👩‍👧‍👦 is ready."),
            "ready template cleaning should keep family emoji"
        );
        assert!(
            sentences
                .iter()
                .any(|(_, sentence)| sentence.contains("👨‍👩‍👧‍👦 can continue with the next suggestion")),
            "guided template cleaning should keep family emoji"
        );
    }
}
