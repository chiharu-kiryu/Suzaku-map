//! Draft handoff tests with real panel state, synthetic acknowledgements and no host connection.
use super::*;
use crate::{PanelChromeState, panel_window_attributes};
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    platform::x11::EventLoopBuilderExtX11,
    window::WindowId,
};

/// A finite private listener exercises the actual NativeSync Drop path. Its
/// failsafe closes every socket even if a future regression makes join block.
pub(crate) fn assert_workers_cancel_on_shutdown(proxy: EventLoopProxy<PanelUserEvent>) {
    use std::os::{fd::AsRawFd, unix::net::UnixListener};
    use suzaku_map::platform::linux_ipc::Deadline;
    let path = suzaku_map::platform::linux_ime_sync::socket_path().unwrap();
    assert!(path.starts_with(std::env::temp_dir()));
    assert!(!path.exists(), "must not replace an existing host socket");
    for full_queue in [true, false] {
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let queued = if full_queue {
            // SAFETY: this descriptor belongs to the fresh private test listener.
            assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 0) }, 0);
            Some(
                Deadline::new(Duration::from_millis(200))
                    .connect(&path)
                    .unwrap(),
            )
        } else {
            None
        };
        let (finish, done) = mpsc::channel();
        let (ready, started) = mpsc::channel();
        let fixture = std::thread::spawn(move || {
            let mut peers = Vec::new();
            if !full_queue {
                let until = Instant::now() + Duration::from_millis(500);
                while peers.len() < 2 && Instant::now() < until {
                    match listener.accept() {
                        Ok((mut peer, _)) => {
                            let request = Deadline::new(Duration::from_millis(100))
                                .read_to_end(&mut peer, 1)
                                .unwrap();
                            assert!(request == b"W" || request == b"Q");
                            peers.push(peer); // Deliberately withhold both frame and ack.
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2))
                        }
                        Err(e) => panic!("private accept: {e}"),
                    }
                }
                assert_eq!(peers.len(), 2);
            }
            let _ = ready.send(());
            let _ = done.recv_timeout(Duration::from_millis(500));
            drop(peers);
            drop(listener);
        });
        let sync = start(proxy.clone()).unwrap();
        sync.sender
            .try_send(ActionRequest {
                command: "Q".into(),
                host: "synthetic".into(),
                revision: 0,
            })
            .unwrap();
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        if full_queue {
            std::thread::sleep(Duration::from_millis(30));
        }
        let before = Instant::now();
        drop(sync);
        let elapsed = before.elapsed();
        let _ = finish.send(());
        fixture.join().unwrap();
        drop(queued);
        std::fs::remove_file(&path).unwrap();
        assert!(
            elapsed < Duration::from_millis(250),
            "shutdown stalled: {elapsed:?}, full_queue={full_queue}"
        );
        println!(
            "PASS: native reader/action workers cancel and join in {elapsed:?}, full_queue={full_queue}"
        );
    }
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_source_insertions_preserve_drafts_until_acknowledged() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = SourceProbe { completed: false };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct SourceProbe {
    completed: bool,
}

