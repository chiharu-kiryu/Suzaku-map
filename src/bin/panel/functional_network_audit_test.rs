//! Private Xvfb functional-network regressions; no real host/model is used.
use super::*;
use std::sync::mpsc;
use suzaku_map::ime::{CommitOptions, settings::ImeSettings};
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};
use suzaku_map::languages::{
    BuiltinLanguage,
    model::{ModelProtocol, ModelScope},
};
use winit::platform::x11::EventLoopBuilderExtX11;

struct Record(mpsc::Sender<LlmCompletionRequest>);
impl LlmCompletionProvider for Record {
    fn provider_id(&self) -> &str {
        "new-provider-in-memory-audit"
    }
    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        let _ = self.0.send(request.clone());
        Vec::new()
    }
}

#[test]
#[ignore = "requires private Xvfb and SUZAKU_PANEL_NATIVE_QA=1; run by test-linux-ci.sh ui"]
fn audit_provider_change_must_forget_standalone_commit_context() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in [
        "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "SUZAKU_IME_CONFIG",
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(key).expect("private test path"));
        assert!(
            path.starts_with(std::env::temp_dir()),
            "private fixture required: {key}"
        );
    }
    let settings = ImeSettings::default();
    assert!(!settings.llm_enabled);
    settings.save().unwrap();
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    let mut probe = Probe {
        proxy: events.create_proxy(),
        completed: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

#[test]
#[ignore = "requires private Xvfb/configuration; run by test-linux-ci.sh ui"]
fn model_configuration_stays_bound_until_confirmed_reload() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "SUZAKU_IME_CONFIG"] {
        assert!(
            std::path::PathBuf::from(std::env::var_os(key).unwrap())
                .starts_with(std::env::temp_dir())
        );
    }
    let server = ModelRecorder::new();
    let settings = server.settings("original");
    settings.save().unwrap();
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    struct ModelProbe {
        proxy: EventLoopProxy<PanelUserEvent>,
        server: ModelRecorder,
        done: bool,
    }
    impl ApplicationHandler<PanelUserEvent> for ModelProbe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let panel = pollster::block_on(PanelState::new(window)).unwrap();
            let mut app = PanelApp::new(self.proxy.clone(), None);
            app.panel = Some(panel);
            let original = self.server.settings("original");
            app.apply_native_settings(original.clone(), false);
            let panel = app.panel.as_mut().unwrap();
            panel.engine.seed("SYNTHETIC_OLD_CONTEXT");
            assert!(panel.engine.commit(CommitOptions { force: true }).ok);
            panel.chrome.set_seed_text("hel".into());
            panel.refresh_seed();
            let replacement = self.server.settings("replacement");
            replacement.save().unwrap();
            // A tone/hint refresh is not permission to adopt a new endpoint from disk.
            panel.chrome.llm_enabled = true;
            panel.reconfigure_model_provider();
            let (route, request) = self.server.request();
            assert_eq!(route, "POST /original/v1/chat/completions HTTP/1.1");
            assert_eq!(request["model"], "original");
            let input: serde_json::Value =
                serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert_eq!(input["committed_context"], "SYNTHETIC_OLD_CONTEXT");
            panel.engine.configure_prediction(None);

            // Translation must not silently adopt either endpoint while disk/host disagree.
            panel.chrome.llm_enabled = false;
            panel.chrome.active_input_mode = InputMode::Translation;
            panel.chrome.input_modes_expanded = true;
            panel.start_translation();
            assert_eq!(
                panel.chrome.translation.phase,
                suzaku_map::ime::gpu::TranslationPhase::Failed
            );
            assert_eq!(panel.chrome.translation.message, "Reload model settings");
            assert!(self.server.requests.try_recv().is_err());
            assert_eq!(panel.chrome.seed_text, "hel");
            panel.cancel_translation();

            // Failed external edits cannot silently select the default endpoint.
            let path = std::path::PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap());
            std::fs::write(&path, b"{").unwrap();
            panel.chrome.llm_enabled = true;
            panel.reconfigure_model_provider();
            assert_eq!(self.server.request().1["model"], "original");
            panel.engine.configure_prediction(None);
            assert_eq!(std::fs::read(&path).unwrap(), b"{");
            // A new window with no valid model snapshot must not trust a cached display On.
            panel.persist_display_settings();
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let mut invalid = pollster::block_on(PanelState::new(window)).unwrap();
            assert!(!invalid.chrome.llm_enabled);
            assert!(invalid.confirmed_model_config.is_none());
            invalid
                .chrome
                .set_seed_text("private synthetic draft".into());
            invalid.chrome.llm_enabled = true;
            invalid.reconfigure_model_provider();
            assert_eq!(
                invalid.engine.prediction_status(),
                suzaku_map::ime::PredictionStatus::Disabled
            );
            assert!(!invalid.engine.candidates().is_empty());
            assert!(invalid.last_commit_feedback.is_some());
            invalid.chrome.active_input_mode = InputMode::Translation;
            invalid.chrome.input_modes_expanded = true;
            invalid.start_translation();
            assert_eq!(
                invalid.chrome.translation.phase,
                suzaku_map::ime::gpu::TranslationPhase::Failed
            );
            assert_eq!(invalid.chrome.seed_text, "private synthetic draft");
            assert!(self.server.requests.try_recv().is_err());
            drop(invalid);
            replacement.save().unwrap();

            // A confirmed old host snapshot also must not accidentally load the new disk file.
            app.apply_native_settings(original, false);
            let panel = app.panel.as_mut().unwrap();
            panel.chrome.llm_enabled = true;
            panel.reconfigure_model_provider();
            assert_eq!(self.server.request().1["model"], "original");
            panel.engine.configure_prediction(None);

            panel.chrome.llm_enabled = false;
            self.server.settings("original").save().unwrap();
            panel.start_translation();
            assert_eq!(self.server.request().1["model"], "original");
            // The old translation may finish on its worker, but must not survive reload.

            // After an explicit successful reload, the new model receives only the current draft.
            replacement.save().unwrap();
            app.apply_native_settings(replacement, false);
            let panel = app.panel.as_mut().unwrap();
            assert!(!panel.poll_translation());
            assert_eq!(
                panel.chrome.translation.phase,
                suzaku_map::ime::gpu::TranslationPhase::Idle
            );
            assert!(panel.chrome.translation.text.is_empty());
            panel.chrome.llm_enabled = true;
            panel.reconfigure_model_provider();
            let (route, request) = self.server.request();
            assert_eq!(route, "POST /replacement/v1/chat/completions HTTP/1.1");
            assert_eq!(request["model"], "replacement");
            let input: serde_json::Value =
                serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert_eq!(input["raw_composition"], "hel");
            assert_eq!(input["committed_context"], "");
            panel.engine.configure_prediction(None);
            panel.chrome.llm_enabled = false;
            panel.start_translation();
            assert_eq!(self.server.request().1["model"], "replacement");
            settle_translation(panel);
            assert_eq!(panel.chrome.translation.text, "你好，世界！");
            assert_eq!(panel.chrome.seed_text, "hel");
            panel.cancel_translation();

            // Revoked cloud consent remains a no-network guard for both operations.
            let mut cloud = self.server.settings("cloud");
            cloud.provider.scope = ModelScope::Cloud;
            cloud.provider.endpoint = "https://never-contact.invalid/v1/chat/completions".into();
            cloud.save().unwrap();
            let mut stale_authorized = cloud.clone();
            stale_authorized.provider.cloud_consent = true;
            app.apply_native_settings(stale_authorized, false);
            let panel = app.panel.as_mut().unwrap();
            panel.start_translation();
            assert_eq!(
                panel.chrome.translation.phase,
                suzaku_map::ime::gpu::TranslationPhase::Failed
            );
            assert_eq!(panel.chrome.translation.message, "Reload model settings");
            assert!(self.server.requests.try_recv().is_err());
            app.apply_native_settings(cloud, false);
            let panel = app.panel.as_mut().unwrap();
            panel.start_translation();
            let deadline = Instant::now() + Duration::from_secs(3);
            while panel.chrome.translation.phase == suzaku_map::ime::gpu::TranslationPhase::Pending
            {
                assert!(Instant::now() < deadline);
                panel.poll_translation();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(
                panel.chrome.translation.phase,
                suzaku_map::ime::gpu::TranslationPhase::Failed
            );
            assert!(panel.chrome.translation.cloud);
            assert!(panel.chrome.translation.message.contains("not authorized"));
            panel.chrome.llm_enabled = true;
            panel.reconfigure_model_provider();
            let deadline = Instant::now() + Duration::from_secs(3);
            while panel.engine.prediction_pending() {
                assert!(Instant::now() < deadline);
                panel.engine.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(
                panel.engine.prediction_error(),
                Some(&suzaku_map::languages::llm::LlmProviderError::CloudConsentRequired)
            );
            assert!(!panel.engine.candidates().is_empty());
            assert!(self.server.requests.try_recv().is_err());
            println!(
                "PASS: pinned provider, invalid startup, confirmed reload, translation cancellation and consent guards; only synthetic loopback traffic"
            );
            self.done = true;
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = ModelProbe {
        proxy: events.create_proxy(),
        server,
        done: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.done);
}

#[test]
#[ignore = "requires private Xvfb/configuration; run by test-linux-ci.sh ui"]
fn translation_requests_reject_stale_press_targets() {
    use suzaku_map::ime::{companion::NativeComposition, gpu::TranslationPhase};
    use suzaku_map::languages::translation::TranslationLanguage;

    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in [
        "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "SUZAKU_IME_CONFIG",
    ] {
        assert!(
            std::path::PathBuf::from(std::env::var_os(key).unwrap())
                .starts_with(std::env::temp_dir())
        );
    }
    let server = ModelRecorder::new();
    server.settings("translation-gesture").save().unwrap();
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    struct TranslationProbe {
        server: ModelRecorder,
        findings: Option<Vec<String>>,
    }
    impl ApplicationHandler for TranslationProbe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let mut panel = pollster::block_on(PanelState::new(window)).unwrap();
            assert!(panel.runs_without_window_focus);
            let mut findings = Vec::new();
            for touch in [false, true] {
                for case in [
                    "unchanged",
                    "metadata",
                    "new draft",
                    "new context",
                    "new host",
                    "language",
                    "disconnect then return",
                    "private then return",
                    "unfocused then return",
                    "context away and back",
                    "local unchanged",
                    "local edit",
                    "local edit then restore",
                    "local to native",
                    "cancel",
                    "source language",
                    "target language",
                ] {
                    panel.cancel_translation();
                    panel.cancel_primary_interaction();
                    panel.native = Default::default();
                    panel.chrome = PanelChromeState {
                        llm_enabled: false,
                        input_focused: false,
                        ..Default::default()
                    };
                    panel.engine.configure_prediction(None);
                    panel.is_focused = false;
                    let initial = NativeComposition {
                        host: "00000000-0000-0000-0000-000000000001".into(),
                        context: 1,
                        revision: 10,
                        focused: true,
                        private: false,
                        cursor: None,
                        language: "en".into(),
                        seed: "alpha draft".into(),
                        selected: 0,
                        candidates: vec![],
                    };
                    panel.receive_native_frame(Some(initial.clone()));
                    assert!(
                        panel.native.showing,
                        "native fixture did not enter its view"
                    );
                    assert_eq!(panel.chrome.seed_text, initial.seed);
                    if case.starts_with("local") {
                        panel.leave_native_view();
                        panel.chrome.set_seed_text("local draft".into());
                        panel.refresh_seed();
                    }
                    panel.chrome.active_input_mode = InputMode::Translation;
                    panel.chrome.input_modes_expanded = true;
                    let height = panel.renderer.preferred_input_panel_height(&panel.chrome) as u32;
                    panel.resize(panel.size.width, height);
                    point_translate(&mut panel);
                    panel.interaction.last_input_was_touch = touch;
                    panel.begin_primary_press(touch);
                    assert_eq!(
                        panel.interaction.pressed_interaction,
                        Some(InteractionKind::TranslateText)
                    );
                    let mut next = initial.clone();
                    next.revision += 1;
                    match case {
                        "unchanged" | "local unchanged" => {}
                        "metadata" | "local to native" => panel.receive_native_frame(Some(next)),
                        "new draft" | "new context" => {
                            next.seed = "beta draft".into();
                            if case == "new context" {
                                next.context += 1;
                            }
                            panel.receive_native_frame(Some(next));
                        }
                        "new host" => {
                            next.host = "00000000-0000-0000-0000-000000000002".into();
                            next.revision = 1;
                            panel.receive_native_frame(Some(next));
                        }
                        "language" => {
                            next.language = "ja".into();
                            panel.receive_native_frame(Some(next));
                        }
                        "disconnect then return" => {
                            panel.receive_native_frame(None);
                            panel.receive_native_frame(Some(next));
                        }
                        "private then return"
                        | "unfocused then return"
                        | "context away and back" => {
                            let mut intermediate = next.clone();
                            intermediate.private = case == "private then return";
                            intermediate.focused = case != "unfocused then return";
                            if case == "context away and back" {
                                intermediate.context += 1;
                            }
                            panel.receive_native_frame(Some(intermediate));
                            next.revision += 1;
                            panel.receive_native_frame(Some(next));
                        }
                        "local edit" | "local edit then restore" => {
                            panel.chrome.set_seed_text("edited draft".into());
                            panel.refresh_seed();
                            if case == "local edit then restore" {
                                panel.chrome.set_seed_text("local draft".into());
                                panel.refresh_seed();
                            }
                        }
                        "cancel" => panel.cancel_translation(),
                        "source language" | "target language" => {
                            panel.cancel_translation();
                            if case == "source language" {
                                panel.chrome.translation.source =
                                    Some(TranslationLanguage::English);
                            } else {
                                panel.chrome.translation.target = TranslationLanguage::English;
                            }
                        }
                        _ => unreachable!(),
                    }
                    let allowed = matches!(case, "unchanged" | "metadata" | "local unchanged");
                    let current_draft = panel.chrome.seed_text.clone();
                    let expected_user = format!(
                        "Translate this {} text into {}:\n\n{}",
                        panel
                            .chrome
                            .translation
                            .source
                            .map(|language| language.name())
                            .unwrap_or("auto-detected"),
                        panel.chrome.translation.target.name(),
                        current_draft
                    );
                    if !allowed && panel.interaction.pressed_interaction.is_some() {
                        findings.push(format!(
                            "N24 touch={touch}, {case}: old Translate press survived"
                        ));
                    }
                    panel.complete_primary_release(touch);
                    if panel.chrome.translation.phase == TranslationPhase::Pending {
                        let (_, request) = self.server.request();
                        assert_eq!(request["messages"][1]["content"], expected_user);
                        if !allowed {
                            findings.push(format!("N24 touch={touch}, {case}: stale release sent {:?} to the synthetic model", current_draft));
                        }
                        settle_translation(&mut panel);
                    } else {
                        assert!(
                            !allowed,
                            "normal explicit Translate must still work: {case}"
                        );
                        assert_eq!(
                            panel.chrome.translation.phase,
                            TranslationPhase::Idle,
                            "{case}"
                        );
                    }
                    assert_eq!(panel.chrome.seed_text, current_draft);
                    let phase_before_repeat = panel.chrome.translation.phase;
                    panel.complete_primary_release(touch);
                    assert_eq!(
                        panel.chrome.translation.phase, phase_before_repeat,
                        "duplicate release must not enqueue a new request: {case}"
                    );
                    assert!(self.server.requests.try_recv().is_err());
                    panel.cancel_translation();
                    // Explicitly reopening after local-to-native folding is a new gesture.
                    panel.chrome.input_modes_expanded = true;
                    point_translate(&mut panel);
                    panel.begin_primary_press(touch);
                    panel.complete_primary_release(touch);
                    assert_eq!(
                        self.server.request().1["messages"][1]["content"],
                        expected_user
                    );
                    settle_translation(&mut panel);
                    assert_eq!(panel.chrome.seed_text, current_draft);
                    assert!(panel.engine.snapshot().committed_text.is_empty());
                    assert!(self.server.requests.try_recv().is_err());
                }
            }
            panel.cancel_translation();
            self.findings = Some(findings);
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = TranslationProbe {
        server,
        findings: None,
    };
    events.run_app(&mut probe).unwrap();
    let findings = probe.findings.unwrap();
    for finding in &findings {
        eprintln!("AUDIT: {finding}");
    }
    assert!(
        findings.is_empty(),
        "{} stale translation gesture failures",
        findings.len()
    );
    println!(
        "PASS: 34 mouse/touch translation-request gesture cases; only synthetic loopback traffic"
    );
}

fn point_translate(panel: &mut PanelState) {
    let scene = panel.current_scene();
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::TranslateText)
        .expect("Translate control")
        .rect;
    let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(
        scene.hit_interaction(point.0, point.1),
        Some(InteractionKind::TranslateText)
    );
    panel.cursor_position = Some(point);
    panel.last_scene = Some(scene);
}

