use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::gpu::{
    CandidateQuad, InteractionKind, RenderScene, TextAlign, TextBlock, TextRole,
    VirtualKeyboardKey, VoiceCaptureState,
};

impl PanelState {
    pub(super) fn append_hover_tooltip(&self, scene: &mut RenderScene) {
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

    pub(super) fn append_commit_feedback(&self, scene: &mut RenderScene) {
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
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::VirtualKeyboard) => {
                Some("Virtual keyboard".to_string())
            }
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::Dictation) => {
                Some("Voice input".to_string())
            }
            InteractionKind::InputModeButton(suzaku_map::ime::gpu::InputMode::Handwriting) => {
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
            InteractionKind::DecreaseWindowScale => Some("Shrink window scale".to_string()),
            InteractionKind::DragWindowScale => Some("Drag to resize window".to_string()),
            InteractionKind::IncreaseWindowScale => Some("Enlarge window scale".to_string()),
            InteractionKind::ResetWindowScale => Some("Reset scale to 100%".to_string()),
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
            InteractionKind::SetPointerTapSlopTenths(value) => {
                Some(format!("Tap slop: {:.1}px", value as f32 / 10.0))
            }
            InteractionKind::SetPointerTapMaxMs(value) => Some(format!("Tap timeout: {}ms", value)),
            InteractionKind::SetPointerTargetSlopTenths(value) => {
                Some(format!("Target slop: {:.1}px", value as f32 / 10.0))
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

pub(super) fn display_text_scale_label(
    value: suzaku_map::ime::gpu::DisplayTextScale,
) -> &'static str {
    match value {
        suzaku_map::ime::gpu::DisplayTextScale::Small => "Small",
        suzaku_map::ime::gpu::DisplayTextScale::Medium => "Medium",
        suzaku_map::ime::gpu::DisplayTextScale::Large => "Large",
    }
}

pub(super) fn density_label(value: suzaku_map::ime::gpu::CandidateDensity) -> &'static str {
    match value {
        suzaku_map::ime::gpu::CandidateDensity::Compact => "Compact",
        suzaku_map::ime::gpu::CandidateDensity::Cozy => "Cozy",
    }
}

pub(super) fn preview_style_label(value: suzaku_map::ime::gpu::PreviewStyle) -> &'static str {
    match value {
        suzaku_map::ime::gpu::PreviewStyle::Compact => "Compact",
        suzaku_map::ime::gpu::PreviewStyle::Full => "Full",
    }
}

pub(super) fn font_face_label(value: suzaku_map::ime::gpu::FontFaceChoice) -> &'static str {
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

pub(super) fn text_spacing_label(value: suzaku_map::ime::gpu::TextSpacing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSpacing::Tight => "Tight",
        suzaku_map::ime::gpu::TextSpacing::Normal => "Normal",
        suzaku_map::ime::gpu::TextSpacing::Relaxed => "Relaxed",
    }
}

pub(super) fn smoothing_label(value: suzaku_map::ime::gpu::TextSmoothing) -> &'static str {
    match value {
        suzaku_map::ime::gpu::TextSmoothing::Sharp => "Sharp",
        suzaku_map::ime::gpu::TextSmoothing::Smooth => "Smooth",
    }
}

pub(super) fn theme_preset_label(value: suzaku_map::ime::gpu::ThemePreset) -> &'static str {
    match value {
        suzaku_map::ime::gpu::ThemePreset::Daylight => "Daylight",
        suzaku_map::ime::gpu::ThemePreset::DeviceDark => "Device Dark",
        suzaku_map::ime::gpu::ThemePreset::HighContrast => "High Contrast",
        suzaku_map::ime::gpu::ThemePreset::Solarized => "Solarized",
    }
}

pub(super) fn llm_temperature_label(
    value: suzaku_map::ime::gpu::LlmTemperaturePreset,
) -> &'static str {
    match value {
        suzaku_map::ime::gpu::LlmTemperaturePreset::Focused => "Focused",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced => "Balanced",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Expressive => "Expressive",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        density_label, display_text_scale_label, font_face_label, llm_temperature_label,
        preview_style_label, smoothing_label, text_spacing_label, theme_preset_label,
    };
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, LlmTemperaturePreset, PreviewStyle,
        TextSmoothing, TextSpacing, ThemePreset,
    };

    #[test]
    fn panel_hint_scale_labels_cover_expected_values() {
        assert_eq!(display_text_scale_label(DisplayTextScale::Small), "Small");
        assert_eq!(display_text_scale_label(DisplayTextScale::Medium), "Medium");
        assert_eq!(display_text_scale_label(DisplayTextScale::Large), "Large");
        assert_eq!(density_label(CandidateDensity::Compact), "Compact");
        assert_eq!(density_label(CandidateDensity::Cozy), "Cozy");
    }

    #[test]
    fn panel_hint_style_labels_cover_expected_values() {
        assert_eq!(preview_style_label(PreviewStyle::Compact), "Compact");
        assert_eq!(preview_style_label(PreviewStyle::Full), "Full");
        assert_eq!(text_spacing_label(TextSpacing::Tight), "Tight");
        assert_eq!(text_spacing_label(TextSpacing::Normal), "Normal");
        assert_eq!(text_spacing_label(TextSpacing::Relaxed), "Relaxed");
        assert_eq!(smoothing_label(TextSmoothing::Sharp), "Sharp");
        assert_eq!(smoothing_label(TextSmoothing::Smooth), "Smooth");
    }

    #[test]
    fn panel_hint_font_and_theme_labels_are_stable() {
        assert_eq!(font_face_label(FontFaceChoice::Auto), "Auto");
        assert_eq!(font_face_label(FontFaceChoice::Monaco), "Monaco");
        assert_eq!(font_face_label(FontFaceChoice::ArialUnicode), "Arial Unicode");
        assert_eq!(theme_preset_label(ThemePreset::Daylight), "Daylight");
        assert_eq!(theme_preset_label(ThemePreset::DeviceDark), "Device Dark");
        assert_eq!(theme_preset_label(ThemePreset::HighContrast), "High Contrast");
    }

    #[test]
    fn panel_hint_temperature_labels_are_readable() {
        assert_eq!(llm_temperature_label(LlmTemperaturePreset::Focused), "Focused");
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Balanced),
            "Balanced"
        );
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Expressive),
            "Expressive"
        );
    }
}
