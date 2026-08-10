use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

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
    "（╯°□°）╯︵┻━┻",
    "(╯°□°)╯︵┻━┻",
    "(╯°╰°)╯︵┻━┻",
    "ლ(ಠ益ಠ)ლ",
    "ᕕ(ಠ‿ಠ)ᕗ",
    "ᕕ(ᐛ)ᕗ",
    "ʘ_ʘ",
    "(ᵔᴗᵔ)",
];
const NEXT_TOKEN_KAOMOJI_PRIORITY_BONUS: i32 = 240;
const NEXT_TOKEN_EMOJI_PRIORITY_BONUS: i32 = 280;
const NEXT_TOKEN_CONTEXT_HINT_BONUS: i32 = 360;
const MAX_CONTEXT_HINTS_PER_SEED: usize = 4;
const MAX_CONTEXT_HINT_WORDS: usize = 3;
const CONTEXT_HINT_PUNCTUATION: &str = "'\"!?.,;:-)()][？！；：、，。]";
const NEXT_TOKEN_OPEN_SOURCE_MODEL_ENDPOINT_ENV: &str = "IME_NEXT_TOKEN_MODEL_ENDPOINT";
const NEXT_TOKEN_OPEN_SOURCE_MODEL_NAME_ENV: &str = "IME_NEXT_TOKEN_MODEL_NAME";
const NEXT_TOKEN_OPEN_SOURCE_MODEL_TIMEOUT_ENV: &str = "IME_NEXT_TOKEN_MODEL_TIMEOUT_MS";
const NEXT_TOKEN_OPEN_SOURCE_MODEL_DEFAULT_NAME: &str = "llama3.2:3b";
const NEXT_TOKEN_OPEN_SOURCE_MODEL_MAX_TOKENS: u32 = 24;
const NEXT_TOKEN_OPEN_SOURCE_MODEL_TIMEOUT_MS: u64 = 800;
const NEXT_TOKEN_OPEN_SOURCE_MODEL_TEMPERATURE_TENTHS: u32 = 3;
const NEXT_TOKEN_OPEN_SOURCE_MODEL_BONUS: i32 = 560;
const NEXT_TOKEN_CONTEXT_HINTS: &[(&str, &[&str])] = &[
    (
        "thanks",
        &["🙏", "😊", "👍", ":)", "<3"],
    ),
    (
        "thank you",
        &["🙏", "😊", "😄", ":D"],
    ),
    (
        "sorry",
        &["🙇", "😅", "😢", ":)"],
    ),
    (
        "happy",
        &["😄", "🎉", ":-)", "😊"],
    ),
    (
        "love",
        &["❤️", "🥹", "😍", ":')"],
    ),
    (
        "congrats",
        &["🎉", "👏", "👏🏽", "😀"],
    ),
    (
        "good job",
        &["👏", "🎉", "👍", "😊"],
    ),
    (
        "great job",
        &["🎉", "👏", "🙌", "😄"],
    ),
    (
        "happy birthday",
        &["🎉", "🎂", "🎊", "😊"],
    ),
    (
        "you are welcome",
        &["😊", "🙂", "🙌", "😄"],
    ),
    (
        "good morning",
        &["🌞", "☀️", "👋", "😀"],
    ),
    (
        "good night",
        &["🌙", "😴", "💤", "🛌"],
    ),
    (
        "see you",
        &["🙂", "👍", "🙌", "😁"],
    ),
    (
        "what's up",
        &["🙂", "😄", "😉", "😌"],
    ),
    (
        "how are you",
        &["🙂", "😊", "😄", "🙌"],
    ),
    (
        "well done",
        &["👏", "🎉", "🙌", "✅"],
    ),
    (
        "no problem",
        &["🙂", "👌", "😊", "👍"],
    ),
    (
        "nice to",
        &["😊", "😄", "🙌", "😌"],
    ),
    (
        "good to",
        &["😊", "🙌", "😁", "😄"],
    ),
    (
        "good luck",
        &["🍀", "✨", "🙌", "🤞"],
    ),
    (
        "all the best",
        &["🍀", "✨", "💪", "🙌"],
    ),
    (
        "see you later",
        &["🙂", "👋", "😄", "👍"],
    ),
    (
        "get well",
        &["🌱", "😌", "🙌", "💪"],
    ),
    (
        "you know",
        &["🙂", "😅", "🤔", "🙄"],
    ),
    (
        "i mean",
        &["🤔", "😅", "🙂", "😌"],
    ),
    (
        "what if",
        &["🤔", "😄", "🤩", "🙈"],
    ),
    (
        "asap",
        &["⏱️", "🙂", "📌", "💬"],
    ),
    (
        "for real",
        &["😄", "🙂", "🎯", "🙌"],
    ),
    (
        "thank you 吧",
        &["🙏", "😄", "🙂", "👍"],
    ),
    (
        "thanks 吧",
        &["🙂", "😄", "🙏", "👍"],
    ),
    (
        "no problem 吧",
        &["🙂", "👌", "🙌", "😄"],
    ),
    (
        "good job 吧",
        &["👏", "🙌", "😄", "👍"],
    ),
    (
        "谢谢",
        &["🙏", "🙂", "😊", "😄"],
    ),
    (
        "不客气",
        &["🙂", "🙌", "😊", "👍"],
    ),
    (
        "加油",
        &["💪", "🔥", "👍", "🙌"],
    ),
    (
        "辛苦",
        &["🙏", "❤️", "🙂", "😌"],
    ),
    (
        "对吧",
        &["🙂", "😄", "😊", "👍"],
    ),
    (
        "可以吧",
        &["🙂", "😄", "👍", "🙌"],
    ),
    (
        "行吧",
        &["🙂", "😄", "👍", "🙌"],
    ),
    (
        "好的",
        &["🙂", "👌", "👍", "😊"],
    ),
    (
        "真的吗",
        &["😮", "🤔", "😄", "🙄"],
    ),
    (
        "哈哈",
        &["😄", "😂", "😁", "😌"],
    ),
    (
        "没事的",
        &["🙂", "👍", "🙌", "😄"],
    ),
    (
        "辛苦了",
        &["🙏", "❤️", "💪", "🙌"],
    ),
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
    let model_limit = limit.min(4);
    for token in next_token_model_candidates(seed_text, &seed_tokens, model_limit) {
        let score = score_next_token_candidate(0, true, 0) + NEXT_TOKEN_OPEN_SOURCE_MODEL_BONUS;
        push_ranked_token(&mut ranked, token, score, 0);
    }

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
                let score =
                    score_next_token_candidate(source_index, true, 0) + next_token_expression_bonus(&token);
                (
                    token,
                    score,
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
            let score =
                score_next_token_candidate(source_index, false, distance)
                    + next_token_expression_bonus(&token);
            push_ranked_token(&mut ranked, token, score, candidate_index);
        }
    }

    for hint in contextual_expression_hints(&seed_tokens) {
        let score = score_next_token_candidate(0, true, 0) + NEXT_TOKEN_CONTEXT_HINT_BONUS;
        push_ranked_token(&mut ranked, hint, score, 0);
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

fn next_token_expression_bonus(token: &str) -> i32 {
    if looks_like_kaomoji_token(token) {
        NEXT_TOKEN_KAOMOJI_PRIORITY_BONUS
    } else if looks_like_emoji_token(token) {
        NEXT_TOKEN_EMOJI_PRIORITY_BONUS
    } else {
        0
    }
}

fn contextual_expression_hints(seed_tokens: &[String]) -> Vec<String> {
    let mut hints = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if seed_tokens.is_empty() {
        return hints;
    };

    let normalized_seed_tokens = seed_tokens
        .iter()
        .map(|seed_token| {
            seed_token
                .trim_matches(|ch: char| CONTEXT_HINT_PUNCTUATION.contains(ch))
                .to_lowercase()
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if normalized_seed_tokens.is_empty() {
        return hints;
    }
    let normalized_seed_refs: Vec<&str> = normalized_seed_tokens.iter().map(String::as_str).collect();
    let seed_len = normalized_seed_tokens.len();

    for (trigger, candidates) in NEXT_TOKEN_CONTEXT_HINTS.iter() {
        let trigger_words = trigger.split_whitespace().collect::<Vec<&str>>();
        if trigger_words.is_empty() || trigger_words.len() > MAX_CONTEXT_HINT_WORDS {
            continue;
        }
        if trigger_words.len() > seed_len {
            continue;
        }

        let candidate_start = seed_len - trigger_words.len();
        let matched = trigger_words
            .iter()
            .zip(normalized_seed_refs[candidate_start..].iter())
            .all(|(trigger_word, seed_word)| trigger_word == seed_word);

        if !matched {
            continue;
        }

        for candidate in candidates.iter() {
            if seen.len() >= MAX_CONTEXT_HINTS_PER_SEED {
                break;
            }
            if seen.insert((*candidate).to_string()) {
                hints.push((*candidate).to_string());
            }
        }
    }

    hints
}

fn next_token_model_candidates(
    seed_text: &str,
    seed_tokens: &[String],
    limit: usize,
) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let endpoint = match std::env::var(NEXT_TOKEN_OPEN_SOURCE_MODEL_ENDPOINT_ENV) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Vec::new(),
    };
    let model = std::env::var(NEXT_TOKEN_OPEN_SOURCE_MODEL_NAME_ENV)
        .unwrap_or_else(|_| NEXT_TOKEN_OPEN_SOURCE_MODEL_DEFAULT_NAME.to_string());

    let timeout = std::env::var(NEXT_TOKEN_OPEN_SOURCE_MODEL_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(NEXT_TOKEN_OPEN_SOURCE_MODEL_TIMEOUT_MS);

    let Some(endpoint_parsed) = parse_http_endpoint(&endpoint) else {
        return Vec::new();
    };
    if !is_loopback_host(&endpoint_parsed.host) {
        return Vec::new();
    }

    let request = build_next_token_model_request(
        &endpoint,
        &model,
        &endpoint_parsed.path,
        seed_text,
        limit,
        timeout,
);

    let address = format!("{}:{}", endpoint_parsed.host, endpoint_parsed.port);
    let mut stream = match TcpStream::connect(address) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let timeout = Some(Duration::from_millis(timeout));
    let _ = stream.set_read_timeout(timeout);
    let _ = stream.set_write_timeout(timeout);
    let _ = stream.write_all(request.as_bytes());
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);

    let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
    parse_next_token_model_candidates(body, seed_tokens, limit)
}

fn build_next_token_model_request(
    endpoint: &str,
    model: &str,
    path: &str,
    seed_text: &str,
    limit: usize,
    timeout_ms: u64,
) -> String {
    let body = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"system\",\"content\":\"{}\"}},{{\"role\":\"user\",\"content\":\"{}\"}}],\"temperature\":{},\"max_tokens\":{},\"stream\":false}}",
        escape_json_string(model),
        escape_json_string(&next_token_system_prompt()),
        escape_json_string(&next_token_model_prompt(seed_text, limit)),
        NEXT_TOKEN_OPEN_SOURCE_MODEL_TEMPERATURE_TENTHS as f32 / 10.0,
        NEXT_TOKEN_OPEN_SOURCE_MODEL_MAX_TOKENS
);
    let endpoint_host = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))
        .unwrap_or(endpoint);
    let _ = timeout_ms;
    format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        path,
        endpoint_host,
        body.len(),
        body
    )
}