fn settle_translation(panel: &mut PanelState) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while panel.chrome.translation.phase == suzaku_map::ime::gpu::TranslationPhase::Pending {
        assert!(Instant::now() < deadline);
        panel.poll_translation();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        panel.chrome.translation.phase,
        suzaku_map::ime::gpu::TranslationPhase::Ready
    );
}

/// Owns only an ephemeral loopback listener. Never probes a default model port.
struct ModelRecorder {
    address: std::net::SocketAddr,
    requests: mpsc::Receiver<(String, serde_json::Value)>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl ModelRecorder {
    fn new() -> Self {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = stop.clone();
        let (sender, requests) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while !stopped.load(std::sync::atomic::Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut wire = Vec::new();
                let mut buffer = [0; 4096];
                while wire.len() < 128 * 1024 {
                    let Ok(count) = stream.read(&mut buffer) else {
                        break;
                    };
                    if count == 0 {
                        break;
                    }
                    wire.extend_from_slice(&buffer[..count]);
                    let Some(split) = wire.windows(4).position(|p| p == b"\r\n\r\n") else {
                        continue;
                    };
                    let header = std::str::from_utf8(&wire[..split]).unwrap();
                    let length: usize = header
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if wire.len() < split + 4 + length {
                        continue;
                    }
                    let request: serde_json::Value =
                        serde_json::from_slice(&wire[split + 4..split + 4 + length]).unwrap();
                    let is_translation = request["messages"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("You are a translator.");
                    let content = if is_translation {
                        "你好，世界！"
                    } else {
                        "hello from the synthetic model"
                    };
                    let response = serde_json::json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}]}).to_string();
                    // Respond before publishing so receiver-side assertions cannot strand the client.
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                        response.len()
                    );
                    let _ = sender.send((header.lines().next().unwrap().into(), request));
                    break;
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            worker: Some(worker),
        }
    }

    fn settings(&self, model: &str) -> ImeSettings {
        let mut settings = ImeSettings::default();
        settings.provider.endpoint = format!("http://{}/{model}/v1/chat/completions", self.address);
        settings.provider.protocol = ModelProtocol::OpenAiCompatible;
        settings.provider.model = model.into();
        settings
    }

    fn request(&self) -> (String, serde_json::Value) {
        self.requests.recv_timeout(Duration::from_secs(3)).unwrap()
    }
}

