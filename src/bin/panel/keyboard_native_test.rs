//! Sends X11 key events only to this test's own windows on an isolated Xvfb.
//! No desktop capture, global hooks, IME switch or input injection into other apps.
use super::*;
use winit::{
    event::Ime,
    platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        ConnectionExt, EventMask, InputFocus, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, KeyButMask,
        KeyPressEvent,
    },
    rust_connection::RustConnection,
};

fn xid(window: &Window) -> u32 {
    match window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        _ => panic!("X11 test window required"),
    }
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_keyboard_editing_and_focus_return() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let (connection, _) = x11rb::connect(None).unwrap();
    let mut probe = KeyboardProbe {
        state: None,
        target: None,
        other: None,
        connection,
        step: 0,
        started: Instant::now(),
        key_events: 0,
    };
    event_loop.run_app(&mut probe).unwrap();
    assert_eq!(probe.step, 9);
    assert!(
        probe.key_events >= 28,
        "test must traverse real winit key events"
    );
}

struct KeyboardProbe {
    state: Option<PanelState>,
    target: Option<Arc<Window>>,
    other: Option<Arc<Window>>,
    connection: RustConnection,
    step: usize,
    started: Instant,
    key_events: usize,
}

fn click_input(state: &mut PanelState) {
    state.last_scene = None;
    let scene = state.current_scene();
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::SeedInput)
        .unwrap()
        .rect;
    state.last_scene = Some(scene);
    state.cursor_position = Some((rect[0] + 8.0, rect[1] + rect[3] / 2.0));
    state.select_at_cursor();
}

impl KeyboardProbe {
    fn focus(&self, window: u32) {
        self.connection
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
    }

    fn focused(&self) -> u32 {
        self.connection
            .get_input_focus()
            .unwrap()
            .reply()
            .unwrap()
            .focus
    }

    fn send_keys(&self, keysyms: &[u32]) {
        let setup = self.connection.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap();
        let width = usize::from(mapping.keysyms_per_keycode);
        let panel = xid(&self.state.as_ref().unwrap().window);
        for keysym in keysyms {
            let code = mapping
                .keysyms
                .chunks(width)
                .position(|row| row[0] == *keysym)
                .unwrap() as u8
                + setup.min_keycode;
            for (response_type, mask) in [
                (KEY_PRESS_EVENT, EventMask::KEY_PRESS),
                (KEY_RELEASE_EVENT, EventMask::KEY_RELEASE),
            ] {
                let event = KeyPressEvent {
                    response_type,
                    detail: code,
                    sequence: 0,
                    time: x11rb::CURRENT_TIME,
                    root: setup.roots[0].root,
                    event: panel,
                    child: 0,
                    root_x: 0,
                    root_y: 0,
                    event_x: 0,
                    event_y: 0,
                    state: KeyButMask::default(),
                    same_screen: true,
                };
                self.connection
                    .send_event(false, panel, mask, event)
                    .unwrap()
                    .check()
                    .unwrap();
            }
        }
        self.connection.flush().unwrap();
    }
}

impl ApplicationHandler for KeyboardProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let target = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Synthetic target"))
                .unwrap(),
        );
        let other = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Synthetic other app"))
                .unwrap(),
        );
        self.focus(xid(&target));
        let window = Arc::new(
            event_loop
                .create_window(
                    panel_window_attributes()
                        .with_override_redirect(true)
                        .with_visible(true)
                        .with_active(false),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        state.runs_without_window_focus = true;
        state.chrome.llm_enabled = false;
        state.engine.configure_prediction(None);
        self.state = Some(state);
        self.target = Some(target);
        self.other = Some(other);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window != state.window.id() {
            return;
        }
        if matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.state == winit::event::ElementState::Pressed)
        {
            self.key_events += 1;
        }
        handle_panel_window_event(state, event_loop, event, false);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        assert!(
            self.started.elapsed() < Duration::from_secs(20),
            "keyboard probe stuck at step {}, seed={:?}",
            self.step,
            self.state.as_ref().map(|s| &s.chrome.seed_text)
        );
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(15),
        ));
        if self.started.elapsed() < Duration::from_millis(250) {
            return;
        }
        let target = xid(self.target.as_ref().unwrap());
        let panel = xid(&self.state.as_ref().unwrap().window);
        match self.step {
            0 => {
                assert_eq!(
                    self.focused(),
                    target,
                    "showing a companion must not take focus"
                );
                let state = self.state.as_mut().unwrap();
                native_sync::assert_native_view(state);
                state.chrome.active_input_mode = InputMode::Handwriting;
                click_input(state);
                self.step = 1;
            }
            1 | 3 | 5 if self.state.as_ref().unwrap().is_focused => {
                assert_eq!(self.focused(), panel);
                self.send_keys(&"v123 n i ".chars().map(u32::from).collect::<Vec<_>>());
                self.step += 1;
            }
            2 | 4 | 6 if self.state.as_ref().unwrap().chrome.seed_text == "v123 n i " => {
                let step = self.step;
                let state = self.state.as_mut().unwrap();
                assert_eq!(
                    state.chrome.active_input_mode,
                    match step {
                        2 => InputMode::Handwriting,
                        4 => InputMode::Dictation,
                        _ => InputMode::VirtualKeyboard,
                    }
                );
                assert!(state.chrome.voice_state != VoiceCaptureState::Listening);
                assert!(!state.engine.candidates().is_empty());
                // Preedit is visible but never enters the candidate engine until committed.
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("日本".into(), Some((3, 3)))),
                    false,
                );
                assert_eq!(state.chrome.seed_text, "v123 n i ");
                assert!(state.text_input.composing());
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("".into(), None)),
                    false,
                );
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Commit("日本語中文é".into())),
                    false,
                );
                assert_eq!(state.chrome.seed_text, "v123 n i 日本語中文é");
                assert_eq!(state.engine.snapshot().seed_text, state.chrome.seed_text);
                if step < 6 {
                    state.chrome.active_input_mode = if step == 2 {
                        InputMode::Dictation
                    } else {
                        InputMode::VirtualKeyboard
                    };
                    state.chrome.set_seed_text(String::new());
                    state.refresh_seed();
                } else {
                    self.send_keys(&[0xff0d]); // Enter finishes direct editing.
                }
                self.step += 1;
            }
            7 if !self.state.as_ref().unwrap().is_focused => {
                assert_eq!(
                    self.focused(),
                    target,
                    "Enter must return the borrowed focus"
                );
                let state = self.state.as_mut().unwrap();
                assert!(!state.chrome.input_focused);
                let before = state.chrome.seed_text.clone();
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Commit("late commit".into())),
                    false,
                );
                assert_eq!(state.chrome.seed_text, before);
                click_input(state);
                self.step = 8;
            }
            8 if self.state.as_ref().unwrap().is_focused => {
                let state = self.state.as_mut().unwrap();
                // Sending into the panel itself is forbidden, without touching native IPC.
                assert!(
                    !state.commit_selected_candidate_to_host(suzaku_map::ime::CommitOptions {
                        force: true
                    })
                );
                assert!(
                    state
                        .last_commit_feedback
                        .as_ref()
                        .unwrap()
                        .contains("target app")
                );
                click_input(state);
                let other = xid(self.other.as_ref().unwrap());
                self.focus(other);
                self.state.as_mut().unwrap().finish_text_editing();
                assert_eq!(
                    self.focused(),
                    other,
                    "a later user app switch wins over restoration"
                );
                self.step = 9;
                event_loop.exit();
            }
            _ => {}
        }
    }
}
