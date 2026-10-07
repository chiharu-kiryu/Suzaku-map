//! Native candidate actions and follow-up keys, without a real host or target app.
use super::tests::{assert_panel_visibility, keyboard_frame, publish_wake_frames};
use super::*;
use crate::{PanelApp, PanelChromeState, PanelWindowKind, panel_window_attributes};
use suzaku_map::ime::gpu::{InputMode, VirtualKeyboardKey};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    platform::x11::EventLoopBuilderExtX11,
    window::WindowId,
};

#[test]
#[ignore = "requires private Xvfb/D-Bus/XDG; run by scripts/test-linux-ci.sh ui"]
fn native_screen_keyboard_preserves_host_editing_semantics() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in ["XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
        assert!(
            std::path::Path::new(&std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir())
        );
    }
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
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
            for operation in ["backspace", "space"] {
                let receiver = prepare(&mut app, events);
                let state = app.panel.as_mut().unwrap();
                let index = state
                    .native
                    .frame
                    .as_ref()
                    .unwrap()
                    .candidates
                    .iter()
                    .position(|candidate| candidate.text == "hello")
                    .unwrap();
                if operation == "backspace" {
                    state.continue_sentence_candidate(index);
                } else {
                    state.native_action(NativeOperation::Select(index));
                }
                let request = receiver.try_recv().unwrap();
                acknowledge(&mut app, events, request);
                let mut applied = keyboard_frame(
                    if operation == "backspace" {
                        "hello"
                    } else {
                        "hel"
                    },
                    11,
                );
                if operation == "space" {
                    applied.selected = index;
                }
                publish_wake_frames(&mut app, events, [Some(applied)]);
                let state = app.panel.as_mut().unwrap();
                state.chrome.focus_input();
                if operation == "backspace" {
                    state.backspace_seed();
                } else {
                    state.handle_text_input(" ");
                }
                let request = receiver.try_recv().unwrap();
                let suffix = if operation == "backspace" {
                    " 11 B"
                } else {
                    " 11 S"
                };
                assert!(
                    request.command.starts_with('E') && request.command.ends_with(suffix),
                    "{operation} must ask the host to resolve undo/explicit selection, not guess a replacement: {}",
                    request.command
                );
                let expected = if operation == "backspace" {
                    "hel"
                } else {
                    "hello "
                };
                finish_keyboard(&mut app, events, request, Ok(keyboard_frame(expected, 12)));
                assert_eq!(app.panel.as_ref().unwrap().chrome.seed_text, expected);
                assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
                assert!(receiver.try_recv().is_err());
            }
            for language in ["en", "zh-Hans"] {
                for frame_first in [false, true] {
                    check_semantic_adoption_barrier(&mut app, events, language, frame_first);
                }
            }
            check_semantic_order(&mut app, events);
            check_semantic_empty_and_compose(&mut app, events);
            check_semantic_tool_barrier(&mut app, events);
            check_semantic_disconnected_recovery(&mut app, events);
            check_semantic_selection_barrier(&mut app, events);
            check_semantic_pending_dock_geometry(&mut app, events);
            check_semantic_boundaries(&mut app, events);
            println!(
                "PASS: host-owned screen Backspace/Space, ordered follow-up edits and bounded no-replay boundaries"
            );
            self.completed = true;
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = Probe {
        proxy: events.create_proxy(),
        completed: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

fn finish_keyboard(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    request: ActionRequest,
    result: Result<NativeComposition, String>,
) {
    app.user_event(
        events,
        PanelUserEvent::NativeKeyboardFinished {
            host: request.host,
            revision: request.revision,
            result,
        },
    );
}

fn dock_keyboard_geometry(state: &PanelState) -> (u32, Vec<(InteractionKind, [f32; 4])>) {
    use suzaku_map::ime::gpu::WgpuCandidateRenderer;
    let width = 1000.0;
    let height = WgpuCandidateRenderer::new(width, 1.0)
        .preferred_input_panel_height(&state.chrome)
        .ceil() as u32;
    let scene = WgpuCandidateRenderer::new(width, height as f32).build_panel_scene(
        &state.view_snapshot(),
        &state.chrome,
        None,
        None,
        None,
        None,
    );
    let origin = [12.0, 768.0 - 12.0 - height as f32];
    let keys = scene
        .interactive_targets
        .iter()
        .filter(|target| matches!(target.kind, InteractionKind::VirtualKeyboardKey(_)))
        .map(|target| {
            let mut rect = target.rect;
            rect[0] += origin[0];
            rect[1] += origin[1];
            (target.kind, rect)
        })
        .collect();
    (height, keys)
}

fn check_semantic_pending_dock_geometry(app: &mut PanelApp, events: &ActiveEventLoop) {
    for backspace in [false, true] {
        let receiver = prepare(app, events);
        let state = app.panel.as_mut().unwrap();
        state.chrome.panel_layout_mode = suzaku_map::ime::gpu::PanelLayoutMode::BottomDock;
        state.chrome.active_input_mode = InputMode::VirtualKeyboard;
        state.chrome.input_modes_expanded = true;
        let labels = state.chrome.sentence_candidates.clone();
        assert!(!labels.is_empty());
        let before = dock_keyboard_geometry(state);
        assert!(!before.1.is_empty());
        if backspace {
            state.backspace_seed();
        } else {
            state.continue_composition_with_space();
        }
        let request = receiver.try_recv().unwrap();
        assert!(request.command.starts_with('E'));
        assert!(state.native.typing.is_none());
        assert!(state.chrome.native_candidate_page.unwrap().busy);
        assert_eq!(
            state.chrome.sentence_candidates, labels,
            "read-only host candidates reserve their existing space while B/S waits"
        );
        assert_eq!(
            dock_keyboard_geometry(state),
            before,
            "pending semantic keys must not shrink/move the bottom keyboard"
        );
        state.native_action(NativeOperation::Select(0));
        state.change_native_candidate_page(true);
        assert!(
            receiver.try_recv().is_err(),
            "placeholder candidates and paging remain unavailable"
        );
        let expected = if backspace { "he" } else { "hel " };
        finish_keyboard(app, events, request, Ok(keyboard_frame(expected, 11)));
        let state = app.panel.as_mut().unwrap();
        assert_eq!(state.chrome.seed_text, expected);
        assert!(!state.native_candidates_busy());
        assert!(!state.chrome.native_candidate_page.unwrap().busy);
        // Ordinary literal text remains a different contract: its optimistic
        // spelling cannot display the old host's candidate list as selectable.
        state.native_keyboard_edit(Some("x"));
        assert!(state.native.typing.is_some());
        assert!(state.chrome.sentence_candidates.is_empty());
        assert!(
            receiver
                .try_recv()
                .unwrap()
                .command
                .ends_with(&format!(" 11 T{expected}x"))
        );
    }
}

fn check_semantic_adoption_barrier(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    language: &str,
    frame_first: bool,
) {
    let receiver = prepare(app, events);
    let (reading, adopted) = if language == "en" {
        ("hel", "hello")
    } else {
        ("nihao", "你好")
    };
    let mut initial = keyboard_frame(reading, 11);
    initial.language = language.into();
    initial.candidates = vec![suzaku_map::ime::companion::NativeCandidate {
        text: adopted.into(),
        label: adopted.into(),
        ..Default::default()
    }];
    publish_wake_frames(app, events, [Some(initial.clone())]);
    let state = app.panel.as_mut().unwrap();
    state.continue_sentence_candidate(0);
    let mut adoption = Some(receiver.try_recv().unwrap());
    state.backspace_seed();
    state.chrome.focus_input();
    state.handle_text_input("x");
    assert!(
        receiver.try_recv().is_err(),
        "semantic key waits for adoption"
    );
    let mut applied = initial.clone();
    applied.revision = 12;
    applied.seed = adopted.into();
    if frame_first {
        publish_wake_frames(app, events, [Some(applied.clone())]);
    } else {
        acknowledge(app, events, adoption.take().unwrap());
    }
    assert!(
        receiver.try_recv().is_err(),
        "one half of the barrier is insufficient"
    );
    if frame_first {
        acknowledge(app, events, adoption.take().unwrap());
    } else {
        publish_wake_frames(app, events, [Some(applied)]);
    }
    let backspace = receiver.try_recv().unwrap();
    assert!(backspace.command.starts_with('E') && backspace.command.ends_with(" 12 B"));
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        app.panel.as_ref().unwrap().chrome.seed_text,
        adopted,
        "do not guess undo or append x before the host response"
    );
    let mut restored = initial;
    restored.revision = 13;
    // Exercise the independently arriving subscription frame as well as the reply.
    if frame_first {
        publish_wake_frames(app, events, [Some(restored.clone())]);
    }
    assert!(receiver.try_recv().is_err());
    finish_keyboard(app, events, backspace, Ok(restored));
    let text = receiver.try_recv().unwrap();
    assert!(text.command.ends_with(&format!(" 13 T{reading}x")));
    acknowledge(app, events, text);
    let mut final_frame = keyboard_frame(&format!("{reading}x"), 14);
    final_frame.language = language.into();
    publish_wake_frames(app, events, [Some(final_frame)]);
    assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
    assert!(receiver.try_recv().is_err());
}

