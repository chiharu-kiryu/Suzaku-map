use super::{PanelState, PanelWindowKind, window_title};
use crate::app_state::{PersistedDisplaySettings, apply_display_settings, save_display_settings};
use crate::render::{build_shape_vertices, build_text_vertices};
use suzaku_map::ime::gpu::{
    CandidateQuad, InputMode, InteractionKind, RenderScene, TextAlign, TextBlock, TextRole,
    VirtualKeyboardKey, VoiceCaptureState, VoicePermissionState,
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

        self.chrome.blur_input();
        let result = self.engine.commit(options);
        if !result.ok {
            return false;
        }

        let output = commit_text_to_active_target(&candidate.text);
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
        self.reset_after_commit();
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
        let Some(candidate) = self.engine.candidates().get(index).cloned() else {
            return false;
        };
        self.chrome.blur_input();
        self.engine.select_candidate(index);
        let result = self
            .engine
            .commit(suzaku_map::ime::CommitOptions { force: true });
        if !result.ok {
            return false;
        }
        let output = commit_text_to_active_target(&candidate.text);
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
        self.reset_after_commit();
        true
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
                    self.chrome.blur_input();
                    self.chrome.input_modes_expanded = !self.chrome.input_modes_expanded;
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
            return;
        };
        let scene = self.current_scene();
        self.pressed_interaction = scene.hit_interaction(x, y);
    }

    pub(super) fn clear_pressed_interaction(&mut self) {
        self.pressed_interaction = None;
    }

    pub(super) fn is_quit_shortcut(&self, key: &PhysicalKey) -> bool {
        matches!(key, PhysicalKey::Code(KeyCode::KeyQ)) && is_quit_shortcut(self.modifiers)
    }

    fn append_hover_tooltip(&self, scene: &mut RenderScene) {
        if self.kind == PanelWindowKind::Main && self.chrome.compact_mode {
            return;
        }
        let Some((x, y)) = self.cursor_position else {
            return;
        };
        let Some(kind) = scene.hit_interaction(x, y) else {
            return;
        };
        let Some(text) = self.interaction_hint(kind) else {
            return;
        };

        let estimated_width = (text.chars().count() as f32 * 8.0 + 16.0).clamp(72.0, 260.0);
        let origin_x = (x + 14.0).min(self.renderer.scene_width - estimated_width - 12.0);
        let origin_y = if y > self.renderer.scene_height - 54.0 {
            y - 26.0
        } else {
            y + 16.0
        };
        let layout = TextBlock {
            text,
            origin: [origin_x + 8.0, origin_y + 7.0],
            max_width: estimated_width - 16.0,
            pixel_size: 2.0,
            letter_spacing: 0.0,
            line_gap: 4.0,
            max_lines: 2,
            color: [0.18, 0.23, 0.32, 1.0],
            align: TextAlign::Left,
            role: TextRole::HeaderStatus,
        }
        .layout();
        let bounds = layout.bounds;
        scene.quads.push(CandidateQuad {
            rect: [
                bounds[0] - 8.0,
                bounds[1] - 5.0,
                bounds[2] + 16.0,
                bounds[3] + 10.0,
            ],
            color: [0.97, 0.98, 1.0, 0.98],
        });
        scene.text_quads.extend(layout.quads.iter().copied());
        scene
            .atlas_glyphs
            .extend(layout.atlas_glyphs.iter().cloned());
        scene.text_sections.push(suzaku_map::ime::gpu::TextSection {
            role: TextRole::HeaderStatus,
            layouts: vec![layout],
        });
    }

    fn append_commit_feedback(&self, scene: &mut RenderScene) {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return;
        }
        let Some(text) = self.last_commit_feedback.as_ref() else {
            return;
        };

        let origin_x = 28.0;
        let origin_y = 18.0;
        let layout = TextBlock {
            text: text.clone(),
            origin: [origin_x + 12.0, origin_y + 10.0],
            max_width: (self.renderer.scene_width - 56.0).max(180.0),
            pixel_size: 2.0,
            letter_spacing: 0.0,
            line_gap: 4.0,
            max_lines: 2,
            color: [0.10, 0.23, 0.34, 1.0],
            align: TextAlign::Left,
            role: TextRole::HeaderStatus,
        }
        .layout();
        let bounds = layout.bounds;
        scene.quads.push(CandidateQuad {
            rect: [
                bounds[0] - 8.0,
                bounds[1] - 2.0,
                bounds[2] + 16.0,
                bounds[3] + 14.0,
            ],
            color: [0.32, 0.42, 0.58, 0.14],
        });
        scene.quads.push(CandidateQuad {
            rect: [
                bounds[0] - 12.0,
                bounds[1] - 8.0,
                bounds[2] + 24.0,
                bounds[3] + 16.0,
            ],
            color: [0.84, 0.94, 1.0, 0.96],
        });
        scene.quads.push(CandidateQuad {
            rect: [bounds[0] - 12.0, bounds[1] - 8.0, bounds[2] + 24.0, 2.0],
            color: [1.0, 1.0, 1.0, 0.18],
        });
        scene.quads.push(CandidateQuad {
            rect: [
                bounds[0] - 12.0,
                bounds[1] + bounds[3] + 6.0,
                bounds[2] + 24.0,
                2.0,
            ],
            color: [0.28, 0.56, 0.82, 0.22],
        });
        scene.text_quads.extend(layout.quads.iter().copied());
        scene
            .atlas_glyphs
            .extend(layout.atlas_glyphs.iter().cloned());
        scene.text_sections.push(suzaku_map::ime::gpu::TextSection {
            role: TextRole::HeaderStatus,
            layouts: vec![layout],
        });
    }

    fn interaction_hint(&self, kind: InteractionKind) -> Option<String> {
        match kind {
            InteractionKind::SeedInput => Some(if self.chrome.seed_text.is_empty() {
                "Seed input".to_string()
            } else {
                format!("Seed input: {}", self.chrome.seed_text)
            }),
            InteractionKind::ToggleCompactMode => Some("Toggle floating bubble".to_string()),
            InteractionKind::InputModesToggle => Some(if self.chrome.input_modes_expanded {
                "Hide input methods".to_string()
            } else {
                "Show input methods".to_string()
            }),
            InteractionKind::InputModeButton(InputMode::VirtualKeyboard) => {
                Some("Virtual keyboard".to_string())
            }
            InteractionKind::InputModeButton(InputMode::Dictation) => {
                Some("Voice input".to_string())
            }
            InteractionKind::InputModeButton(InputMode::Handwriting) => {
                Some("Handwriting input".to_string())
            }
            InteractionKind::SettingsToggle => Some("Panel settings".to_string()),
            InteractionKind::SetTextScale(scale) => {
                Some(format!("Text size: {}", display_text_scale_label(scale)))
            }
            InteractionKind::SetCandidateDensity(density) => {
                Some(format!("Candidate density: {}", density_label(density)))
            }
            InteractionKind::SetPreviewStyle(style) => {
                Some(format!("Preview style: {}", preview_style_label(style)))
            }
            InteractionKind::SetFontFace(face) => Some(format!("Font: {}", font_face_label(face))),
            InteractionKind::SetTextSpacing(spacing) => {
                Some(format!("Text spacing: {}", text_spacing_label(spacing)))
            }
            InteractionKind::SetTextSmoothing(smoothing) => {
                Some(format!("Text smoothing: {}", smoothing_label(smoothing)))
            }
            InteractionKind::SetThemePreset(theme) => {
                Some(format!("Theme: {}", theme_preset_label(theme)))
            }
            InteractionKind::SetVoiceAutoInsert(enabled) => Some(if enabled {
                "Voice auto insert: on".to_string()
            } else {
                "Voice auto insert: off".to_string()
            }),
            InteractionKind::SetLlmEnabled(enabled) => Some(if enabled {
                "LLM suggestions: on".to_string()
            } else {
                "LLM suggestions: off".to_string()
            }),
            InteractionKind::SetLlmModel(_) => Some("LLM model preset".to_string()),
            InteractionKind::SetLlmTemperature(temp) => {
                Some(format!("LLM creativity: {}", llm_temperature_label(temp)))
            }
            InteractionKind::SelectNextToken(index) => {
                self.chrome.next_token_candidates.get(index).cloned()
            }
            InteractionKind::RewindNextToken => Some("Go back one token".to_string()),
            InteractionKind::ToggleVoiceCapture => {
                Some(if self.chrome.voice_state == VoiceCaptureState::Listening {
                    "Stop listening".to_string()
                } else {
                    "Start listening".to_string()
                })
            }
            InteractionKind::OpenVoiceSettings => Some("Open system voice settings".to_string()),
            InteractionKind::RefreshVoicePermissions => {
                Some("Refresh microphone and speech permissions".to_string())
            }
            InteractionKind::CycleVoiceSample => Some("Use next sample transcript".to_string()),
            InteractionKind::InsertVoiceTranscript => {
                Some("Insert transcript into seed input".to_string())
            }
            InteractionKind::ClearVoiceTranscript => Some("Clear captured transcript".to_string()),
            InteractionKind::HandwritingCanvas => Some("Handwriting canvas".to_string()),
            InteractionKind::UndoHandwritingStroke => {
                Some("Undo last handwriting stroke".to_string())
            }
            InteractionKind::ClearHandwriting => Some("Clear handwriting strokes".to_string()),
            InteractionKind::UseHandwritingCandidate(index) => {
                self.chrome.handwriting_candidates.get(index).cloned()
            }
            InteractionKind::VirtualKeyboardKey(key) => Some(match key {
                VirtualKeyboardKey::Character(ch) => ch.to_string(),
                VirtualKeyboardKey::Text(text) => text.to_string(),
                VirtualKeyboardKey::Space => "Space".to_string(),
                VirtualKeyboardKey::Backspace => "Backspace".to_string(),
                VirtualKeyboardKey::Shift => "Shift".to_string(),
                VirtualKeyboardKey::ToggleNumeric => "Numbers".to_string(),
                VirtualKeyboardKey::ToggleAlphabetic => "Letters".to_string(),
            }),
            InteractionKind::Candidate(index) => self
                .chrome
                .sentence_candidate_source_indices
                .iter()
                .position(|candidate_index| *candidate_index == index)
                .and_then(|display_index| {
                    self.chrome.sentence_candidates.get(display_index).cloned()
                })
                .or_else(|| self.chrome.sentence_candidates.get(index).cloned()),
        }
    }
}

