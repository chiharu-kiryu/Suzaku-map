use super::{PanelState, PanelWindowKind};
use crate::app_state::{
    PersistedDisplaySettings, apply_display_settings, normalize_pointer_stability_settings,
    save_display_settings,
};
use crate::helpers::point_in_rect;
use crate::helpers::window_title;
use crate::render::{build_shape_vertices, build_text_vertices};
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{
    InputMode, InteractionKind, RenderScene, VirtualKeyboardKey, VoiceCaptureState,
    VoicePermissionState,
};
use suzaku_map::platform::gpu_host::is_quit_shortcut;
use suzaku_map::platform::text_output_host::commit_text_to_active_target;
use suzaku_map::platform::voice_host::open_voice_permission_settings;
use wgpu::SurfaceError;
use wgpu::util::DeviceExt;
use winit::keyboard::{KeyCode, PhysicalKey};

impl PanelState {
    pub(super) fn commit_selected_candidate_to_host(
        &mut self,
        options: suzaku_map::ime::CommitOptions,
    ) -> bool {
        let Some(candidate) = self
            .engine
            .candidates()
            .get(self.engine.snapshot().selected_index)
            .cloned()
        else {
            return false;
        };

        if self.is_duplicate_commit(self.engine.snapshot().selected_index, &candidate.text) {
            return false;
        }

        self.chrome.blur_input();
        let selected_index = self.engine.snapshot().selected_index;
        let result = self.engine.commit(options);
        if !result.ok {
            return false;
        }

        let committed_text = result.text.unwrap_or_else(|| candidate.text.clone());
        let output = commit_text_to_active_target(&candidate.text);
        self.note_commit_attempt(selected_index, &candidate.text);
        self.last_commit_feedback = Some(if output.delivered_successfully() {
            format!("Sent to active app: {}", candidate.text)
        } else {
            format!("Committed locally · {}", output.message)
        });
        self.commit_feedback_ticks = if output.delivered_successfully() {
            24
        } else {
            40
        };
        self.reset_after_commit(&committed_text);
        true
    }

    pub(super) fn commit_primary_sentence_candidate(&mut self) -> bool {
        let Some(index) = self
            .chrome
            .sentence_candidate_source_indices
            .first()
            .copied()
            .or_else(|| (!self.chrome.sentence_candidates.is_empty()).then_some(0))
        else {
            return false;
        };
        self.commit_sentence_candidate(index)
    }

    pub(super) fn commit_sentence_candidate(&mut self, index: usize) -> bool {
        let Some(index) = self.resolve_sentence_candidate_index(index) else {
            return false;
        };
        let Some(candidate) = self.engine.candidates().get(index).cloned() else {
            return false;
        };

        if self.is_duplicate_commit(index, &candidate.text) {
            return false;
        }

        self.chrome.blur_input();
        self.engine.select_candidate(index);
        let result = self
            .engine
            .commit(suzaku_map::ime::CommitOptions { force: true });
        if !result.ok {
            return false;
        }
        let committed_text = result.text.unwrap_or_else(|| candidate.text.clone());
        let output = commit_text_to_active_target(&candidate.text);
        self.note_commit_attempt(index, &candidate.text);
        self.last_commit_feedback = Some(if output.delivered_successfully() {
            format!("Sent to active app: {}", candidate.text)
        } else {
            format!("Committed locally · {}", output.message)
        });
        self.commit_feedback_ticks = if output.delivered_successfully() {
            24
        } else {
            40
        };
        self.reset_after_commit(&committed_text);
        true
    }

    fn resolve_sentence_candidate_index(&self, index: usize) -> Option<usize> {
        let candidates = self.engine.candidates();
        if candidates.is_empty() {
            return None;
        }

        if let Some(maybe_match) =
            self.resolve_candidate_index_by_displayed_label(index, candidates, |display_index| {
                self.chrome
                    .sentence_candidates
                    .get(display_index)
                    .map(String::as_str)
            })
        {
            return Some(maybe_match);
        }

        if index < candidates.len() {
            return Some(index);
        }

        if let Some(label) = self.chrome.sentence_candidates.get(index) {
            if let Some(fallback) = self.candidate_index_matching_sentence_label(label, candidates)
            {
                return Some(fallback);
            }
        }

        let selected_index = self.engine.snapshot().selected_index;
        if selected_index < candidates.len() {
            Some(selected_index)
        } else {
            Some(candidates.len() - 1)
        }
    }