fn check_semantic_order(app: &mut PanelApp, events: &ActiveEventLoop) {
    let receiver = prepare(app, events);
    let state = app.panel.as_mut().unwrap();
    state.continue_composition_with_space();
    let space = receiver.try_recv().unwrap();
    assert!(space.command.ends_with(" 10 S"));
    state.backspace_seed();
    state.chrome.focus_input();
    state.handle_text_input("x");
    state.continue_composition_with_space();
    assert_eq!(state.chrome.seed_text, "hel");
    assert!(receiver.try_recv().is_err());
    let duplicate = ActionRequest {
        command: space.command.clone(),
        host: space.host.clone(),
        revision: space.revision,
    };
    finish_keyboard(app, events, space, Ok(keyboard_frame("hel ", 11)));
    let backspace = receiver.try_recv().unwrap();
    assert!(backspace.command.ends_with(" 11 B"));
    finish_keyboard(app, events, duplicate, Ok(keyboard_frame("hel ", 11)));
    assert!(
        receiver.try_recv().is_err(),
        "duplicate reply must not consume the next operation"
    );
    finish_keyboard(app, events, backspace, Ok(keyboard_frame("hel", 12)));
    let text = receiver.try_recv().unwrap();
    assert!(text.command.ends_with(" 12 Thelx"));
    acknowledge(app, events, text);
    assert!(receiver.try_recv().is_err());
    publish_wake_frames(app, events, [Some(keyboard_frame("helx", 13))]);
    let final_space = receiver.try_recv().unwrap();
    assert!(final_space.command.ends_with(" 13 S"));
    finish_keyboard(app, events, final_space, Ok(keyboard_frame("helx ", 14)));
    assert_eq!(app.panel.as_ref().unwrap().chrome.seed_text, "helx ");
    assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
    assert!(receiver.try_recv().is_err());
}

fn check_semantic_tool_barrier(app: &mut PanelApp, events: &ActiveEventLoop) {
    for handwriting in [false, true] {
        for frame_first in [false, true] {
            let receiver = prepare(app, events);
            let state = app.panel.as_mut().unwrap();
            let insertion = if handwriting {
                state.chrome.handwriting_strokes = vec![vec![[1.0, 2.0], [3.0, 4.0]]];
                state.chrome.handwriting_candidates = vec!["hello".into()];
                NativeInsertion::Handwriting {
                    strokes: state.chrome.handwriting_strokes.clone(),
                    candidates: state.chrome.handwriting_candidates.clone(),
                    generation: state.handwriting_generation,
                }
            } else {
                state.chrome.voice_transcript = "hello".into();
                NativeInsertion::Voice {
                    transcript: "hello".into(),
                    generation: state.voice_progress.generation(),
                }
            };
            assert!(state.native_action_with_source(
                NativeOperation::Replace("hello".into()),
                Some(insertion)
            ));
            let mut replacement = Some(receiver.try_recv().unwrap());
            state.backspace_seed();
            assert!(receiver.try_recv().is_err());
            if frame_first {
                publish_wake_frames(app, events, [Some(keyboard_frame("hello", 11))]);
                let state = app.panel.as_ref().unwrap();
                assert!(
                    if handwriting {
                        !state.chrome.handwriting_candidates.is_empty()
                    } else {
                        !state.chrome.voice_transcript.is_empty()
                    },
                    "source persists until ACK, not merely an observed frame"
                );
            } else {
                acknowledge(app, events, replacement.take().unwrap());
            }
            assert!(receiver.try_recv().is_err());
            if frame_first {
                acknowledge(app, events, replacement.take().unwrap());
            } else {
                publish_wake_frames(app, events, [Some(keyboard_frame("hello", 11))]);
            }
            let state = app.panel.as_ref().unwrap();
            assert!(
                if handwriting {
                    state.chrome.handwriting_candidates.is_empty()
                        && state.chrome.handwriting_strokes.is_empty()
                } else {
                    state.chrome.voice_transcript.is_empty()
                },
                "successful source consumed exactly after confirmation"
            );
            let backspace = receiver.try_recv().unwrap();
            assert!(backspace.command.ends_with(" 11 B"));
            finish_keyboard(app, events, backspace, Ok(keyboard_frame("hell", 12)));
            assert_eq!(app.panel.as_ref().unwrap().chrome.seed_text, "hell");
            assert!(receiver.try_recv().is_err());
        }
    }
}

fn check_semantic_empty_and_compose(app: &mut PanelApp, events: &ActiveEventLoop) {
    let receiver = prepare(app, events);
    app.panel.as_mut().unwrap().backspace_seed();
    let cancel_compose = receiver.try_recv().unwrap();
    finish_keyboard(app, events, cancel_compose, Ok(keyboard_frame("hel", 11)));
    assert_eq!(
        app.panel.as_ref().unwrap().chrome.seed_text,
        "hel",
        "host may cancel Compose without deleting a character"
    );
    assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
    publish_wake_frames(app, events, [Some(keyboard_frame("", 12))]);
    let state = app.panel.as_mut().unwrap();
    state.backspace_seed();
    let empty_backspace = receiver.try_recv().unwrap();
    state.continue_composition_with_space();
    assert!(receiver.try_recv().is_err());
    finish_keyboard(app, events, empty_backspace, Ok(keyboard_frame("", 13)));
    let space = receiver.try_recv().unwrap();
    assert!(space.command.ends_with(" 13 S"));
    finish_keyboard(app, events, space, Ok(keyboard_frame(" ", 14)));
    assert_eq!(app.panel.as_ref().unwrap().chrome.seed_text, " ");
    assert_panel_visibility(app, true);
    assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
    assert!(receiver.try_recv().is_err());
}