fn display_text_scale_label(value: suzaku_map::ime::gpu::DisplayTextScale) -> &'static str {
    match value {
        suzaku_map::ime::gpu::DisplayTextScale::Small => "Small",
        suzaku_map::ime::gpu::DisplayTextScale::Medium => "Medium",
        suzaku_map::ime::gpu::DisplayTextScale::Large => "Large",
    }
}

fn density_label(value: suzaku_map::ime::gpu::CandidateDensity) -> &'static str {
    match value {
        suzaku_map::ime::gpu::CandidateDensity::Compact => "Compact",
        suzaku_map::ime::gpu::CandidateDensity::Cozy => "Cozy",
    }
}

fn preview_style_label(value: suzaku_map::ime::gpu::PreviewStyle) -> &'static str {
    match value {
        suzaku_map::ime::gpu::PreviewStyle::Compact => "Compact",
        suzaku_map::ime::gpu::PreviewStyle::Full => "Full",
    }
}

fn font_face_label(value: suzaku_map::ime::gpu::FontFaceChoice) -> &'static str {
    match value {
        suzaku_map::ime::gpu::FontFaceChoice::Auto => "Auto",
        suzaku_map::ime::gpu::FontFaceChoice::Monaco => "Monaco",
        suzaku_map::ime::gpu::FontFaceChoice::Menlo => "Menlo",
        suzaku_map::ime::gpu::FontFaceChoice::Geneva => "Geneva",
        suzaku_map::ime::gpu::FontFaceChoice::Helvetica => "Helvetica",
        suzaku_map::ime::gpu::FontFaceChoice::PingFang => "PingFang",
        suzaku_map::ime::gpu::FontFaceChoice::ArialUnicode => "Arial Unicode",
    }
}

fn text_spacing_label(value: suzaku_map::ime::gpu::TextSpacing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSpacing::Tight => "Tight",
        suzaku_map::ime::gpu::TextSpacing::Normal => "Normal",
        suzaku_map::ime::gpu::TextSpacing::Relaxed => "Relaxed",
    }
}

fn smoothing_label(value: suzaku_map::ime::gpu::TextSmoothing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSmoothing::Sharp => "Sharp",
        suzaku_map::ime::gpu::TextSmoothing::Smooth => "Smooth",
    }
}

fn theme_preset_label(value: suzaku_map::ime::gpu::ThemePreset) -> &'static str {
    match value {
        suzaku_map::ime::gpu::ThemePreset::Daylight => "Daylight",
        suzaku_map::ime::gpu::ThemePreset::DeviceDark => "Device Dark",
    }
}

fn llm_temperature_label(value: suzaku_map::ime::gpu::LlmTemperaturePreset) -> &'static str {
    match value {
        suzaku_map::ime::gpu::LlmTemperaturePreset::Focused => "Focused",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced => "Balanced",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Expressive => "Expressive",
    }
}
