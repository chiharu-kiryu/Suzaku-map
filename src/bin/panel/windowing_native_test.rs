//! Synthetic native-window QA. Run under Xvfb with an isolated XDG_CONFIG_HOME;
//! this never captures the desktop, switches its IME or sends keyboard events.
use super::*;
use winit::platform::x11::EventLoopBuilderExtX11;

#[test]
#[ignore = "requires Xvfb, GPU/software adapter and SUZAKU_PANEL_NATIVE_QA=1 with an isolated config"]
fn native_window_fits_content_through_fold_zoom_and_restore() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config directory");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    let mut builder = EventLoop::<()>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut probe = FitProbe {
        state: None,
        step: 0,
        started: Instant::now(),
        changed: Instant::now(),
        applying: true,
        drag_destination: None,
    };
    event_loop.run_app(&mut probe).unwrap();
    assert_eq!(probe.step, STEPS.len());
    assert!(probe.drag_destination.is_some());
}

const STEPS: &[(f32, bool, InputMode)] = &[
    (1.0, true, InputMode::VirtualKeyboard),
    (1.0, false, InputMode::VirtualKeyboard),
    (0.75, false, InputMode::VirtualKeyboard),
    (0.75, true, InputMode::Dictation),
    (0.75, true, InputMode::Handwriting),
    (1.3, true, InputMode::Handwriting),
    (1.3, false, InputMode::Handwriting),
    (1.2, true, InputMode::VirtualKeyboard),
    (1.2, true, InputMode::VirtualKeyboard),
    (1.0, true, InputMode::VirtualKeyboard),
    (1.0, false, InputMode::VirtualKeyboard),
    (1.0, true, InputMode::VirtualKeyboard),
    (1.0, true, InputMode::VirtualKeyboard),
    (1.0, true, InputMode::VirtualKeyboard),
];

struct FitProbe {
    state: Option<PanelState>,
    step: usize,
    started: Instant,
    changed: Instant,
    applying: bool,
    drag_destination: Option<PhysicalPosition<i32>>,
}

fn assert_gesture_routing(state: &mut PanelState) {
    let before = state.chrome.clone();
    let scene = state.current_scene();
    state.last_scene = Some(scene.clone());
    let background = (0.1, 0.1);
    assert_eq!(
        scene.hit_interaction(background.0, background.1),
        Some(InteractionKind::DragWindow)
    );
    let release_over_control = scene
        .interactive_targets
        .iter()
        .rev()
        .find(|target| target.kind != InteractionKind::DragWindow)
        .map(|target| {
            (
                target.rect[0] + target.rect[2] / 2.0,
                target.rect[1] + target.rect[3] / 2.0,
            )
        })
        .unwrap();
    for is_touch in [false, true] {
        state.interaction.last_input_was_touch = is_touch;
        state.cursor_position = Some(background);
        state.update_hovered_interaction();
        state.begin_primary_press(is_touch);
        assert!(state.interaction.panel_dragging);
        assert_eq!(
            state.interaction.press_target_rect,
            scene
                .hit_interactive_target(background.0, background.1)
                .map(|target| target.rect)
        );
        assert!(!state.interaction.handwriting_dragging);
        // Releasing over a different control cannot click it or expand the bubble.
        state.cursor_position = Some(release_over_control);
        state.complete_primary_release(is_touch);
        assert!(!state.interaction.panel_dragging);
        assert_eq!(state.chrome.compact_mode, before.compact_mode);
        assert_eq!(state.chrome.seed_text, before.seed_text);
        assert!(!state.close_requested);
    }

    for target in &scene.interactive_targets {
        if target.kind == InteractionKind::DragWindow {
            continue;
        }
        let (x, y) = (
            target.rect[0] + target.rect[2] / 2.0,
            target.rect[1] + target.rect[3] / 2.0,
        );
        if scene.hit_interaction(x, y) != Some(target.kind) {
            continue; // Overlapping hit slop belongs to the topmost visible control.
        }
        state.cursor_position = Some((x, y));
        state.begin_primary_press(false);
        assert_eq!(state.interaction.pressed_interaction, Some(target.kind));
        assert_eq!(
            state.interaction.panel_dragging,
            state.chrome.compact_mode && target.kind == InteractionKind::ToggleCompactMode,
            "control {:?} started a window drag",
            target.kind
        );
        assert_eq!(
            state.interaction.handwriting_dragging,
            target.kind == InteractionKind::HandwritingCanvas
        );
        // Cancel before dispatching any action/commit; synthetic ink never reaches recognition.
        state.interaction.handwriting_dragging = false;
        state.cancel_primary_interaction();
        state.chrome = before.clone();
    }
    // Recover even when the platform consumed a previous drag's release event.
    state.cursor_position = Some(background);
    state.begin_primary_press(false);
    state.cursor_position = Some(release_over_control);
    state.begin_primary_press(false);
    assert_eq!(
        state.interaction.pressed_interaction,
        scene.hit_interaction(release_over_control.0, release_over_control.1)
    );
    assert_eq!(state.interaction.panel_dragging, state.chrome.compact_mode);
    state.interaction.handwriting_dragging = false;
    state.cancel_primary_interaction();
    state.chrome = before;
    assert!(!state.interaction.panel_dragging);
    assert!(state.interaction.pressed_interaction.is_none());
    state.clear_pointer_hover();
}