fn check_semantic_selection_barrier(app: &mut PanelApp, events: &ActiveEventLoop) {
    for selected in [0, 1] {
        for frame_first in [false, true] {
            for changed in [false, true] {
                let receiver = prepare(app, events);
                let state = app.panel.as_mut().unwrap();
                let candidate = state.native.frame.as_ref().unwrap().candidates[selected]
                    .text
                    .clone();
                state.native_action(NativeOperation::Select(selected));
                let mut selection = Some(receiver.try_recv().unwrap());
                state.continue_composition_with_space();
                state.chrome.focus_input();
                state.handle_text_input("x");
                let revision = if changed { 12 } else { 11 };
                let mut frame = keyboard_frame("hel", revision);
                frame.selected = if changed { 1 - selected } else { selected };
                if frame_first {
                    publish_wake_frames(app, events, [Some(frame.clone())]);
                } else {
                    acknowledge(app, events, selection.take().unwrap());
                }
                assert!(
                    receiver.try_recv().is_err(),
                    "both ACK and selection frame are required"
                );
                if frame_first {
                    acknowledge(app, events, selection.take().unwrap());
                } else {
                    publish_wake_frames(app, events, [Some(frame)]);
                }
                if changed {
                    assert!(
                        receiver.try_recv().is_err(),
                        "same text does not authorize Space over a changed selection"
                    );
                    assert_eq!(
                        app.panel
                            .as_mut()
                            .unwrap()
                            .take_native_typing_draft()
                            .as_deref(),
                        Some("helx"),
                        "recover literal text without guessing Space"
                    );
                } else {
                    let space = receiver.try_recv().unwrap();
                    assert!(
                        space.command.ends_with(" 11 S"),
                        "explicit selection, including the current row, permits Space"
                    );
                    finish_keyboard(
                        app,
                        events,
                        space,
                        Ok(keyboard_frame(&format!("{candidate} "), 12)),
                    );
                    let text = receiver.try_recv().unwrap();
                    assert!(text.command.ends_with(&format!(" 12 T{candidate} x")));
                    acknowledge(app, events, text);
                    publish_wake_frames(
                        app,
                        events,
                        [Some(keyboard_frame(&format!("{candidate} x"), 13))],
                    );
                    assert!(!app.panel.as_ref().unwrap().native_candidates_busy());
                }
                assert!(receiver.try_recv().is_err());
            }
        }
    }
}

fn check_semantic_disconnected_recovery(app: &mut PanelApp, events: &ActiveEventLoop) {
    for case in ["initial", "adopted", "second flight", "restored byte limit"] {
        let receiver = prepare(app, events);
        if case == "adopted" {
            let state = app.panel.as_mut().unwrap();
            let index = state
                .native
                .frame
                .as_ref()
                .unwrap()
                .candidates
                .iter()
                .position(|c| c.text == "hello")
                .unwrap();
            state.continue_sentence_candidate(index);
            let adoption = receiver.try_recv().unwrap();
            acknowledge(app, events, adoption);
            publish_wake_frames(app, events, [Some(keyboard_frame("hello", 11))]);
        }
        let state = app.panel.as_mut().unwrap();
        if case == "second flight" {
            state.continue_composition_with_space();
        } else {
            state.backspace_seed();
        }
        let first = receiver.try_recv().unwrap();
        if case == "second flight" {
            state.backspace_seed();
        }
        state.chrome.focus_input();
        state.handle_text_input("x");
        assert!(receiver.try_recv().is_err());
        let (pending, expected) = match case {
            "second flight" => {
                finish_keyboard(app, events, first, Ok(keyboard_frame("hel ", 11)));
                let second = receiver.try_recv().unwrap();
                assert!(second.command.ends_with(" 11 B"));
                (Some(second), "hel x".to_owned())
            }
            "restored byte limit" => {
                let mut reply = keyboard_frame("", 11);
                reply.seed = "z".repeat(suzaku_map::ime::companion::MAX_TEXT_BYTES);
                let expected = format!("{}x", reply.seed);
                finish_keyboard(app, events, first, Ok(reply));
                assert!(receiver.try_recv().is_err());
                (None, expected)
            }
            "adopted" => (Some(first), "hellox".to_owned()),
            _ => (Some(first), "helx".to_owned()),
        };
        publish_wake_frames(app, events, [None]);
        if let Some(pending) = pending {
            finish_keyboard(app, events, pending, Err("disconnected".into()));
        }
        let state = app.panel.as_mut().unwrap();
        assert!(
            state.native.frame.is_none(),
            "recover before any reconnect frame"
        );
        assert_eq!(
            state.take_native_typing_draft().as_deref(),
            Some(expected.as_str()),
            "{case}: retain confirmed prefix without guessing an unconfirmed key"
        );
        assert!(
            receiver.try_recv().is_err(),
            "local recovery must never resend"
        );
    }
}

fn check_semantic_boundaries(app: &mut PanelApp, events: &ActiveEventLoop) {
    for boundary in [
        "newer revision",
        "private",
        "focus",
        "context",
        "language",
        "host",
        "disconnect",
        "failure",
        "settings round trip",
        "panel focus round trip",
    ] {
        let receiver = prepare(app, events);
        let state = app.panel.as_mut().unwrap();
        state.backspace_seed();
        let request = receiver.try_recv().unwrap();
        state.chrome.focus_input();
        state.handle_text_input("x");
        let reply = keyboard_frame("he", 11);
        let mut changed = keyboard_frame("hel", 12);
        match boundary {
            "private" => changed.private = true,
            "focus" => changed.focused = false,
            "context" => changed.context += 1,
            "language" => changed.language = "ja".into(),
            "host" => changed.host = "22222222-2222-2222-2222-222222222222".into(),
            _ => (),
        }
        if boundary == "settings round trip" {
            let state = app.panel.as_mut().unwrap();
            state.suspend_native_keyboard();
            state.chrome.settings_open = true;
            state.chrome.settings_open = false;
        } else if boundary == "panel focus round trip" {
            let state = app.panel.as_mut().unwrap();
            state.set_window_focus(true);
            state.set_window_focus(false);
        }
        if boundary == "disconnect" {
            publish_wake_frames(app, events, [None]);
        } else if !matches!(
            boundary,
            "failure" | "settings round trip" | "panel focus round trip"
        ) {
            publish_wake_frames(app, events, [Some(changed)]);
        }
        finish_keyboard(
            app,
            events,
            request,
            if boundary == "failure" {
                Err("timeout".into())
            } else {
                Ok(reply)
            },
        );
        publish_wake_frames(app, events, [Some(keyboard_frame("hel", 13))]);
        assert!(
            receiver.try_recv().is_err(),
            "never replay after {boundary}"
        );
        if matches!(boundary, "newer revision" | "disconnect" | "failure") {
            assert_eq!(
                app.panel
                    .as_mut()
                    .unwrap()
                    .take_native_typing_draft()
                    .as_deref(),
                Some("helx")
            );
            assert!(
                receiver.try_recv().is_err(),
                "recovery is explicit and local"
            );
        }
    }
    for frame_first in [false, true] {
        let receiver = prepare(app, events);
        let state = app.panel.as_mut().unwrap();
        let index = state
            .native
            .frame
            .as_ref()
            .unwrap()
            .candidates
            .iter()
            .position(|c| c.text == "hello")
            .unwrap();
        state.continue_sentence_candidate(index);
        let mut adoption = Some(receiver.try_recv().unwrap());
        state.backspace_seed();
        state.chrome.focus_input();
        state.handle_text_input("x");
        if !frame_first {
            acknowledge(app, events, adoption.take().unwrap());
        }
        // ABA: the applied "hello" frame was coalesced away after a physical edit.
        publish_wake_frames(app, events, [Some(keyboard_frame("hel", 12))]);
        if frame_first {
            acknowledge(app, events, adoption.take().unwrap());
        }
        assert!(
            receiver.try_recv().is_err(),
            "do not rebase semantic keys over physical edits"
        );
        assert_eq!(
            app.panel
                .as_mut()
                .unwrap()
                .take_native_typing_draft()
                .as_deref(),
            Some("hellox")
        );
    }
    let receiver = prepare(app, events);
    let state = app.panel.as_mut().unwrap();
    state.backspace_seed();
    let request = receiver.try_recv().unwrap();
    state.chrome.focus_input();
    state.handle_text_input("x");
    let mut reply = keyboard_frame("", 11);
    reply.seed = "z".repeat(suzaku_map::ime::companion::MAX_TEXT_BYTES);
    finish_keyboard(app, events, request, Ok(reply));
    assert!(
        receiver.try_recv().is_err(),
        "restored reading can exceed the follow-up byte budget"
    );
    assert!(
        app.panel
            .as_ref()
            .unwrap()
            .native
            .keyboard
            .as_ref()
            .unwrap()
            .blocked
    );
}

