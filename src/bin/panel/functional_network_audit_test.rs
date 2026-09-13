//! Private Xvfb regression from audit N01; no real host/model is used.
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
