//! Synthetic sockets only: observe peer closure, not merely discarded results.
use super::*;
use crate::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};
use std::{net::TcpListener, sync::mpsc, thread};

fn peer_closed(stream: &mut TcpStream) -> bool {
    // Closing with unread response bytes may reset rather than half-close TCP.
    match stream.read(&mut [0]) {
        Ok(0) => true,
        Err(error) => matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
        ),
        _ => false,
    }
}

#[test]
fn newest_draft_does_not_queue_behind_an_obsolete_local_generation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let (entered, started) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut old, _) = test_server::accept(&listener);
        let wire = test_server::read_request(&mut old).unwrap();
        assert!(wire.contains("old"));
        old.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        entered.send(()).unwrap();
        let closed = peer_closed(&mut old);
        drop(old);
        let (mut latest, _) = test_server::accept(&listener);
        let wire = test_server::read_request(&mut latest).unwrap();
        let body: Value = serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
        let input: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(input["raw_composition"], "hel");
        test_server::respond(&mut latest, "200 OK", &json!({"choices":[{"finish_reason":"stop","message":{"content":"helmet from the model"}}]}).to_string()).unwrap();
        closed
    });
    let provider = HttpModelProvider::new(ModelProviderConfig {
        endpoint,
        model: "synthetic-model".into(),
        timeout_ms: 2000,
        ..Default::default()
    });
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.configure_prediction(Some(Arc::new(provider)));
    engine.seed("old");
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    let changed = Instant::now();
    for seed in ["h", "he", "hel"] {
        engine.seed(seed);
    }
    while engine.prediction_pending() && changed.elapsed() < Duration::from_secs(3) {
        engine.poll_prediction();
        thread::sleep(Duration::from_millis(2));
    }
    let elapsed = changed.elapsed();
    let closed = server.join().unwrap();
    assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
    assert!(
        engine
            .candidates()
            .iter()
            .any(|item| item.text == "helmet from the model")
    );
    assert!(
        closed,
        "old request stayed connected; latest draft waited {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(600),
        "latest draft waited {elapsed:?}"
    );
    eprintln!("cancelled old socket; newest draft ready in {elapsed:?}");
}

fn local_config(listener: &TcpListener) -> ModelProviderConfig {
    ModelProviderConfig {
        endpoint: format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        ),
        model: "synthetic-model".into(),
        timeout_ms: 2000,
        ..Default::default()
    }
}

fn request() -> LlmCompletionRequest {
    LlmCompletionRequest {
        language_id: "en".into(),
        seed_text: "hel".into(),
        normalized_phrase: "hel".into(),
        context_before_cursor: String::new(),
        confidence: 1.0,
        degraded: false,
    }
}

#[test]
fn input_boundaries_close_local_requests_without_waiting_or_publishing_cancellation_errors() {
    for boundary in [
        "empty",
        "cancel",
        "commit",
        "focus",
        "selection",
        "disable",
        "reload",
        "drop",
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let provider = HttpModelProvider::new(local_config(&listener));
        let (entered, started) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            test_server::read_request(&mut stream).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(700)))
                .unwrap();
            entered.send(()).unwrap();
            assert!(
                peer_closed(&mut stream),
                "{boundary}: old socket not closed"
            );
        });
        let mut engine = XRTabletImeEngine::new(EngineConfig::default());
        engine.configure_prediction(Some(Arc::new(provider)));
        engine.seed("hel");
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut engine = Some(engine);
        let started = Instant::now();
        match boundary {
            "empty" => {
                engine.as_mut().unwrap().seed("");
            }
            "cancel" => engine.as_mut().unwrap().cancel_prediction(),
            "commit" => {
                engine
                    .as_mut()
                    .unwrap()
                    .commit(CommitOptions { force: true });
            }
            "focus" => engine.as_mut().unwrap().clear_session_context(),
            "selection" => {
                engine.as_mut().unwrap().move_selection(1);
            }
            "disable" => engine.as_mut().unwrap().configure_prediction(None),
            "reload" => engine
                .as_mut()
                .unwrap()
                .configure_prediction(Some(Arc::new(FixedProvider))),
            "drop" => drop(engine.take()),
            _ => unreachable!(),
        }
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "{boundary}: foreground waited"
        );
        server.join().unwrap();
        if let Some(mut engine) = engine {
            if boundary == "reload" {
                while engine.prediction_pending() && started.elapsed() < Duration::from_secs(1) {
                    engine.poll_prediction();
                    thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(engine.prediction_status(), PredictionStatus::Ready);
            } else {
                let candidates = engine.candidates().to_vec();
                assert!(!engine.poll_prediction());
                assert_eq!(engine.candidates(), candidates);
                assert!(!engine.prediction_pending());
            }
            assert_eq!(engine.prediction_error(), None, "{boundary}");
        }
    }
}

