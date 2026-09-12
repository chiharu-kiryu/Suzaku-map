//! Local requests never resolve DNS or use proxies. Remote requests require HTTPS
//! and separate consent; credentials, URLs and response bodies never enter errors.
use super::{MAX_RESPONSE_BYTES, ModelProviderConfig, ModelScope, http_request};
use crate::languages::llm::LlmProviderError;
use reqwest::{
    blocking::{Client, ClientBuilder},
    header::{AUTHORIZATION, HeaderValue},
};
use std::{cell::RefCell, io::Read, time::Duration};

pub(super) fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().enumerate().all(|(index, ch)| {
            ch == b'_' || ch.is_ascii_alphabetic() || (index > 0 && ch.is_ascii_digit())
        })
}

pub(super) fn is_cloud_endpoint(endpoint: &str) -> bool {
    if endpoint.len() > 2048
        || endpoint.contains(['@', '?', '#', '\\'])
        || endpoint
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return false;
    }
    reqwest::Url::parse(endpoint).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.port() != Some(0)
            && url.path() != "/"
    })
}

fn client_builder() -> ClientBuilder {
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .http1_only()
        .connect_timeout(Duration::from_secs(5))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
}

// Construct AND drop the blocking runtime on the prediction worker, not the UI.
// Reuse connections without retaining an Authorization header on the client.
thread_local! {
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
}

pub(super) fn request(
    config: &ModelProviderConfig,
    method: &str,
    body: Option<&str>,
    timeout: Duration,
) -> Result<String, LlmProviderError> {
    config.validate()?;
    if config.scope == ModelScope::Local {
        return http_request(&config.endpoint, method, body, timeout);
    }
    if !config.cloud_consent {
        return Err(LlmProviderError::CloudConsentRequired);
    }
    let authorization = authorization(config.api_key_env.as_deref(), |name| {
        std::env::var(name).ok()
    })?;
    CLIENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(client_builder().build().map_err(map_error)?);
        }
        https_request(
            slot.as_ref().unwrap(),
            &config.endpoint,
            method,
            body,
            authorization,
            timeout,
        )
    })
}

fn authorization(
    name: Option<&str>,
    read: impl FnOnce(&str) -> Option<String>,
) -> Result<Option<HeaderValue>, LlmProviderError> {
    let Some(name) = name else {
        return Ok(None);
    };
    if !valid_env_name(name) {
        return Err(LlmProviderError::MissingCredentials);
    }
    let key = read(name).ok_or(LlmProviderError::MissingCredentials)?;
    if key.is_empty() || key.len() > 8192 || key.bytes().any(|ch| !ch.is_ascii_graphic()) {
        return Err(LlmProviderError::MissingCredentials);
    }
    let mut header = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| LlmProviderError::MissingCredentials)?;
    header.set_sensitive(true);
    Ok(Some(header))
}

fn map_error(error: reqwest::Error) -> LlmProviderError {
    if error.is_timeout() {
        LlmProviderError::Timeout
    } else {
        LlmProviderError::Unavailable
    }
}