fn next_token_system_prompt() -> &'static str {
    "You are a next-token suggestion engine for an IME. Reply with token-level predictions only."
}

fn next_token_model_prompt(seed_text: &str, limit: usize) -> String {
    format!(
        "Seed: {seed_text}\nReturn {limit} concise next-token suggestions, one token per line, no numbering, no punctuation."
    )
}

fn parse_next_token_model_candidates(
    body: &str,
    seed_tokens: &[String],
    limit: usize,
) -> Vec<String> {
    if limit == 0 || body.trim().is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut search = body;
    let mut seen = std::collections::HashSet::new();
    while let Some(start_index) = search.find("\"content\":\"") {
        let start = start_index + "\"content\":\"".len();
        let Some((raw_content, rest)) = consume_json_string(&search[start..]) else {
            break;
        };
        for line in normalize_model_content_lines(&raw_content) {
            if let Some(token) = model_line_to_token(seed_tokens, &line) {
                if seen.insert(token.clone()) {
                    candidates.push(token);
                    if candidates.len() >= limit {
                        return candidates;
                    }
                }
            }
        }
        search = rest;
    }
    candidates
}

fn normalize_model_content_lines(content: &str) -> Vec<String> {
    let normalized = content
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t");
    normalized
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '.' || ch == '-')
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn model_line_to_token(seed_tokens: &[String], raw_line: &str) -> Option<String> {
    let normalized_seed_tokens = seed_tokens.iter().collect::<std::collections::HashSet<_>>();
    let line = raw_line
        .trim()
        .trim_matches(|ch: char| ch == '\"' || ch == '\'' || ch == '`');
    if line.is_empty() {
        return None;
    }

    let candidate = line.split_whitespace().next()?;
    let normalized = normalize_candidate_token(candidate)?;
    if normalized_seed_tokens.contains(&normalized) {
        return None;
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_http_endpoint(endpoint: &str) -> Option<ParsedHttpEndpoint> {
    let without_scheme = endpoint.strip_prefix("http://")?;
    let (host_port, path) = if let Some((head, tail)) = without_scheme.split_once('/') {
        (head, format!("/{tail}"))
    } else {
        (without_scheme, "/v1/chat/completions".to_string())
    };
    let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
        (host.to_string(), port.parse().ok()?)
    } else {
        (host_port.to_string(), 80)
    };

    Some(ParsedHttpEndpoint { host, port, path })
}

fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" {
        return true;
    }
    host.parse::<std::net::IpAddr>().map_or(false, |ip| ip.is_loopback())
}

