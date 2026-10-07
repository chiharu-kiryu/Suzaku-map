//! Regression coverage for F32/F33 source-to-draft boundaries (N05/N06/N23).
//! Uses private Xvfb, the Linux simulated speech bridge and an in-memory action queue.
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

fn run_probe(check: fn(&mut PanelState) -> Vec<String>) {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("SUZAKU_LINUX_VOICE_FORCE_READY").as_deref(),
        Ok("1")
    );
    assert_eq!(
        std::env::var("SUZAKU_LINUX_VOICE_SAMPLE").as_deref(),
        Ok("hello world")
    );
    for key in [
        "XDG_RUNTIME_DIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "SUZAKU_IME_CONFIG",
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(key).expect("private test path"));
        assert!(
            path.starts_with(std::env::temp_dir()),
            "private path required: {key}"
        );
    }
    assert_ne!(
        std::env::var("DISPLAY").unwrap().split('.').next().unwrap(),
        ":0"
    );
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    struct Probe {
        check: fn(&mut PanelState) -> Vec<String>,
        findings: Option<Vec<String>>,
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
            let mut state = pollster::block_on(PanelState::new(window)).unwrap();
            assert!(!state.voice.bridge.as_ref().unwrap().supports_live_capture());
            assert!(
                state.runs_without_window_focus,
                "exercise the Linux no-focus companion"
            );
            state.chrome.llm_enabled = false;
            state.engine.configure_prediction(None);
            self.findings = Some((self.check)(&mut state));
            state.stop_voice_capture();
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = Probe {
        check,
        findings: None,
    };
    events.run_app(&mut probe).unwrap();
    let findings = probe.findings.expect("diagnostic probe completed");
    for finding in &findings {
        eprintln!("AUDIT: {finding}");
    }
    assert!(
        findings.is_empty(),
        "{} source-to-draft boundary failures",
        findings.len()
    );
}

fn frame(seed: &str, context: u64, revision: u64) -> NativeComposition {
    NativeComposition {
        host: "00000000-0000-0000-0000-000000000001".into(),
        context,
        revision,
        focused: true,
        private: false,
        cursor: None,
        language: "en".into(),
        seed: seed.into(),
        selected: 0,
        candidates: vec![],
    }
}

fn reset(state: &mut PanelState) {
    state.stop_voice_capture();
    state.clear_voice_transcript();
    state.native = NativeView::default();
    state.chrome = PanelChromeState {
        llm_enabled: false,
        input_focused: false,
        ..Default::default()
    };
    state.engine.configure_prediction(None);
    state.engine.clear_session_context();
    state.is_focused = false;
    state.last_scene = None;
}

