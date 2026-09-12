//! Bounded metadata-only local discovery and explicit empty-request warmup.
use super::{
    DEFAULT_LOCAL_ENDPOINT, DEFAULT_MODEL, LlamaProviderConfig, ModelProtocol, ModelProviderConfig,
    ModelScope, http_request, parse_http_endpoint,
};
use crate::languages::llm::LlmProviderError;
use serde_json::{Value, json};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

/// Fixed loopback allowlist, not a port scan. Custom endpoints are never expanded.
pub const LOCAL_ENDPOINTS: &[&str] = &[
    DEFAULT_LOCAL_ENDPOINT,
    "http://127.0.0.1:8080/v1/chat/completions",
    "http://127.0.0.1:1234/v1/chat/completions",
];

#[derive(Clone, Debug)]
pub(super) struct CachedResolution {
    expires: Instant,
    result: Result<ModelProviderConfig, LlmProviderError>,
}

pub(super) fn resolve_cached(
    config: &ModelProviderConfig,
    cache: &Mutex<Option<CachedResolution>>,
    deadline: Instant,
) -> Result<ModelProviderConfig, LlmProviderError> {
    if config.scope == ModelScope::Cloud
        || (config.model != DEFAULT_MODEL && !config.uses_ollama_api())
    {
        return Ok(config.clone());
    }
    // This lock belongs only to the provider worker, never to the engine or UI.
    let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(cached) = cache
        .as_ref()
        .filter(|cached| Instant::now() < cached.expires)
    {
        return cached.result.clone();
    }
    let result = resolve_local(
        config,
        deadline.min(Instant::now() + Duration::from_millis(800)),
    );
    let ttl = if result.is_ok() {
        Duration::from_secs(60)
    } else {
        Duration::from_secs(5)
    };
    *cache = Some(CachedResolution {
        expires: Instant::now() + ttl,
        result: result.clone(),
    });
    result
}

pub(super) fn invalidate_failed_resolution(
    cache: &Mutex<Option<CachedResolution>>,
    resolved: &ModelProviderConfig,
    error: &LlmProviderError,
) {
    if resolved.scope != ModelScope::Local
        || !matches!(
            error,
            LlmProviderError::HttpStatus(404) | LlmProviderError::Unavailable
        )
    {
        return;
    }
    let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());
    // A late failure from a clone must not erase a newer discovery result.
    if cache
        .as_ref()
        .is_some_and(|cached| cached.result.as_ref() == Ok(resolved))
    {
        *cache = None;
    }
}

fn is_llama(model: &LocalModel) -> bool {
    model.family.to_ascii_lowercase().starts_with("llama")
        || model.name.to_ascii_lowercase().contains("llama")
}

fn endpoints(config: &ModelProviderConfig) -> Vec<&str> {
    if config.model == DEFAULT_MODEL
        && config.endpoint == DEFAULT_LOCAL_ENDPOINT
        && config.protocol == ModelProtocol::Auto
    {
        LOCAL_ENDPOINTS.to_vec()
    } else {
        vec![config.endpoint.as_str()]
    }
}

/// Only GET metadata. Never downloads weights, warms a model or calls a cloud API.
pub fn discover(config: &ModelProviderConfig) -> Result<ModelProviderConfig, LlmProviderError> {
    config.validate()?;
    resolve_local(config, Instant::now() + Duration::from_millis(800))
}

