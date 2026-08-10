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
use suzaku_map::ime::CommitOptions;
use suzaku_map::platform::gpu_host::is_quit_shortcut;
use suzaku_map::platform::ime_host_adapter::{
    shared_session_bridge, ImeHostSessionBridge,
};
use suzaku_map::platform::ime_host_dispatch::current_ime_host_dispatch;
use suzaku_map::platform::text_output_host::{
    commit_text_to_active_target, HostTextOutputResult, HostTextOutputStatus,
};
use suzaku_map::platform::voice_host::open_voice_permission_settings;
use wgpu::SurfaceError;
use wgpu::util::DeviceExt;
use winit::keyboard::{KeyCode, PhysicalKey};

fn host_commit_fallback_text(fallback_text: &str, committed: Option<String>) -> String {
    committed
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| {
            if fallback_text.trim().is_empty() {
                "Candidate commit".to_string()
            } else {
                fallback_text.to_string()
            }
        })
}

fn commit_feedback_text(candidate_text: &str, delivered: bool, message: &str) -> String {
    if delivered {
        format!("Sent to active app: {candidate_text}")
    } else {
        format!("Committed locally · {message}")
    }
}

fn commit_feedback_ticks_for_delivery(delivered: bool) -> u8 {
    if delivered { 24 } else { 40 }
}

impl PanelState {
    pub(super) fn dispatch_input<F, R>(&mut self, handler: F) -> Option<R>
    where
        F: FnOnce(&mut Self) -> R,
    {
        if self.input_dispatch_guard {
            return None;
        }
        self.input_dispatch_guard = true;
        let result = handler(self);
        self.input_dispatch_guard = false;
        Some(result)
    }

    fn is_host_marked_text_roundtrip_ready(&self) -> bool {
        current_ime_host_dispatch().marked_text_roundtrip
    }

    fn is_host_commit_roundtrip_ready(&self) -> bool {
        current_ime_host_dispatch().commit_roundtrip
    }

    fn is_host_roundtrip_ready(&self) -> bool {
        self.is_host_marked_text_roundtrip_ready() && self.is_host_commit_roundtrip_ready()
    }

    fn sync_host_candidate_selection(&self, selected_index: usize) {
        if !self.is_host_marked_text_roundtrip_ready() {
            return;
        }
        let bridge = shared_session_bridge();
        let _ = bridge.activate_session();
        bridge.select_candidate(selected_index);
    }

    pub(super) fn move_candidate_selection(&mut self, delta: isize) {
        self.engine.move_selection(delta);
        if !self.is_host_marked_text_roundtrip_ready() {
            return;
        }
        let bridge = shared_session_bridge();
        let _ = bridge.activate_session();
        bridge.move_selection(delta);
    }

    fn host_commit_selected_candidate(
        &self,
        selected_index: usize,
        force: bool,
        fallback_text: &str,
    ) -> Option<HostTextOutputResult> {
        if !self.is_host_roundtrip_ready() {
            return None;
        }

        let bridge = shared_session_bridge();
        let _ = bridge.activate_session();
        bridge.select_candidate(selected_index);
        if !bridge.commit_selected(force) {
            return None;
        }

        let committed = bridge
            .take_last_committed_text()
            .filter(|text: &String| !text.trim().is_empty());
        let committed = host_commit_fallback_text(fallback_text, committed);

        Some(HostTextOutputResult {
            status: HostTextOutputStatus::Delivered,
            message: format!("Sent to active app via IME host: {committed}"),
        })
    }

    pub(super) fn set_window_focus(&mut self, focused: bool) {
        if self.is_focused == focused {
            return;
        }
        self.is_focused = focused;
        if !focused {
            if self.kind == PanelWindowKind::Main {
                self.chosen_canvas_abandon_state();
            } else {
                self.clear_pressed_interaction();
            }
            if self.is_host_marked_text_roundtrip_ready() {
                let bridge = shared_session_bridge();
                let _ = bridge.activate_session();
                bridge.clear_marked_text();
            }
            self.clear_sentence_candidate_scroll();
            self.voice_stability_ticks = 0;
            self.last_polled_voice_transcript.clear();
            self.chrome.blur_input();
            self.interaction.touch_tap_pending = false;
            self.interaction.touch_start_position = None;
            self.interaction.scale_dragging = false;
            self.interaction.scale_drag_start_cursor_x = None;
            self.interaction.compact_dragging = false;
            self.interaction.compact_drag_moved = false;
            self.interaction.compact_hovered = false;
            self.interaction.compact_drag_start_cursor = None;
            self.interaction.compact_drag_start_window_pos = None;
            self.interaction.last_input_was_touch = false;
            self.interaction.compact_drag_start_instant = None;
            if self.chrome.voice_state == VoiceCaptureState::Listening {
                self.stop_voice_capture();
            }
            self.interaction.handwriting_last_sample = None;
            self.interaction.handwriting_last_sample_position = None;
        }
    }