#[test]
#[ignore = "requires private Xvfb and SUZAKU_PANEL_NATIVE_QA=1; run by test-linux-ci.sh ui"]
fn tool_adoption_gestures_must_not_cross_native_targets() {
    use suzaku_map::ime::gpu::InteractionKind;
    run_probe(|state| {
        let mut findings = Vec::new();
        for mode in [InputMode::Dictation, InputMode::Handwriting] {
            for touch in [false, true] {
                for case in [
                    "same target",
                    "metadata refresh",
                    "same-field edit",
                    "new context",
                    "host restart",
                    "language",
                    "disconnect then return",
                    "private then return",
                    "unfocused then return",
                    "context away and back",
                    "settings during boundary",
                    "starts private",
                    "starts unfocused",
                    "starts disconnected",
                    "local to native",
                    "local stays local",
                ] {
                    reset(state);
                    state.cancel_primary_interaction();
                    state.last_interaction_action = None;
                    let initial = frame("alpha", 1, 10);
                    state.receive_native_frame(Some(initial.clone()));
                    assert!(
                        state.native.showing,
                        "native fixture did not enter its view"
                    );
                    assert_eq!(state.chrome.seed_text, initial.seed);
                    if case.starts_with("starts") {
                        let mut unavailable = initial.clone();
                        unavailable.revision += 1;
                        unavailable.seed.clear();
                        unavailable.private = case == "starts private";
                        unavailable.focused = case != "starts unfocused";
                        state.receive_native_frame(
                            (case != "starts disconnected").then_some(unavailable),
                        );
                    }
                    if case.starts_with("local") {
                        state.leave_native_view();
                        state.chrome.set_seed_text("local".into());
                        state.chrome.move_caret_to_end();
                        state.refresh_seed();
                    }
                    state.chrome.active_input_mode = mode;
                    state.chrome.input_modes_expanded = true;
                    state.chrome.voice_transcript = "world".into();
                    state.chrome.handwriting_strokes = vec![vec![[1.0, 2.0], [2.0, 3.0]]];
                    state.chrome.handwriting_candidates = vec!["world".into()];
                    let (sender, receiver) = mpsc::sync_channel(4);
                    state.native.sender = Some(sender);
                    let kind = match mode {
                        InputMode::Dictation => InteractionKind::InsertVoiceTranscript,
                        InputMode::Handwriting => InteractionKind::UseHandwritingCandidate(0),
                        _ => unreachable!(),
                    };
                    point_tool(state, kind);
                    state.interaction.last_input_was_touch = touch;
                    state.begin_primary_press(touch);
                    assert_eq!(state.interaction.pressed_interaction, Some(kind));
                    let mut next = initial.clone();
                    next.revision = 20;
                    match case {
                        "same target" => {}
                        "metadata refresh"
                        | "local to native"
                        | "starts private"
                        | "starts unfocused"
                        | "starts disconnected" => state.receive_native_frame(Some(next)),
                        "same-field edit" => {
                            next.seed = "beta".into();
                            state.receive_native_frame(Some(next));
                        }
                        "new context" | "settings during boundary" => {
                            next.context = 2;
                            next.seed = "beta".into();
                            state.chrome.settings_open = case == "settings during boundary";
                            state.receive_native_frame(Some(next.clone()));
                            state.chrome.settings_open = false;
                            if case == "settings during boundary" {
                                next.revision += 1;
                                state.receive_native_frame(Some(next));
                            }
                        }
                        "host restart" => {
                            next.host = "00000000-0000-0000-0000-000000000002".into();
                            next.revision = 1;
                            state.receive_native_frame(Some(next));
                        }
                        "language" => {
                            next.language = "ja".into();
                            state.receive_native_frame(Some(next));
                        }
                        "disconnect then return" => {
                            state.receive_native_frame(None);
                            state.receive_native_frame(Some(next));
                        }
                        "private then return"
                        | "unfocused then return"
                        | "context away and back" => {
                            let mut intermediate = next.clone();
                            intermediate.seed.clear();
                            intermediate.private = case == "private then return";
                            intermediate.focused = case != "unfocused then return";
                            if case == "context away and back" {
                                intermediate.context = 2;
                            }
                            state.receive_native_frame(Some(intermediate));
                            next.revision += 1;
                            state.receive_native_frame(Some(next));
                        }
                        "local stays local" => {
                            state.is_focused = true;
                            state.receive_native_frame(Some(next));
                        }
                        _ => unreachable!(),
                    }
                    let same_target =
                        matches!(case, "same target" | "metadata refresh" | "same-field edit");
                    let local = case == "local stays local";
                    let canceled = !same_target && !local;
                    // Layout may shift when entering native view; cancellation
                    // must come from target identity, not an accidental miss.
                    if canceled && state.interaction.pressed_interaction.is_some() {
                        findings.push(format!(
                            "N23 {mode:?}, touch={touch}, {case}: old press survived"
                        ));
                    }
                    // A frame refresh can invalidate layout measurements, but
                    // cannot recreate a gesture canceled by a target boundary.
                    super::tests::present_test_frame(state);
                    state.complete_primary_release(touch);
                    if same_target {
                        let request = receiver
                            .try_recv()
                            .expect("stable tool adoption must still work");
                        let seed = if case == "same-field edit" {
                            "beta"
                        } else {
                            "alpha"
                        };
                        assert!(request.command.ends_with(&format!("T{seed} world")));
                        assert!(receiver.try_recv().is_err());
                    } else if local {
                        assert_eq!(state.chrome.seed_text, "local world");
                        assert!(receiver.try_recv().is_err());
                    } else {
                        if let Ok(request) = receiver.try_recv() {
                            findings.push(format!(
                                "N23 {mode:?}, touch={touch}, {case}: stale release sent {}",
                                request.command
                            ));
                            // Keep probing independent failures without a pending action.
                            state.native_action_finished(request.host, request.revision, Ok(false));
                        }
                        assert_eq!(state.chrome.voice_transcript, "world");
                        assert_eq!(state.chrome.handwriting_candidates, ["world"]);
                        assert!(!state.chrome.handwriting_strokes.is_empty());
                        // A fresh gesture is an explicit choice of the now-visible target.
                        state.last_interaction_action = None;
                        state.chrome.input_modes_expanded = true;
                        state.last_scene = None;
                        point_tool(state, kind);
                        state.begin_primary_press(touch);
                        state.complete_primary_release(touch);
                        assert!(
                            receiver.try_recv().is_ok(),
                            "explicit retry: {mode:?}, {case}"
                        );
                        assert!(receiver.try_recv().is_err());
                    }
                    state.complete_primary_release(touch);
                    assert!(
                        receiver.try_recv().is_err(),
                        "duplicate release must not insert again"
                    );
                }
            }
        }
        findings
    });
}

