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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;
    use crate::ime::LanguagePlugin;

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
        let parsed = parse_http_endpoint("http://127.0.0.1").expect("parsed endpoint");
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
    fn parse_http_endpoint_rejects_empty_endpoint_string() {
        assert!(parse_http_endpoint("").is_none());
    }

    #[test]
    fn parse_http_endpoint_with_invalid_port_is_none() {
        assert!(parse_http_endpoint("http://127.0.0.1:notaport/v1/chat/completions").is_none());
    }

    #[test]
    fn parse_http_endpoint_keeps_nested_paths() {
        let parsed =
            parse_http_endpoint("http://127.0.0.1:11434/api/v1/chat/completions").expect("parsed");
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
    fn consume_json_string_treats_unknown_escape_as_character() {
        let input = r#"abc\qdef"tail"#;
        let (value, rest) = consume_json_string(input).expect("parsed");

        assert_eq!(value, "abcqdef");
        assert_eq!(rest, "tail");
    }

    #[test]
    fn consume_json_string_returns_none_without_closing_quote() {
        let input = r#"abc\nde"#;
        assert!(consume_json_string(input).is_none());
    }

    #[test]
    fn default_provider_config_exposes_expected_defaults() {
        let config = LlamaProviderConfig::default();

        assert_eq!(config.endpoint, "http://127.0.0.1:11434/v1/chat/completions");
        assert_eq!(config.model, "llama3.2:3b");
        assert_eq!(
            config.system_prompt,
            "You are a sentence-completion engine for an XR and tablet IME. Expand the user's seed into 3 short, tap-friendly English sentence candidates. Return plain text only, one candidate per line, no numbering."
        );
        assert_eq!(config.max_tokens, 96);
        assert_eq!(config.temperature_tenths, 4);
        assert_eq!(config.timeout_ms, 1200);
        assert!(config.handwriting_hint.is_none());
    }

    #[test]
    fn provider_constructor_and_accessors_are_reachable() {
        let mut config = LlamaProviderConfig::default();
        config.endpoint = "http://127.0.0.1:11434/custom/v1".to_string();

        let provider = OpenAiCompatibleLlamaProvider::new(config.clone());

        assert_eq!(provider.config(), &config);
        assert_eq!(provider.parsed_endpoint().as_ref().map(|endpoint| endpoint.path.clone()), Some("/custom/v1".to_string()));
        assert_eq!(provider.provider_id(), "llama-openai-compatible");
    }

    #[test]
    fn prompt_and_request_body_derive_from_seed_and_hint() {
        let mut config = LlamaProviderConfig::default();
        config.handwriting_hint = Some("trace summary".to_string());
        let provider = OpenAiCompatibleLlamaProvider::new(config);
        let request = LlmCompletionRequest {
            language_id: "en".to_string(),
            seed_text: "hello\nworld".to_string(),
            normalized_phrase: "hello world".to_string(),
            confidence: 0.92,
            degraded: false,
        };

        let prompt = provider.prompt_for(&request);

        assert!(prompt.contains("Seed: hello"));
        assert!(prompt.contains("Confidence: 0.92"));
        assert!(prompt.contains("Degraded: no"));
        assert!(prompt.contains("Handwriting trace summary: trace summary"));

        let body = provider.request_body(&request);
        assert!(body.contains(r#""model":"llama3.2:3b""#));
        assert!(
            body.contains(
                r#""Seed: hello\nworld\nConfidence: 0.92\nDegraded: no\nHandwriting trace summary: trace summary"#
            )
        );
    }

    #[test]
    fn plugin_factory_uses_expected_metadata() {
        let plugin = default_llama_english_plugin();

        assert_eq!(plugin.id(), "llama-en");
        assert_eq!(plugin.display_name(), "Llama English");
        let custom = llama_english_plugin_with_config(LlamaProviderConfig {
            endpoint: "http://127.0.0.1:0/v1/chat/completions".to_string(),
            model: "unit-test-model".to_string(),
            system_prompt: "system".to_string(),
            max_tokens: 10,
            temperature_tenths: 6,
            timeout_ms: 10,
            handwriting_hint: None,
        });

        assert_eq!(custom.id(), "llama-en");
        assert_eq!(custom.display_name(), "Llama English");
    }

    fn serve_local_llama_response(listener: TcpListener, expected_request_body: &'static str, response_body: String) {
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("local llama client");
            let mut raw = String::new();
            let mut buffer = [0u8; 4096];
            let mut content_length = 0usize;

            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }

                raw.push_str(&String::from_utf8_lossy(&buffer[..read]));

                if let Some((headers, body)) = raw.split_once("\r\n\r\n") {
                    if content_length == 0 {
                        content_length = headers
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("Content-Length:")
                                    .map(str::trim)
                                    .and_then(|raw| raw.parse().ok())
                            })
                            .unwrap_or(0);
                    }

                    if body.len() >= content_length {
                        break;
                    }
                }
            }

            assert!(raw.starts_with("POST "));
            assert!(raw.contains(expected_request_body));

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write local llm response");
        });
    }

    #[test]
    fn generate_reads_candidates_from_http_bridge() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
        let response_body =
            r#"{"choices":[{"message":{"role":"assistant","content":"1. one\n2. two\n3. three"}}]}"#.to_string();
        let expected_request_body = "Seed: canary";
        let address = listener.local_addr().expect("listen address");

        serve_local_llama_response(
            listener,
            expected_request_body,
            response_body.clone(),
        );

        let provider = OpenAiCompatibleLlamaProvider::new(LlamaProviderConfig {
            endpoint: format!("http://{}:{}/v1/chat/completions", address.ip(), address.port()),
            timeout_ms: 200,
            ..LlamaProviderConfig::default()
        });
        let request = LlmCompletionRequest {
            language_id: "en".to_string(),
            seed_text: "canary".to_string(),
            normalized_phrase: "canary".to_string(),
            confidence: 0.91,
            degraded: false,
        };

        let output = provider.generate(&request);

        assert_eq!(output.len(), 3);
        assert_eq!(output[0].text, "one");
        assert_eq!(output[1].text, "two");
        assert_eq!(output[2].text, "three");
        assert!((output[0].score_bias - 0.55).abs() < 0.0001);
        assert!((output[1].score_bias - 0.50).abs() < 0.0001);
    }

    #[test]
    fn generate_returns_empty_when_server_not_reachable() {
        let provider = OpenAiCompatibleLlamaProvider::new(LlamaProviderConfig {
            endpoint: "http://127.0.0.1:1/v1/chat/completions".to_string(),
            timeout_ms: 10,
            ..LlamaProviderConfig::default()
        });
        let request = LlmCompletionRequest {
            language_id: "en".to_string(),
            seed_text: "silent".to_string(),
            normalized_phrase: "silent".to_string(),
            confidence: 0.1,
            degraded: true,
        };

        assert!(provider.generate(&request).is_empty());
    }

    #[test]
    fn normalize_model_lines_trims_numbering_and_empty_lines() {
        let normalized =
            normalize_model_lines("1. first\n2. second\n- third\n4) fourth\n\n  final ");
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
            vec!["one".to_string(), "two".to_string(), "three".to_string()]
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

    #[test]
    fn parse_chat_completion_candidates_rejects_missing_content_payload() {
        let body = r#"{"choices":[{"message":{"role":"assistant"}}]}"#;
        let parsed = parse_chat_completion_candidates(body);

        assert!(parsed.is_empty());
    }

    #[test]
    fn parse_chat_completion_candidates_handles_escaped_newlines_from_llm_output() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"1. one\\n2. two\\n3. three"}}]}"#;
        let parsed = parse_chat_completion_candidates(body);

        assert_eq!(parsed, vec!["one".to_string(), "two".to_string(), "three".to_string()]);
    }
}