#[test]
#[ignore = "requires private Xvfb/D-Bus/XDG; run by scripts/test-linux-ci.sh ui"]
fn native_candidate_actions_preserve_followup_input() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for key in ["XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
        assert!(
            std::path::Path::new(&std::env::var_os(key).unwrap()).starts_with(std::env::temp_dir())
        );
    }
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
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
            let mut failures = Vec::new();
            for language in ["en", "zh-Hans"] {
                for touch in [false, true] {
                    check_truncated_native_commit(&mut app, events, language, touch);
                    check_truncated_standalone_preview(&mut app, events, language, touch);
                }
            }
            for touch in [false, true] {
                for boundary in [
                    "normal",
                    "empty only",
                    "manual hide",
                    "late manual hide",
                    "private",
                    "unfocused",
                    "disconnect",
                ] {
                    check_empty_transition(&mut app, events, touch, boundary, &mut failures);
                }
                check_unconfirmed_empty(&mut app, events, touch);
                for frame_first in [false, true] {
                    for boundary in ["normal acknowledgement", "commit empty", "commit retyped"] {
                        check_keyboard_commit_boundary(
                            &mut app,
                            events,
                            touch,
                            frame_first,
                            boundary,
                        );
                    }
                }
            }
            for case in ["word", "sentence", "commit", "clear", "select"] {
                for touch in [false, true] {
                    for frame_first in [false, true] {
                        check_followup(&mut app, events, case, touch, frame_first, &mut failures);
                        for boundary in ["language round trip", "input cancellation"] {
                            check_coalesced_target_change(
                                &mut app,
                                events,
                                case,
                                touch,
                                frame_first,
                                boundary,
                            );
                        }
                    }
                }
                check_late_reply_boundaries(&mut app, events, case);
            }
            check_native_pointer_delta_dispatch(&mut app, events, &mut failures);
            check_native_navigation_boundaries(&mut app, events);
            assert!(
                failures.is_empty(),
                "N26/N27: candidate action follow-ups: {failures:?}"
            );
            println!(
                "PASS: N26/N27 20 candidate action/order, 50 late reply and 16 visibility cases; no host connection"
            );
            println!(
                "PASS: N35 20 coalesced language round trips cancel queued keys and presses, ignore old acknowledgements and permit fresh input"
            );
            println!(
                "PASS: N36 20 coalesced input cancellations retire old queued keys and presses without replay after same-text retyping"
            );
            println!(
                "PASS: N37 12 keyboard-flight cases retire external commits without replay or stale presses; acknowledged same-target edits still continue"
            );
            println!(
                "PASS: truncated English/Chinese native cards commit once on the first mouse/touch click, cancel stale presses, and leave standalone first-click previews intact"
            );
            println!(
                "PASS: production wheel/pinch dispatch ignores horizontal, zero and nonfinite deltas, including settings scroll; finite vertical events still page, zoom or scroll settings"
            );
            println!(
                "PASS: native row boundaries send no redundant selection; actual moves and explicit same-row choices retain revision-bound requests"
            );
            self.completed = true;
            events.exit();
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    }
    let mut probe = Probe {
        proxy: events.create_proxy(),
        completed: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

fn prepare(app: &mut PanelApp, events: &ActiveEventLoop) -> mpsc::Receiver<ActionRequest> {
    let state = app.panel.as_mut().unwrap();
    state.cancel_translation();
    state.native = Default::default();
    state.chrome = PanelChromeState {
        llm_enabled: false,
        input_focused: false,
        ..Default::default()
    };
    state.engine.configure_prediction(None);
    state.engine.seed("");
    state.runs_without_window_focus = true;
    state.is_focused = false;
    state.last_interaction_action = None;
    state.clear_pressed_interaction();
    state.interaction.touch_tap_pending = false;
    let (sender, receiver) = mpsc::sync_channel(8);
    state.native.sender = Some(sender.clone());
    app.native_sync = Some(NativeSync {
        mailbox: Arc::new(Mutex::new(Mailbox::default())),
        sender,
        stop: Arc::new(AtomicBool::new(false)),
        threads: vec![],
    });
    app.native_hidden_context = None;
    app.panel_visible = false;
    app.native_auto_shown = false;
    let mut frame = keyboard_frame("hel", 10);
    frame
        .candidates
        .push(suzaku_map::ime::companion::NativeCandidate {
            text: "hello world".into(),
            label: "hello world".into(),
            kind: suzaku_map::ime::candidate_mix::CandidateKind::Sentence,
            ..Default::default()
        });
    publish_wake_frames(app, events, [Some(frame)]);
    assert!(app.panel.as_ref().unwrap().native.showing);
    assert_eq!(app.panel.as_ref().unwrap().chrome.seed_text, "hel");
    assert_panel_visibility(app, true);
    assert!(app.native_auto_shown);
    receiver
}

fn prepare_paged_pointer(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
) -> mpsc::Receiver<ActionRequest> {
    use suzaku_map::ime::companion::NativeCandidate;
    let receiver = prepare(app, events);
    {
        let state = app.panel.as_mut().unwrap();
        state.modifiers = Default::default();
        state.set_window_scale_continuous(1.0);
    }
    let mut frame = keyboard_frame("hel", 11);
    frame.selected = 6;
    frame.candidates = (0..13)
        .map(|index| NativeCandidate {
            text: format!("hello {index}"),
            label: format!("hello {index}"),
            ..Default::default()
        })
        .collect();
    publish_wake_frames(app, events, [Some(frame)]);
    let state = app.panel.as_mut().unwrap();
    assert_eq!(state.chrome.native_candidate_page.unwrap().start, 6);
    let rect = state
        .interaction_rect(InteractionKind::Candidate(6))
        .unwrap();
    state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
    assert!(!state.native_candidates_busy());
    receiver
}

fn check_native_navigation_boundaries(app: &mut PanelApp, events: &ActiveEventLoop) {
    use suzaku_map::ime::companion::NativeCandidate;
    for (count, selected, deltas, movement) in [
        (5, 0, vec![-1, -2, isize::MIN, 0], Some(1)),
        (5, 4, vec![1, 2, isize::MAX, 0], Some(-1)),
        (5, 2, vec![0], Some(1)),
        (1, 0, vec![-1, 1, 0, isize::MIN, isize::MAX], None),
        (0, 0, vec![-1, 1, 0, isize::MIN, isize::MAX], None),
    ] {
        let receiver = prepare(app, events);
        let mut frame = keyboard_frame("hel", 11);
        frame.selected = selected;
        frame.candidates = (0..count)
            .map(|index| NativeCandidate {
                text: format!("hello {index}"),
                label: format!("hello {index}"),
                ..Default::default()
            })
            .collect();
        publish_wake_frames(app, events, [Some(frame.clone())]);
        let state = app.panel.as_mut().unwrap();
        state.last_commit_feedback = None;
        let seed = state.chrome.seed_text.clone();
        for delta in deltas {
            state.move_candidate_selection(delta);
            assert!(matches!(
                receiver.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            assert!(state.native.pending.is_none());
            assert!(state.native.typing.is_none());
            assert_eq!(state.native.frame.as_ref(), Some(&frame));
            assert_eq!(state.chrome.seed_text, seed);
            assert!(state.last_commit_feedback.is_none());
        }
        if count == 0 {
            continue;
        }
        let target = if let Some(delta) = movement {
            state.move_candidate_selection(delta);
            selected.saturating_add_signed(delta)
        } else {
            // Unlike relative no-ops, explicitly choosing the current row is
            // intentional and still needs the host's normal acknowledgement.
            state.native_action(NativeOperation::Select(selected));
            selected
        };
        let request = receiver.try_recv().unwrap();
        assert_eq!(
            request.command,
            suzaku_map::platform::linux_ime_sync::action_command(
                &frame,
                &NativeOperation::Select(target)
            )
            .unwrap()
        );
        assert!(receiver.try_recv().is_err());
        assert_eq!(state.chrome.seed_text, seed);
    }
}

fn prepare_scrollable_settings(app: &mut PanelApp, events: &ActiveEventLoop) {
    // Main-window rendering deliberately excludes settings. Exercise the real
    // separate settings window with a viewport that requires vertical scroll.
    app.open_settings(events);
    let sender = app.panel.as_ref().unwrap().native.sender.clone();
    let state = app.settings.as_mut().unwrap();
    assert_eq!(state.kind, PanelWindowKind::Settings);
    state.native = Default::default();
    state.native.sender = sender;
    state.modifiers = Default::default();
    // Focus events are not pumped during this synchronous dispatch probe.
    state.set_window_focus(true);
    state.chrome.settings_category = suzaku_map::ime::gpu::SettingsCategory::Appearance;
    state.chrome.settings_search_query.clear();
    state.chrome.settings_keyboard_focus = None;
    state.chrome.settings_scroll_offset = 0.0;
    let _ = state
        .window
        .request_inner_size(winit::dpi::PhysicalSize::new(400, 270));
    let actual = state.window.inner_size();
    assert_eq!(actual, winit::dpi::PhysicalSize::new(400, 270));
    state.resize(actual.width, actual.height);
    state.last_scene = None;
    state.current_scene();
    assert!(state.interaction.settings_scroll_max_offset > 24.0);
    state.set_settings_scroll_offset(24.0);
    assert_eq!(state.chrome.settings_scroll_offset, 24.0);
}

fn check_native_pointer_delta_dispatch(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    failures: &mut Vec<String>,
) {
    use winit::{
        dpi::PhysicalPosition,
        event::{DeviceId, MouseScrollDelta, TouchPhase},
        keyboard::ModifiersState,
    };

    // Drive the actual event dispatcher: a helper-only test would miss routing
    // through candidate paging, Ctrl-wheel zoom or the independent pinch arm.
    for phase in [TouchPhase::Moved, TouchPhase::Ended] {
        for delta in [0.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for route in [
                "pixel page",
                "line page",
                "pixel zoom",
                "line zoom",
                "pixel settings",
                "line settings",
                "pinch",
            ] {
                let receiver = prepare_paged_pointer(app, events);
                let state = if route.ends_with("settings") {
                    prepare_scrollable_settings(app, events);
                    app.settings.as_mut().unwrap()
                } else {
                    app.panel.as_mut().unwrap()
                };
                let frame = state.native.frame.clone();
                let page = state.chrome.native_candidate_page;
                let scale = state.window_scale;
                if route.ends_with("zoom") {
                    state.modifiers = ModifiersState::CONTROL;
                }
                let scroll_offset = state.chrome.settings_scroll_offset;
                let event = if route == "pinch" {
                    WindowEvent::PinchGesture {
                        device_id: DeviceId::dummy(),
                        delta,
                        phase,
                    }
                } else {
                    WindowEvent::MouseWheel {
                        device_id: DeviceId::dummy(),
                        delta: if route.starts_with("pixel") {
                            MouseScrollDelta::PixelDelta(PhysicalPosition::new(60.0, delta))
                        } else {
                            MouseScrollDelta::LineDelta(1.0, delta as f32)
                        },
                        phase,
                    }
                };
                crate::input::handle_panel_window_event(state, events, event, false);
                if receiver.try_recv().is_ok()
                    || state.native.pending.is_some()
                    || state.native.frame != frame
                    || state.chrome.native_candidate_page != page
                    || state.window_scale != scale
                    || state.chrome.settings_scroll_offset != scroll_offset
                {
                    failures.push(format!(
                        "inert {route} delta={delta:?}, phase={phase:?} changed page/scale/settings or sent an action"
                    ));
                }
            }
        }
    }

    // Preserve the pre-existing policy for finite deltas, including nonzero
    // Ended events; this fix must not redefine platform gesture phases.
    for phase in [TouchPhase::Moved, TouchPhase::Ended] {
        for (delta, target) in [(0.25, 0), (-0.25, 12)] {
            for route in [
                "pixel page",
                "line page",
                "pixel zoom",
                "line zoom",
                "pixel settings",
                "line settings",
                "pinch",
            ] {
                let receiver = prepare_paged_pointer(app, events);
                let state = if route.ends_with("settings") {
                    prepare_scrollable_settings(app, events);
                    app.settings.as_mut().unwrap()
                } else {
                    app.panel.as_mut().unwrap()
                };
                let frame = state.native.frame.clone();
                let page = state.chrome.native_candidate_page;
                let scale = state.window_scale;
                if route.ends_with("zoom") {
                    state.modifiers = ModifiersState::CONTROL;
                }
                let scroll_offset = state.chrome.settings_scroll_offset;
                let event = if route == "pinch" {
                    WindowEvent::PinchGesture {
                        device_id: DeviceId::dummy(),
                        delta,
                        phase,
                    }
                } else {
                    WindowEvent::MouseWheel {
                        device_id: DeviceId::dummy(),
                        delta: if route.starts_with("pixel") {
                            MouseScrollDelta::PixelDelta(PhysicalPosition::new(60.0, delta))
                        } else {
                            MouseScrollDelta::LineDelta(1.0, delta as f32)
                        },
                        phase,
                    }
                };
                crate::input::handle_panel_window_event(state, events, event, false);
                if route.ends_with("page") {
                    let request = receiver
                        .try_recv()
                        .expect("finite vertical wheel must page");
                    assert!(request.command.ends_with(&format!(" N{target}")));
                    assert!(state.native.pending.is_some());
                    assert_eq!(state.window_scale, scale);
                } else if route.ends_with("settings") {
                    assert!(receiver.try_recv().is_err());
                    assert!(state.native.pending.is_none());
                    assert_eq!(state.window_scale, scale);
                    if delta > 0.0 {
                        assert!(state.chrome.settings_scroll_offset > scroll_offset);
                    } else {
                        assert!(state.chrome.settings_scroll_offset < scroll_offset);
                    }
                } else {
                    assert!(receiver.try_recv().is_err());
                    assert!(state.native.pending.is_none());
                    if delta > 0.0 {
                        assert!(state.window_scale > scale);
                    } else {
                        assert!(state.window_scale < scale);
                    }
                }
                assert_eq!(state.native.frame, frame);
                let expected_page = page.map(|mut page| {
                    // A valid native page request enters flight without moving
                    // its displayed range until acknowledgement. Settings and
                    // zoom gestures must preserve the entire optional state.
                    if route.ends_with("page") {
                        page.busy = true;
                    }
                    page
                });
                assert_eq!(state.chrome.native_candidate_page, expected_page);
            }
        }
    }
    app.panel.as_mut().unwrap().modifiers = Default::default();
}

fn click(state: &mut PanelState, action: InteractionKind, touch: bool) {
    super::tests::present_test_frame(state);
    let rect = state
        .interaction_rect(action)
        .expect("visible native action");
    state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(action));
    state.complete_primary_release(touch);
}

fn long_candidate(language: &str) -> String {
    if language == "en" {
        format!("{}.", "candidate ".repeat(200))
    } else {
        format!("{}。", "候选".repeat(1000))
    }
}

fn press_truncated_candidate(state: &mut PanelState, index: usize, touch: bool) {
    let action = InteractionKind::Candidate(index);
    super::tests::present_test_frame(state);
    let scene = state.last_scene.as_ref().unwrap();
    assert!(
        scene.sentence_candidate_truncated.contains(&index),
        "the real rendered card must truncate this fixture before testing its gesture"
    );
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == action)
        .expect("visible truncated candidate")
        .rect;
    let point = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    assert_eq!(scene.hit_interaction(point.0, point.1), Some(action));
    state.cursor_position = Some(point);
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(action));
}