    fn resolve_candidate_index_by_displayed_label<'a, F>(
        &self,
        index: usize,
        candidates: &[suzaku_map::ime::Candidate],
        label_for_display: F,
    ) -> Option<usize>
    where
        F: Fn(usize) -> Option<&'a str>,
    {
        let display_index = self
            .chrome
            .sentence_candidate_source_indices
            .iter()
            .position(|candidate_index| *candidate_index == index)
            .or_else(|| {
                self.chrome
                    .sentence_candidate_source_indices
                    .get(index)
                    .copied()
            })?;

        let label = label_for_display(display_index)?;
        self.candidate_index_matching_sentence_label(label, candidates)
    }

    fn candidate_index_matching_sentence_label(
        &self,
        label: &str,
        candidates: &[suzaku_map::ime::Candidate],
    ) -> Option<usize> {
        let normalized_label = Self::normalize_candidate_text(label);
        candidates
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| {
                let normalized_text = Self::normalize_candidate_text(&candidate.text);
                let normalized_label_text = Self::normalize_candidate_text(&candidate.label);
                if normalized_text == normalized_label || normalized_label_text == normalized_label
                {
                    Some(index)
                } else {
                    None
                }
            })
    }

    fn normalize_candidate_text(text: &str) -> String {
        text.to_ascii_lowercase()
            .trim()
            .trim_end_matches(|ch| matches!(ch, '.' | '!' | '?'))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn persist_display_settings(&self) {
        let _ = save_display_settings(&PersistedDisplaySettings::from(&self.chrome));
    }

    pub(super) fn current_scene(&self) -> RenderScene {
        let mut scene = match self.kind {
            PanelWindowKind::Main => {
                let mut chrome = self.chrome.clone();
                chrome.settings_open = false;
                chrome.hovered_interaction = self.hovered_interaction;
                chrome.pressed_interaction = self.pressed_interaction;
                if chrome.compact_mode {
                    self.renderer.build_compact_scene(
                        &self.engine.snapshot(),
                        &chrome,
                        self.compact_hovered,
                        self.compact_dragging,
                    )
                } else {
                    self.renderer
                        .build_panel_scene(&self.engine.snapshot(), &chrome)
                }
            }
            PanelWindowKind::Settings => {
                let mut chrome = self.chrome.clone();
                chrome.hovered_interaction = self.hovered_interaction;
                chrome.pressed_interaction = self.pressed_interaction;
                self.renderer.build_settings_scene(&chrome)
            }
        };
        self.append_hover_tooltip(&mut scene);
        self.append_commit_feedback(&mut scene);
        scene
    }

    pub(super) fn adopt_settings_from(&mut self, other: &suzaku_map::ime::gpu::PanelChromeState) {
        let needs_font_rebuild = self.chrome.font_face != other.font_face
            || self.chrome.text_smoothing != other.text_smoothing;
        let needs_llm_reconfigure = self.chrome.llm_enabled != other.llm_enabled
            || self.chrome.llm_model != other.llm_model
            || self.chrome.llm_temperature != other.llm_temperature;

        apply_display_settings(&mut self.chrome, &PersistedDisplaySettings::from(other));
        self.persist_display_settings();

        if needs_font_rebuild {
            self.rebuild_font_atlas();
        }
        if self.kind == PanelWindowKind::Main && needs_llm_reconfigure {
            self.reconfigure_llama_plugin();
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.kind == PanelWindowKind::Main && !self.chrome.compact_mode {
            self.expanded_window_size = Some(
                self.window
                    .inner_size()
                    .to_logical::<f64>(self.window.scale_factor()),
            );
            self.note_expanded_window_position();
        }
        self.size.width = width;
        self.size.height = height;
        self.config.width = width;
        self.config.height = height;
        self.renderer =
            suzaku_map::ime::gpu::WgpuCandidateRenderer::new(width as f32, height as f32);
        self.surface.configure(&self.device, &self.config);
    }

    pub(super) fn render(&mut self) -> Result<(), SurfaceError> {
        let snapshot = self.engine.snapshot();
        let scene = self.current_scene();
        match self.kind {
            PanelWindowKind::Main => self.window.set_title(&window_title(
                &scene,
                &snapshot.committed_text,
                self.font_atlas.uses_runtime_font,
                &self.font_atlas.font_label,
            )),
            PanelWindowKind::Settings => self.window.set_title("Suzaku Panel Settings"),
        }

        let shape_vertices =
            build_shape_vertices(&scene, self.config.width as f32, self.config.height as f32);
        let shape_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("suzaku-panel-shape-vertices"),
                    contents: bytemuck::cast_slice(&shape_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let text_vertices = build_text_vertices(
            &scene.atlas_glyphs,
            &self.font_atlas,
            self.config.width as f32,
            self.config.height as f32,
        );
        let text_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("suzaku-panel-text-vertices"),
                    contents: bytemuck::cast_slice(&text_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let background = match (self.chrome.theme_preset, snapshot.degraded) {
            (suzaku_map::ime::gpu::ThemePreset::Daylight, true) => wgpu::Color {
                r: 0.68,
                g: 0.75,
                b: 0.85,
                a: 1.0,
            },
            (suzaku_map::ime::gpu::ThemePreset::Daylight, false) => wgpu::Color {
                r: 0.74,
                g: 0.80,
                b: 0.89,
                a: 1.0,
            },
            (suzaku_map::ime::gpu::ThemePreset::DeviceDark, true) => wgpu::Color {
                r: 0.12,
                g: 0.15,
                b: 0.20,
                a: 1.0,
            },
            (suzaku_map::ime::gpu::ThemePreset::DeviceDark, false) => wgpu::Color {
                r: 0.15,
                g: 0.18,
                b: 0.24,
                a: 1.0,
            },
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("suzaku-panel-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("suzaku-panel-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.shape_pipeline);
            pass.set_vertex_buffer(0, shape_vertex_buffer.slice(..));
            pass.draw(0..shape_vertices.len() as u32, 0..1);
            if !text_vertices.is_empty() {
                pass.set_pipeline(&self.text_pipeline);
                pass.set_bind_group(0, &self.font_atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, text_vertex_buffer.slice(..));
                pass.draw(0..text_vertices.len() as u32, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }

    pub(super) fn select_at_cursor(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            return;
        };

        let scene = self.current_scene();
        if let Some(kind) = scene.hit_interaction(x, y) {
            match kind {
                InteractionKind::SeedInput => {
                    self.chrome.focus_input();
                    self.chrome.move_caret_to_end();
                }
                InteractionKind::ToggleCompactMode => {
                    self.apply_compact_mode(!self.chrome.compact_mode);
                }
                InteractionKind::InputModesToggle => {
                    if self.chrome.input_modes_expanded {
                        self.chrome.input_modes_expanded = false;
                        if self.chrome.active_input_mode == InputMode::VirtualKeyboard {
                            self.chrome.focus_input();
                        }
                    } else {
                        self.chrome.blur_input();
                        self.chrome.input_modes_expanded = true;
                    }
                }
                InteractionKind::InputModeButton(mode) => {
                    if self.chrome.input_modes_expanded && self.chrome.active_input_mode == mode {
                        if mode == InputMode::Dictation
                            && self.chrome.voice_state == VoiceCaptureState::Listening
                        {
                            self.stop_voice_capture();
                        }
                        self.chrome.input_modes_expanded = false;
                        self.chrome.focus_input();
                    } else if mode == InputMode::Dictation {
                        self.chrome.input_modes_expanded = true;
                        self.enter_voice_mode();
                    } else if mode == InputMode::VirtualKeyboard {
                        self.chrome.active_input_mode = mode;
                        self.chrome.input_modes_expanded = true;
                        self.chrome.focus_input();
                    } else {
                        self.chrome.blur_input();
                        self.chrome.active_input_mode = mode;
                        self.chrome.input_modes_expanded = true;
                    }
                }
                InteractionKind::SettingsToggle => {
                    self.chrome.settings_open = !self.chrome.settings_open;
                }
                InteractionKind::SetTextScale(scale) => {
                    self.chrome.text_scale = scale;
                    self.persist_display_settings();
                }
                InteractionKind::SetCandidateDensity(density) => {
                    self.chrome.candidate_density = density;
                    self.persist_display_settings();
                }
                InteractionKind::SetPreviewStyle(style) => {
                    self.chrome.preview_style = style;
                    self.persist_display_settings();
                }
                InteractionKind::SetFontFace(font_face) => {
                    self.chrome.font_face = font_face;
                    self.rebuild_font_atlas();
                    self.persist_display_settings();
                }
                InteractionKind::SetTextSpacing(spacing) => {
                    self.chrome.text_spacing = spacing;
                    self.persist_display_settings();
                }
                InteractionKind::SetTextSmoothing(smoothing) => {
                    self.chrome.text_smoothing = smoothing;
                    self.rebuild_font_atlas();
                    self.persist_display_settings();
                }
                InteractionKind::SetThemePreset(theme) => {
                    self.chrome.theme_preset = theme;
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmEnabled(enabled) => {
                    self.chrome.llm_enabled = enabled;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SetPointerTapSlopTenths(value) => {
                    self.chrome.pointer_tap_slop_tenths = value;
                    normalize_pointer_stability_settings(&mut self.chrome);
                    self.persist_display_settings();
                }
                InteractionKind::SetPointerTapMaxMs(value) => {
                    self.chrome.pointer_tap_max_ms = value;
                    normalize_pointer_stability_settings(&mut self.chrome);
                    self.persist_display_settings();
                }
                InteractionKind::SetPointerTargetSlopTenths(value) => {
                    self.chrome.pointer_target_slop_tenths = value;
                    normalize_pointer_stability_settings(&mut self.chrome);
                    self.persist_display_settings();
                }
                InteractionKind::SetVoiceAutoInsert(enabled) => {
                    self.chrome.voice_auto_insert = enabled;
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmModel(model) => {
                    self.chrome.llm_model = model;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SetLlmTemperature(temp) => {
                    self.chrome.llm_temperature = temp;
                    self.reconfigure_llama_plugin();
                    self.persist_display_settings();
                }
                InteractionKind::SelectNextToken(index) => self.select_next_token(index),
                InteractionKind::RewindNextToken => self.rewind_next_token(),
                InteractionKind::ToggleVoiceCapture => {
                    if matches!(
                        self.chrome.voice_permission,
                        VoicePermissionState::Denied | VoicePermissionState::Error
                    ) {
                        return;
                    }
                    if self.chrome.voice_state == VoiceCaptureState::Listening {
                        self.stop_voice_capture();
                    } else {
                        self.start_voice_capture();
                    }
                }
                InteractionKind::OpenVoiceSettings => {
                    let _ = open_voice_permission_settings();
                }
                InteractionKind::RefreshVoicePermissions => {
                    self.refresh_voice_permission_state();
                }
                InteractionKind::CycleVoiceSample => self.advance_voice_sample(),
                InteractionKind::InsertVoiceTranscript => {
                    if !self.chrome.voice_transcript.is_empty()
                        && self.chrome.voice_state != VoiceCaptureState::Listening
                    {
                        self.insert_voice_transcript();
                    }
                }
                InteractionKind::ClearVoiceTranscript => {
                    if self.chrome.voice_transcript.is_empty() {
                        return;
                    }
                    self.chrome.voice_transcript.clear();
                    self.chrome.voice_state = VoiceCaptureState::Idle;
                }
                InteractionKind::HandwritingCanvas => {}
                InteractionKind::UndoHandwritingStroke => {
                    if !self.chrome.handwriting_strokes.is_empty() {
                        self.undo_handwriting_stroke();
                    }
                }
                InteractionKind::ClearHandwriting => self.clear_handwriting(),
                InteractionKind::UseHandwritingCandidate(index) => {
                    self.insert_handwriting_candidate(index);
                }
                InteractionKind::VirtualKeyboardKey(key) => {
                    self.chrome.active_input_mode = InputMode::VirtualKeyboard;
                    self.chrome.focus_input();
                    match key {
                        VirtualKeyboardKey::Character(ch) => {
                            let mut text = String::new();
                            text.push(ch);
                            self.handle_text_input(&text);
                            if self.chrome.keyboard_shifted && ch.is_ascii_alphabetic() {
                                self.chrome.keyboard_shifted = false;
                            }
                        }
                        VirtualKeyboardKey::Text(text) => self.handle_text_input(text),
                        VirtualKeyboardKey::Space => self.handle_text_input(" "),
                        VirtualKeyboardKey::Backspace => self.backspace_seed(),
                        VirtualKeyboardKey::Shift => self.chrome.toggle_shift(),
                        VirtualKeyboardKey::ToggleNumeric => self.chrome.use_numeric_keyboard(),
                        VirtualKeyboardKey::ToggleAlphabetic => self.chrome.use_alpha_keyboard(),
                    }
                }
                InteractionKind::Candidate(index) => {
                    let action = InteractionKind::Candidate(index);
                    if self.is_repeating_interaction(action) {
                        return;
                    }
                    self.note_interaction_action(action);
                    let _ = self.commit_sentence_candidate(index);
                }
            }
        }
    }

    pub(super) fn update_hovered_interaction(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            self.hovered_interaction = None;
            return;
        };
        let scene = self.current_scene();
        self.hovered_interaction = scene.hit_interaction(x, y);
    }

    pub(super) fn update_pressed_interaction(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            self.pressed_interaction = None;
            self.press_target_rect = None;
            self.press_start_cursor = None;
            self.press_start_instant = None;
            return;
        };
        let scene = self.current_scene();
        self.pressed_interaction = scene.hit_interaction(x, y);
        self.press_target_rect = self.pressed_interaction.and_then(|target| {
            scene
                .interactive_targets
                .iter()
                .find(|candidate| candidate.kind == target)
                .map(|candidate| candidate.rect)
        });
        self.press_start_cursor = Some((x, y));
        self.press_start_instant = Some(Instant::now());
    }

    pub(super) fn press_target_is_stable(&self, expected: InteractionKind) -> bool {
        if self.pressed_interaction != Some(expected) {
            return false;
        }
        let Some((start_x, start_y)) = self.press_start_cursor else {
            return false;
        };
        let Some(started_at) = self.press_start_instant else {
            return false;
        };
        let tap_max_ms = Duration::from_millis(self.chrome.pointer_tap_max_ms as u64);
        if started_at.elapsed() > tap_max_ms {
            return false;
        }
        let Some((x, y)) = self.cursor_position else {
            return false;
        };
        let delta_x = (x - start_x).abs();
        let delta_y = (y - start_y).abs();
        let tap_slop = self.chrome.pointer_tap_slop_tenths as f32 / 10.0;
        if delta_x > tap_slop || delta_y > tap_slop {
            return false;
        }

        if let Some(rect) = self.press_target_rect {
            let target_slop = self.chrome.pointer_target_slop_tenths as f32 / 10.0;
            let expanded_rect = [
                rect[0] - target_slop,
                rect[1] - target_slop,
                rect[2] + target_slop * 2.0,
                rect[3] + target_slop * 2.0,
            ];
            if !point_in_rect(x, y, expanded_rect) {
                return false;
            }
        }
        let scene = self.current_scene();
        scene.hit_interaction(x, y) == Some(expected)
    }

    pub(super) fn clear_pressed_interaction(&mut self) {
        self.pressed_interaction = None;
        self.press_target_rect = None;
        self.press_start_cursor = None;
        self.press_start_instant = None;
    }

    pub(super) fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ)) && is_quit_shortcut(self.modifiers)
    }
}
