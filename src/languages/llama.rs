use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use crate::languages::english::english_sentence_variants;
use crate::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmLanguagePlugin,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlamaProviderConfig {
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: u32,
    pub temperature_tenths: u32,
    pub timeout_ms: u64,
    pub handwriting_hint: Option<String>,
}

impl Default for LlamaProviderConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:11434/v1/chat/completions".to_string(),
            model: "llama3.2:3b".to_string(),
            system_prompt: "You are a sentence-completion engine for an XR and tablet IME. Expand the user's seed into 3 short, tap-friendly English sentence candidates. Return plain text only, one candidate per line, no numbering.".to_string(),
            max_tokens: 96,
            temperature_tenths: 4,
            timeout_ms: 1200,
            handwriting_hint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHttpEndpoint {
    host: String,
    port: u16,
    path: String,
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleLlamaProvider {
    config: LlamaProviderConfig,
}

impl OpenAiCompatibleLlamaProvider {
    pub fn new(config: LlamaProviderConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &LlamaProviderConfig {
        &self.config
    }

    fn parsed_endpoint(&self) -> Option<ParsedHttpEndpoint> {
        parse_http_endpoint(&self.config.endpoint)
    }

    fn prompt_for(&self, request: &LlmCompletionRequest) -> String {
        format!(
            "Seed: {}\nConfidence: {:.2}\nDegraded: {}\n{}\nReturn three concise continuations, each on its own line.",
            request.seed_text,
            request.confidence,
            if request.degraded { "yes" } else { "no" },
            self.config
                .handwriting_hint
                .as_deref()
                .map(|hint| format!("Handwriting trace summary: {hint}"))
                .unwrap_or_default()
        )
    }

    fn request_body(&self, request: &LlmCompletionRequest) -> String {
        format!(
            "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"system\",\"content\":\"{}\"}},{{\"role\":\"user\",\"content\":\"{}\"}}],\"temperature\":{},\"max_tokens\":{},\"stream\":false}}",
            escape_json_string(&self.config.model),
            escape_json_string(&self.config.system_prompt),
            escape_json_string(&self.prompt_for(request)),
            self.config.temperature_tenths as f32 / 10.0,
            self.config.max_tokens,
        )
    }

    fn send_request(&self, request: &LlmCompletionRequest) -> Option<String> {
        let endpoint = self.parsed_endpoint()?;
        let body = self.request_body(request);
        let address = format!("{}:{}", endpoint.host, endpoint.port);
        let mut stream = TcpStream::connect(address).ok()?;
        let timeout = Some(Duration::from_millis(self.config.timeout_ms));
        let _ = stream.set_read_timeout(timeout);
        let _ = stream.set_write_timeout(timeout);
        let http_request = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            endpoint.path,
            endpoint.host,
            body.len(),
            body
        );
        stream.write_all(http_request.as_bytes()).ok()?;
        let mut response = String::new();
        stream.read_to_string(&mut response).ok()?;
        response.split("\r\n\r\n").nth(1).map(str::to_string)
    }
}

impl LlmCompletionProvider for OpenAiCompatibleLlamaProvider {
    fn provider_id(&self) -> &str {
        "llama-openai-compatible"
    }

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        let Some(body) = self.send_request(request) else {
            return Vec::new();
        };
        parse_chat_completion_candidates(&body)
            .into_iter()
            .enumerate()
            .map(|(index, text)| LlmCompletion {
                text,
                score_bias: 0.55 - index as f32 * 0.05,
            })
            .collect()
    }
}

pub fn llama_english_plugin_with_config(config: LlamaProviderConfig) -> LlmLanguagePlugin {
    let provider = OpenAiCompatibleLlamaProvider::new(config);
    LlmLanguagePlugin::new(
        "llama-en",
        "Llama English",
        Arc::new(provider),
        english_sentence_variants,
    )
}

pub fn default_llama_english_plugin() -> LlmLanguagePlugin {
    llama_english_plugin_with_config(LlamaProviderConfig::default())
}