fn check_truncated_native_commit(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    language: &str,
    touch: bool,
) {
    use suzaku_map::ime::{candidate_mix::CandidateKind, companion::NativeCandidate};
    let receiver = prepare(app, events);
    let text = long_candidate(language);
    assert_eq!(text.chars().count(), 2001);
    let mut frame = keyboard_frame(
        if language == "en" {
            "candidate"
        } else {
            "houxuan"
        },
        11,
    );
    frame.language = language.into();
    frame.candidates = vec![
        NativeCandidate {
            text: frame.seed.clone(),
            label: frame.seed.clone(),
            kind: CandidateKind::Literal,
            ..Default::default()
        },
        NativeCandidate {
            text: text.clone(),
            label: text.clone(),
            kind: CandidateKind::Sentence,
            ..Default::default()
        },
    ];
    publish_wake_frames(app, events, [Some(frame.clone())]);

    // A newer revision cancels a held card even when its text and index match.
    press_truncated_candidate(app.panel.as_mut().unwrap(), 1, touch);
    frame.revision += 1;
    publish_wake_frames(app, events, [Some(frame.clone())]);
    let state = app.panel.as_mut().unwrap();
    assert!(state.interaction.pressed_interaction.is_none());
    state.complete_primary_release(touch);
    assert!(receiver.try_recv().is_err(), "a stale card press committed");
    assert!(state.interaction.sentence_candidate_scroll_index.is_none());

    // A fresh gesture is a full host commit, not the standalone marquee preview.
    press_truncated_candidate(state, 1, touch);
    state.complete_primary_release(touch);
    let request = receiver.try_recv().unwrap_or_else(|error| {
        panic!("truncated native first click did not commit: {language}/{touch}: {error}")
    });
    assert_eq!(
        request.command,
        suzaku_map::platform::linux_ime_sync::action_command(&frame, &NativeOperation::Commit(1))
            .unwrap()
    );
    assert_eq!(
        state.native.frame.as_ref().unwrap().candidates[1].text,
        text
    );
    assert!(state.interaction.sentence_candidate_scroll_index.is_none());
    state.complete_primary_release(touch);
    assert!(
        receiver.try_recv().is_err(),
        "duplicate release resent commit"
    );
    assert!(state.engine.snapshot().committed_text.is_empty());

    acknowledge(app, events, request);
    frame.revision += 1;
    frame.context += 1;
    frame.seed.clear();
    frame.candidates.clear();
    publish_wake_frames(app, events, [Some(frame)]);
    app.panel.as_mut().unwrap().complete_primary_release(touch);
    assert!(
        receiver.try_recv().is_err(),
        "duplicate release after acknowledgement resent commit"
    );
}