impl ApplicationHandler for FitProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    panel_window_attributes()
                        .with_title("Suzaku synthetic resize QA")
                        .with_inner_size(LogicalSize::new(900.0, 480.0)),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        // Exercise the non-focusing X11 IME drag path without a window manager
        // or synthetic global mouse events on the user's desktop.
        state.runs_without_window_focus = true;
        state.expanded_window_base_size = Some(LogicalSize::new(900.0, 480.0));
        state.chrome.seed_text = "hel".into();
        state.engine.seed("hel");
        state.refresh_composition_candidates();
        self.state = Some(state);
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let WindowEvent::Resized(size) = event {
            self.state.as_mut().unwrap().resize(size.width, size.height);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.step == STEPS.len() {
            let state = self.state.as_mut().unwrap();
            if let Some(destination) = self.drag_destination {
                assert!(
                    self.changed.elapsed() < Duration::from_secs(3),
                    "native drag did not move"
                );
                if state.window.outer_position().unwrap() == destination {
                    state.complete_primary_release(false);
                    assert!(!state.interaction.panel_dragging);
                    assert!(!state.close_requested);
                    println!("background drag moved the native window to {destination:?}");
                    event_loop.exit();
                    return;
                }
            } else {
                // Standalone settings use the same routing, but keep their own controls.
                state.kind = PanelWindowKind::Settings;
                assert_gesture_routing(state);
                state.kind = PanelWindowKind::Main;
                state.last_scene = Some(state.current_scene());
                let start = state.window.outer_position().unwrap();
                state.cursor_position = Some((state.size.width as f32 - 1.0, 1.0));
                state.interaction.last_input_was_touch = false;
                state.begin_primary_press(false);
                assert!(state.interaction.panel_dragging);
                let (x, y) = state.cursor_position.unwrap();
                state.update_panel_drag_motion(x + 50.0, y + 40.0);
                self.drag_destination = Some(PhysicalPosition::new(start.x + 50, start.y + 40));
                self.changed = Instant::now();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
            return;
        }
        assert!(
            self.started.elapsed() < Duration::from_secs(15),
            "native size did not settle at step {}",
            self.step
        );
        let state = self.state.as_mut().unwrap();
        let (scale, expanded, mode) = STEPS[self.step];
        if self.applying {
            if self.step == 12 {
                state.apply_compact_mode(true);
            }
            if self.step == 13 {
                state.apply_compact_mode(false);
            }
            state.chrome.input_modes_expanded = expanded;
            state.chrome.active_input_mode = mode;
            if self.step == 7 {
                // Deliberately queue two zoom requests before X11 delivers either ack.
                state.set_window_scale_continuous(0.8);
            }
            if self.step == 8 {
                // Return to the still-current size while the earlier shrink is pending.
                state.set_window_scale_continuous(0.85);
            }
            state.set_window_scale_continuous(scale);
            self.changed = Instant::now();
            self.applying = false;
        }
        state.fit_window_to_content();
        let width = (if state.chrome.compact_mode {
            COMPACT_PANEL_INNER_WIDTH
        } else {
            900.0 * scale as f64
        } * state.window.scale_factor())
        .round() as u32;
        let height = if state.chrome.compact_mode {
            (COMPACT_PANEL_INNER_HEIGHT * state.window.scale_factor()).round() as u32
        } else {
            WgpuCandidateRenderer::new(width as f32, 1.0)
                .preferred_input_panel_height(&state.chrome) as u32
        };
        let actual = state.window.inner_size();
        if self.changed.elapsed() >= Duration::from_millis(200)
            && actual == winit::dpi::PhysicalSize::new(width, height)
        {
            assert!(
                (state.expanded_window_base_size.unwrap().width - 900.0).abs() < 0.01,
                "programmatic resize changed the zoom base"
            );
            if self.step == 12 {
                assert!(state.chrome.compact_mode);
            }
            if self.step == 13 {
                assert!(!state.chrome.compact_mode);
                assert_eq!(
                    state.window.is_decorated(),
                    !state.chrome.hide_system_titlebar
                );
            }
            println!(
                "step {}: {:?}, expanded={expanded}, scale={scale}, native={}x{}",
                self.step, mode, actual.width, actual.height
            );
            assert_gesture_routing(state);
            self.step += 1;
            self.applying = true;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }
}
