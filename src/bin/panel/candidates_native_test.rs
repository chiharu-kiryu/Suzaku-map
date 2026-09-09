//! Candidate gesture regressions on an isolated Xvfb, never the desktop session.
use super::*;
use std::sync::{Mutex, mpsc};
use suzaku_map::languages::llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest};
use winit::platform::x11::EventLoopBuilderExtX11;

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_candidate_clicks_and_async_refresh_are_safe() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::<()>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = CandidateProbe { completed: false };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct CandidateProbe {
    completed: bool,
}

impl ApplicationHandler for CandidateProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    panel_window_attributes()
                        .with_title("Synthetic candidate QA")
                        .with_inner_size(LogicalSize::new(900.0, 480.0)),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        state.runs_without_window_focus = true;
        state.chrome.llm_enabled = false;
        state.chrome.input_modes_expanded = false;
        for touch in [false, true] {
            reset_composition(&mut state);
            press(&mut state, InteractionKind::SelectNextToken(0), touch);
            // A refresh with identical candidates must not eat a valid click.
            state.refresh_composition_candidates();
            assert_eq!(
                state.interaction.pressed_interaction,
                Some(InteractionKind::SelectNextToken(0))
            );
            state.complete_primary_release(touch);
            assert_eq!(
                state.chrome.seed_text, "hello",
                "first completion click was lost"
            );
            assert_eq!(state.engine.snapshot().seed_text, "hello");
            assert_eq!(state.chrome.composed_tokens, ["hello"]);
            press(&mut state, InteractionKind::RewindNextToken, touch);
            state.complete_primary_release(touch);
            assert_eq!(state.chrome.seed_text, "hel");
            assert!(state.chrome.composed_tokens.is_empty());
        }
        for touch in [false, true] {
            for target in [
                InteractionKind::Candidate(2),
                InteractionKind::SelectNextToken(1),
            ] {
                check_async_refresh(&mut state, target, touch);
            }
        }
        println!(
            "PASS: mouse/touch completion and rewind; asynchronous candidate replacement cancels stale presses"
        );
        self.completed = true;
        event_loop.exit();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn reset_composition(state: &mut PanelState) {
    state.engine.configure_prediction(None);
    state.chrome.set_seed_text("hel".into());
    state.chrome.move_caret_to_end();
    state.completion_history.clear();
    state.last_interaction_action = None;
    state.engine.seed("hel");
    state.refresh_composition_candidates();
    state.last_scene = None;
}

fn press(state: &mut PanelState, target: InteractionKind, touch: bool) {
    let scene = state.current_scene();
    let rect = scene
        .interactive_targets
        .iter()
        .find(|item| item.kind == target)
        .unwrap()
        .rect;
    let (x, y) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(x, y), Some(target));
    state.last_scene = Some(scene);
    state.cursor_position = Some((x, y));
    state.interaction.last_input_was_touch = touch;
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(target));
}

struct GatedProvider {
    started: mpsc::Sender<()>,
    reply: Mutex<mpsc::Receiver<()>>,
}

impl LlmCompletionProvider for GatedProvider {
    fn provider_id(&self) -> &str {
        "candidate-gesture-fixture"
    }

    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        assert_eq!(request.seed_text, "hel");
        self.started.send(()).unwrap();
        self.reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        vec![LlmCompletion {
            text: "helipad".into(),
            score_bias: 1.0,
        }]
    }
}

fn check_async_refresh(state: &mut PanelState, target: InteractionKind, touch: bool) {
    reset_composition(state);
    let (started_tx, started) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    state
        .engine
        .configure_prediction(Some(Arc::new(GatedProvider {
            started: started_tx,
            reply: Mutex::new(reply_rx),
        })));
    state.refresh_composition_candidates();
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(state.engine.candidates()[2].text, "help");
    press(state, target, touch);
    reply.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !state.engine.poll_prediction() {
        assert!(Instant::now() < deadline, "fixture result did not arrive");
        std::thread::sleep(Duration::from_millis(2));
    }
    state.refresh_composition_candidates();
    assert_eq!(state.engine.candidates()[2].text, "helipad");
    assert!(
        state.interaction.pressed_interaction.is_none(),
        "a replaced candidate retained the old press"
    );
    assert!(!state.interaction.touch_tap_pending);
    let before = state.engine.snapshot();
    state.complete_primary_release(touch);
    assert_eq!(
        state.engine.snapshot(),
        before,
        "release applied a replacement candidate"
    );
    assert_eq!(state.chrome.seed_text, "hel");
    state.engine.configure_prediction(None);
}