fn https_request(
    client: &Client,
    endpoint: &str,
    method: &str,
    body: Option<&str>,
    authorization: Option<HeaderValue>,
    timeout: Duration,
) -> Result<String, LlmProviderError> {
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| LlmProviderError::InvalidEndpoint)?;
    let mut request = client
        .request(method, endpoint)
        .timeout(timeout)
        .header("Accept", "application/json");
    if let Some(auth) = authorization {
        request = request.header(AUTHORIZATION, auth);
    }
    if let Some(body) = body {
        request = request
            .header("Content-Type", "application/json")
            .body(body.to_owned());
    }
    let response = request.send().map_err(map_error)?;
    if response.status().as_u16() != 200 {
        return Err(LlmProviderError::HttpStatus(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err(LlmProviderError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut
                || error
                    .get_ref()
                    .and_then(|source| source.downcast_ref::<reqwest::Error>())
                    .is_some_and(reqwest::Error::is_timeout)
            {
                LlmProviderError::Timeout
            } else {
                LlmProviderError::InvalidResponse
            }
        })?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(LlmProviderError::ResponseTooLarge);
    }
    String::from_utf8(bytes).map_err(|_| LlmProviderError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::super::test_server;
    use super::*;
    use std::{net::TcpListener, sync::Arc, thread, time::Instant};

    fn tls_server(
        handler: impl FnOnce(rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>)
        + Send
        + 'static,
    ) -> (String, reqwest::Certificate, thread::JoinHandle<()>) {
        let key = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
        let certificate = reqwest::Certificate::from_der(key.cert.der()).unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![key.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(key.key_pair.serialize_der()).into(),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "https://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = thread::spawn(move || {
            let (stream, _) = test_server::accept(&listener);
            handler(rustls::StreamOwned::new(
                rustls::ServerConnection::new(Arc::new(config)).unwrap(),
                stream,
            ));
        });
        (url, certificate, server)
    }

    #[test]
    fn https_supports_bearer_auth_unicode_and_arbitrary_model_ids() {
        let (url, cert, server) = tls_server(|mut stream| {
            let request = test_server::read_request(&mut stream).unwrap();
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer synthetic-key\r\n")
            );
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["model"], "unrelated-family/custom-model");
            assert!(
                body["messages"][1]["content"]
                    .as_str()
                    .unwrap()
                    .contains("你好")
            );
            test_server::respond(
                &mut stream,
                "200 OK",
                r#"{"choices":[{"message":{"content":"你好世界\n你好朋友"}}]}"#,
            )
            .unwrap();
        });
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            cloud_consent: true,
            endpoint: url.clone(),
            model: "unrelated-family/custom-model".into(),
            ..Default::default()
        };
        let provider = super::super::HttpModelProvider::new(config);
        let request = crate::languages::llm::LlmCompletionRequest {
            language_id: "zh-Hans".into(),
            seed_text: "你好".into(),
            normalized_phrase: "你好".into(),
            context_before_cursor: String::new(),
            confidence: 1.0,
            degraded: false,
        };
        // Only this test client trusts the ephemeral fixture certificate.
        let client = client_builder().add_root_certificate(cert).build().unwrap();
        let response = https_request(
            &client,
            &url,
            "POST",
            Some(&provider.request_body(&request)),
            authorization(Some("TEST_KEY"), |_| Some("synthetic-key".into())).unwrap(),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            super::super::parse_chat_completion_candidates(&response, None),
            ["你好世界", "你好朋友"]
        );
        server.join().unwrap();
    }

    #[test]
    fn cloud_provider_runs_all_three_language_profiles_through_verified_https() {
        use crate::languages::llm::{LlmCompletionProvider, LlmCompletionRequest};
        for (language, seed, expected) in [
            ("en", "hel", "hello"),
            ("zh-Hans", "你好", "你好世界"),
            ("ja", "日本語", "日本語入力"),
        ] {
            let (url, cert, server) = tls_server(move |mut stream| {
                let request = test_server::read_request(&mut stream).unwrap();
                let body: serde_json::Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                let input: serde_json::Value =
                    serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
                assert_eq!(input["language"], language);
                assert_eq!(body["model"], "any-model-family");
                test_server::respond(
                    &mut stream,
                    "200 OK",
                    &serde_json::json!({"choices":[{"message":{"content":expected}}]}).to_string(),
                )
                .unwrap();
            });
            CLIENT.with(|slot| {
                *slot.borrow_mut() =
                    Some(client_builder().add_root_certificate(cert).build().unwrap())
            });
            let provider = super::super::HttpModelProvider::new(ModelProviderConfig {
                scope: ModelScope::Cloud,
                cloud_consent: true,
                endpoint: url,
                model: "any-model-family".into(),
                ..Default::default()
            });
            let candidates = provider
                .generate_checked(&LlmCompletionRequest {
                    language_id: language.into(),
                    seed_text: seed.into(),
                    normalized_phrase: seed.into(),
                    context_before_cursor: String::new(),
                    confidence: 1.0,
                    degraded: false,
                })
                .unwrap();
            assert_eq!(candidates[0].text, expected);
            server.join().unwrap();
            CLIENT.with(|slot| *slot.borrow_mut() = None);
        }
    }

    #[test]
    fn https_rejects_untrusted_certificates_before_sending_input() {
        let (url, _, server) = tls_server(|mut stream| {
            assert!(test_server::read_request(&mut stream).is_err());
        });
        let client = client_builder().build().unwrap();
        assert_eq!(
            https_request(
                &client,
                &url,
                "POST",
                Some("synthetic"),
                None,
                Duration::from_secs(1)
            ),
            Err(LlmProviderError::Unavailable)
        );
        server.join().unwrap();
    }

    #[test]
    fn redirects_never_forward_credentials_or_input_to_another_service() {
        use std::io::Write;
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        let location = format!("https://{}/stolen", destination.local_addr().unwrap());
        let (url, cert, server) = tls_server(move |mut stream| {
            test_server::read_request(&mut stream).unwrap();
            write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            stream.flush().unwrap();
        });
        let client = client_builder().add_root_certificate(cert).build().unwrap();
        assert_eq!(
            https_request(
                &client,
                &url,
                "POST",
                Some("synthetic"),
                authorization(Some("TEST_KEY"), |_| Some("synthetic-key".into())).unwrap(),
                Duration::from_secs(1)
            ),
            Err(LlmProviderError::HttpStatus(307))
        );
        server.join().unwrap();
        destination.set_nonblocking(true).unwrap();
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn https_enforces_size_caps_and_a_total_body_deadline() {
        use std::io::Write;
        for (response, expected) in [
            (
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                    MAX_RESPONSE_BYTES + 1
                ),
                LlmProviderError::ResponseTooLarge,
            ),
            (
                format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
                    "x".repeat(MAX_RESPONSE_BYTES + 1)
                ),
                LlmProviderError::ResponseTooLarge,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nx".into(),
                LlmProviderError::Timeout,
            ),
            (
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".into(),
                LlmProviderError::HttpStatus(401),
            ),
        ] {
            let slow = expected == LlmProviderError::Timeout;
            let (url, cert, server) = tls_server(move |mut stream| {
                test_server::read_request(&mut stream).unwrap();
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                if slow {
                    thread::sleep(Duration::from_millis(600));
                }
            });
            let client = client_builder().add_root_certificate(cert).build().unwrap();
            let started = Instant::now();
            assert_eq!(
                https_request(
                    &client,
                    &url,
                    "POST",
                    Some("synthetic"),
                    None,
                    Duration::from_millis(300)
                ),
                Err(expected)
            );
            assert!(started.elapsed() < Duration::from_secs(1));
            server.join().unwrap();
        }
    }

    #[test]
    fn cloud_urls_require_tls_and_cannot_embed_credentials() {
        assert!(is_cloud_endpoint(
            "https://models.example/v1/chat/completions"
        ));
        for url in [
            "http://models.example/v1/chat/completions",
            "https://user:key@models.example/v1",
            "https://models.example/v1?key=secret",
            "https://models.example/v1#secret",
            "https://models.example:0/v1",
            "https://models.example",
            "https://models.example/\nsecret",
        ] {
            assert!(!is_cloud_endpoint(url), "{url}");
        }
    }

    #[test]
    fn credentials_are_explicit_sensitive_and_reject_header_injection() {
        assert_eq!(
            authorization(None, |_| panic!("must not search the environment")).unwrap(),
            None
        );
        let header = authorization(Some("TEST_MODEL_KEY"), |_| Some("synthetic-key".into()))
            .unwrap()
            .unwrap();
        assert!(header.is_sensitive());
        assert_eq!(header.to_str().unwrap(), "Bearer synthetic-key");
        for key in [
            None,
            Some(""),
            Some("secret\r\nx-extra: injected"),
            Some(" secret"),
        ] {
            assert_eq!(
                authorization(Some("TEST_MODEL_KEY"), |_| key.map(String::from)),
                Err(LlmProviderError::MissingCredentials)
            );
        }
        for name in ["", "1KEY", "KEY=secret", "KEY\n", "模型"] {
            assert!(!valid_env_name(name));
        }
    }

    #[test]
    fn consent_is_checked_before_credentials_or_network() {
        let config = ModelProviderConfig {
            scope: ModelScope::Cloud,
            endpoint: "https://unreachable.invalid/v1/chat/completions".into(),
            model: "any-model".into(),
            api_key_env: Some("SUZAKU_MISSING_TEST_KEY".into()),
            ..Default::default()
        };
        assert_eq!(
            request(
                &config,
                "POST",
                Some("synthetic"),
                Duration::from_millis(20)
            ),
            Err(LlmProviderError::CloudConsentRequired)
        );
    }
}