struct FixedProvider;
impl LlmCompletionProvider for FixedProvider {
    fn provider_id(&self) -> &str {
        "synthetic-reloaded"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        vec![LlmCompletion {
            text: "helmet from reloaded provider".into(),
            ..Default::default()
        }]
    }
}

#[test]
fn cancellation_guards_legacy_providers_before_and_after_generation() {
    struct LegacyProvider(LlmCancellation);
    impl LlmCompletionProvider for LegacyProvider {
        fn provider_id(&self) -> &str {
            "synthetic-legacy"
        }
        fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
            assert!(
                !self.0.is_cancelled(),
                "pre-cancelled provider must not be entered"
            );
            self.0.cancel();
            FixedProvider.generate(&request())
        }
    }
    let token = LlmCancellation::default();
    let provider = LegacyProvider(token.clone());
    assert_eq!(
        provider.generate_cancellable(&request(), &token),
        Err(LlmProviderError::Cancelled)
    );
    assert!(token.is_cancelled());
    assert_eq!(
        provider.generate_cancellable(&request(), &token),
        Err(LlmProviderError::Cancelled)
    );
    let fresh = LlmCancellation::default();
    assert!(!fresh.is_cancelled());
    assert!(
        FixedProvider
            .generate_cancellable(&request(), &fresh)
            .is_ok()
    );
}

#[test]
fn pre_cancelled_local_and_cloud_requests_never_open_a_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let local = local_config(&listener);
    let cloud = ModelProviderConfig {
        scope: ModelScope::Cloud,
        endpoint: local.endpoint.replacen("http:", "https:", 1),
        cloud_consent: true,
        ..local.clone()
    };
    let token = LlmCancellation::default();
    token.cancel();
    for config in [local, cloud] {
        assert_eq!(
            HttpModelProvider::new(config).generate_cancellable(&request(), &token),
            Err(LlmProviderError::Cancelled)
        );
    }
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn quiet_poll_intervals_do_not_end_a_valid_fragmented_response_early() {
    for chunked in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = local_config(&listener).endpoint;
        let server = thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            test_server::read_request(&mut stream).unwrap();
            thread::sleep(Duration::from_millis(130));
            let body = "完整 Unicode 😀".as_bytes();
            let header = if chunked {
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_string()
            } else {
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len())
            };
            stream.write_all(header.as_bytes()).unwrap();
            for part in body.chunks(2) {
                if chunked {
                    write!(stream, "{:x}\r\n", part.len()).unwrap();
                }
                stream.write_all(part).unwrap();
                if chunked {
                    stream.write_all(b"\r\n").unwrap();
                }
            }
            if chunked {
                stream.write_all(b"0\r\n\r\n").unwrap();
            }
        });
        assert_eq!(
            http_request_with_cancel(
                &endpoint,
                "GET",
                None,
                Duration::from_secs(1),
                Some(&LlmCancellation::default())
            )
            .unwrap(),
            "完整 Unicode 😀"
        );
        server.join().unwrap();
    }
}