impl Drop for ModelRecorder {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.worker.take().unwrap().join();
    }
}

struct Probe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
}
impl ApplicationHandler<PanelUserEvent> for Probe {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
        let panel = pollster::block_on(PanelState::new(window)).unwrap();
        let mut app = PanelApp::new(self.proxy.clone(), None);
        app.panel = Some(panel);
        for change in [
            "unchanged",
            "temperature",
            "timeout",
            "model",
            "endpoint",
            "protocol",
            "scope",
            "credential",
            "language",
        ] {
            let mut original = ImeSettings::default();
            if change == "credential" {
                original.provider.scope = ModelScope::Cloud;
                original.provider.endpoint = "https://example.invalid/v1/chat/completions".into();
                original.provider.model = "synthetic-cloud-model".into();
                original.provider.api_key_env = Some("SUZAKU_AUDIT_OLD_KEY".into());
            }
            original.save().unwrap();
            app.apply_native_settings(original.clone(), false);
            let panel = app.panel.as_mut().unwrap();
            panel.engine.configure_prediction(None);
            panel.engine.clear_session_context();
            // Simulate the local engine state after acknowledged delivery;
            // this marker is not real user text and has no target application.
            panel.engine.seed("AUDIT_OLD_PROVIDER_CONTEXT");
            assert!(panel.engine.commit(CommitOptions { force: true }).ok);
            panel.chrome.set_seed_text("hel".into());
            panel.refresh_seed();
            let mut next = original;
            match change {
                "unchanged" => {}
                "temperature" => next.provider.temperature_tenths = 8,
                "timeout" => next.provider.timeout_ms = 2500,
                "model" => next.provider.model = "synthetic-new-provider".into(),
                "endpoint" => {
                    next.provider.endpoint = "http://127.0.0.1:9/v1/chat/completions".into()
                }
                "protocol" => next.provider.protocol = ModelProtocol::Ollama,
                "scope" => {
                    next.provider.scope = ModelScope::Cloud;
                    next.provider.endpoint = "https://example.invalid/v1/chat/completions".into();
                    next.provider.model = "synthetic-cloud-model".into();
                }
                "credential" => next.provider.api_key_env = Some("SUZAKU_AUDIT_NEW_KEY".into()),
                "language" => next.language = BuiltinLanguage::Japanese,
                _ => unreachable!(),
            }
            assert!(
                !next.llm_enabled,
                "never connect to the configured endpoint"
            );
            next.save().unwrap();
            app.apply_native_settings(next, false);
            let panel = app.panel.as_mut().unwrap();
            assert_eq!(
                panel.chrome.seed_text, "hel",
                "{change}: reload lost current draft"
            );
            let expected_context = if matches!(change, "unchanged" | "temperature" | "timeout") {
                "AUDIT_OLD_PROVIDER_CONTEXT"
            } else {
                assert!(
                    panel.engine.undo().is_none(),
                    "{change}: old context remains in undo history"
                );
                ""
            };
            let (sender, receiver) = mpsc::channel();
            panel
                .engine
                .configure_prediction(Some(Arc::new(Record(sender))));
            let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(request.seed_text, "hel");
            assert_eq!(
                request.context_before_cursor, expected_context,
                "N01: {change}: incorrect standalone context boundary"
            );
        }
        self.completed = true;
        events.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