impl ApplicationHandler for SourceProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(panel_window_attributes()).unwrap());
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        // Never start a microphone or connect an action writer in this fixture.
        let voice_bridge = state.voice.bridge.take();
        assert_screen_keyboard_handoffs(&mut state);
        for source in [InputMode::Dictation, InputMode::Handwriting] {
            for blocked in [
                "no sender",
                "panel focused",
                "settings open",
                "queue full",
                "disconnected",
                "private",
            ] {
                seed_source(&mut state, source);
                let (sender, receiver) = mpsc::sync_channel(0);
                match blocked {
                    "no sender" => {}
                    "panel focused" => state.is_focused = true,
                    "settings open" => state.chrome.settings_open = true,
                    "private" => state.native.frame.as_mut().unwrap().private = true,
                    _ => state.native.sender = Some(sender),
                }
                if blocked == "disconnected" {
                    drop(receiver);
                }
                insert(&mut state, source);
                assert_source_retained(&state, source);
                assert_eq!(state.chrome.seed_text, "hello", "{blocked}");
                assert!(state.native.pending.is_none(), "{blocked}");
                assert!(state.last_commit_feedback.is_some(), "{blocked}");
                assert!(state.engine.snapshot().seed_text.is_empty());
            }

            for result in [
                Ok(false),
                Err("disconnected".into()),
                Err("timeout".into()),
                Ok(true),
            ] {
                seed_source(&mut state, source);
                let (sender, receiver) = mpsc::sync_channel(1);
                state.native.sender = Some(sender);
                insert(&mut state, source);
                let request = receiver.try_recv().unwrap();
                assert!(request.command.ends_with(" 10 Thello world"));
                assert_source_retained(&state, source);
                // A double click must neither enqueue nor clear the pending source.
                insert(&mut state, source);
                assert!(receiver.try_recv().is_err());
                assert_source_retained(&state, source);
                state.native_action_finished(request.host.clone(), request.revision + 1, Ok(true));
                assert_source_retained(&state, source);
                if result == Ok(true) {
                    let mut frame = state.native.frame.clone().unwrap();
                    frame.revision += 1;
                    frame.seed = "hello world".into();
                    state.receive_native_frame(Some(frame));
                    assert_source_retained(&state, source);
                }
                state.native_action_finished(request.host, request.revision, result.clone());
                assert!(state.native.pending.is_none());
                if result == Ok(true) {
                    assert_source_cleared(&state, source);
                    assert_eq!(state.chrome.active_input_mode, InputMode::VirtualKeyboard);
                    assert_eq!(state.chrome.seed_text, "hello world");
                } else {
                    assert_source_retained(&state, source);
                    assert!(
                        state
                            .last_commit_feedback
                            .as_ref()
                            .unwrap()
                            .contains("retained")
                    );
                    assert!(receiver.try_recv().is_err(), "never replay a failed action");
                }
            }

            seed_source(&mut state, source);
            let (sender, receiver) = mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            insert(&mut state, source);
            let request = receiver.try_recv().unwrap();
            // Later work must survive an earlier successful acknowledgement.
            match source {
                InputMode::Dictation => state.chrome.voice_transcript = "new transcript".into(),
                InputMode::Handwriting => state.chrome.handwriting_strokes.push(vec![[3.0, 4.0]]),
                _ => unreachable!(),
            }
            state.native_action_finished(request.host, request.revision, Ok(true));
            assert_eq!(state.chrome.active_input_mode, source);
            assert!(state.chrome.input_modes_expanded);
            match source {
                InputMode::Dictation => assert_eq!(state.chrome.voice_transcript, "new transcript"),
                InputMode::Handwriting => assert_eq!(state.chrome.handwriting_strokes.len(), 2),
                _ => unreachable!(),
            }

            // Even identical content belongs to a new draft after Clear/restart.
            seed_source(&mut state, source);
            let (sender, receiver) = mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            insert(&mut state, source);
            let request = receiver.try_recv().unwrap();
            match source {
                InputMode::Dictation => {
                    state.clear_voice_transcript();
                    state.chrome.voice_transcript = "world".into();
                }
                InputMode::Handwriting => {
                    let strokes = state.chrome.handwriting_strokes.clone();
                    state.clear_handwriting();
                    state.chrome.handwriting_strokes = strokes;
                    state.chrome.handwriting_candidates = vec!["world".into()];
                }
                _ => unreachable!(),
            }
            state.native_action_finished(request.host, request.revision, Ok(true));
            assert_source_retained(&state, source);

            seed_source(&mut state, source);
            let (sender, receiver) = mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            insert(&mut state, source);
            let request = receiver.try_recv().unwrap();
            let other_mode = if source == InputMode::Dictation {
                InputMode::Handwriting
            } else {
                InputMode::Dictation
            };
            state.chrome.active_input_mode = other_mode;
            state.native_action_finished(request.host, request.revision, Ok(true));
            assert_source_cleared(&state, source);
            assert_eq!(state.chrome.active_input_mode, other_mode);
            assert!(state.chrome.input_modes_expanded);

            // A disconnect/context change followed by rejection retains the input without replay.
            seed_source(&mut state, source);
            let (sender, receiver) = mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            insert(&mut state, source);
            let request = receiver.try_recv().unwrap();
            state.receive_native_frame(None);
            state.native_action_finished(request.host, request.revision, Ok(false));
            assert_source_retained(&state, source);
            assert!(receiver.try_recv().is_err());

            // The standalone editor still inserts locally and cleans up immediately.
            seed_source(&mut state, source);
            state.native = Default::default();
            insert(&mut state, source);
            assert_eq!(state.engine.snapshot().seed_text, "hello world");
            assert_source_cleared(&state, source);
        }
        state.voice.bridge = voice_bridge;
        assert_consume_once_voice(&mut state);
        println!(
            "PASS: voice and handwriting drafts survive blocked, busy, stale, rejected and timed-out handoffs; only acknowledged, unchanged drafts are cleared"
        );
        self.completed = true;
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