fn check_truncated_standalone_preview(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    language: &str,
    touch: bool,
) {
    let receiver = prepare(app, events);
    app.native_sync = None;
    let state = app.panel.as_mut().unwrap();
    state.native = Default::default();
    state.chrome.native_candidate_page = None;
    state.chrome.input_modes_expanded = false;
    state.last_scene = None;
    state.last_interaction_action = None;
    state.last_commit_feedback = None;
    // If this control regresses into commit, the focused panel must trap it;
    // this test never sends text to a real host or any external target app.
    state.is_focused = true;
    let text = long_candidate(language);
    let previous_language = state.engine.snapshot().active_language;
    state.engine.set_language(language);
    state.chrome.set_seed_text(text.clone());
    state.engine.seed(&text);
    state.chrome.sentence_candidates = vec![text.clone()];
    state.chrome.sentence_candidate_source_indices = vec![0];
    state.clear_sentence_candidate_scroll();
    press_truncated_candidate(state, 0, touch);
    state.complete_primary_release(touch);
    assert_eq!(state.interaction.sentence_candidate_scroll_index, Some(0));
    assert!(
        state
            .interaction
            .sentence_candidate_scroll_started_at
            .is_some()
    );
    assert!(state.last_interaction_action.is_none());
    assert!(state.last_commit_feedback.is_none());
    assert_eq!(state.chrome.seed_text, text);
    assert!(state.engine.snapshot().committed_text.is_empty());
    state.complete_primary_release(touch);
    assert!(state.last_commit_feedback.is_none());
    assert!(receiver.try_recv().is_err());
    state.engine.set_language(previous_language);
}

fn key(state: &mut PanelState, character: char, touch: bool) {
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.chrome.input_modes_expanded = true;
    state.last_scene = None;
    click(
        state,
        InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character(character)),
        touch,
    );
}

fn start_action(state: &mut PanelState, case: &str, touch: bool) -> (NativeOperation, String) {
    match case {
        "word" => {
            // Native numeric adoption addresses the host's real candidate;
            // standalone next-token chips are deliberately not mirrored here.
            assert!(state.next_token_completions.is_empty());
            let index = 0;
            let text = state.native.frame.as_ref().unwrap().candidates[index]
                .text
                .clone();
            state.continue_sentence_candidate(index);
            (NativeOperation::Adopt(index), text)
        }
        "sentence" => {
            let frame = state.native.frame.as_ref().unwrap();
            let index = frame
                .candidates
                .iter()
                .position(|candidate| candidate.text == "hello world")
                .expect("synthetic sentence");
            let text = frame.candidates[index].text.clone();
            state.continue_sentence_candidate(index);
            (NativeOperation::Adopt(index), text)
        }
        "commit" => {
            let index = state.chrome.sentence_candidate_source_indices[0];
            click(state, InteractionKind::Candidate(index), touch);
            (NativeOperation::Commit(index), String::new())
        }
        "clear" => {
            state.native_action(NativeOperation::Clear);
            (NativeOperation::Clear, String::new())
        }
        "select" => {
            state.move_candidate_selection(1);
            (NativeOperation::Select(1), state.chrome.seed_text.clone())
        }
        _ => unreachable!(),
    }
}

fn acknowledge(app: &mut PanelApp, events: &ActiveEventLoop, request: ActionRequest) {
    app.user_event(
        events,
        PanelUserEvent::NativeActionFinished {
            host: request.host,
            revision: request.revision,
            result: Ok(true),
        },
    );
}