    fn chosen_canvas_abandon_state(&mut self) {
        self.clear_pressed_interaction();
        self.clear_sentence_candidate_scroll();
        self.interaction.touch_tap_pending = false;
        self.interaction.touch_start_position = None;
        self.interaction.scale_dragging = false;
        self.interaction.scale_drag_start_cursor_x = None;
        self.interaction.scale_drag_start_scale = self.window_scale;
        self.interaction.compact_dragging = false;
        self.interaction.compact_drag_moved = false;
        self.interaction.compact_hovered = false;
        self.interaction.compact_drag_start_cursor = None;
        self.interaction.compact_drag_start_window_pos = None;
        self.interaction.compact_drag_start_instant = None;
        self.interaction.handwriting_dragging = false;
        if self.chrome.active_input_mode == InputMode::Handwriting {
            self.chrome.handwriting_hint = "Draw a seed word with mouse or touch.".to_string();
        }
        if self.chrome.voice_state == VoiceCaptureState::Listening {
            self.stop_voice_capture();
        }
        self.chrome.blur_input();
        self.interaction.last_input_was_touch = false;
        self.interaction.handwriting_last_sample = None;
        self.interaction.handwriting_last_sample_position = None;
    }

    pub(super) fn commit_selected_candidate_to_host(
        &mut self,
        options: CommitOptions,
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
        self.sync_host_candidate_selection(selected_index);
        let force = options.force;
        let result = self.engine.commit(CommitOptions { force });
        if !result.ok {
            return false;
        }

        let committed_text = result.text.unwrap_or_else(|| candidate.text.clone());
        let output = self
            .host_commit_selected_candidate(selected_index, force, &committed_text)
            .unwrap_or_else(|| commit_text_to_active_target(&candidate.text));
        self.note_commit_attempt(selected_index, &candidate.text);
        let delivered = output.delivered_successfully();
        self.last_commit_feedback = Some(commit_feedback_text(
            &candidate.text,
            delivered,
            &output.message,
        ));
        self.commit_feedback_ticks = commit_feedback_ticks_for_delivery(delivered);
        self.clear_sentence_candidate_scroll();
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
        self.sync_host_candidate_selection(index);
        self.engine.select_candidate(index);
        let result = self
            .engine
            .commit(CommitOptions { force: true });
        if !result.ok {
            return false;
        }
        let committed_text = result.text.unwrap_or_else(|| candidate.text.clone());
        let output = self
            .host_commit_selected_candidate(index, true, &committed_text)
            .unwrap_or_else(|| commit_text_to_active_target(&candidate.text));
        self.note_commit_attempt(index, &candidate.text);
        let delivered = output.delivered_successfully();
        self.last_commit_feedback = Some(commit_feedback_text(
            &candidate.text,
            delivered,
            &output.message,
        ));
        self.commit_feedback_ticks = commit_feedback_ticks_for_delivery(delivered);
        self.clear_sentence_candidate_scroll();
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

    pub(super) fn normalize_candidate_text(text: &str) -> String {
        text.to_lowercase()
            .trim()
            .trim_end_matches(|ch| matches!(ch, '.' | '!' | '?'))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(super) fn persist_display_settings(&self) {
        let mut settings = PersistedDisplaySettings::from(&self.chrome);
        settings.window_scale = self.window_scale;
        let _ = save_display_settings(&settings);
    }

    pub(super) fn current_scene(&self) -> RenderScene {
        let mut scene = match self.kind {
            PanelWindowKind::Main => {
                let mut chrome = self.chrome.clone();
                chrome.settings_open = false;
                chrome.hovered_interaction = self.interaction.hovered_interaction;
                chrome.pressed_interaction = self.interaction.pressed_interaction;
                chrome.window_scale = self.window_scale;
                let sentence_candidate_scroll = self
                    .interaction
                    .sentence_candidate_scroll_index
                    .and_then(|index| {
                        self.interaction
                            .sentence_candidate_scroll_started_at
                            .as_ref()
                            .map(|started_at| (index, *started_at))
                    });
                let next_token_candidate_scroll = self
                    .interaction
                    .next_token_candidate_scroll_index
                    .and_then(|index| {
                        self.interaction
                            .next_token_candidate_scroll_started_at
                            .as_ref()
                            .map(|started_at| (index, *started_at))
                    });
                let handwriting_candidate_scroll = self
                    .interaction
                    .handwriting_candidate_scroll_index
                    .and_then(|index| {
                        self.interaction
                            .handwriting_candidate_scroll_started_at
                            .as_ref()
                            .map(|started_at| (index, *started_at))
                    });
                if chrome.compact_mode {
                    self.renderer.build_compact_scene(
                        &self.engine.snapshot(),
                        &chrome,
                        self.interaction.compact_hovered,
                        self.interaction.compact_dragging,
                    )
                } else {
                    self.renderer.build_panel_scene(
                        &self.engine.snapshot(),
                        &chrome,
                        sentence_candidate_scroll,
                        next_token_candidate_scroll,
                        handwriting_candidate_scroll,
                    )
                }
            }
            PanelWindowKind::Settings => {
                let mut chrome = self.chrome.clone();
                chrome.hovered_interaction = self.interaction.hovered_interaction;
                chrome.pressed_interaction = self.interaction.pressed_interaction;
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
            let logical_size = winit::dpi::PhysicalSize::new(width, height)
                .to_logical::<f64>(self.window.scale_factor());
            self.record_scaled_expanded_size(logical_size);
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
            (suzaku_map::ime::gpu::ThemePreset::Solarized, true) => wgpu::Color {
                r: 0.80,
                g: 0.74,
                b: 0.63,
                a: 1.0,
            },
            (suzaku_map::ime::gpu::ThemePreset::Solarized, false) => wgpu::Color {
                r: 0.86,
                g: 0.80,
                b: 0.67,
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
            (suzaku_map::ime::gpu::ThemePreset::HighContrast, true) => wgpu::Color {
                r: 0.03,
                g: 0.04,
                b: 0.07,
                a: 1.0,
            },
            (suzaku_map::ime::gpu::ThemePreset::HighContrast, false) => wgpu::Color {
                r: 0.04,
                g: 0.05,
                b: 0.08,
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
                InteractionKind::DecreaseWindowScale => {
                    if self.chrome.can_decrease_window_scale() {
                        self.adjust_window_scale(-1);
                    }
                }
                InteractionKind::DragWindowScale => {}
                InteractionKind::IncreaseWindowScale => {
                    if self.chrome.can_increase_window_scale() {
                        self.adjust_window_scale(1);
                    }
                }
                InteractionKind::ResetWindowScale => {
                    if self.chrome.can_reset_window_scale() {
                        self.reset_window_scale();
                    }
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
                    if theme == suzaku_map::ime::gpu::ThemePreset::HighContrast
                        && self.chrome.text_smoothing
                            != suzaku_map::ime::gpu::TextSmoothing::Sharp
                    {
                        self.chrome.text_smoothing = suzaku_map::ime::gpu::TextSmoothing::Sharp;
                        self.rebuild_font_atlas();
                    }
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
                InteractionKind::SelectNextToken(index) => {
                    let action = InteractionKind::SelectNextToken(index);
                    let is_truncated = scene.next_token_candidate_truncated.contains(&index);
                    let is_scrolling = self.interaction.next_token_candidate_scroll_index
                        == Some(index)
                        && self.interaction.next_token_candidate_scroll_started_at.is_some();

                    if is_truncated && !is_scrolling {
                        self.interaction.next_token_candidate_scroll_index = Some(index);
                        self.interaction.next_token_candidate_scroll_started_at =
                            Some(Instant::now());
                        return;
                    }

                    self.clear_sentence_candidate_scroll();
                    if self.is_repeating_interaction(action) {
                        return;
                    }
                    self.note_interaction_action(action);
                    self.select_next_token(index);
                }
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
                    let action = InteractionKind::UseHandwritingCandidate(index);
                    let is_truncated = scene.handwriting_candidate_truncated.contains(&index);
                    let is_scrolling = self.interaction.handwriting_candidate_scroll_index
                        == Some(index)
                        && self.interaction.handwriting_candidate_scroll_started_at.is_some();

                    if is_truncated && !is_scrolling {
                        self.interaction.handwriting_candidate_scroll_index = Some(index);
                        self.interaction.handwriting_candidate_scroll_started_at =
                            Some(Instant::now());
                        return;
                    }

                    self.clear_sentence_candidate_scroll();
                    if self.is_repeating_interaction(action) {
                        return;
                    }
                    self.note_interaction_action(action);
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
                    let is_truncated = scene.sentence_candidate_truncated.contains(&index);
                    let is_scrolling = self.interaction.sentence_candidate_scroll_index
                        == Some(index)
                        && self.interaction.sentence_candidate_scroll_started_at.is_some();

                    if is_truncated && !is_scrolling {
                        self.interaction.sentence_candidate_scroll_index = Some(index);
                        self.interaction.sentence_candidate_scroll_started_at = Some(Instant::now());
                        return;
                    }

                    self.clear_sentence_candidate_scroll();
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
            self.interaction.hovered_interaction = None;
            return;
        };
        let scene = self.current_scene();
        self.interaction.hovered_interaction = scene.hit_interaction(x, y);
    }

    pub(super) fn update_pressed_interaction(&mut self) {
        let Some((x, y)) = self.cursor_position else {
            self.interaction.pressed_interaction = None;
            self.interaction.press_target_rect = None;
            self.interaction.press_start_cursor = None;
            self.interaction.press_start_instant = None;
            return;
        };
        let scene = self.current_scene();
        self.interaction.pressed_interaction = scene.hit_interaction(x, y);
        self.interaction.press_target_rect = self.interaction.pressed_interaction.and_then(|target| {
            scene
                .interactive_targets
                .iter()
                .find(|candidate| candidate.kind == target)
                .map(|candidate| candidate.rect)
        });
        self.interaction.press_start_cursor = Some((x, y));
        self.interaction.press_start_instant = Some(Instant::now());
    }

    pub(super) fn begin_primary_press(&mut self, is_touch: bool) {
        self.update_pressed_interaction();
        if !matches!(
            self.interaction.pressed_interaction,
            Some(InteractionKind::Candidate(_)
                | InteractionKind::SelectNextToken(_)
                | InteractionKind::UseHandwritingCandidate(_))
        ) {
            self.clear_sentence_candidate_scroll();
        }

        if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            if self.interaction.pressed_interaction == Some(InteractionKind::ToggleCompactMode) {
                self.interaction.touch_tap_pending = false;
                self.begin_compact_drag();
                return;
            }
        }

        if self.interaction.pressed_interaction == Some(InteractionKind::DragWindowScale) {
            self.interaction.touch_tap_pending = false;
            self.begin_window_scale_drag();
            return;
        }

        if is_touch {
            self.interaction.touch_start_position = self.cursor_position;
            self.interaction.touch_tap_pending = !self.try_begin_handwriting_stroke();
            return;
        }

        if self.kind == PanelWindowKind::Main {
            let _ = self.try_begin_handwriting_stroke();
        }
    }

    pub(super) fn update_touch_move_stability(&mut self) {
        if self.interaction.handwriting_dragging {
            self.extend_handwriting_stroke();
            return;
        }
        if self.interaction.compact_dragging {
            if let Some((x, y)) = self.cursor_position {
                self.record_compact_drag_motion(x, y);
            }
            return;
        }
        if self.interaction.scale_dragging {
            if let Some((x, _)) = self.cursor_position {
                self.update_window_scale_drag(x);
            }
            return;
        }

        if let (Some((start_x, start_y)), Some((x, y))) =
            (self.interaction.touch_start_position, self.cursor_position)
        {
            let tap_slop = self.effective_tap_slop_tenths() / 10.0;
            if (x - start_x).abs() > tap_slop || (y - start_y).abs() > tap_slop {
                self.interaction.touch_tap_pending = false;
            }
        }
    }

    pub(super) fn complete_primary_release(&mut self, is_touch: bool) {
        let pressed_interaction = self.interaction.pressed_interaction;
        let selected_from_pressed = self
            .interaction
            .pressed_interaction
            .is_some_and(|target| self.press_target_is_stable(target));

        if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            if self.interaction.pressed_interaction == Some(InteractionKind::ToggleCompactMode) {
                if !self.end_compact_drag() && selected_from_pressed {
                    self.select_at_cursor();
                }
            } else if selected_from_pressed {
                self.select_at_cursor();
            }
        } else if self.interaction.scale_dragging {
            self.end_window_scale_drag();
        } else if is_touch {
            if self.interaction.handwriting_dragging {
                self.finish_handwriting_stroke();
            } else if self.interaction.touch_tap_pending && selected_from_pressed {
                self.select_at_cursor();
            }
        } else if selected_from_pressed && (self.kind != PanelWindowKind::Main || !self.interaction.handwriting_dragging)
        {
            self.select_at_cursor();
            self.finish_handwriting_stroke();
        } else {
            self.finish_handwriting_stroke();
        }

        if is_touch {
            self.interaction.touch_tap_pending = false;
            self.interaction.touch_start_position = None;
        }
        if !matches!(
            pressed_interaction,
            Some(InteractionKind::Candidate(_)
                | InteractionKind::SelectNextToken(_)
                | InteractionKind::UseHandwritingCandidate(_))
        ) || !selected_from_pressed
        {
            self.clear_sentence_candidate_scroll();
        }
        self.clear_pressed_interaction();
    }

    pub(super) fn cancel_primary_interaction(&mut self) {
        self.finish_handwriting_stroke();
        if self.interaction.compact_dragging {
            self.interaction.compact_dragging = false;
            self.interaction.compact_drag_moved = false;
            self.interaction.compact_drag_start_cursor = None;
            self.interaction.compact_drag_start_window_pos = None;
            self.interaction.compact_drag_start_instant = None;
            self.update_compact_hover();
        }
        if self.interaction.scale_dragging {
            self.end_window_scale_drag();
        }
        self.clear_sentence_candidate_scroll();
        self.clear_pressed_interaction();
        self.interaction.touch_tap_pending = false;
        self.interaction.touch_start_position = None;
    }

    pub(super) fn press_target_is_stable(&self, expected: InteractionKind) -> bool {
        if self.interaction.pressed_interaction != Some(expected) {
            return false;
        }
        let Some((start_x, start_y)) = self.interaction.press_start_cursor else {
            return false;
        };
        let Some(started_at) = self.interaction.press_start_instant else {
            return false;
        };
        let tap_max_ms = Duration::from_millis(self.effective_tap_max_ms());
        if started_at.elapsed() > tap_max_ms {
            return false;
        }
        let Some((x, y)) = self.cursor_position else {
            return false;
        };
        let delta_x = (x - start_x).abs();
        let delta_y = (y - start_y).abs();
        let tap_slop = self.effective_tap_slop_tenths() / 10.0;
        if delta_x > tap_slop || delta_y > tap_slop {
            return false;
        }

        if let Some(rect) = self.interaction.press_target_rect {
            let target_slop = self.effective_target_slop_tenths() / 10.0;
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

    fn effective_tap_slop_tenths(&self) -> f32 {
        if self.interaction.last_input_was_touch {
            (self.chrome.pointer_tap_slop_tenths as f32 * 1.45).min(120.0)
        } else {
            self.chrome.pointer_tap_slop_tenths as f32
        }
    }

    fn effective_target_slop_tenths(&self) -> f32 {
        if self.interaction.last_input_was_touch {
            (self.chrome.pointer_target_slop_tenths as f32 * 1.6).min(120.0)
        } else {
            self.chrome.pointer_target_slop_tenths as f32
        }
    }

    fn effective_tap_max_ms(&self) -> u64 {
        if self.interaction.last_input_was_touch {
            let boosted = (self.chrome.pointer_tap_max_ms as f32 * 1.4).round();
            boosted.min(1200.0) as u64
        } else {
            self.chrome.pointer_tap_max_ms as u64
        }
    }

    pub(super) fn clear_pressed_interaction(&mut self) {
        self.interaction.pressed_interaction = None;
        self.interaction.press_target_rect = None;
        self.interaction.press_start_cursor = None;
        self.interaction.press_start_instant = None;
        if !self.interaction.handwriting_dragging {
            self.interaction.handwriting_last_sample_position = None;
            self.interaction.handwriting_last_sample = None;
        }
    }

    fn clear_sentence_candidate_scroll(&mut self) {
        self.interaction.sentence_candidate_scroll_index = None;
        self.interaction.sentence_candidate_scroll_started_at = None;
        self.interaction.next_token_candidate_scroll_index = None;
        self.interaction.next_token_candidate_scroll_started_at = None;
        self.interaction.handwriting_candidate_scroll_index = None;
        self.interaction.handwriting_candidate_scroll_started_at = None;
    }

    pub(super) fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ)) && is_quit_shortcut(self.modifiers)
    }
}

#[cfg(test)]
mod tests {
    use super::{commit_feedback_text, commit_feedback_ticks_for_delivery, host_commit_fallback_text};

    #[test]
    fn host_commit_fallback_prefers_non_empty_committed_text() {
        let committed = host_commit_fallback_text("fallback", Some("from_host".to_string()));

        assert_eq!(committed, "from_host");
    }

    #[test]
    fn host_commit_fallback_falls_back_to_candidate_text_when_committed_empty() {
        let committed = host_commit_fallback_text("Candidate  ", Some("   ".to_string()));

        assert_eq!(committed, "Candidate  ");
    }

    #[test]
    fn host_commit_fallback_uses_default_when_candidate_empty() {
        let committed = host_commit_fallback_text("", None);

        assert_eq!(committed, "Candidate commit");
    }

    #[test]
    fn host_commit_fallback_trims_fallback_text_before_defaulting() {
        let committed = host_commit_fallback_text("   ", None);

        assert_eq!(committed, "Candidate commit");
    }

    #[test]
    fn host_commit_fallback_defaults_when_committed_empty_and_candidate_empty() {
        let committed = host_commit_fallback_text("", Some("\n  \t".to_string()));

        assert_eq!(committed, "Candidate commit");
    }

    #[test]
    fn host_commit_fallback_keeps_non_empty_fallback_with_spaces() {
        let committed = host_commit_fallback_text("  hello  ", Some(" ".to_string()));

        assert_eq!(committed, "  hello  ");
    }

    #[test]
    fn host_commit_fallback_keeps_candidate_with_unicode_and_emojis() {
        let committed = host_commit_fallback_text("😀 hello", Some("\n".to_string()));

        assert_eq!(committed, "😀 hello");
    }

    #[test]
    fn host_commit_fallback_defaults_when_candidate_empty_and_committed_blank() {
        let committed = host_commit_fallback_text("", Some("\n\t \r".to_string()));

        assert_eq!(committed, "Candidate commit");
    }

    #[test]
    fn host_commit_fallback_prefers_emoji_candidate_when_host_text_blank() {
        let committed = host_commit_fallback_text(" 😀", Some("\n".to_string()));

        assert_eq!(committed, " 😀");
    }

    #[test]
    fn host_commit_fallback_keeps_emoji_from_host_when_present() {
        let committed = host_commit_fallback_text("fallback", Some("👍🏽 hello".to_string()));

        assert_eq!(committed, "👍🏽 hello");
    }

    #[test]
    fn commit_feedback_text_marks_success_path_with_candidate() {
        let feedback = commit_feedback_text("hello", true, "ignored message");

        assert_eq!(feedback, "Sent to active app: hello");
    }

    #[test]
    fn commit_feedback_text_marks_local_path_with_message() {
        let feedback = commit_feedback_text("hello", false, "Candidate commit");

        assert_eq!(feedback, "Committed locally · Candidate commit");
    }

    #[test]
    fn commit_feedback_text_keeps_emoji_message_for_local_path() {
        let feedback = commit_feedback_text("ignored", false, "fallback: ✅ done");

        assert_eq!(feedback, "Committed locally · fallback: ✅ done");
    }

    #[test]
    fn commit_feedback_ticks_for_delivery_matches_success_and_failure_paths() {
        assert_eq!(commit_feedback_ticks_for_delivery(true), 24);
        assert_eq!(commit_feedback_ticks_for_delivery(false), 40);
    }

    #[test]
    fn commit_feedback_text_honors_candidate_on_success_even_with_empty_candidate_text() {
        let feedback = commit_feedback_text("", true, "ignored message");

        assert_eq!(feedback, "Sent to active app: ");
    }

    #[test]
    fn commit_feedback_text_local_with_empty_message_keeps_separator() {
        let feedback = commit_feedback_text("hello", false, "");

        assert_eq!(feedback, "Committed locally · ");
    }

    #[test]
    fn commit_feedback_text_uses_candidate_for_success_path() {
        let feedback = commit_feedback_text("sentinel ✨", true, "host said something odd");

        assert_eq!(feedback, "Sent to active app: sentinel ✨");
    }
}