fn keyboard_frame(seed: &str, revision: u64) -> NativeComposition {
    use suzaku_map::ime::{EngineConfig, XRTabletImeEngine, companion::NativeCandidate};
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    NativeComposition {
        host: "11111111-1111-1111-1111-111111111111".into(),
        context: 1,
        revision,
        focused: true,
        private: false,
        language: "en".into(),
        seed: seed.into(),
        selected: 0,
        candidates: engine
            .candidates()
            .iter()
            .map(|candidate| NativeCandidate {
                text: candidate.text.clone(),
                label: candidate.label.clone(),
                kind: candidate.kind,
                source: candidate.source,
                weight: candidate.score.round() as u8,
            })
            .collect(),
    }
}

fn prepare_keyboard(state: &mut PanelState) -> mpsc::Receiver<ActionRequest> {
    state.native = Default::default();
    state.is_focused = false;
    state.chrome = PanelChromeState::default();
    state.chrome.llm_enabled = false;
    state.clear_pressed_interaction();
    state.interaction.touch_tap_pending = false;
    state.receive_native_frame(Some(keyboard_frame("hel", 10)));
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.chrome.input_modes_expanded = true;
    state.last_scene = None;
    let (sender, receiver) = mpsc::sync_channel(8);
    state.native.sender = Some(sender);
    receiver
}

fn point_key(state: &mut PanelState, ch: char) {
    use suzaku_map::ime::gpu::{InteractionKind, VirtualKeyboardKey};
    let key = InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character(ch));
    let rect = state.interaction_rect(key).unwrap();
    state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
}