fn point_tool(state: &mut PanelState, kind: suzaku_map::ime::gpu::InteractionKind) {
    super::tests::present_test_frame(state);
    let scene = state.last_scene.as_ref().unwrap();
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == kind)
        .expect("visible tool control")
        .rect;
    let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(point.0, point.1), Some(kind));
    state.cursor_position = Some(point);
}

#[test]
#[ignore = "requires private Xvfb and SUZAKU_PANEL_NATIVE_QA=1; run by test-linux-ci.sh ui"]
fn audit_voice_auto_insert_must_not_follow_an_unrelated_native_context() {
    run_probe(|state| {
        let mut findings = Vec::new();
        for case in [
            "unchanged target",
            "candidate refresh",
            "new context",
            "host restart",
            "disconnect then new context",
            "disconnect then same context",
            "switch away and back",
            "native to local",
            "unfocused context",
            "private context",
            "private then return",
            "auto insert disabled",
            "auto insert disabled unchanged",
            "explicit manual retarget",
        ] {
            reset(state);
            state.receive_native_frame(Some(frame("alpha draft", 1, 10)));
            state.chrome.active_input_mode = InputMode::Dictation;
            state.chrome.input_modes_expanded = true;
            let (sender, receiver) = mpsc::sync_channel(1);
            state.native.sender = Some(sender);
            state.start_voice_capture();
            assert_eq!(state.chrome.voice_state, VoiceCaptureState::Listening);
            let now = Instant::now();
            state.poll_voice_bridge_at(now);
            state.poll_voice_bridge_at(now + Duration::from_millis(899));
            assert!(receiver.try_recv().is_err(), "quiet period: {case}");
            assert_eq!(state.chrome.voice_transcript, "hello world");

            match case {
                "unchanged target" => {}
                "candidate refresh" => {
                    state.receive_native_frame(Some(frame("alpha draft", 1, 11)))
                }
                "host restart" => {
                    let mut next = frame("beta draft", 2, 1);
                    next.host = "00000000-0000-0000-0000-000000000002".into();
                    state.receive_native_frame(Some(next));
                }
                "disconnect then new context" => {
                    state.receive_native_frame(None);
                    state.receive_native_frame(Some(frame("beta draft", 2, 11)));
                }
                "disconnect then same context" => {
                    state.receive_native_frame(None);
                    state.receive_native_frame(Some(frame("alpha draft", 1, 11)));
                }
                "switch away and back" => {
                    state.receive_native_frame(Some(frame("beta draft", 2, 11)));
                    state.receive_native_frame(Some(frame("alpha draft", 1, 12)));
                }
                "native to local" => state.leave_native_view(),
                "unfocused context" => {
                    let mut next = frame("alpha draft", 1, 11);
                    next.focused = false;
                    state.receive_native_frame(Some(next));
                }
                "private context" | "private then return" => {
                    let mut next = frame("", 2, 11);
                    next.private = true;
                    state.receive_native_frame(Some(next));
                    if case == "private then return" {
                        state.receive_native_frame(Some(frame("alpha draft", 1, 12)));
                    }
                }
                "auto insert disabled" | "explicit manual retarget" => {
                    state.chrome.voice_auto_insert = false;
                    state.receive_native_frame(Some(frame("beta draft", 2, 11)));
                }
                "new context" => state.receive_native_frame(Some(frame("beta draft", 2, 11))),
                "auto insert disabled unchanged" => state.chrome.voice_auto_insert = false,
                _ => unreachable!(),
            }
            state.poll_voice_bridge_at(now + Duration::from_millis(900));
            if case == "explicit manual retarget" {
                assert!(receiver.try_recv().is_err());
                state.insert_voice_transcript();
            }
            let request = receiver.try_recv().ok();
            if matches!(
                case,
                "unchanged target" | "candidate refresh" | "explicit manual retarget"
            ) {
                let request = request.expect("allowed insertion should be queued");
                let current = state.native.frame.as_ref().unwrap();
                let expected = suzaku_map::platform::linux_ime_sync::action_command(
                    current,
                    &NativeOperation::Replace(format!("{} hello world", current.seed)),
                )
                .unwrap();
                assert_eq!(request.command, expected, "{case}");
                state.native_action_finished(request.host, request.revision, Ok(true));
                assert!(state.chrome.voice_transcript.is_empty());
                state.poll_voice_bridge_at(now + Duration::from_secs(3));
                assert!(receiver.try_recv().is_err(), "no replay: {case}");
                println!("PASS: voice control {case}");
            } else {
                if let Some(request) = request {
                    findings.push(format!(
                        "N05 {case}: capture started in context 1, but queued replacement for context {}: {:?}",
                        state.native.frame.as_ref().unwrap().context, request.command,
                    ));
                } else {
                    println!("PASS: voice control {case} retains source without sending");
                }
                assert_eq!(state.chrome.voice_transcript, "hello world", "{case}");
                assert_eq!(
                    state.chrome.voice_state,
                    if case == "auto insert disabled unchanged" {
                        VoiceCaptureState::Listening
                    } else {
                        VoiceCaptureState::Idle
                    },
                    "a changed target stops capture without discarding the source: {case}"
                );
                state.poll_voice_bridge_at(now + Duration::from_secs(3));
                assert!(receiver.try_recv().is_err(), "never replay: {case}");
            }
            assert!(
                state.engine.snapshot().committed_text.is_empty(),
                "never commit: {case}"
            );
        }
        // A capture can switch to the native view even before its first poll.
        // Preserve that queued partial, but do not send it into the new target.
        for local_origin in [false, true] {
            for poll_before_switch in [false, true] {
                reset(state);
                if local_origin {
                    state.chrome.set_seed_text("local draft".into());
                    state.refresh_seed();
                } else {
                    state.receive_native_frame(Some(frame("alpha draft", 1, 10)));
                }
                state.chrome.active_input_mode = InputMode::Dictation;
                let (sender, receiver) = mpsc::sync_channel(1);
                state.native.sender = Some(sender);
                state.start_voice_capture();
                let now = Instant::now();
                if poll_before_switch {
                    state.poll_voice_bridge_at(now);
                }
                state.receive_native_frame(Some(frame("beta draft", 2, 11)));
                assert_eq!(state.chrome.voice_state, VoiceCaptureState::Idle);
                assert_eq!(state.chrome.voice_transcript, "hello world");
                state.poll_voice_bridge_at(now + Duration::from_secs(3));
                assert!(receiver.try_recv().is_err());
                assert_eq!(state.view_snapshot().seed_text, "beta draft");
                // The stopped source remains available for deliberate adoption.
                state.insert_voice_transcript();
                let request = receiver.try_recv().expect("explicit retarget after pause");
                assert!(request.command.ends_with(" 11 Tbeta draft hello world"));
                state.native_action_finished(request.host, request.revision, Ok(true));
                assert!(state.chrome.voice_transcript.is_empty());
                println!(
                    "PASS: local_origin={local_origin}, poll_before_switch={poll_before_switch}: pause retains partial; manual adoption works"
                );
            }
        }
        findings
    });
}