fn parse_http_endpoint(endpoint: &str) -> Option<ParsedHttpEndpoint> {
    let without_scheme = endpoint.strip_prefix("http://")?;
    let (host_port, path) = if let Some((host_port, path)) = without_scheme.split_once('/') {
        (host_port, format!("/{}", path))
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

fn parse_chat_completion_candidates(body: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut search = body;
    while let Some(index) = search.find("\"content\":\"") {
        let start = index + "\"content\":\"".len();
        let Some((raw, rest)) = consume_json_string(&search[start..]) else {
            break;
        };
        for line in normalize_model_lines(&raw) {
            if !candidates.iter().any(|existing| existing == &line) {
                candidates.push(line);
            }
        }
        search = rest;
    }
    candidates
}

fn normalize_model_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '.' || ch == '-')
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .take(3)
        .collect()
}

fn consume_json_string(input: &str) -> Option<(String, &str)> {
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

fn escape_json_string(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_compatible_chat_content_into_candidates() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"1. apple is ready to commit\n2. apple keeps the next phrase ready\n3. apple can continue by tapping"}}]}"#;

        let parsed = parse_chat_completion_candidates(body);

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "apple is ready to commit");
    }

    #[test]
    fn parses_http_endpoint_with_custom_port() {
        let parsed = parse_http_endpoint("http://127.0.0.1:11434/v1/chat/completions")
            .expect("parsed endpoint");

        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 11434);
        assert_eq!(parsed.path, "/v1/chat/completions");
    }

    #[test]
    fn parse_http_endpoint_defaults_path_and_port() {
        let parsed = parse_http_endpoint("http://127.0.0.1")
            .expect("parsed endpoint");
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 80);
        assert_eq!(parsed.path, "/v1/chat/completions");
    }

    #[test]
    fn parse_http_endpoint_rejects_missing_scheme() {
        assert!(parse_http_endpoint("127.0.0.1:11434").is_none());
        assert!(parse_http_endpoint("https://127.0.0.1:11434").is_none());
    }

    #[test]
    fn parse_http_endpoint_with_invalid_port_is_none() {
        assert!(parse_http_endpoint("http://127.0.0.1:notaport/v1/chat/completions").is_none());
    }

    #[test]
    fn parse_http_endpoint_keeps_nested_paths() {
        let parsed = parse_http_endpoint("http://127.0.0.1:11434/api/v1/chat/completions").expect("parsed");
        assert_eq!(parsed.path, "/api/v1/chat/completions");
    }

    #[test]
    fn escapes_json_control_chars() {
        let escaped = escape_json_string("a\\b\"c\nd\re\tf");
        assert_eq!(escaped, "a\\\\b\\\"c\\nd\\re\\tf");
    }

    #[test]
    fn consume_json_string_reads_escapes_and_reports_rest() {
        let input = r#"foo\\bar\"baz\nline\tone\rend"tail"#;
        let (value, rest) = consume_json_string(input).expect("parsed");
        assert_eq!(value, "foo\\bar\"baz\nline\tone\rend");
        assert_eq!(rest, "tail");
    }

    #[test]
    fn consume_json_string_returns_none_on_truncated_escape() {
        let input = r#"broken\"#;
        assert!(consume_json_string(input).is_none());
    }

    #[test]
    fn consume_json_string_returns_none_without_closing_quote() {
        let input = r#"abc\nde"#;
        assert!(consume_json_string(input).is_none());
    }

    #[test]
    fn normalize_model_lines_trims_numbering_and_empty_lines() {
        let normalized = normalize_model_lines("1. first\n2. second\n- third\n4) fourth\n\n  final ");
        assert_eq!(
            normalized,
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
            ]
        );
    }

    #[test]
    fn normalize_model_lines_limits_to_three_lines() {
        let normalized = normalize_model_lines("1. one\n2. two\n3. three\n4. four\n5. five\n");
        assert_eq!(
            normalized,
            vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string()
            ]
        );
    }

    #[test]
    fn parse_chat_completion_candidates_deduplicates_and_filters_blank_lines() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"1. alpha\n\n2. alpha\n3. bravo"}},{"message":{"role":"assistant","content":"1. bravo\n2. charlie"}}]}"#;
        let parsed = parse_chat_completion_candidates(body);
        assert_eq!(
            parsed,
            vec![
                "alpha".to_string(),
                "bravo".to_string(),
                "charlie".to_string(),
            ]
        );
    }

    #[test]
    fn parse_chat_completion_candidates_ignores_malformed_content_fields() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"alpha"}},{"message":{"role":"assistant","content":"unterminated"#;
        let parsed = parse_chat_completion_candidates(body);
        assert_eq!(parsed, vec!["alpha".to_string()]);
    }
}