fn assert_screen_keyboard_handoffs(state: &mut PanelState) {
    for frame_first in [true, false] {
        let receiver = prepare_keyboard(state);
        point_key(state, 'l');
        state.select_at_cursor();
        let first = receiver.try_recv().unwrap();
        assert!(first.command.ends_with(" 10 Thell"));
        point_key(state, 'o');
        state.select_at_cursor();
        assert_eq!(state.chrome.seed_text, "hello");
        assert!(state.chrome.sentence_candidates.is_empty());
        assert!(receiver.try_recv().is_err(), "one write at a time");
        // Neither a stale reply nor another action may consume the retained draft.
        state.native_action_finished(first.host.clone(), 9, Ok(true));
        state.native_action(NativeOperation::Clear);
        assert_eq!(state.native.typing.as_ref().unwrap().draft, "hello");
        assert!(receiver.try_recv().is_err());
        if frame_first {
            state.receive_native_frame(Some(keyboard_frame("hell", 11)));
            assert!(receiver.try_recv().is_err());
        }
        state.native_action_finished(first.host, first.revision, Ok(true));
        if !frame_first {
            assert!(
                receiver.try_recv().is_err(),
                "ack alone cannot advance the revision"
            );
            state.receive_native_frame(Some(keyboard_frame("hell", 11)));
        }
        let second = receiver.try_recv().unwrap();
        assert!(second.command.ends_with(" 11 Thello"));
        state.native_action_finished(second.host, second.revision, Ok(true));
        state.receive_native_frame(Some(keyboard_frame("hello", 12)));
        assert!(state.native.typing.is_none());
        assert_eq!(state.chrome.seed_text, "hello");
        assert!(!state.chrome.sentence_candidates.is_empty());
        assert!(receiver.try_recv().is_err());
    }

    // Backspace/Unicode edits coalesce too, including an entirely empty draft.
    let receiver = prepare_keyboard(state);
    state.native_keyboard_edit(Some("日本😀"));
    let first = receiver.try_recv().unwrap();
    state.backspace_seed();
    assert_eq!(state.chrome.seed_text, "hel日本");
    state.native_action_finished(first.host, first.revision, Ok(true));
    state.receive_native_frame(Some(keyboard_frame("hel日本😀", 11)));
    let second = receiver.try_recv().unwrap();
    assert!(second.command.ends_with(" 11 Thel日本"));
    state.native_action_finished(second.host, second.revision, Ok(true));
    state.receive_native_frame(Some(keyboard_frame("hel日本", 12)));
    for _ in 0..5 {
        state.backspace_seed();
    }
    assert_eq!(state.chrome.seed_text, "");
    assert_eq!(state.view_snapshot().mode, Mode::Idle);
    let deletion = receiver.try_recv().unwrap();
    state.native_action_finished(deletion.host, deletion.revision, Ok(true));
    state.receive_native_frame(Some(keyboard_frame("hel日", 13)));
    let empty = receiver.try_recv().unwrap();
    assert!(empty.command.ends_with(" 13 T"));
    state.native_action_finished(empty.host, empty.revision, Ok(true));
    state.receive_native_frame(Some(keyboard_frame("", 14)));
    assert!(state.native.typing.is_none());

    for result in [Ok(false), Err("timeout".into()), Err("disconnected".into())] {
        let receiver = prepare_keyboard(state);
        state.native_keyboard_edit(Some("l"));
        let first = receiver.try_recv().unwrap();
        state.native_keyboard_edit(Some("o"));
        state.native_action_finished(first.host, first.revision, result);
        state.receive_native_frame(Some(keyboard_frame("hell", 11)));
        state.native_keyboard_edit(Some("!"));
        assert!(
            receiver.try_recv().is_err(),
            "never replay uncertain writes"
        );
        assert_eq!(state.take_native_typing_draft().as_deref(), Some("hello!"));
    }
    let receiver = prepare_keyboard(state);
    state.native_keyboard_edit(Some("l"));
    let first = receiver.try_recv().unwrap();
    state.native_keyboard_edit(Some("o"));
    state.receive_native_frame(Some(keyboard_frame("physical edit", 11)));
    state.native_action_finished(first.host, first.revision, Ok(true));
    assert!(
        receiver.try_recv().is_err(),
        "never overwrite physical edits"
    );
    assert_eq!(state.take_native_typing_draft().as_deref(), Some("hello"));

    let receiver = prepare_keyboard(state);
    state.native_keyboard_edit(Some("l"));
    let first = receiver.try_recv().unwrap();
    state.native_keyboard_edit(Some("o"));
    state.receive_native_frame(None);
    state.native_action_finished(first.host, first.revision, Err("disconnected".into()));
    state.receive_native_frame(Some(keyboard_frame("hell", 11)));
    assert!(receiver.try_recv().is_err());
    state.begin_text_editing(); // Explicit recovery edits locally, never replays to the host.
    assert!(!state.native.showing);
    assert_eq!(state.chrome.seed_text, "hello");
    assert_eq!(state.engine.snapshot().seed_text, "hello");
    assert!(receiver.try_recv().is_err());

    for change in ["context", "host", "private", "focus", "language"] {
        let receiver = prepare_keyboard(state);
        state.native_keyboard_edit(Some("l"));
        let first = receiver.try_recv().unwrap();
        state.native_keyboard_edit(Some("o"));
        let mut next = keyboard_frame("", 11);
        match change {
            "context" => next.context += 1,
            "host" => next.host = "22222222-2222-2222-2222-222222222222".into(),
            "private" => next.private = true,
            "focus" => next.focused = false,
            "language" => next.language = "ja".into(),
            _ => unreachable!(),
        }
        state.receive_native_frame(Some(next));
        state.native_action_finished(first.host, first.revision, Ok(true));
        assert!(state.native.typing.is_none(), "{change}");
        assert!(
            receiver.try_recv().is_err(),
            "must not replay across {change}"
        );
    }

    // A metadata refresh may invalidate candidate presses, but not a stable key.
    for touch in [false, true] {
        for new_context in [false, true] {
            let receiver = prepare_keyboard(state);
            point_key(state, 'l');
            state.begin_primary_press(touch);
            assert!(state.interaction.pressed_interaction.is_some());
            let mut next = keyboard_frame("hel", 11);
            next.candidates[1].weight = next.candidates[1].weight.saturating_add(1);
            if new_context {
                next.context += 1;
            }
            state.receive_native_frame(Some(next));
            state.complete_primary_release(touch);
            if new_context {
                assert!(receiver.try_recv().is_err());
            } else {
                assert!(receiver.try_recv().unwrap().command.ends_with(" 11 Thell"));
            }
        }
    }
    state.native = Default::default();
    println!(
        "PASS: screen-keyboard edits survive delayed replies and candidate refreshes; uncertain writes retain recovery text and never replay across fields"
    );
}