fn check_empty_transition(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    touch: bool,
    boundary: &str,
    failures: &mut Vec<String>,
) {
    let receiver = prepare(app, events);
    publish_wake_frames(app, events, [Some(keyboard_frame("q", 11))]);
    let state = app.panel.as_mut().unwrap();
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.chrome.input_modes_expanded = true;
    state.last_scene = None;
    click(
        state,
        InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Backspace),
        touch,
    );
    let deletion = receiver.try_recv().unwrap();
    assert!(deletion.command.starts_with('E') && deletion.command.ends_with(" 11 B"));
    if boundary != "empty only" {
        key(app.panel.as_mut().unwrap(), 'x', touch);
    }
    assert!(receiver.try_recv().is_err());
    let mut frame = keyboard_frame("", 12);
    if boundary == "manual hide" {
        app.user_event(events, PanelUserEvent::HidePanel);
    } else if boundary == "private" {
        frame.private = true;
    } else if boundary == "unfocused" {
        frame.focused = false;
    }
    publish_wake_frames(
        app,
        events,
        [if boundary == "disconnect" {
            None
        } else {
            Some(frame)
        }],
    );
    finish_keyboard(app, events, deletion, Ok(keyboard_frame("", 12)));
    let expected_visible = matches!(boundary, "normal" | "late manual hide");
    assert_panel_visibility(app, app.panel_visible);
    if app.panel_visible != expected_visible {
        let failure = format!(
            "N27 touch={touch}, {boundary}: visibility={}, expected={expected_visible}, draft={:?}",
            app.panel_visible,
            app.panel.as_ref().unwrap().chrome.seed_text
        );
        println!("AUDIT: {failure}");
        failures.push(failure);
    }
    let state = app.panel.as_mut().unwrap();
    if matches!(
        boundary,
        "empty only" | "private" | "unfocused" | "disconnect"
    ) {
        assert!(state.chrome.seed_text.is_empty());
        assert!(receiver.try_recv().is_err());
        if boundary == "disconnect" {
            assert_eq!(state.take_native_typing_draft().as_deref(), Some("qx"));
        } else {
            assert!(state.native.typing.is_none());
        }
        return;
    }
    assert_eq!(state.chrome.seed_text, "x");
    if boundary == "late manual hide" {
        app.user_event(events, PanelUserEvent::HidePanel);
        assert_panel_visibility(app, false);
    }
    let next = receiver.try_recv().unwrap();
    assert!(next.command.ends_with(" 12 Tx"));
    acknowledge(app, events, next);
    publish_wake_frames(app, events, [Some(keyboard_frame("x", 13))]);
    assert_panel_visibility(app, !matches!(boundary, "manual hide" | "late manual hide"));
    assert!(receiver.try_recv().is_err());
}

fn check_keyboard_commit_boundary(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    touch: bool,
    frame_first: bool,
    boundary: &str,
) {
    let receiver = prepare(app, events);
    key(app.panel.as_mut().unwrap(), 'l', touch);
    let mut request = Some(receiver.try_recv().unwrap());
    assert!(request.as_ref().unwrap().command.ends_with(" 10 Thell"));
    key(app.panel.as_mut().unwrap(), 'x', touch);
    assert!(receiver.try_recv().is_err());
    let state = app.panel.as_mut().unwrap();
    assert_eq!(state.chrome.seed_text, "hellx");
    assert!(!state.native.typing.as_ref().unwrap().blocked);
    super::tests::present_test_frame(state);
    let held_key = InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character('y'));
    let rect = state.interaction_rect(held_key).unwrap();
    state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(held_key));
    if !frame_first {
        acknowledge(app, events, request.take().unwrap());
    }

    let applied = keyboard_frame("hell", 11);
    let mut empty = keyboard_frame("", 12);
    empty.context += 1;
    let mut retyped = keyboard_frame("hell", 13);
    retyped.context = empty.context;
    // The retyped text equals the acknowledged write, not merely its old
    // base. Without the new target, queued `x` would overwrite the next draft.
    let mut current = match boundary {
        "normal acknowledgement" => applied.clone(),
        "commit empty" => empty.clone(),
        "commit retyped" => retyped.clone(),
        _ => unreachable!(),
    };
    let frames = match boundary {
        "normal acknowledgement" => vec![Some(applied)],
        "commit empty" => vec![Some(applied), Some(empty)],
        _ => vec![Some(applied), Some(empty), Some(retyped)],
    };
    publish_wake_frames(app, events, frames);
    if let Some(request) = request {
        acknowledge(app, events, request);
    }
    let state = app.panel.as_mut().unwrap();
    if boundary == "normal acknowledgement" {
        assert_eq!(state.chrome.seed_text, "hellx");
        assert_eq!(state.interaction.pressed_interaction, Some(held_key));
        let next = receiver.try_recv().unwrap();
        assert!(next.command.ends_with(" 11 Thellx"));
        state.complete_primary_release(touch);
        assert_eq!(state.chrome.seed_text, "hellxy");
        assert!(receiver.try_recv().is_err());
        acknowledge(app, events, next);
        publish_wake_frames(app, events, [Some(keyboard_frame("hellx", 12))]);
        let last = receiver.try_recv().unwrap();
        assert!(last.command.ends_with(" 12 Thellxy"));
        acknowledge(app, events, last);
        publish_wake_frames(app, events, [Some(keyboard_frame("hellxy", 13))]);
        let state = app.panel.as_ref().unwrap();
        assert_eq!(state.chrome.seed_text, "hellxy");
        assert!(state.native.typing.is_none());
        assert!(receiver.try_recv().is_err());
        return;
    }
    assert!(
        state.native.typing.is_none(),
        "{boundary}/{touch}/{frame_first}"
    );
    assert!(state.native.confirmed_action.is_none());
    assert!(state.native.pending.is_none());
    assert_eq!(state.chrome.seed_text, current.seed);
    assert!(state.interaction.pressed_interaction.is_none());
    assert!(!state.interaction.touch_tap_pending);
    state.complete_primary_release(touch);
    assert!(
        receiver.try_recv().is_err(),
        "old keyboard input crossed a commit"
    );
    assert_panel_visibility(app, !current.seed.is_empty());

    if current.seed.is_empty() {
        let mut fresh = keyboard_frame("h", current.revision + 1);
        fresh.context = current.context;
        current = fresh;
        publish_wake_frames(app, events, [Some(current.clone())]);
        assert_panel_visibility(app, true);
    }
    key(app.panel.as_mut().unwrap(), 'z', touch);
    let fresh = receiver.try_recv().unwrap();
    let text = current.seed + "z";
    assert!(
        fresh
            .command
            .ends_with(&format!(" {} T{text}", current.revision))
    );
    acknowledge(app, events, fresh);
    let mut final_frame = keyboard_frame(&text, current.revision + 1);
    final_frame.context = current.context;
    publish_wake_frames(app, events, [Some(final_frame)]);
    let state = app.panel.as_ref().unwrap();
    assert_eq!(state.chrome.seed_text, text);
    assert!(state.native.typing.is_none());
    assert!(receiver.try_recv().is_err());
}

fn check_coalesced_target_change(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    case: &str,
    touch: bool,
    frame_first: bool,
    boundary: &str,
) {
    let receiver = prepare(app, events);
    let original = app.panel.as_ref().unwrap().native.frame.clone().unwrap();
    start_action(app.panel.as_mut().unwrap(), case, touch);
    let mut request = Some(receiver.try_recv().unwrap());
    if !frame_first {
        acknowledge(app, events, request.take().unwrap());
    }
    key(app.panel.as_mut().unwrap(), 'x', touch);
    assert!(receiver.try_recv().is_err());
    let state = app.panel.as_mut().unwrap();
    assert!(state.native.typing.is_some());
    super::tests::present_test_frame(state);
    let held_key = InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Character('y'));
    let rect = state.interaction_rect(held_key).unwrap();
    state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
    state.begin_primary_press(touch);
    assert_eq!(state.interaction.pressed_interaction, Some(held_key));

    let mut away = keyboard_frame("", original.revision + 1);
    away.context = original.context + 1;
    if boundary == "language round trip" {
        away.language = "ja".into();
    }
    // The final text, language and candidates deliberately match the original.
    // Only the target ID records the intervening cancellation/language change. The real
    // host's generation of these IDs is exercised by the native IBus fixture.
    let mut returned = original.clone();
    returned.context += if boundary == "language round trip" {
        2
    } else {
        1
    };
    returned.revision += 2;
    publish_wake_frames(app, events, [Some(away), Some(returned.clone())]);
    if let Some(request) = request {
        acknowledge(app, events, request);
    }
    let state = app.panel.as_mut().unwrap();
    assert!(
        state.native.typing.is_none(),
        "{boundary}/{case}/{touch}/{frame_first}"
    );
    assert!(state.native.confirmed_action.is_none());
    assert!(state.native.pending.is_none());
    assert_eq!(state.native.frame.as_ref(), Some(&returned));
    assert_eq!(state.chrome.seed_text, original.seed);
    assert!(state.interaction.pressed_interaction.is_none());
    assert!(!state.interaction.touch_tap_pending);
    state.complete_primary_release(touch);
    assert!(
        receiver.try_recv().is_err(),
        "old input replayed after {boundary}"
    );

    key(app.panel.as_mut().unwrap(), 'z', touch);
    let fresh = receiver.try_recv().unwrap();
    assert_eq!(fresh.revision, returned.revision);
    assert!(fresh.command.ends_with(" 12 Thelz"));
    acknowledge(app, events, fresh);
    let mut final_frame = keyboard_frame("helz", returned.revision + 1);
    final_frame.context = returned.context;
    publish_wake_frames(app, events, [Some(final_frame)]);
    let state = app.panel.as_ref().unwrap();
    assert_eq!(state.chrome.seed_text, "helz");
    assert!(state.native.typing.is_none());
    assert!(receiver.try_recv().is_err());
}

