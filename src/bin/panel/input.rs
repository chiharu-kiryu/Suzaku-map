use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState};
use suzaku_map::ime::{CommitOptions, SignalState};
use wgpu::SurfaceError;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

pub(super) fn handle_panel_window_event(
    state: &mut PanelState,
    event_loop: &ActiveEventLoop,
    event: WindowEvent,
    allow_exit: bool,
) {
    let _ = state.dispatch_input(|state| {
        if !state.is_focused
            && !event_is_safe_while_unfocused(&event, state.runs_without_window_focus)
        {
            return;
        }

        if dismisses_hover_tooltip(&event) && state.interaction.tooltip.dismiss() {
            state.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                if allow_exit {
                    state.close_requested = true;
                }
            }
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
                state.window.request_redraw();
            }
            WindowEvent::Moved(_) => state.constrain_expanded_window_position(),
            WindowEvent::ScaleFactorChanged { .. } => state.window.request_redraw(),
            WindowEvent::Focused(focused) => {
                state.set_window_focus(focused);
                if focused {
                    if state.kind == PanelWindowKind::Main
                        && state.chrome.active_input_mode == InputMode::Dictation
                    {
                        state.refresh_voice_permission_state();
                    }
                    if state.kind == PanelWindowKind::Main {
                        state.chrome.focus_input();
                    }
                }
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.interaction.last_input_was_touch = false;
                state.cursor_position = Some((position.x as f32, position.y as f32));
                let mut needs_redraw = false;
                if state.interaction.panel_dragging {
                    if let Some((x, y)) = state.cursor_position {
                        state.update_panel_drag_motion(x, y);
                    }
                    needs_redraw = true;
                } else if state.kind == PanelWindowKind::Main && state.chrome.compact_mode {
                    needs_redraw |= state.update_compact_hover();
                } else if state.interaction.scale_dragging {
                    state.update_window_scale_drag(position.x as f32);
                    needs_redraw = true;
                } else if state.interaction.settings_scroll_dragging {
                    state.update_settings_scroll_drag(position.y as f32);
                    needs_redraw = true;
                } else if state.kind == PanelWindowKind::Main {
                    needs_redraw |= state.interaction.handwriting_dragging;
                    state.extend_handwriting_stroke();
                }
                if !state.interaction.handwriting_dragging && !state.interaction.panel_dragging {
                    needs_redraw |= state.update_hovered_interaction();
                }
                if needs_redraw {
                    state.window.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if state.clear_pointer_hover() {
                    state.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }
            WindowEvent::Touch(touch) => {
                state.interaction.last_input_was_touch = true;
                state.cursor_position = Some((touch.location.x as f32, touch.location.y as f32));
                if !state.interaction.handwriting_dragging {
                    state.update_hovered_interaction();
                }
                match touch.phase {
                    TouchPhase::Started => state.begin_primary_press(true),
                    TouchPhase::Moved => state.update_touch_move_stability(),
                    TouchPhase::Ended => state.complete_primary_release(true),
                    TouchPhase::Cancelled => state.cancel_primary_interaction(),
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.interaction.last_input_was_touch = false;
                state.begin_primary_press(false);
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                state.interaction.last_input_was_touch = false;
                state.complete_primary_release(false);
                state.update_hovered_interaction();
                state.window.request_redraw();
            }
            WindowEvent::Ime(event) => state.handle_ime_event(event),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if event.state == ElementState::Pressed && !is_synthetic {
                    // IME preedit owns its editing keys; do not run panel shortcuts.
                    if state.text_input.composing() {
                        return;
                    }
                    let scale_modifier =
                        super::keyboard::primary_shortcut_modifier(state.modifiers);
                    let scale_shortcut_handled = if scale_modifier {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Equal) if state.modifiers.shift_key() => {
                                state.adjust_window_scale(1);
                                true
                            }
                            PhysicalKey::Code(KeyCode::Minus) => {
                                state.adjust_window_scale(-1);
                                true
                            }
                            PhysicalKey::Code(KeyCode::NumpadAdd) => {
                                state.adjust_window_scale(1);
                                true
                            }
                            PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                                state.adjust_window_scale(-1);
                                true
                            }
                            PhysicalKey::Code(KeyCode::Digit0) => {
                                state.reset_window_scale();
                                true
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };
                    if scale_shortcut_handled {
                        state.window.request_redraw();
                        return;
                    }

                    if allow_exit && scale_modifier && state.is_quit_shortcut(&event.physical_key) {
                        event_loop.exit();
                        return;
                    }
                    if state.kind == PanelWindowKind::Main
                        && !state.chrome.settings_open
                        && state.chrome.input_focused
                    {
                        state.handle_editing_key(
                            &event.logical_key,
                            event.text.as_deref(),
                            event.repeat,
                        );
                        state.window.request_redraw();
                        return;
                    }
                    if !super::keyboard::text_modifiers_allowed(state.modifiers) {
                        return;
                    }

                    if event.repeat
                        && state.kind == PanelWindowKind::Main
                        && !state.chrome.settings_open
                        && matches!(
                            event.physical_key,
                            PhysicalKey::Code(KeyCode::Space)
                                | PhysicalKey::Code(KeyCode::Enter)
                                | PhysicalKey::Code(KeyCode::NumpadEnter)
                                | PhysicalKey::Code(KeyCode::Digit1)
                                | PhysicalKey::Code(KeyCode::Digit2)
                                | PhysicalKey::Code(KeyCode::Digit3)
                                | PhysicalKey::Code(KeyCode::Digit4)
                                | PhysicalKey::Code(KeyCode::Tab)
                                | PhysicalKey::Code(KeyCode::KeyV)
                                | PhysicalKey::Code(KeyCode::KeyN)
                                | PhysicalKey::Code(KeyCode::KeyI)
                                | PhysicalKey::Code(KeyCode::KeyR)
                                | PhysicalKey::Code(KeyCode::KeyD)
                        )
                    {
                        state.window.request_redraw();
                        return;
                    }

                    if state.kind == PanelWindowKind::Settings
                        || (state.kind == PanelWindowKind::Main && state.chrome.settings_open)
                    {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => {
                                state.chrome.settings_open = false;
                            }
                            PhysicalKey::Code(KeyCode::PageUp) => {
                                state.current_scene();
                                state.adjust_settings_scroll(-34.0);
                                state.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(KeyCode::PageDown) => {
                                state.current_scene();
                                state.adjust_settings_scroll(34.0);
                                state.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                state.current_scene();
                                state.set_settings_scroll_offset(0.0);
                                state.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(KeyCode::End) => {
                                state.current_scene();
                                state.set_settings_scroll_offset(
                                    state.interaction.settings_scroll_max_offset,
                                );
                                state.window.request_redraw();
                                return;
                            }
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                state.backspace_settings_search_text();
                                state.window.request_redraw();
                                return;
                            }
                            _ => {}
                        }

                        if let Some(text) = event.text.as_deref() {
                            state.handle_settings_search_text(text);
                            state.window.request_redraw();
                            return;
                        }
                    } else {
                        // Chords such as AltGr are text only, never tab/voice commands.
                        if !state.modifiers.is_empty() {
                            return;
                        }
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => state.finish_text_editing(),
                            PhysicalKey::Code(KeyCode::ArrowLeft) => state.chrome.move_caret_left(),
                            PhysicalKey::Code(KeyCode::ArrowRight) => {
                                state.chrome.move_caret_right()
                            }
                            PhysicalKey::Code(KeyCode::ArrowDown) => {
                                if state.chrome.input_focused {
                                    state.chrome.blur_input();
                                } else {
                                    state.move_candidate_selection(1);
                                }
                            }
                            PhysicalKey::Code(KeyCode::ArrowUp) => {
                                state.move_candidate_selection(-1);
                            }
                            PhysicalKey::Code(KeyCode::Digit1) => {
                                if !state.chrome.input_modes_expanded
                                    && !state.chrome.sentence_candidate_source_indices.is_empty()
                                {
                                    let index = state.chrome.sentence_candidate_source_indices[0];
                                    let _ = state.commit_sentence_candidate(index);
                                } else {
                                    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
                                    state.chrome.input_modes_expanded = true;
                                    state.chrome.focus_input();
                                }
                            }
                            PhysicalKey::Code(KeyCode::Digit2) => {
                                if !state.chrome.input_modes_expanded
                                    && state.chrome.sentence_candidate_source_indices.len() > 1
                                {
                                    let index = state.chrome.sentence_candidate_source_indices[1];
                                    let _ = state.commit_sentence_candidate(index);
                                } else {
                                    state.chrome.input_modes_expanded = true;
                                    state.enter_voice_mode();
                                }
                            }
                            PhysicalKey::Code(KeyCode::Digit3) => {
                                if !state.chrome.input_modes_expanded
                                    && state.chrome.sentence_candidate_source_indices.len() > 2
                                {
                                    let index = state.chrome.sentence_candidate_source_indices[2];
                                    let _ = state.commit_sentence_candidate(index);
                                } else {
                                    state.chrome.active_input_mode = InputMode::Handwriting;
                                    state.chrome.input_modes_expanded = true;
                                    state.chrome.blur_input();
                                }
                            }
                            PhysicalKey::Code(KeyCode::Digit4) => {
                                if !state.chrome.input_modes_expanded
                                    && state.chrome.sentence_candidate_source_indices.len() > 3
                                {
                                    let index = state.chrome.sentence_candidate_source_indices[3];
                                    let _ = state.commit_sentence_candidate(index);
                                }
                            }
                            PhysicalKey::Code(KeyCode::Tab) => {
                                state.chrome.input_modes_expanded =
                                    !state.chrome.input_modes_expanded;
                            }
                            PhysicalKey::Code(KeyCode::KeyD) => {
                                if !state.chrome.input_focused {
                                    state.engine.update_signal(SignalState {
                                        pointer_precision: 0.2,
                                        gaze_stability: 0.2,
                                        host_intent_weight: 0.4,
                                        source_confidence: 0.4,
                                    });
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyV) => {
                                if state.chrome.active_input_mode == InputMode::Dictation {
                                    if state.chrome.voice_state == VoiceCaptureState::Listening {
                                        state.stop_voice_capture();
                                    } else {
                                        state.start_voice_capture();
                                    }
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyN) => {
                                if state.chrome.active_input_mode == InputMode::Dictation {
                                    state.advance_voice_sample();
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyI) => {
                                if state.chrome.active_input_mode == InputMode::Dictation {
                                    state.insert_voice_transcript();
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyR) => {
                                if !state.chrome.input_focused {
                                    state.reset_signal();
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyZ) => {
                                if state.chrome.active_input_mode == InputMode::Handwriting
                                    && !state.chrome.input_focused
                                {
                                    state.undo_handwriting_stroke();
                                }
                            }
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                if state.chrome.input_focused {
                                    state.backspace_seed();
                                }
                            }
                            PhysicalKey::Code(KeyCode::Space) => {
                                let _ = state.commit_selected_candidate_to_host(CommitOptions {
                                    force: true,
                                });
                            }
                            PhysicalKey::Code(KeyCode::Enter)
                            | PhysicalKey::Code(KeyCode::NumpadEnter) => {
                                if state.chrome.input_focused {
                                    state.chrome.blur_input();
                                } else if state.commit_primary_sentence_candidate() {
                                    // The main sentence action should feel like a direct submit.
                                } else {
                                    let _ =
                                        state.commit_selected_candidate_to_host(CommitOptions {
                                            force: true,
                                        });
                                }
                            }
                            _ => {}
                        }
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => (position.y as f32 / 80.0).signum(),
                };
                if state.kind == PanelWindowKind::Main
                    && (state.modifiers.control_key() || state.modifiers.super_key())
                {
                    state.scale_window_by_wheel_delta(zoom_delta);
                } else if (state.kind == PanelWindowKind::Main && state.chrome.settings_open)
                    || state.kind == PanelWindowKind::Settings
                {
                    state.current_scene();
                    let scroll_delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y * 24.0,
                        MouseScrollDelta::PixelDelta(position) => {
                            (position.y as f32 * 0.85).clamp(-32.0, 32.0)
                        }
                    };
                    state.adjust_settings_scroll(scroll_delta);
                }
                state.window.request_redraw();
            }
            WindowEvent::PinchGesture { delta, .. } => {
                if state.kind == PanelWindowKind::Main && !state.chrome.compact_mode {
                    let magnitude = (delta as f32).signum();
                    if magnitude > 0.0 {
                        state.adjust_window_scale(1);
                    } else if magnitude < 0.0 {
                        state.adjust_window_scale(-1);
                    }
                }
                state.window.request_redraw();
            }
            WindowEvent::DoubleTapGesture { .. } => {
                if state.kind == PanelWindowKind::Main && !state.chrome.compact_mode {
                    state.reset_window_scale();
                }
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render() {
                    match err {
                        SurfaceError::Lost | SurfaceError::Outdated => {
                            state.resize(state.size.width, state.size.height);
                        }
                        SurfaceError::OutOfMemory => {
                            if allow_exit {
                                event_loop.exit();
                            }
                        }
                        SurfaceError::Timeout | SurfaceError::Other => {}
                    }
                }
            }
            _ => {}
        }
    });
    state.sync_text_input_state();
    state.update_pointer_cursor();
}

fn dismisses_hover_tooltip(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } | WindowEvent::KeyboardInput { .. }
            | WindowEvent::Ime(_)
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::Touch(_)
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::DoubleTapGesture { .. }
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
    )
}