fn assert_consume_once_voice(state: &mut PanelState) {
    assert_eq!(
        std::env::var("SUZAKU_LINUX_VOICE_FORCE_READY").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::var("SUZAKU_LINUX_VOICE_SAMPLE").as_deref(),
        Ok("hello world")
    );
    assert!(!state.voice.bridge.as_ref().unwrap().supports_live_capture());
    state.native = Default::default();
    state.chrome = PanelChromeState::default();
    state.chrome.active_input_mode = InputMode::Dictation;
    state.engine.seed("");
    state.start_voice_capture();
    assert_eq!(state.chrome.voice_state, VoiceCaptureState::Listening);
    let now = Instant::now();
    state.poll_voice_bridge_at(now);
    assert_eq!(state.chrome.voice_transcript, "hello world");
    assert!(
        state
            .voice
            .bridge
            .as_ref()
            .unwrap()
            .poll_transcript()
            .is_none(),
        "Linux consumes each update once"
    );
    for ms in [1, 16, 48, 500, 899] {
        state.poll_voice_bridge_at(now + Duration::from_millis(ms));
        assert!(
            state.engine.snapshot().seed_text.is_empty(),
            "not stable yet"
        );
    }
    state.poll_voice_bridge_at(now + Duration::from_millis(900));
    assert_eq!(state.engine.snapshot().seed_text, "hello world");
    assert!(state.chrome.voice_transcript.is_empty());
    assert_eq!(state.chrome.voice_state, VoiceCaptureState::Idle);
    state.poll_voice_bridge_at(now + Duration::from_secs(2));
    assert_eq!(
        state.engine.snapshot().seed_text,
        "hello world",
        "insert exactly once"
    );

    // A later partial resets the timer, even though intermediate polls return None.
    state.chrome.active_input_mode = InputMode::Dictation;
    state.chrome.set_seed_text(String::new());
    state.engine.seed("");
    state.start_voice_capture();
    state.poll_voice_bridge_at(now);
    unsafe extern "C" {
        fn suzaku_linux_speech_debug_seed_transcript(text: *const std::ffi::c_char);
    }
    unsafe {
        suzaku_linux_speech_debug_seed_transcript(c"hello world again".as_ptr());
    }
    state.poll_voice_bridge_at(now + Duration::from_millis(800));
    state.poll_voice_bridge_at(now + Duration::from_millis(1699));
    assert!(state.engine.snapshot().seed_text.is_empty());
    state.poll_voice_bridge_at(now + Duration::from_millis(1700));
    assert_eq!(state.engine.snapshot().seed_text, "hello world again");

    // A failed native auto-insert must not become an automatic retry after recovery.
    seed_source(state, InputMode::Dictation);
    state.start_voice_capture();
    state.poll_voice_bridge_at(now);
    state.poll_voice_bridge_at(now + Duration::from_millis(900));
    assert_eq!(state.chrome.voice_transcript, "hello world");
    assert_eq!(state.chrome.voice_state, VoiceCaptureState::Idle);
    let (sender, receiver) = mpsc::sync_channel(1);
    state.native.sender = Some(sender);
    state.poll_voice_bridge_at(now + Duration::from_secs(3));
    assert!(receiver.try_recv().is_err());
    state.insert_voice_transcript();
    let request = receiver.try_recv().unwrap();
    state.native_action_finished(request.host, request.revision, Ok(true));
    assert!(state.chrome.voice_transcript.is_empty());

    // Disabling automatic insertion retains the captured text for manual use.
    state.chrome.active_input_mode = InputMode::Dictation;
    state.chrome.voice_auto_insert = false;
    state.start_voice_capture();
    state.poll_voice_bridge_at(now);
    state.poll_voice_bridge_at(now + Duration::from_secs(3));
    assert_eq!(state.chrome.voice_state, VoiceCaptureState::Listening);
    assert_eq!(state.chrome.voice_transcript, "hello world");
    assert!(receiver.try_recv().is_err());
    // Losing the bridge stops capture and retains the draft, regardless of elapsed time.
    let bridge = state.voice.bridge.take();
    state.poll_voice_bridge_at(now + Duration::from_secs(4));
    assert_eq!(state.chrome.voice_state, VoiceCaptureState::Idle);
    assert_eq!(state.chrome.voice_transcript, "hello world");
    state.voice.bridge = bridge;
    state.stop_voice_capture();
    state.clear_voice_transcript();
    // A stale backend update cannot refill an explicitly cleared, idle transcript.
    unsafe {
        suzaku_linux_speech_debug_seed_transcript(c"stale transcript".as_ptr());
    }
    state.poll_voice_bridge_at(now + Duration::from_secs(5));
    assert!(state.chrome.voice_transcript.is_empty());
    unsafe {
        suzaku_linux_speech_debug_seed_transcript(c"".as_ptr());
    }
    println!(
        "PASS: consume-once voice capture waits 900 ms, resets on partial updates, inserts once, respects opt-out and never retries rejected handoffs"
    );
}