fn consume_json_string(input: &str) -> Option<(String, &str)> {
    let input = input.strip_prefix('"').unwrap_or(input);
    let mut output = String::new();
    let mut chars = input.char_indices();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\\' => {
                let (_, escaped) = chars.next()?;
                match escaped {
                    '\\' => output.push('\\'),
                    '"' => output.push('"'),
                    'n' => output.push('\n'),
                    'r' => output.push('\r'),
                    't' => output.push('\t'),
                    other => output.push(other),
                }
            }
            '"' => return Some((output, &input[index + 1..])),
            other => output.push(other),
        }
    }
    None
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[derive(Debug, Clone)]
struct ParsedHttpEndpoint {
    host: String,
    port: u16,
    path: String,
}

fn looks_like_emoji_token(raw: &str) -> bool {
    raw.chars().any(|ch| {
        let codepoint = ch as u32;
        matches!(
            codepoint,
            0x1F300..=0x1F9FF
                | 0x1FA70..=0x1FAFF
                | 0x2600..=0x27BF
                | 0x1F190..=0x1F251
                | 0x1F1E6..=0x1F1FF
        )
            || ch == '❤'
    })
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
        | '3'
        | '7'
        | '9'
        | '_'
        | '‿'
        | '¬'
        | 'ᕕ'
        | 'ᕗ'
        | 'ᐛ'
        | 'ʘ'
        | '◴'
        | '◷'
        | '◶'
        | '◵'
        | '┌'
        | '┐'
        | '└'
        | '┘'
        | '₍'
        | '╯'
        | '╰'
        | '╭'
        | '╮'
        | '□'
        | '・'
        | '⊂'
        | 'ヽ'
        | 'ノ'
        | '￣'
    )
}