fn check_late_reply_boundaries(app: &mut PanelApp, events: &ActiveEventLoop, case: &str) {
    for boundary in [
        "rejected",
        "timeout",
        "context",
        "host",
        "language",
        "private",
        "unfocused",
        "disconnect",
        "physical edit",
        "local edit",
    ] {
        let receiver = prepare(app, events);
        start_action(app.panel.as_mut().unwrap(), case, false);
        let request = receiver.try_recv().unwrap();
        let mut frame = keyboard_frame("fresh draft", 12);
        match boundary {
            "context" => frame.context += 1,
            "host" => frame.host = "22222222-2222-2222-2222-222222222222".into(),
            "language" => frame.language = "ja".into(),
            "private" => {
                frame.private = true;
                frame.seed.clear();
                frame.candidates.clear();
            }
            "unfocused" => {
                frame.focused = false;
                frame.seed.clear();
                frame.candidates.clear();
            }
            _ => {}
        }
        if boundary == "local edit" {
            let state = app.panel.as_mut().unwrap();
            state.begin_text_editing();
            state.chrome.set_seed_text("new local work".into());
            state.refresh_seed();
        } else if boundary == "disconnect" {
            publish_wake_frames(app, events, [None]);
        } else if !matches!(boundary, "rejected" | "timeout") {
            publish_wake_frames(app, events, [Some(frame)]);
        }
        let before = app.panel.as_ref().unwrap().chrome.seed_text.clone();
        app.user_event(
            events,
            PanelUserEvent::NativeActionFinished {
                host: request.host,
                revision: request.revision,
                result: match boundary {
                    "rejected" => Ok(false),
                    "timeout" => Err("synthetic timeout".into()),
                    _ => Ok(true),
                },
            },
        );
        let state = app.panel.as_ref().unwrap();
        assert!(state.native.pending.is_none(), "{case}: {boundary}");
        assert!(
            state.native.confirmed_action.is_none(),
            "{case}: {boundary}"
        );
        assert!(state.native.typing.is_none(), "{case}: {boundary}");
        assert_eq!(state.chrome.seed_text, before, "{case}: {boundary}");
        assert!(
            receiver.try_recv().is_err(),
            "late/uncertain {case} must not replay across {boundary}"
        );
    }
}

fn check_unconfirmed_empty(app: &mut PanelApp, events: &ActiveEventLoop, touch: bool) {
    let receiver = prepare(app, events);
    publish_wake_frames(app, events, [Some(keyboard_frame("q", 11))]);
    let state = app.panel.as_mut().unwrap();
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    state.chrome.input_modes_expanded = true;
    state.last_scene = None;
    click(
        state,
        InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Backspace),
        touch,
    );
    let deletion = receiver.try_recv().unwrap();
    // A candidate refresh can win the revision race before deletion is applied.
    // No speculative empty local buffer may hide the public host draft.
    publish_wake_frames(app, events, [Some(keyboard_frame("q", 12))]);
    assert_panel_visibility(app, true);
    assert!(app.native_hidden_context.is_none());
    app.user_event(
        events,
        PanelUserEvent::NativeKeyboardFinished {
            host: deletion.host,
            revision: deletion.revision,
            result: Err("stale native revision".into()),
        },
    );
    assert_panel_visibility(app, true);
    assert!(app.native_hidden_context.is_none());
    assert!(
        app.panel
            .as_ref()
            .unwrap()
            .native
            .keyboard
            .as_ref()
            .unwrap()
            .blocked
    );
    assert!(receiver.try_recv().is_err());
}

fn check_followup(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    case: &str,
    touch: bool,
    frame_first: bool,
    failures: &mut Vec<String>,
) {
    let receiver = prepare(app, events);
    let initial = app.panel.as_ref().unwrap().native.frame.clone().unwrap();
    let (operation, replacement) = start_action(app.panel.as_mut().unwrap(), case, touch);
    let request = receiver.try_recv().expect("candidate action must send");
    assert_eq!(
        request.command,
        suzaku_map::platform::linux_ime_sync::action_command(&initial, &operation).unwrap()
    );
    let mut frame = keyboard_frame(&replacement, 11);
    if case == "select" {
        frame.selected = 1;
    }
    if frame_first {
        publish_wake_frames(app, events, [Some(frame.clone())]);
    }
    acknowledge(app, events, request);
    // A completed commit/clear legitimately hides an automatic panel. Reopen
    // explicitly in the frame-first control; ACK-first still has a visible panel.
    if !app.panel_visible {
        app.user_event(events, PanelUserEvent::ShowPanel);
    }
    key(app.panel.as_mut().unwrap(), 'x', touch);
    let next = receiver.try_recv().ok();
    let draft = &app.panel.as_ref().unwrap().chrome.seed_text;
    if draft != &format!("{replacement}x") || (!frame_first && next.is_some()) {
        let failure = format!(
            "{case}, touch={touch}, frame_first={frame_first}: draft={draft:?}, early command={:?}",
            next.as_ref().map(|request| &request.command)
        );
        println!("AUDIT: N26 {failure}");
        failures.push(failure);
        return;
    }
    key(app.panel.as_mut().unwrap(), 'y', touch);
    if frame_first {
        let next = next.expect("matching frame permits the next edit");
        assert!(next.command.ends_with(&format!(" 11 T{replacement}x")));
        acknowledge(app, events, next);
        frame = keyboard_frame(&format!("{replacement}x"), 12);
    }
    assert!(
        receiver.try_recv().is_err(),
        "do not reuse the acknowledged revision"
    );
    publish_wake_frames(app, events, [Some(frame.clone())]);
    assert_panel_visibility(app, true); // A queued new draft must not blink closed.
    let next = receiver.try_recv().unwrap();
    assert!(
        next.command
            .ends_with(&format!(" {} T{replacement}xy", frame.revision))
    );
    acknowledge(app, events, next);
    publish_wake_frames(
        app,
        events,
        [Some(keyboard_frame(
            &format!("{replacement}xy"),
            frame.revision + 1,
        ))],
    );
    let state = app.panel.as_ref().unwrap();
    assert_eq!(state.chrome.seed_text, format!("{replacement}xy"));
    assert!(state.native.typing.is_none());
    assert!(state.native.pending.is_none());
    assert!(
        receiver.try_recv().is_err(),
        "never replay a commit or completed edit"
    );
    assert!(
        state.engine.snapshot().committed_text.is_empty(),
        "the independent panel engine is not the native host"
    );
}