fn seed_source(state: &mut PanelState, source: InputMode) {
    state.native = Default::default();
    state.is_focused = false;
    state.chrome = PanelChromeState::default();
    state.last_commit_feedback = None;
    state.engine.seed("");
    state.receive_native_frame(Some(NativeComposition {
        host: "00000000-0000-0000-0000-000000000001".into(),
        context: 1,
        revision: 10,
        focused: true,
        private: false,
        language: "en".into(),
        seed: "hello".into(),
        selected: 0,
        candidates: vec![],
    }));
    state.chrome.active_input_mode = source;
    state.chrome.input_modes_expanded = true;
    state.chrome.voice_transcript = "world".into();
    state.chrome.voice_state = VoiceCaptureState::Idle;
    state.chrome.handwriting_strokes = vec![vec![[1.0, 2.0], [2.0, 3.0]]];
    state.chrome.handwriting_candidates = vec!["world".into()];
}

fn insert(state: &mut PanelState, source: InputMode) {
    match source {
        InputMode::Dictation => state.insert_voice_transcript(),
        InputMode::Handwriting => state.insert_handwriting_candidate(0),
        _ => unreachable!(),
    }
}

fn assert_source_retained(state: &PanelState, source: InputMode) {
    assert_eq!(state.chrome.active_input_mode, source);
    assert!(state.chrome.input_modes_expanded);
    match source {
        InputMode::Dictation => assert_eq!(state.chrome.voice_transcript, "world"),
        InputMode::Handwriting => {
            assert_eq!(
                state.chrome.handwriting_strokes,
                [vec![[1.0, 2.0], [2.0, 3.0]]]
            );
            assert_eq!(state.chrome.handwriting_candidates, ["world"]);
        }
        _ => unreachable!(),
    }
}

fn assert_source_cleared(state: &PanelState, source: InputMode) {
    match source {
        InputMode::Dictation => assert!(state.chrome.voice_transcript.is_empty()),
        InputMode::Handwriting => {
            assert!(state.chrome.handwriting_strokes.is_empty());
            assert!(state.chrome.handwriting_candidates.is_empty());
        }
        _ => unreachable!(),
    }
}