#[test]
#[ignore = "requires private Xvfb and SUZAKU_PANEL_NATIVE_QA=1; run by test-linux-ci.sh ui"]
fn audit_local_tool_insertion_must_use_the_caret_word_boundary() {
    run_probe(|state| {
        let mut findings = Vec::new();
        for mode in [InputMode::Dictation, InputMode::Handwriting] {
            for (seed, caret, expected) in [
                ("", 0, "world"),
                ("hello", 5, "hello world"),
                ("hello ", 6, "hello world"),
                ("hello there ", 5, "hello world there "),
                ("hello there", 6, "hello world there"),
                (" there", 0, "world there"),
                ("there", 0, "world there"),
                ("hellothere", 5, "hello world there"),
                ("hello, there", 5, "hello world, there"),
                ("hello () there", 7, "hello (world) there"),
                ("hello\u{a0}", 6, "hello\u{a0}world"),
                ("hello\u{3000}", 6, "hello\u{3000}world"),
            ] {
                reset(state);
                state.chrome.set_seed_text(seed.into());
                state.chrome.caret_index = caret;
                state.chrome.active_input_mode = mode;
                state.chrome.input_modes_expanded = true;
                state.refresh_seed();
                state.chrome.voice_transcript = "world".into();
                state.chrome.handwriting_strokes = vec![vec![[1.0, 2.0], [2.0, 3.0]]];
                state.chrome.handwriting_candidates = vec!["world".into()];
                match mode {
                    InputMode::Dictation => state.insert_voice_transcript(),
                    InputMode::Handwriting => state.insert_handwriting_candidate(0),
                    _ => unreachable!(),
                }
                if state.chrome.seed_text != expected {
                    findings.push(format!(
                        "N06 {mode:?}: seed={seed:?}, caret={caret}, expected={expected:?}, actual={:?}",
                        state.chrome.seed_text,
                    ));
                } else {
                    println!("PASS: local {mode:?} seed={seed:?}, caret={caret}");
                }
                let word_start = expected.find("world").unwrap();
                assert_eq!(
                    state.chrome.caret_index,
                    expected[..word_start].chars().count() + "world".len(),
                    "caret stays after the adopted word: {mode:?}, seed={seed:?}"
                );
                assert_eq!(state.engine.snapshot().seed_text, state.chrome.seed_text);
                assert!(
                    state.engine.snapshot().committed_text.is_empty(),
                    "draft only"
                );
            }
            // The native append path already treats these as word separators.
            for separator in ["\u{a0}", "\u{3000}"] {
                reset(state);
                state.receive_native_frame(Some(frame(&format!("hello{separator}"), 1, 10)));
                state.chrome.active_input_mode = mode;
                state.chrome.voice_transcript = "world".into();
                state.chrome.handwriting_strokes = vec![vec![[1.0, 2.0], [2.0, 3.0]]];
                state.chrome.handwriting_candidates = vec!["world".into()];
                let (sender, receiver) = mpsc::sync_channel(1);
                state.native.sender = Some(sender);
                match mode {
                    InputMode::Dictation => state.insert_voice_transcript(),
                    InputMode::Handwriting => state.insert_handwriting_candidate(0),
                    _ => unreachable!(),
                }
                let request = receiver.try_recv().expect("native append control");
                assert!(
                    request
                        .command
                        .ends_with(&format!("Thello{separator}world"))
                );
                state.native_action_finished(request.host, request.revision, Ok(true));
                println!(
                    "PASS: native {mode:?} preserves separator={separator:?} without extra spaces"
                );
            }
        }
        findings
    });
}
