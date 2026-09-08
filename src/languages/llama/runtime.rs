//! Explicit runtime diagnostics and empty-request warmup. Never reads or submits typed text.
use super::{
    LlamaProviderConfig, OpenAiCompatibleLlamaProvider, http_request, parse_http_endpoint,
};
use crate::languages::llm::LlmProviderError;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModel {
    pub name: String,
    pub family: String,
    pub size_bytes: u64,
    pub remote: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub model: String,
    pub installed: bool,
    /// None when the runtime does not expose residency, never a claim that it is loaded.
    pub loaded: Option<bool>,
    pub models: Vec<LocalModel>,
}

impl RuntimeStatus {
    pub fn summary(&self) -> String {
        if !self.installed {
            format!("未找到本机模型：{}", self.model)
        } else if self.loaded == Some(true) {
            format!("Llama 模型已加载：{}", self.model)
        } else {
            format!("模型已安装，可预热：{}", self.model)
        }
    }
}

fn route(config: &LlamaProviderConfig, path: &str) -> Result<String, LlmProviderError> {
    let endpoint =
        parse_http_endpoint(&config.endpoint).ok_or(LlmProviderError::InvalidEndpoint)?;
    Ok(format!("http://{}:{}{path}", endpoint.host, endpoint.port))
}

fn model_matches(installed: &str, requested: &str) -> bool {
    installed == requested
        || (!requested.contains(':') && installed == format!("{requested}:latest"))
}

fn parse_models(value: &Value, ollama: bool) -> Result<Vec<LocalModel>, LlmProviderError> {
    let rows = value[if ollama { "models" } else { "data" }]
        .as_array()
        .ok_or(LlmProviderError::InvalidResponse)?;
    let mut models = Vec::new();
    for row in rows.iter().take(256) {
        let Some(name) = row[if ollama { "name" } else { "id" }].as_str() else {
            continue;
        };
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            continue;
        }
        models.push(LocalModel {
            name: name.to_string(),
            family: row
                .pointer("/details/family")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(64)
                .collect(),
            size_bytes: row["size"].as_u64().unwrap_or_default(),
            remote: row
                .get("remote_model")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
                || row
                    .get("remote_host")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty())
                || name.ends_with(":cloud")
                || name.ends_with("-cloud"),
        });
    }
    models.sort_by_key(|model| {
        (
            !model.family.starts_with("llama") && !model.name.starts_with("llama"),
            model.name.clone(),
        )
    });
    Ok(models)
}

pub fn inspect(config: &LlamaProviderConfig) -> Result<RuntimeStatus, LlmProviderError> {
    let ollama = OpenAiCompatibleLlamaProvider::new(config.clone()).uses_ollama_api();
    let endpoint =
        parse_http_endpoint(&config.endpoint).ok_or(LlmProviderError::InvalidEndpoint)?;
    let path = if ollama {
        "/api/tags".to_string()
    } else {
        format!(
            "{}/models",
            endpoint
                .path
                .strip_suffix("/chat/completions")
                .ok_or(LlmProviderError::InvalidEndpoint)?
        )
    };
    let timeout = Duration::from_millis(800);
    let response = http_request(&route(config, &path)?, "GET", None, timeout)?;
    let json: Value =
        serde_json::from_str(&response).map_err(|_| LlmProviderError::InvalidResponse)?;
    let models = parse_models(&json, ollama)?;
    let installed = models
        .iter()
        .any(|model| model_matches(&model.name, &config.model) && !model.remote);
    let loaded = if ollama && installed {
        http_request(&route(config, "/api/ps")?, "GET", None, timeout)
            .ok()
            .and_then(|response| serde_json::from_str::<Value>(&response).ok())
            .and_then(|json| parse_models(&json, true).ok())
            .map(|models| {
                models
                    .iter()
                    .any(|model| model_matches(&model.name, &config.model) && !model.remote)
            })
    } else {
        None
    };
    Ok(RuntimeStatus {
        model: config.model.clone(),
        installed,
        loaded,
        models,
    })
}

/// May occupy a GPU for up to five idle minutes. Must only run on an explicit user action.
pub fn warm_up(config: &LlamaProviderConfig) -> Result<(), LlmProviderError> {
    if !OpenAiCompatibleLlamaProvider::new(config.clone()).uses_ollama_api() {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    if !inspect(config)?.installed {
        return Err(LlmProviderError::HttpStatus(404));
    }
    let body = json!({"model":config.model, "messages":[], "stream":false,
        "keep_alive":"5m", "options":{"num_ctx":2048}})
    .to_string();
    let response = http_request(
        &config.endpoint,
        "POST",
        Some(&body),
        Duration::from_secs(30),
    )?;
    let value: Value =
        serde_json::from_str(&response).map_err(|_| LlmProviderError::InvalidResponse)?;
    if value["done"] != true {
        return Err(LlmProviderError::InvalidResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_prioritize_llama_and_do_not_treat_cloud_models_as_local() {
        let models = parse_models(&json!({"models":[
            {"name":"another:3b"}, {"name":"llama3.2:3b", "size":123,"details":{"family":"llama"}},
            {"name":"llama-cloud", "remote_model":"llama", "remote_host":"https://ollama.com"},
            {"name":"bad\nname"}
        ]}), true).unwrap();
        assert_eq!(models.len(), 3);
        assert!(models[0].name.starts_with("llama"));
        assert!(
            models
                .iter()
                .find(|model| model.name == "llama-cloud")
                .unwrap()
                .remote
        );
        assert_eq!(
            models
                .iter()
                .find(|model| model.name == "llama3.2:3b")
                .unwrap()
                .size_bytes,
            123
        );
    }
    #[test]
    fn model_names_require_an_exact_tag_except_for_implicit_latest() {
        assert!(model_matches("llama3.2:3b", "llama3.2:3b"));
        assert!(model_matches("llama3.2:latest", "llama3.2"));
        assert!(!model_matches("llama3.2:1b", "llama3.2:3b"));
        assert!(!model_matches("llama3.2:3b", "llama3.2"));
    }
    #[test]
    fn compatible_model_inventory_is_parsed_without_guessing_residency() {
        let models = parse_models(&json!({"data":[{"id":"llama-local"}]}), false).unwrap();
        assert_eq!(models[0].name, "llama-local");
        assert!(parse_models(&json!({"error":"not ready"}), false).is_err());
    }

    #[test]
    fn warmup_checks_local_inventory_then_sends_only_an_empty_request() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            for expected in ["GET /api/tags ", "GET /api/ps ", "POST /api/chat "] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(split) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..split]).unwrap();
                        let length: usize = header
                            .lines()
                            .find_map(|line| line.strip_prefix("Content-Length: "))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if bytes.len() >= split + 4 + length {
                            assert!(header.starts_with(expected), "{header}");
                            if expected.starts_with("POST") {
                                let body: Value =
                                    serde_json::from_slice(&bytes[split + 4..]).unwrap();
                                assert_eq!(body["messages"], json!([]));
                                assert_eq!(body["options"]["num_ctx"], 2048);
                                assert!(body.get("prompt").is_none());
                            }
                            break;
                        }
                    }
                }
                let response = if expected.starts_with("POST") {
                    json!({"done":true})
                } else if expected.contains("/tags") {
                    json!({"models":[{"name":"llama3.2:3b"}]})
                } else {
                    json!({"models":[]})
                }
                .to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{response}",
                    response.len()
                )
                .unwrap();
            }
        });
        warm_up(&LlamaProviderConfig {
            endpoint: format!("http://{address}/api/chat"),
            ..Default::default()
        })
        .unwrap();
        server.join().unwrap();
    }
}