fn is_kaomoji_connector(ch: char) -> bool {
    matches!(
        ch,
        ':' | ';' | '=' | '-' | '_' | '.' | '/' | '\\' | '(' | ')' | '[' | ']' | '<' | '>' | '{'
            | '}' | 'ノ' | '◡' | '︿' | '╲' | '╱' | '┐' | '┘' | '└' | '┌' | '◍' | '◉'
            | 'ᕕ' | 'ᕗ' | '╭' | '╮' | '╰' | '╯' | '┻' | '┳' | '━' | '╯' | '╰'
    )
}

fn looks_like_ascii_emoticon_token(raw: &str) -> bool {
    let token = raw.trim();

    if !token.is_ascii()
        || token.len() < 2
        || token.len() > KAO_MOJI_MAX_LENGTH
        || token.chars().any(char::is_whitespace)
        || token.contains("://")
    {
        return false;
    }

    if let Some(head) = token.chars().next() {
        if (head == 'x' || head == 'X') && token.len() <= 5 {
            return token
                .chars()
                .skip(1)
                .all(|ch| ch == 'd' || ch == 'D');
        }

        if !matches!(head, ':' | ';' | '<' | '>') {
            return false;
        }

        let mut tail = token.chars().skip(1).peekable();
        while let Some(&ch) = tail.peek() {
            if matches!(ch, '-' | '^' | '~' | '\'') {
                tail.next();
            } else {
                break;
            }
        }

        let mut has_face_char = false;
        let mut has_side_char = false;
        let mut saw_only_slashes = true;
        let mut body_len = 0usize;

        for ch in tail {
            if !matches!(
                ch,
                ':' | ')' | '(' | 'D' | 'd' | 'P' | 'p' | 'O' | 'o' | '3' | '/' | '\\' | '|'
                    | '*'
                    | '_' | '.' | '<' | '>' | '[' | ']' | '¬'
                    | '\''
            ) {
                return false;
            }

            if matches!(ch, '/' | '\\' | '|' | '*' | '<' | '>' | '[' | ']' ) {
                has_side_char = true;
            }
            if matches!(ch, ')' | '(' | 'D' | 'd' | 'P' | 'p' | 'O' | 'o' | '3' | '0') {
                has_face_char = true;
            }
            if ch != '/' {
                saw_only_slashes = false;
            }
            body_len += 1;
        }

        if body_len == 0 || body_len > 8 {
            return false;
        }
        if body_len > 1 && saw_only_slashes {
            return false;
        }
        if !has_face_char && !has_side_char {
            return false;
        }

        return true;
    }

    false
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

fn has_table_flip_mark(token: &str) -> bool {
    token.contains('┬')
        || token.contains('┴')
        || token.contains('┻')
        || token.contains('┳')
        || token.contains('┐')
        || token.contains('┘')
        || token.contains('└')
        || token.contains('┌')
        || token.contains('︵')
        || token.contains('︶')
        || token.contains('┗')
        || token.contains('┛')
        || token.contains('┏')
        || token.contains('┓')
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

    if looks_like_ascii_emoticon_token(token) {
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
    let has_table_flip_marks = has_table_flip_mark(token);

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
        matching_prefix_len_str, normalize_handwriting_strokes, normalize_model_content_lines,
        parse_http_endpoint, parse_next_token_model_candidates, consume_json_string,
        finalize_sentence, tokenize_seed_words,
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
    fn next_token_candidates_keep_ascii_emoticon() {
        let tokens = derive_next_token_candidates(
            "hey",
            &["hey :-) then continue".into(), "hey there".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == ":-)"));
        assert!(tokens.iter().any(|token| token == "then"));

        let tokens = derive_next_token_candidates(
            "wow",
            &["wow >:) so far".into(), "wow there".into()],
            8,
        );
        assert!(tokens.iter().any(|token| token == ">:)"));

        let tokens = derive_next_token_candidates(
            "smile",
            &["smile :'-) here".into(), "smile there".into()],
            8,
        );
        assert!(tokens.iter().any(|token| token == ":'-)"));
        let tokens = derive_next_token_candidates(
            "chat",
            &["chat >.< now".into(), "chat later".into()],
            8,
        );
        assert!(tokens.iter().any(|token| token == ">.<"));
    }

    #[test]
    fn next_token_candidates_prefers_expression_by_ranking() {
        let tokens = derive_next_token_candidates(
            "i",
            &["i can".into(), "i 😀".into(), "i :-D".into()],
            6,
        );

        assert_eq!(tokens.first(), Some(&"😀".to_string()));
        assert!(tokens.iter().any(|token| token == ":-D"));
        assert!(tokens.iter().any(|token| token == "can"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints() {
        let tokens = derive_next_token_candidates(
            "thank you",
            &["thank you".into(), "thank you is ready as the next full sentence".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙏"));
        assert!(tokens.iter().any(|token| token == "😊"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_three_word_seed() {
        let tokens = derive_next_token_candidates(
            "you are welcome",
            &["you are welcome".into(), "you are welcome always".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "😊"));
        assert!(tokens.iter().any(|token| token == "😄"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_two_word_seed() {
        let tokens = derive_next_token_candidates(
            "see you",
            &["see you".into(), "see you later".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "👍"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_three_word_seed_with_punctuated_tokens() {
        let tokens = derive_next_token_candidates(
            "how are you",
            &["how are you".into(), "how are you doing".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "😊"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_three_word_seed_good_luck() {
        let tokens = derive_next_token_candidates(
            "good luck",
            &["good luck".into(), "good luck my friend".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🍀"));
        assert!(tokens.iter().any(|token| token == "✨"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_three_word_seed_all_the_best() {
        let tokens = derive_next_token_candidates(
            "all the best",
            &["all the best".into(), "all the best to you".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🍀"));
        assert!(tokens.iter().any(|token| token == "💪"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_three_word_seed_get_well() {
        let tokens = derive_next_token_candidates(
            "get well",
            &["get well".into(), "get well soon".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🌱"));
        assert!(tokens.iter().any(|token| token == "😌"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_punctuated_seed_tokens() {
        let tokens = derive_next_token_candidates(
            "thank you!",
            &["thank you".into(), "thank you very much".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙏"));
        assert!(tokens.iter().any(|token| token == "😊"));
    }

    #[test]
    fn next_token_candidates_adds_tail_phrase_with_filler_word() {
        let tokens = derive_next_token_candidates(
            "you know",
            &["you know".into(), "you know what".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "😅"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_seed() {
        let tokens = derive_next_token_candidates(
            "谢谢！",
            &["谢谢".into(), "谢谢你".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙏"));
        assert!(tokens.iter().any(|token| token == "🙂"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_phrase_with_particle() {
        let tokens = derive_next_token_candidates(
            "可以吧",
            &["可以吧".into(), "可以吧".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "😄"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_encouragement_phrase() {
        let tokens = derive_next_token_candidates(
            "加油",
            &["加油".into(), "加油 继续".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "💪"));
        assert!(tokens.iter().any(|token| token == "🔥"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_friendly_endings() {
        let tokens = derive_next_token_candidates(
            "好的",
            &["好的".into(), "好的 我知道".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "👌"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_filler_particles() {
        let tokens = derive_next_token_candidates(
            "行吧",
            &["行吧".into(), "行吧 那就".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙂"));
        assert!(tokens.iter().any(|token| token == "😄"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_laugh() {
        let tokens = derive_next_token_candidates(
            "哈哈",
            &["哈哈".into(), "哈哈 太好了".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "😄"));
        assert!(tokens.iter().any(|token| token == "😂"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_chinese_seed_with_punctuation() {
        let tokens = derive_next_token_candidates(
            "谢谢，",
            &["谢谢".into(), "谢谢 你".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙏"));
        assert!(tokens.iter().any(|token| token == "🙂"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_mixed_chinese_english_seed() {
        let tokens = derive_next_token_candidates(
            "thank you 吧",
            &["thank you".into(), "thank you 吧".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "🙏"));
        assert!(tokens.iter().any(|token| token == "😄"));
    }

    #[test]
    fn next_token_candidates_adds_contextual_expression_hints_for_mixed_english_chinese_tail() {
        let tokens = derive_next_token_candidates(
            "good job 吧",
            &["good job".into(), "good job 吧".into()],
            8,
        );

        assert!(tokens.iter().any(|token| token == "👏"));
        assert!(tokens.iter().any(|token| token == "🙌"));
    }

    #[test]
    fn candidate_normalization_rejects_url_like_input_as_emoticon() {
        assert_eq!(normalize_candidate_token("http://").as_deref(), Some("http"));
    }

    #[test]
    fn candidate_normalization_treats_non_emoticon_colon_prefix_as_word() {
        assert_eq!(normalize_candidate_token(":alpha").as_deref(), Some("alpha"));
        assert_eq!(normalize_candidate_token("xD1").as_deref(), Some("xd1"));
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
        assert_eq!(normalize_candidate_token(":-))").as_deref(), Some(":-))"));
        assert_eq!(normalize_candidate_token(";-P").as_deref(), Some(";-P"));
        assert_eq!(normalize_candidate_token(":))").as_deref(), Some(":))"));
        assert_eq!(normalize_candidate_token(":-]").as_deref(), Some(":-]"));
        assert_eq!(normalize_candidate_token(";]").as_deref(), Some(";]"));
        assert_eq!(normalize_candidate_token(":0").as_deref(), Some(":0"));
        assert_eq!(normalize_candidate_token(":/").as_deref(), Some(":/"));
        assert_eq!(normalize_candidate_token("xD").as_deref(), Some("xD"));
        assert_eq!(normalize_candidate_token("xDD").as_deref(), Some("xDD"));
        assert_eq!(normalize_candidate_token(":'-)").as_deref(), Some(":'-)"));
        assert_eq!(normalize_candidate_token(">:(").as_deref(), Some(">:("));
        assert_eq!(normalize_candidate_token("<3").as_deref(), Some("<3"));
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
        assert_eq!(
            normalize_candidate_token("(╯°□°)╯︵┻━┻").as_deref(),
            Some("(╯°□°)╯︵┻━┻")
        );
        assert_eq!(
            normalize_candidate_token("ლ(ಠ益ಠ)ლ").as_deref(),
            Some("ლ(ಠ益ಠ)ლ")
        );
        assert_eq!(normalize_candidate_token("ᕕ(ಠ‿ಠ)ᕗ").as_deref(), Some("ᕕ(ಠ‿ಠ)ᕗ"));
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

    #[test]
    fn parse_http_endpoint_handles_default_path_and_ports() {
        let parsed = parse_http_endpoint("http://127.0.0.1:11434").expect("endpoint should parse");

        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 11434);
        assert_eq!(parsed.path, "/v1/chat/completions");
    }

    #[test]
    fn parse_http_endpoint_rejects_non_http_scheme() {
        assert!(parse_http_endpoint("https://127.0.0.1:11434/v1/chat/completions").is_none());
    }

    #[test]
    fn normalize_model_content_lines_strips_numbered_and_bulleted_lines() {
        let normalized = normalize_model_content_lines("1. hello\n- can\n2. :-)\n");

        assert_eq!(normalized, vec!["hello", "can", ":-)"]);
    }

    #[test]
    fn parse_next_token_model_candidates_extracts_and_filters_tokens() {
        let body = r#"{
            "choices":[{"message":{"role":"assistant","content":" hello\\n:-)\\n1. world\\nfamily 👨‍👩‍👧‍👦\\n"} }]}}
        "#;
        let seed_tokens = vec!["hello".to_string()];
        let candidates = parse_next_token_model_candidates(body, &seed_tokens, 6);

        assert_eq!(
            candidates,
            vec![
                ":-)".to_string(),
                "world".to_string(),
                "family".to_string()
            ]
        );
    }

    #[test]
    fn consume_json_string_unescapes_common_sequences() {
        let source = r#""line1\nline2\"x\t" trailing"#;
        let (decoded, rest) = consume_json_string(source)
            .expect("json string should decode");

        assert_eq!(decoded, "line1\nline2\"x\t");
        assert_eq!(rest, " trailing");
    }
}