#[test]
fn cancelled_partial_envelopes_are_not_completed_from_a_valid_looking_body() {
    for chunked in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let provider = HttpModelProvider::new(local_config(&listener));
        let (entered, started) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            test_server::read_request(&mut stream).unwrap();
            let body = json!({"choices":[{"finish_reason":"stop","message":{"content":"helmet"}}]})
                .to_string();
            let wire = if chunked {
                format!(
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n",
                    body.len()
                )
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len() + 1
                )
            };
            stream.write_all(wire.as_bytes()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(700)))
                .unwrap();
            entered.send(()).unwrap();
            assert!(peer_closed(&mut stream));
        });
        let token = LlmCancellation::default();
        let worker_token = token.clone();
        let worker =
            thread::spawn(move || provider.generate_cancellable(&request(), &worker_token));
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        token.cancel();
        assert_eq!(worker.join().unwrap(), Err(LlmProviderError::Cancelled));
        server.join().unwrap();
    }
}

#[test]
fn repeated_poll_timeouts_and_trickle_bytes_keep_one_total_deadline() {
    for trickle in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = local_config(&listener).endpoint;
        let server = thread::spawn(move || {
            let (mut stream, _) = test_server::accept(&listener);
            test_server::read_request(&mut stream).unwrap();
            if trickle {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n")
                    .unwrap();
                for _ in 0..25 {
                    if stream.write_all(b"x").is_err() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            } else {
                stream
                    .set_read_timeout(Some(Duration::from_millis(700)))
                    .unwrap();
                assert!(peer_closed(&mut stream));
            }
        });
        let started = Instant::now();
        let result = http_request_with_cancel(
            &endpoint,
            "GET",
            None,
            Duration::from_millis(220),
            Some(&LlmCancellation::default()),
        );
        let elapsed = started.elapsed();
        server.join().unwrap();
        assert_eq!(result, Err(LlmProviderError::Timeout));
        assert!(
            (Duration::from_millis(200)..Duration::from_millis(600)).contains(&elapsed),
            "{elapsed:?}"
        );
    }
}

#[test]
fn cancelled_discovery_is_not_negative_cached_and_cancelled_generation_keeps_valid_inventory() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let provider = HttpModelProvider::new(ModelProviderConfig {
        model: DEFAULT_MODEL.into(),
        ..local_config(&listener)
    });
    let (entered, started) = mpsc::channel();
    let server = thread::spawn(move || {
        for index in 0..5 {
            let (mut stream, _) = test_server::accept(&listener);
            let wire = test_server::read_request(&mut stream).unwrap();
            assert!(
                wire.starts_with(if index < 2 {
                    "GET /v1/models "
                } else {
                    "POST /v1/chat/completions "
                }),
                "{index}: {wire}"
            );
            if index == 0 || index == 3 {
                stream
                    .set_read_timeout(Some(Duration::from_millis(700)))
                    .unwrap();
                entered.send(()).unwrap();
                assert!(peer_closed(&mut stream));
            } else {
                let body = if index == 1 {
                    json!({"data":[{"id":"synthetic-model"}]})
                } else {
                    json!({"choices":[{"finish_reason":"stop","message":{"content":"helmet"}}]})
                };
                test_server::respond(&mut stream, "200 OK", &body.to_string()).unwrap();
            }
        }
    });
    for iteration in 0..2 {
        let cloned = provider.clone();
        let token = LlmCancellation::default();
        let worker_token = token.clone();
        let worker = thread::spawn(move || cloned.generate_cancellable(&request(), &worker_token));
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        token.cancel();
        assert_eq!(worker.join().unwrap(), Err(LlmProviderError::Cancelled));
        assert_eq!(provider.resolved.lock().unwrap().is_some(), iteration == 1);
        assert_eq!(
            provider.generate_checked(&request()).unwrap()[0].text,
            "helmet"
        );
    }
    server.join().unwrap();
}
