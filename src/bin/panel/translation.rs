//! Explicit, revision-safe translation of the visible composition only.
use super::{PanelState, PanelWindowKind, native_sync::NativeInsertion};
use std::sync::Arc;
use suzaku_map::{
    ime::{
        companion::NativeOperation,
        gpu::{InputMode, TranslationPhase},
    },
    languages::{
        llm::LlmProviderError,
        model::{HttpModelProvider, ModelScope},
        translation::{
            TranslationError, TranslationProvider, TranslationRequest, TranslationWorker,
        },
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Binding {
    Local,
    Native(String, u64),
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::app_state::PersistedDisplaySettings;
    use crate::*;
    use suzaku_map::{
        ime::{companion::NativeComposition, gpu::InteractionKind},
        languages::translation::TranslationLanguage,
    };
    use winit::platform::x11::EventLoopBuilderExtX11;
    struct Fixed(&'static str);
    impl TranslationProvider for Fixed {
        fn translate(&self, _: &TranslationRequest) -> Result<String, TranslationError> {
            Ok(self.0.into())
        }
    }
    fn ready(state: &mut PanelState) {
        let start = Instant::now();
        while state.chrome.translation.phase == TranslationPhase::Pending {
            state.poll_translation();
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::yield_now();
        }
        assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
    }
    fn click(state: &mut PanelState, kind: InteractionKind) {
        let scene = state.current_scene();
        let rect = scene
            .interactive_targets
            .iter()
            .find(|t| t.kind == kind)
            .expect("translation control")
            .rect;
        assert_eq!(
            scene.hit_interaction(rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0),
            Some(kind)
        );
        state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
        state.last_scene = Some(scene);
        state.begin_primary_press(false);
        state.complete_primary_release(false);
    }

    fn assert_native_handoff_ordering(state: &mut PanelState) {
        for case in [
            "ack first",
            "frame first",
            "rejected",
            "disconnected",
            "cancelled",
            "new request",
            "new draft",
            "new context",
            "private context",
            "other tool",
            "interface changed",
        ] {
            state.cancel_translation();
            state.chrome.active_input_mode = InputMode::Translation;
            state.chrome.input_modes_expanded = true;
            state.chrome.settings_open = false;
            state.is_focused = false;
            let frame = NativeComposition {
                host: "00000000-0000-0000-0000-000000000001".into(),
                context: 20,
                revision: 100,
                focused: true,
                private: false,
                language: "en".into(),
                seed: "original draft".into(),
                selected: 0,
                candidates: vec![],
            };
            state.native.showing = true;
            state.native.frame = Some(frame.clone());
            state.refresh_native_view();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            state.start_translation_with(Arc::new(Fixed("translated draft")));
            ready(state);
            state.apply_translation();
            assert!(receiver.try_recv().is_ok(), "{case}");
            assert_eq!(state.chrome.seed_text, frame.seed, "{case}");
            state.apply_translation();
            assert!(
                receiver.try_recv().is_err(),
                "a double click must not resend"
            );
            let mut next = frame.clone();
            next.revision += 1;
            next.seed = "translated draft".into();
            let original_ui = state.chrome.ui_language;
            match case {
                "frame first" => {
                    state.receive_native_frame(Some(next.clone()));
                    assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
                }
                "cancelled" => state.cancel_translation(),
                "new request" => {
                    state.start_translation_with(Arc::new(Fixed("new translation")));
                    ready(state);
                }
                "new draft" => {
                    next.seed = "new user draft".into();
                    state.receive_native_frame(Some(next.clone()));
                }
                "new context" => {
                    next.context += 1;
                    state.receive_native_frame(Some(next.clone()));
                }
                "private context" => {
                    next.private = true;
                    next.seed.clear();
                    state.receive_native_frame(Some(next.clone()));
                }
                "other tool" => {
                    state.chrome.active_input_mode = InputMode::Handwriting;
                    state.poll_translation();
                }
                "interface changed" => {
                    state.chrome.ui_language = suzaku_map::ui::UiLanguage::German
                }
                _ => {}
            }
            let result = match case {
                "rejected" => Ok(false),
                "disconnected" => Err("synthetic disconnect".into()),
                _ => Ok(true),
            };
            state.native_action_finished(frame.host.clone(), frame.revision, result);
            if matches!(case, "ack first" | "frame first" | "interface changed") {
                assert_eq!(
                    state.chrome.active_input_mode,
                    InputMode::VirtualKeyboard,
                    "{case}"
                );
                assert!(!state.chrome.input_modes_expanded, "{case}");
                assert_eq!(
                    state.chrome.translation.phase,
                    TranslationPhase::Idle,
                    "{case}"
                );
                state.receive_native_frame(Some(next));
                assert_eq!(state.chrome.seed_text, "translated draft", "{case}");
            } else {
                assert!(
                    state.chrome.input_modes_expanded,
                    "late ack changed the tool: {case}"
                );
                if case == "other tool" {
                    assert_eq!(state.chrome.active_input_mode, InputMode::Handwriting);
                } else {
                    assert_eq!(
                        state.chrome.active_input_mode,
                        InputMode::Translation,
                        "{case}"
                    );
                }
                if case == "new request" {
                    assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
                    assert_eq!(state.chrome.translation.text, "new translation");
                } else if matches!(case, "rejected" | "disconnected") {
                    assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
                    assert_eq!(state.chrome.translation.text, "translated draft");
                    assert_eq!(state.chrome.seed_text, "original draft");
                } else {
                    assert_eq!(
                        state.chrome.translation.phase,
                        TranslationPhase::Idle,
                        "{case}"
                    );
                }
            }
            assert!(receiver.try_recv().is_err(), "no automatic retry: {case}");
            assert!(state.engine.snapshot().committed_text.is_empty(), "{case}");
            state.chrome.ui_language = original_ui;
        }
    }
    #[test]
    #[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
    fn native_translation_preserves_drafts_and_rejects_stale_contexts() {
        assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
        assert!(
            std::path::Path::new(&std::env::var_os("XDG_CONFIG_HOME").unwrap())
                .starts_with(std::env::temp_dir())
        );
        let mut builder = EventLoop::<()>::with_user_event();
        builder.with_x11().with_any_thread(true);
        let event_loop = builder.build().unwrap();
        struct Probe(bool);
        impl ApplicationHandler for Probe {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                let window = Arc::new(event_loop.create_window(panel_window_attributes()).unwrap());
                let mut state = pollster::block_on(PanelState::new(window)).unwrap();
                state.chrome.llm_enabled = false;
                state.engine.configure_prediction(None);
                state.chrome.set_seed_text("Hello world".into());
                state.refresh_seed();
                let saved = PersistedDisplaySettings::from(&state.chrome);
                click(
                    &mut state,
                    InteractionKind::InputModeButton(InputMode::Translation),
                );
                // Give the synthetic window a full fitted frame before hit testing its drawer.
                let height = state.renderer.preferred_input_panel_height(&state.chrome) as u32;
                state.resize(state.size.width, height);
                let original = state.chrome.seed_text.clone();
                for target in TranslationLanguage::ALL {
                    click(&mut state, InteractionKind::SetTranslationTarget(target));
                    state.start_translation_with(Arc::new(Fixed("你好，世界！")));
                    ready(&mut state);
                    let prior_language = state.chrome.ui_language;
                    state.chrome.ui_language = suzaku_map::ui::UiLanguage::German;
                    assert!(!state.poll_translation());
                    assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
                    assert_eq!(state.chrome.translation.text, "你好，世界！");
                    state.chrome.ui_language = prior_language;
                    assert_eq!(state.chrome.seed_text, original);
                    assert_eq!(PersistedDisplaySettings::from(&state.chrome), saved);
                    assert!(state.engine.snapshot().committed_text.is_empty());
                    // Re-selecting the current language is a no-op, not a cancellation.
                    for kind in [
                        InteractionKind::SetTranslationSource(None),
                        InteractionKind::SetTranslationTarget(target),
                    ] {
                        click(&mut state, kind);
                        assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
                        assert_eq!(state.chrome.translation.text, "你好，世界！");
                    }
                }
                click(&mut state, InteractionKind::ApplyTranslation);
                assert_eq!(state.chrome.seed_text, "你好，世界！");
                assert!(state.engine.snapshot().committed_text.is_empty());
                assert!(state.chrome.input_focused);
                assert!(!state.chrome.input_modes_expanded);
                click(&mut state, InteractionKind::InputModesToggle);
                click(
                    &mut state,
                    InteractionKind::InputModeButton(InputMode::Translation),
                );
                state.chrome.set_seed_text("New draft".into());
                state.chrome.move_caret_to_end();
                state.refresh_seed();
                state.start_translation_with(Arc::new(Fixed("ignored")));
                state.chrome.insert_text(" changed");
                state.refresh_seed();
                assert_eq!(state.chrome.translation.phase, TranslationPhase::Idle);
                state.apply_translation();
                assert_eq!(state.chrome.seed_text, "New draft changed");
                state.native.showing = true;
                state.native.frame = Some(NativeComposition {
                    host: "00000000-0000-0000-0000-000000000001".into(),
                    context: 1,
                    revision: 10,
                    focused: true,
                    private: false,
                    language: "en".into(),
                    seed: "native original".into(),
                    selected: 0,
                    candidates: vec![],
                });
                state.is_focused = false;
                state.chrome.settings_open = false;
                state.refresh_native_view();
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                state.native.sender = Some(sender);
                state.start_translation_with(Arc::new(Fixed("native translation")));
                ready(&mut state);
                state.apply_translation();
                assert!(receiver.try_recv().is_ok());
                assert_eq!(state.chrome.seed_text, "native original");
                state.native_action_finished(
                    "00000000-0000-0000-0000-000000000001".into(),
                    10,
                    Ok(false),
                );
                assert_eq!(state.chrome.translation.text, "native translation");
                assert_eq!(state.chrome.seed_text, "native original");
                // Only an explicit acknowledgement may finish the native handoff.
                state.apply_translation();
                assert!(receiver.try_recv().is_ok());
                state.native_action_finished(
                    "00000000-0000-0000-0000-000000000001".into(),
                    10,
                    Ok(true),
                );
                assert_eq!(state.chrome.active_input_mode, InputMode::VirtualKeyboard);
                assert!(!state.chrome.input_modes_expanded);
                assert_eq!(state.chrome.translation.phase, TranslationPhase::Idle);
                assert_eq!(state.chrome.seed_text, "native original");
                assert!(state.engine.snapshot().committed_text.is_empty());
                state.chrome.active_input_mode = InputMode::Translation;
                state.chrome.input_modes_expanded = true;
                state.start_translation_with(Arc::new(Fixed("native translation")));
                ready(&mut state);
                state.native.frame.as_mut().unwrap().context = 2;
                state.poll_translation();
                assert_eq!(state.chrome.translation.phase, TranslationPhase::Idle);
                state.apply_translation();
                assert!(receiver.try_recv().is_err());
                state.native.frame.as_mut().unwrap().private = true;
                state.start_translation_with(Arc::new(Fixed("never sent")));
                assert_eq!(state.chrome.translation.phase, TranslationPhase::Failed);
                assert!(state.translation.request.is_none());
                assert_native_handoff_ordering(&mut state);
                println!(
                    "PASS: eight translation targets, editable replacement without commit, changed-input cancellation and native context/privacy guards"
                );
                self.0 = true;
                event_loop.exit();
            }
            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }
        let mut probe = Probe(false);
        event_loop.run_app(&mut probe).unwrap();
        assert!(probe.0);
    }
}

#[derive(Default)]
pub(super) struct PanelTranslation {
    worker: Option<TranslationWorker>,
    request: Option<(TranslationRequest, Binding)>,
    generation: u64,
}

pub(super) struct TranslationInsertion {
    generation: u64,
    binding: Binding,
    source: String,
    replacement: String,
}

impl PanelState {
    fn translation_binding(&self) -> Option<Binding> {
        if !self.native.showing {
            return Some(Binding::Local);
        }
        self.native
            .frame
            .as_ref()
            .filter(|frame| frame.visible() && !self.chrome.settings_open)
            .map(|frame| Binding::Native(frame.host.clone(), frame.context))
    }

    pub(super) fn cancel_translation(&mut self) {
        if let Some(worker) = &self.translation.worker {
            worker.cancel();
        }
        self.translation.request = None;
        self.translation.generation = self.translation.generation.wrapping_add(1);
        self.chrome.translation.phase = TranslationPhase::Idle;
        self.chrome.translation.text.clear();
        self.chrome.translation.message.clear();
        self.chrome.translation.page = 0;
        self.last_scene = None;
    }

    fn translation_is_current(&self) -> bool {
        self.translation
            .request
            .as_ref()
            .is_some_and(|(request, binding)| {
                request.text == self.chrome.seed_text
                    && request.source == self.chrome.translation.source
                    && request.target == self.chrome.translation.target
                    && Some(binding) == self.translation_binding().as_ref()
                    && self.chrome.active_input_mode == InputMode::Translation
                    && self.chrome.input_modes_expanded
                    && !self.chrome.compact_mode
            })
    }

    pub(super) fn start_translation(&mut self) {
        if self.kind != PanelWindowKind::Main
            || self.chrome.translation.phase == TranslationPhase::Pending
        {
            return;
        }
        self.cancel_translation();
        let Some(config) = self.confirmed_model_config.clone() else {
            self.translation_failed(LlmProviderError::InvalidEndpoint.into());
            return;
        };
        // An explicit translation must also respect a consent revocation saved
        // since the last host snapshot. Never adopt a different recipient here;
        // refuse until reload instead of sending to either the old or new target.
        let saved = match suzaku_map::ime::settings::ImeSettings::load() {
            Ok(settings) => settings.provider,
            Err(_) => {
                self.translation_failed(LlmProviderError::InvalidEndpoint.into());
                return;
            }
        };
        if !config.same_identity(&saved) || config.cloud_consent != saved.cloud_consent {
            self.translation_failed(LlmProviderError::InvalidEndpoint.into());
            self.chrome.translation.message = "Reload model settings".into();
            return;
        }
        self.chrome.translation.cloud = config.scope == ModelScope::Cloud;
        self.start_translation_with(Arc::new(HttpModelProvider::new(config)));
    }

    fn start_translation_with(&mut self, provider: Arc<dyn TranslationProvider>) {
        let request = TranslationRequest {
            text: self.chrome.seed_text.clone(),
            source: self.chrome.translation.source,
            target: self.chrome.translation.target,
        };
        if let Err(error) = request.validate() {
            self.translation_failed(error);
            return;
        }
        let Some(binding) = self.translation_binding() else {
            self.translation_failed(LlmProviderError::Unavailable.into());
            return;
        };
        if self.kind != PanelWindowKind::Main
            || self.chrome.active_input_mode != InputMode::Translation
            || !self.chrome.input_modes_expanded
            || self.chrome.compact_mode
        {
            return;
        }
        let worker = self.translation.worker.get_or_insert_with(Default::default);
        if !worker.request(provider, request.clone()) {
            self.translation_failed(LlmProviderError::Unavailable.into());
            return;
        }
        self.translation.request = Some((request, binding));
        self.translation.generation = self.translation.generation.wrapping_add(1);
        self.chrome.translation.phase = TranslationPhase::Pending;
        self.chrome.translation.text.clear();
        self.chrome.translation.page = 0;
        self.chrome.translation.message.clear();
        self.last_scene = None;
    }

    fn translation_failed(&mut self, error: TranslationError) {
        self.chrome.translation.phase = TranslationPhase::Failed;
        self.chrome.translation.text.clear();
        self.chrome.translation.message = error.to_string();
        self.last_scene = None;
    }

    pub(super) fn poll_translation(&mut self) -> bool {
        if self.kind != PanelWindowKind::Main {
            return false;
        }
        if self.translation.request.is_some()
            && !self.translation_is_current()
            && !self.native_translation_replacement_visible()
        {
            self.cancel_translation();
            return true;
        }
        if self.chrome.translation.phase != TranslationPhase::Pending {
            return false;
        }
        let Some(result) = self
            .translation
            .worker
            .as_ref()
            .and_then(|w| w.take_result())
        else {
            return false;
        };
        match result {
            Ok(text) => {
                self.chrome.translation.phase = TranslationPhase::Ready;
                self.chrome.translation.text = text;
                self.chrome.translation.page = 0;
            }
            Err(error) => self.translation_failed(error),
        }
        self.last_scene = None;
        true
    }

    pub(super) fn apply_translation(&mut self) {
        if !self.translation_is_current()
            || self.chrome.translation.phase != TranslationPhase::Ready
        {
            return;
        }
        // Composition is single-line; preview retains the original paragraph breaks.
        let text: String = self
            .chrome
            .translation
            .text
            .chars()
            .map(|ch| {
                if matches!(ch, '\n' | '\r' | '\t') {
                    ' '
                } else {
                    ch
                }
            })
            .collect();
        if text.is_empty() || text.len() > suzaku_map::ime::companion::MAX_TEXT_BYTES {
            return;
        }
        if self.native.showing {
            // Revision-bound native replacement retains the draft until acknowledgement.
            let insertion = TranslationInsertion {
                generation: self.translation.generation,
                binding: self.translation.request.as_ref().unwrap().1.clone(),
                source: self.chrome.seed_text.clone(),
                replacement: text.clone(),
            };
            self.native_action_with_source(
                NativeOperation::Replace(text),
                Some(NativeInsertion::Translation(insertion)),
            );
            return;
        }
        self.chrome.set_seed_text(text);
        self.chrome.move_caret_to_end();
        self.chrome.active_input_mode = InputMode::VirtualKeyboard;
        self.chrome.input_modes_expanded = false;
        self.chrome.focus_input();
        self.sync_manual_seed_base();
        self.refresh_seed();
        self.cancel_translation();
    }

    fn translation_insertion_is_current(&self, insertion: &TranslationInsertion) -> bool {
        self.translation.generation == insertion.generation
            && self.translation_binding().as_ref() == Some(&insertion.binding)
            && self.chrome.active_input_mode == InputMode::Translation
            && self.chrome.input_modes_expanded
            && !self.chrome.compact_mode
    }

    pub(super) fn translation_replacement_visible(&self, insertion: &TranslationInsertion) -> bool {
        // The composition update and action acknowledgement use independent
        // channels. An update for our own replacement is not a new draft; keep
        // the handoff alive until its ack without permitting a second application.
        self.translation_insertion_is_current(insertion)
            && self.chrome.seed_text == insertion.replacement
    }

    pub(super) fn finish_translation_insertion(
        &mut self,
        insertion: &TranslationInsertion,
    ) -> bool {
        if !self.translation_insertion_is_current(insertion)
            || (self.chrome.seed_text != insertion.source
                && self.chrome.seed_text != insertion.replacement)
        {
            return false;
        }
        self.cancel_translation();
        true
    }
}
