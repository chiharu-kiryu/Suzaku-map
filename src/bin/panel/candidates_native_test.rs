//! Candidate gesture regressions on an isolated Xvfb, never the desktop session.
use super::*;
use std::sync::{Mutex, mpsc};
use suzaku_map::ime::CommitOptions;
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
        check_editable_keyboard_choices(&mut state);
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
        check_standalone_commit_delivery(&mut state);
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

fn check_editable_keyboard_choices(state: &mut PanelState) {
    reset_composition(state);
    let committed = state.engine.snapshot().committed_text;
    let word = state
        .engine
        .candidates()
        .iter()
        .position(|candidate| candidate.text == "hello")
        .unwrap();
    state.continue_sentence_candidate(word);
    assert_eq!(state.chrome.seed_text, "hello");
    assert_eq!(state.engine.snapshot().seed_text, "hello");
    assert_eq!(state.engine.snapshot().committed_text, committed);
    for expected in ["hello ", "hello  "] {
        state.continue_composition_with_space();
        assert_eq!(state.chrome.seed_text, expected);
        assert_eq!(state.engine.snapshot().seed_text, expected);
        assert_eq!(state.engine.snapshot().committed_text, committed);
    }
    let before = state.engine.snapshot();
    state.continue_sentence_candidate(usize::MAX);
    assert_eq!(
        state.engine.snapshot(),
        before,
        "an absent candidate slot modified the draft"
    );
    reset_composition(state);
    state.engine.select_candidate(word);
    state.continue_composition_with_space();
    assert_eq!(
        state.chrome.seed_text, "hello ",
        "Space lost an explicit selection"
    );
    assert_eq!(state.engine.snapshot().committed_text, committed);
    reset_composition(state);
    state.continue_composition_with_space();
    assert_eq!(
        state.chrome.seed_text, "hel ",
        "Space changed an unselected literal spelling"
    );
    println!(
        "PASS: panel number choices and repeated spaces retain editable text without committing"
    );
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
            kind: None,
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

fn check_standalone_commit_delivery(state: &mut PanelState) {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let host = current_ime_host_dispatch();
        if host.marked_text_roundtrip && host.commit_roundtrip {
            break;
        }
        assert!(
            Instant::now() < until,
            "fixture must also exercise a ready local bridge"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let path = std::path::PathBuf::from(std::env::var_os("SUZAKU_LINUX_IME_SOCKET").unwrap());
    let runtime = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    assert!(runtime.starts_with(std::env::temp_dir()) && path.starts_with(&runtime));
    assert!(
        !path.exists(),
        "fixture must never replace another host's socket"
    );
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let response = Arc::new(AtomicU8::new(b'1'));
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (server_response, server_requests, server_stop) =
        (response.clone(), requests.clone(), stop.clone());
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !server_stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_millis(200)))
                        .unwrap();
                    let mut text = String::new();
                    if stream.read_to_string(&mut text).is_err() {
                        continue;
                    }
                    if let Some(text) = text.strip_prefix('C') {
                        server_requests.lock().unwrap().push(text.into());
                        let ack = server_response.load(Ordering::Acquire);
                        if ack != 0 {
                            let _ = stream.write_all(&[ack]);
                        }
                    } else {
                        let _ = stream.write_all(b"0");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("fixture accept: {error}"),
            }
        }
    });
    for sentence in [false, true] {
        state.engine.clear_session_context();
        state.native = Default::default();
        state.is_focused = false;
        state.chrome.settings_open = false;
        state.last_commit_attempt = None;
        state
            .chrome
            .set_seed_text("a replacement requiring confirmation".into());
        state.refresh_seed();
        let before = requests.lock().unwrap().len();
        let snapshot = state.engine.snapshot();
        assert!(!state.commit_selected_candidate_to_host(CommitOptions::default()));
        assert_eq!(
            requests.lock().unwrap().len(),
            before,
            "unconfirmed text reached the socket"
        );
        assert_eq!(state.engine.snapshot(), snapshot);
        let commit = |state: &mut PanelState| {
            if sentence {
                state.commit_sentence_candidate(0)
            } else {
                state.commit_selected_candidate_to_host(CommitOptions { force: true })
            }
        };
        for text in ["  first123  ", "second456"] {
            response.store(b'1', Ordering::Release);
            state.chrome.set_seed_text(text.into());
            state.refresh_seed();
            // A separate local bridge can have different provider results or a
            // stale seed. Only the candidate the panel displayed belongs on wire.
            suzaku_map::ime_host::host_bridge_replace_marked_text(
                "wrong-source-candidate",
                suzaku_map::ime::InputSource::OnScreenPanel,
            );
            let before = requests.lock().unwrap().len();
            assert!(commit(state));
            assert_eq!(requests.lock().unwrap()[before..], [text]);
            assert!(
                state.chrome.seed_text.is_empty(),
                "delivered text was copied back into the editable draft"
            );
            assert!(state.engine.snapshot().seed_text.is_empty());
            assert!(!commit(state), "a second click recommitted completed text");
            assert_eq!(requests.lock().unwrap().len(), before + 1);
        }
        for ack in [b'0', 0] {
            response.store(ack, Ordering::Release);
            state.last_commit_attempt = None;
            state.chrome.set_seed_text("  retained789  ".into());
            state.refresh_seed();
            let before = requests.lock().unwrap().len();
            let snapshot = state.engine.snapshot();
            assert!(commit(state));
            assert_eq!(requests.lock().unwrap()[before..], ["  retained789  "]);
            assert_eq!(
                state.engine.snapshot(),
                snapshot,
                "unconfirmed output changed composition or committed context"
            );
            assert_eq!(state.chrome.seed_text, "  retained789  ");
            assert!(
                state
                    .last_commit_feedback
                    .as_ref()
                    .unwrap()
                    .contains("retained")
            );
        }
    }
    stop.store(true, Ordering::Release);
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
    println!(
        "PASS: standalone word/sentence commits clear only acknowledged drafts and retain rejected/uncertain output without replay"
    );
}