fn resolve_local(
    config: &ModelProviderConfig,
    deadline: Instant,
) -> Result<ModelProviderConfig, LlmProviderError> {
    if config.scope != ModelScope::Local {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    let mut fallback = None;
    let mut last_error = LlmProviderError::NoLocalModel;
    for endpoint in endpoints(config) {
        let candidate = ModelProviderConfig {
            endpoint: endpoint.into(),
            ..config.clone()
        };
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let models = match inventory(&candidate, remaining.min(Duration::from_millis(250))) {
            Ok(models) => models,
            Err(error) => {
                last_error = error;
                continue;
            }
        };
        last_error = LlmProviderError::NoLocalModel;
        let selected = models.iter().find(|model| {
            !model.remote
                && (config.model == DEFAULT_MODEL || model_matches(&model.name, &config.model))
        });
        if let Some(model) = selected {
            let resolved = ModelProviderConfig {
                model: model.name.clone(),
                ..candidate
            };
            if config.model != DEFAULT_MODEL || is_llama(model) {
                return Ok(resolved);
            }
            if fallback.is_none() {
                fallback = Some(resolved);
            }
        }
    }
    fallback.ok_or(if config.model == DEFAULT_MODEL {
        LlmProviderError::NoLocalModel
    } else {
        last_error
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModel {
    pub name: String,
    pub family: String,
    pub size_bytes: u64,
    pub remote: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub endpoint: String,
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
            format!("模型已加载：{} · {}", self.model, self.endpoint)
        } else {
            format!("本机模型可用：{} · {}", self.model, self.endpoint)
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
                .or_else(|| row.pointer("/meta/architecture"))
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
    models.sort_by_key(|model| (!is_llama(model), model.name.clone()));
    Ok(models)
}

fn inventory(
    config: &ModelProviderConfig,
    timeout: Duration,
) -> Result<Vec<LocalModel>, LlmProviderError> {
    config.validate()?;
    if config.scope != ModelScope::Local {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    let ollama = config.uses_ollama_api();
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
    let response = http_request(&route(config, &path)?, "GET", None, timeout)?;
    let json: Value =
        serde_json::from_str(&response).map_err(|_| LlmProviderError::InvalidResponse)?;
    parse_models(&json, ollama)
}

pub fn inspect(config: &LlamaProviderConfig) -> Result<RuntimeStatus, LlmProviderError> {
    config.validate()?;
    if config.scope != ModelScope::Local {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    let resolved;
    let config = if config.model == DEFAULT_MODEL {
        resolved = discover(config)?;
        &resolved
    } else {
        config
    };
    let timeout = Duration::from_millis(800);
    let models = inventory(config, timeout)?;
    let installed = models
        .iter()
        .any(|model| model_matches(&model.name, &config.model) && !model.remote);
    let loaded = if config.uses_ollama_api() && installed {
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
        endpoint: config.endpoint.clone(),
        model: config.model.clone(),
        installed,
        loaded,
        models,
    })
}

/// May occupy a GPU for up to five idle minutes. Must only run on an explicit user action.
pub fn warm_up(config: &LlamaProviderConfig) -> Result<(), LlmProviderError> {
    config.validate()?;
    if config.scope != ModelScope::Local {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    let status = inspect(config)?;
    let config = ModelProviderConfig {
        model: status.model,
        endpoint: status.endpoint,
        ..config.clone()
    };
    if !config.uses_ollama_api() {
        return Err(LlmProviderError::InvalidEndpoint);
    }
    if !status.installed {
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
    use super::super::test_server;
    use super::*;

    #[test]
    fn defaults_discover_only_known_loopback_services_and_custom_settings_stay_pinned() {
        let mut config = ModelProviderConfig::default();
        assert_eq!(endpoints(&config), LOCAL_ENDPOINTS);
        config.endpoint = "http://127.0.0.1:4567/v1/chat/completions".into();
        assert_eq!(endpoints(&config), [config.endpoint.as_str()]);
        config.endpoint = DEFAULT_LOCAL_ENDPOINT.into();
        config.model = "custom".into();
        assert_eq!(endpoints(&config), [DEFAULT_LOCAL_ENDPOINT]);
    }

    #[test]
    fn discovery_prefers_llama_excludes_remote_and_caches_metadata() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let config = ModelProviderConfig {
            endpoint: format!("http://{}/api/chat", listener.local_addr().unwrap()),
            ..Default::default()
        };
        let server = std::thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            let request = test_server::read_request(&mut stream).unwrap();
            assert!(request.starts_with("GET /api/tags "));
            assert!(request.ends_with("\r\n\r\n"));
            test_server::respond(&mut stream, "200 OK", r#"{"models":[{"name":"another-model"},{"name":"llama-cloud","remote_model":"remote"},{"name":"meta/Llama-local.gguf"}]}"#).unwrap();
        });
        let cache = Mutex::new(None);
        let first =
            resolve_cached(&config, &cache, Instant::now() + Duration::from_secs(1)).unwrap();
        server.join().unwrap();
        assert_eq!(first.model, "meta/Llama-local.gguf");
        assert_eq!(first.endpoint, config.endpoint);
        assert_eq!(
            resolve_cached(&config, &cache, Instant::now() + Duration::from_secs(1)).unwrap(),
            first
        );
    }

    #[test]
    fn only_missing_or_unavailable_local_models_invalidate_discovery() {
        let resolved = ModelProviderConfig {
            model: "synthetic-model".into(),
            ..Default::default()
        };
        for (error, invalidates) in [
            (LlmProviderError::HttpStatus(404), true),
            (LlmProviderError::Unavailable, true),
            (LlmProviderError::Timeout, false),
            (LlmProviderError::HttpStatus(429), false),
            (LlmProviderError::HttpStatus(401), false),
            (LlmProviderError::InvalidResponse, false),
        ] {
            let cache = Mutex::new(Some(CachedResolution {
                expires: Instant::now() + Duration::from_secs(60),
                result: Ok(resolved.clone()),
            }));
            invalidate_failed_resolution(&cache, &resolved, &error);
            assert_eq!(cache.lock().unwrap().is_none(), invalidates, "{error:?}");
        }
    }

    #[test]
    fn late_local_failures_and_cloud_failures_do_not_erase_other_resolutions() {
        let resolved = ModelProviderConfig {
            model: "synthetic-model".into(),
            ..Default::default()
        };
        let newer = ModelProviderConfig {
            model: "replacement-model".into(),
            ..resolved.clone()
        };
        let cache = Mutex::new(Some(CachedResolution {
            expires: Instant::now() + Duration::from_secs(60),
            result: Ok(newer.clone()),
        }));
        invalidate_failed_resolution(&cache, &resolved, &LlmProviderError::Unavailable);
        assert_eq!(cache.lock().unwrap().as_ref().unwrap().result, Ok(newer));

        let cloud = ModelProviderConfig {
            scope: ModelScope::Cloud,
            ..resolved
        };
        let cache = Mutex::new(Some(CachedResolution {
            expires: Instant::now() + Duration::from_secs(60),
            result: Ok(cloud.clone()),
        }));
        invalidate_failed_resolution(&cache, &cloud, &LlmProviderError::HttpStatus(404));
        assert_eq!(cache.lock().unwrap().as_ref().unwrap().result, Ok(cloud));
    }

    #[test]
    fn explicit_remote_ollama_aliases_are_never_resolved_for_local_inference() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let config = ModelProviderConfig {
            endpoint: format!("http://{}/api/chat", listener.local_addr().unwrap()),
            model: "innocent-alias".into(),
            ..Default::default()
        };
        let server = std::thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            assert!(
                test_server::read_request(&mut stream)
                    .unwrap()
                    .starts_with("GET ")
            );
            test_server::respond(
                &mut stream,
                "200 OK",
                r#"{"models":[{"name":"innocent-alias","remote_host":"https://remote.invalid"}]}"#,
            )
            .unwrap();
        });
        let cache = Mutex::new(None);
        assert_eq!(
            resolve_cached(&config, &cache, Instant::now() + Duration::from_secs(1)),
            Err(LlmProviderError::NoLocalModel)
        );
        server.join().unwrap();
        assert_eq!(
            resolve_cached(&config, &cache, Instant::now() + Duration::from_secs(1)),
            Err(LlmProviderError::NoLocalModel)
        );
    }

    #[test]
    fn compatible_auto_discovery_accepts_other_model_families() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let config = ModelProviderConfig {
            endpoint: format!(
                "http://{}/v1/chat/completions",
                listener.local_addr().unwrap()
            ),
            ..Default::default()
        };
        let server = std::thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            assert!(
                test_server::read_request(&mut stream)
                    .unwrap()
                    .starts_with("GET /v1/models ")
            );
            test_server::respond(&mut stream, "200 OK", r#"{"data":[{"id":"qwen-local"}]}"#)
                .unwrap();
        });
        assert_eq!(discover(&config).unwrap().model, "qwen-local");
        server.join().unwrap();
    }

    #[test]
    fn discovery_and_warmup_cannot_contact_cloud_even_with_consent() {
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            cloud_consent: true,
            endpoint: "https://never-contact.invalid/api/chat".into(),
            model: "anything".into(),
            ..Default::default()
        };
        assert_eq!(discover(&config), Err(LlmProviderError::InvalidEndpoint));
        assert_eq!(inspect(&config), Err(LlmProviderError::InvalidEndpoint));
        assert_eq!(warm_up(&config), Err(LlmProviderError::InvalidEndpoint));
    }
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
            model: "llama3.2:3b".into(),
            ..Default::default()
        })
        .unwrap();
        server.join().unwrap();
    }
}