fn event_is_safe_while_unfocused(event: &WindowEvent, non_focusing_panel: bool) -> bool {
    matches!(
        event,
        WindowEvent::Focused(_)
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::CloseRequested
            | WindowEvent::Resized(_)
            | WindowEvent::Moved(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::RedrawRequested
    ) || (non_focusing_panel
        && matches!(
            event,
            WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::PinchGesture { .. }
                | WindowEvent::DoubleTapGesture { .. }
        ))
}

#[cfg(test)]
mod tests {
    use super::{dismisses_hover_tooltip, event_is_safe_while_unfocused};
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, WindowEvent};

    #[test]
    fn scroll_or_press_hides_tooltips_but_redraw_does_not_cancel_pending_hints() {
        use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase};
        for event in [
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
            WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, -1.0),
                phase: TouchPhase::Moved,
            },
            WindowEvent::Resized(winit::dpi::PhysicalSize::new(420, 300)),
        ] {
            assert!(dismisses_hover_tooltip(&event));
        }
        assert!(!dismisses_hover_tooltip(&WindowEvent::RedrawRequested));
    }

    #[test]
    fn pointer_exit_always_clears_stale_hover_when_unfocused() {
        let pointer_event = WindowEvent::CursorLeft {
            device_id: DeviceId::dummy(),
        };

        assert!(event_is_safe_while_unfocused(&pointer_event, true));
        assert!(event_is_safe_while_unfocused(&pointer_event, false));
        let press = WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: winit::event::ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        };
        assert!(event_is_safe_while_unfocused(&press, true));
        assert!(!event_is_safe_while_unfocused(&press, false));
    }

    #[test]
    fn lifecycle_and_redraw_events_remain_safe_while_unfocused() {
        assert!(event_is_safe_while_unfocused(
            &WindowEvent::RedrawRequested,
            false,
        ));
        assert!(event_is_safe_while_unfocused(
            &WindowEvent::RedrawRequested,
            true,
        ));
        assert!(event_is_safe_while_unfocused(
            &WindowEvent::Moved(PhysicalPosition::new(240, 160)),
            false,
        ));
    }

    #[test]
    fn non_focusing_mode_never_treats_unfocused_ime_events_as_panel_text() {
        for non_focusing in [false, true] {
            for event in [
                WindowEvent::Ime(winit::event::Ime::Commit("日本語".into())),
                WindowEvent::Ime(winit::event::Ime::Preedit("中文".into(), None)),
                WindowEvent::ModifiersChanged(Default::default()),
            ] {
                assert!(!event_is_safe_while_unfocused(&event, non_focusing));
            }
        }
    }
}
