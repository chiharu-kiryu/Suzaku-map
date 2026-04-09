use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::gpu::{InputMode, VoiceCaptureState};
use suzaku_map::ime::{CommitOptions, SignalState};
use wgpu::SurfaceError;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

const TOUCH_TAP_SLOP_PX: f32 = 14.0;

pub(super) fn handle_panel_window_event(
    state: &mut PanelState,
    event_loop: &ActiveEventLoop,
    event: WindowEvent,
    allow_exit: bool,
) {
    match event {
        WindowEvent::CloseRequested => {
            if allow_exit {
                event_loop.exit();
            }
        }
        WindowEvent::Resized(size) => state.resize(size.width, size.height),
        WindowEvent::ScaleFactorChanged { .. } => state.window.request_redraw(),
        WindowEvent::CursorMoved { position, .. } => {
            state.cursor_position = Some((position.x as f32, position.y as f32));
            if state.kind == PanelWindowKind::Main && state.chrome.compact_mode {
                state.update_compact_hover();
            } else if state.kind == PanelWindowKind::Main {
                state.extend_handwriting_stroke();
            }
            state.window.request_redraw();
        }
        WindowEvent::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers.state();
        }
        WindowEvent::Touch(touch) => {
            state.cursor_position = Some((touch.location.x as f32, touch.location.y as f32));
            if state.kind == PanelWindowKind::Main {
                match touch.phase {
                    TouchPhase::Started => {
                        state.touch_start_position = state.cursor_position;
                        state.touch_tap_pending = !state.try_begin_handwriting_stroke();
                    }
                    TouchPhase::Moved => {
                        if state.handwriting_dragging {
                            state.extend_handwriting_stroke();
                        } else if let (Some((start_x, start_y)), Some((x, y))) =
                            (state.touch_start_position, state.cursor_position)
                        {
                            if (x - start_x).abs() > TOUCH_TAP_SLOP_PX
                                || (y - start_y).abs() > TOUCH_TAP_SLOP_PX
                            {
                                state.touch_tap_pending = false;
                            }
                        }
                    }
                    TouchPhase::Ended => {
                        if state.handwriting_dragging {
                            state.finish_handwriting_stroke();
                        } else if state.touch_tap_pending {
                            state.select_at_cursor();
                        }
                        state.touch_tap_pending = false;
                        state.touch_start_position = None;
                    }
                    TouchPhase::Cancelled => {
                        state.finish_handwriting_stroke();
                        state.touch_tap_pending = false;
                        state.touch_start_position = None;
                    }
                }
            }
            state.window.request_redraw();
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        } => {
            if state.kind == PanelWindowKind::Main && state.chrome.compact_mode {
                state.begin_compact_drag();
            } else if !(state.kind == PanelWindowKind::Main && state.try_begin_handwriting_stroke())
            {
                state.select_at_cursor();
            }
            state.window.request_redraw();
        }
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Left,
            ..
        } => {
            if state.kind == PanelWindowKind::Main && state.chrome.compact_mode {
                if !state.end_compact_drag() {
                    state.select_at_cursor();
                }
            } else if state.kind == PanelWindowKind::Main {
                state.finish_handwriting_stroke();
            }
            state.window.request_redraw();
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state == ElementState::Pressed {
                if allow_exit && state.is_quit_shortcut(&event.physical_key) {
                    event_loop.exit();
                    return;
                }
                if state.kind == PanelWindowKind::Settings {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        state.chrome.settings_open = false;
                    }
                } else {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::ArrowLeft) => state.chrome.move_caret_left(),
                        PhysicalKey::Code(KeyCode::ArrowRight) => state.chrome.move_caret_right(),
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            if state.chrome.input_focused {
                                state.chrome.blur_input();
                            } else {
                                state.engine.move_selection(1);
                            }
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            state.engine.move_selection(-1);
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.chrome.active_input_mode = InputMode::VirtualKeyboard;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.enter_voice_mode();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            if !(state.chrome.input_focused
                                && state.chrome.active_input_mode == InputMode::VirtualKeyboard)
                            {
                                state.chrome.active_input_mode = InputMode::Handwriting;
                            }
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            state.chrome.input_modes_expanded = !state.chrome.input_modes_expanded;
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
                            if state.chrome.active_input_mode == InputMode::VirtualKeyboard
                                && state.chrome.input_focused
                            {
                                state.handle_text_input(" ");
                            } else {
                                let _ = state.engine.commit(CommitOptions { force: true });
                            }
                        }
                        PhysicalKey::Code(KeyCode::Enter) => {
                            if state.chrome.input_focused {
                                state.chrome.blur_input();
                            } else {
                                let _ = state.engine.commit(CommitOptions { force: true });
                            }
                        }
                        _ => {}
                    }
                    if let Some(text) = event.text.as_deref() {
                        state.handle_text_input(text);
                    }
                }
                state.window.request_redraw();
            }
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
}
