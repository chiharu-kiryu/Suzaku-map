use super::{PanelState, PanelWindowKind, window_title};
use crate::app_state::{PersistedDisplaySettings, apply_display_settings, save_display_settings};
use crate::render::{build_shape_vertices, build_text_vertices};
use suzaku_map::ime::gpu::{
    InputMode, InteractionKind, RenderScene, VirtualKeyboardKey, VoiceCaptureState,
    VoicePermissionState,
};
use suzaku_map::platform::gpu_host::is_quit_shortcut;
use wgpu::SurfaceError;
use wgpu::util::DeviceExt;
use winit::keyboard::{KeyCode, PhysicalKey};

impl PanelState {
    fn persist_display_settings(&self) {
        let _ = save_display_settings(&PersistedDisplaySettings::from(&self.chrome));
    }

    pub(super) fn current_scene(&self) -> RenderScene {
        match self.kind {
            PanelWindowKind::Main => {
                let mut chrome = self.chrome.clone();
                chrome.settings_open = false;
                self.renderer
                    .build_panel_scene(&self.engine.snapshot(), &chrome)
            }
            PanelWindowKind::Settings => self.renderer.build_settings_scene(&self.chrome),
        }
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

        let background = if snapshot.degraded {
            wgpu::Color {
                r: 0.07,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            }
        } else {
            wgpu::Color {
                r: 0.03,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            }
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
                InteractionKind::InputModesToggle => {
                    self.chrome.blur_input();
                    self.chrome.input_modes_expanded = !self.chrome.input_modes_expanded;
                }
                InteractionKind::InputModeButton(mode) => {
                    self.chrome.blur_input();
                    self.chrome.active_input_mode = mode;
                    if mode == InputMode::Dictation {
                        if let Some(bridge) = self.voice.bridge.as_ref() {
                            bridge.request_permissions();
                            self.chrome.voice_permission = bridge.permission_state();
                        } else {
                            self.chrome.voice_permission = VoicePermissionState::Unavailable;
                        }
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
                InteractionKind::SetLlmEnabled(enabled) => {
                    self.chrome.llm_enabled = enabled;
                    self.reconfigure_llama_plugin();
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
                    if self.chrome.voice_state == VoiceCaptureState::Listening {
                        self.stop_voice_capture();
                    } else {
                        self.start_voice_capture();
                    }
                }
                InteractionKind::CycleVoiceSample => self.advance_voice_sample(),
                InteractionKind::InsertVoiceTranscript => self.insert_voice_transcript(),
                InteractionKind::ClearVoiceTranscript => {
                    self.chrome.voice_transcript.clear();
                    self.chrome.voice_state = VoiceCaptureState::Idle;
                }
                InteractionKind::HandwritingCanvas => {}
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
                    self.chrome.blur_input();
                    self.engine.select_candidate(index);
                }
            }
        }
    }

    pub(super) fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ)) && is_quit_shortcut(self.modifiers)
    }
}
