use super::{PanelState, PanelWindowKind};
use crate::render::PanelOverlay;
use std::time::{Duration, Instant};
use suzaku_map::ime::gpu::{
    CandidateQuad, InteractionKind, InteractiveTarget, LlmModelPreset, RenderScene, TextAlign,
    TextBlock, TextRole, ThemePreset, VirtualKeyboardKey, VoiceCaptureState,
};

const TOOLTIP_DELAY: Duration = Duration::from_millis(600);

#[derive(Default)]
pub(super) struct HoverTooltipState {
    target: Option<InteractiveTarget>,
    reveal_at: Option<Instant>,
    visible: bool,
}

impl HoverTooltipState {
    pub(super) fn track(&mut self, target: Option<InteractiveTarget>, now: Instant) -> bool {
        if self.target == target {
            return false;
        }
        let was_visible = self.visible;
        self.target = target;
        self.visible = false;
        self.reveal_at = target.map(|_| now + TOOLTIP_DELAY);
        was_visible
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.reveal_at
    }

    pub(super) fn advance(&mut self, now: Instant) -> bool {
        if self.reveal_at.is_some_and(|deadline| now >= deadline) {
            self.reveal_at = None;
            self.visible = self.target.is_some();
            return self.visible;
        }
        false
    }

    pub(super) fn visible_target(&self) -> Option<InteractiveTarget> {
        self.target.filter(|_| self.visible)
    }

    // Keep the target so clicking/typing does not immediately rearm the same tooltip.
    pub(super) fn dismiss(&mut self) -> bool {
        self.reveal_at = None;
        std::mem::take(&mut self.visible)
    }

    pub(super) fn clear(&mut self) -> bool {
        let was_visible = self.visible;
        *self = Self::default();
        was_visible
    }
}

fn tooltip_colors(theme: ThemePreset) -> ([f32; 4], [f32; 4], [f32; 4]) {
    match theme {
        ThemePreset::Baihu | ThemePreset::Qinglong | ThemePreset::Xuanwu => theme.tooltip_colors(),
        ThemePreset::Suzaku => (
            suzaku_map::ime::gpu::srgb_color(0xFFFAF4),
            suzaku_map::ime::gpu::srgb_color(0x34272A),
            suzaku_map::ime::gpu::srgb_color(0xC8A580),
        ),
        ThemePreset::Daylight => (
            [0.97, 0.98, 1.0, 1.0],
            [0.18, 0.23, 0.32, 1.0],
            [0.56, 0.66, 0.80, 1.0],
        ),
        ThemePreset::Solarized => (
            [0.98, 0.95, 0.87, 1.0],
            [0.18, 0.16, 0.14, 1.0],
            [0.65, 0.54, 0.39, 1.0],
        ),
        ThemePreset::DeviceDark => (
            [0.12, 0.16, 0.23, 1.0],
            [0.95, 0.97, 1.0, 1.0],
            [0.45, 0.58, 0.74, 1.0],
        ),
        ThemePreset::HighContrast => (
            [0.02, 0.03, 0.05, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            [0.15, 0.86, 1.0, 1.0],
        ),
    }
}

fn hint_overlay(
    text: String,
    anchor: [f32; 4],
    viewport: [f32; 2],
    scale: f32,
    theme: ThemePreset,
) -> Option<PanelOverlay> {
    let margin = 8.0 * scale;
    let padding = [8.0 * scale, 6.0 * scale];
    let available_width = (viewport[0] - margin * 2.0).min(340.0 * scale);
    let available_height = viewport[1] - margin * 2.0;
    if available_width <= padding[0] * 2.0 || available_height <= padding[1] * 2.0 {
        return None;
    }
    let (surface, text_color, border) = tooltip_colors(theme);
    let block = TextBlock {
        text,
        origin: [0.0; 2],
        max_width: available_width - padding[0] * 2.0,
        pixel_size: 2.2 * scale,
        letter_spacing: -0.1 * scale,
        line_gap: 3.0 * scale,
        max_lines: 3,
        color: text_color,
        align: TextAlign::Left,
        role: TextRole::HeaderStatus,
    };
    let measured = block.layout_in_rect([0.0, 0.0, available_width, available_height], padding);
    let width = (measured.bounds[2] + padding[0] * 2.0 + 1.0)
        .max(72.0 * scale)
        .min(available_width);
    let height = (measured.bounds[3] + padding[1] * 2.0).min(available_height);
    let x = (anchor[0] + anchor[2] * 0.5 - width * 0.5).clamp(margin, viewport[0] - margin - width);
    let below = anchor[1] + anchor[3] + margin;
    let above = anchor[1] - height - margin;
    let y = if below + height <= viewport[1] - margin {
        below
    } else {
        above
    }
    .clamp(margin, viewport[1] - margin - height);
    let rect = [x, y, width, height];
    let layout = block.layout_in_rect(rect, padding);
    let stroke = scale.max(1.0).min(width * 0.5).min(height * 0.5);
    Some(PanelOverlay {
        quads: vec![
            CandidateQuad::rounded(rect, surface, 8.0 * scale),
            CandidateQuad::outline(rect, border, 8.0 * scale, stroke),
        ],
        atlas_glyphs: layout.atlas_glyphs,
    })
}

impl PanelState {
    fn tooltip_scale(&self) -> f32 {
        let baseline = if self.kind == PanelWindowKind::Settings {
            520.0
        } else {
            900.0
        };
        (self.renderer.scene_width / baseline).clamp(0.85, 1.55)
    }

    pub(super) fn clear_pointer_hover(&mut self) -> bool {
        self.cursor_position = None;
        let tooltip_visible = self.interaction.tooltip.clear();
        let hovered = self.interaction.hovered_interaction.take().is_some();
        let compact_hovered = std::mem::take(&mut self.interaction.compact_hovered);
        self.update_pointer_cursor();
        tooltip_visible || hovered || compact_hovered
    }

    pub(super) fn tooltip_can_arm(&self) -> bool {
        !self.interaction.last_input_was_touch
            && self.interaction.pressed_interaction.is_none()
            && !self.interaction.panel_dragging
            && !self.interaction.scale_dragging
            && !self.interaction.settings_scroll_dragging
            && !self.interaction.handwriting_dragging
            && !(self.kind == PanelWindowKind::Main && self.chrome.compact_mode)
            && self.last_commit_feedback.is_none()
    }

    pub(super) fn build_hover_tooltip(&self, scene: &RenderScene) -> Option<PanelOverlay> {
        if !self.tooltip_can_arm() {
            return None;
        }
        let target = self.interaction.tooltip.visible_target()?;
        let (x, y) = self.cursor_position?;
        // A settings reflow or a newly arrived candidate can move the control without a pointer event.
        if scene.hit_interaction(x, y) != Some(target.kind)
            || !scene.interactive_targets.contains(&target)
        {
            return None;
        }
        let text = self.interaction_hint(target.kind)?;
        hint_overlay(
            text,
            target.rect,
            [self.renderer.scene_width, self.renderer.scene_height],
            self.tooltip_scale(),
            self.chrome.theme_preset,
        )
    }

    pub(super) fn build_commit_feedback(&self) -> Option<PanelOverlay> {
        if self.kind != PanelWindowKind::Main || self.chrome.compact_mode {
            return None;
        }
        let text = self.last_commit_feedback.as_ref()?;
        hint_overlay(
            text.clone(),
            [0.0, 0.0, self.renderer.scene_width, 0.0],
            [self.renderer.scene_width, self.renderer.scene_height],
            self.tooltip_scale(),
            self.chrome.theme_preset,
        )
    }

    pub(super) fn interaction_hint(&self, kind: InteractionKind) -> Option<String> {
        match kind {
            InteractionKind::SeedInput => Some(if self.is_focused && self.chrome.input_focused {
                "Type here in any input tab; Enter / Esc finishes editing.".into()
            } else {
                "Click here to type with your keyboard.".into()
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
            // A grab cursor is enough; a viewport-wide tooltip would cover other controls.
            InteractionKind::DragWindow => None,
            InteractionKind::ClosePanel => Some("Hide panel to system tray".to_string()),
            InteractionKind::SettingsToggle => Some(if self.kind == PanelWindowKind::Settings {
                "Close settings".to_string()
            } else {
                "Panel settings".to_string()
            }),
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
            InteractionKind::SetLlmModel(model) => {
                Some(format!("LLM model: {}", llm_model_label(model)))
            }
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
            InteractionKind::SettingsSearchInput => None,
            InteractionKind::SettingsSearchClear => Some("Clear settings search".to_string()),
            InteractionKind::ToggleSettingsSection(section_index) => {
                Some(format!("Toggle settings section {section_index}"))
            }
            InteractionKind::SettingsScrollTrack | InteractionKind::SettingsScrollHandle => None,
            InteractionKind::SelectNextToken(index) => self
                .next_token_completions
                .get(index)
                .map(|edit| format!("Complete word: {}", edit.seed_after)),
            InteractionKind::RewindNextToken => Some("Undo last word completion".to_string()),
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
            InteractionKind::HandwritingCanvas => None,
            InteractionKind::UndoHandwritingStroke => {
                Some("Undo last handwriting stroke".to_string())
            }
            InteractionKind::ClearHandwriting => Some("Clear handwriting strokes".to_string()),
            InteractionKind::UseHandwritingCandidate(index) => {
                self.chrome.handwriting_candidates.get(index).cloned()
            }
            InteractionKind::VirtualKeyboardKey(VirtualKeyboardKey::Backspace) => {
                Some("Backspace".to_string())
            }
            InteractionKind::VirtualKeyboardKey(_) => None,
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
    value.label()
}

pub(super) fn llm_temperature_label(
    value: suzaku_map::ime::gpu::LlmTemperaturePreset,
) -> &'static str {
    match value {
        suzaku_map::ime::gpu::LlmTemperaturePreset::Focused => "Focused",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Balanced => "Balanced",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Expressive => "Expressive",
        suzaku_map::ime::gpu::LlmTemperaturePreset::Custom(_) => "Custom",
    }
}

pub(super) fn llm_model_label(value: LlmModelPreset) -> &'static str {
    match value {
        LlmModelPreset::Configured => "Configured service (suzaku_tool model configure)",
    }
}

#[cfg(test)]
mod tests {
    use super::{HoverTooltipState, TOOLTIP_DELAY, hint_overlay};
    use super::{
        density_label, display_text_scale_label, font_face_label, llm_model_label,
        llm_temperature_label, preview_style_label, smoothing_label, text_spacing_label,
        theme_preset_label,
    };
    use std::time::{Duration, Instant};
    use suzaku_map::ime::gpu::{
        CandidateDensity, DisplayTextScale, FontFaceChoice, LlmModelPreset, LlmTemperaturePreset,
        PreviewStyle, TextSmoothing, TextSpacing, ThemePreset,
    };
    use suzaku_map::ime::gpu::{InteractionKind, InteractiveTarget};

    fn target() -> InteractiveTarget {
        InteractiveTarget {
            kind: InteractionKind::SettingsToggle,
            rect: [290.0, 12.0, 22.0, 22.0],
        }
    }

    #[test]
    fn tooltip_reveals_once_after_a_stationary_hover_without_continuous_frames() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY));
        assert!(!tooltip.advance(now + TOOLTIP_DELAY - Duration::from_millis(1)));
        // Motion within the same button keeps the original deadline and anchor.
        tooltip.track(Some(target()), now + Duration::from_millis(200));
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY));
        assert!(tooltip.advance(now + TOOLTIP_DELAY));
        assert_eq!(tooltip.visible_target(), Some(target()));
        assert_eq!(tooltip.deadline(), None);
        assert!(!tooltip.advance(now + TOOLTIP_DELAY * 2));
    }

    #[test]
    fn leaving_or_losing_focus_clears_visible_and_pending_tooltips() {
        for visible in [false, true] {
            let now = Instant::now();
            let mut tooltip = HoverTooltipState::default();
            tooltip.track(Some(target()), now);
            if visible {
                tooltip.advance(now + TOOLTIP_DELAY);
            }
            assert_eq!(tooltip.clear(), visible);
            assert!(tooltip.visible_target().is_none());
            assert!(tooltip.deadline().is_none());
            assert!(!tooltip.advance(now + TOOLTIP_DELAY * 2));
        }
    }

    #[test]
    fn clicking_or_typing_suppresses_the_hint_until_the_pointer_leaves_the_control() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        tooltip.advance(now + TOOLTIP_DELAY);
        assert!(tooltip.dismiss());
        tooltip.track(Some(target()), now + TOOLTIP_DELAY * 2);
        assert!(tooltip.deadline().is_none());
        assert!(tooltip.visible_target().is_none());
        tooltip.track(None, now + TOOLTIP_DELAY * 2);
        tooltip.track(Some(target()), now + TOOLTIP_DELAY * 2);
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY * 3));
    }

    #[test]
    fn changing_targets_or_reflowing_buttons_restarts_the_delay() {
        let now = Instant::now();
        let mut tooltip = HoverTooltipState::default();
        tooltip.track(Some(target()), now);
        tooltip.advance(now + TOOLTIP_DELAY);
        let mut moved = target();
        moved.rect[0] -= 40.0;
        assert!(tooltip.track(Some(moved), now + TOOLTIP_DELAY));
        assert!(tooltip.visible_target().is_none());
        assert_eq!(tooltip.deadline(), Some(now + TOOLTIP_DELAY * 2));
    }

    #[test]
    fn tooltip_surfaces_are_opaque_and_text_stays_inside_the_window_at_all_edges() {
        for viewport in [
            [420.0, 300.0],
            [520.0, 340.0],
            [900.0, 480.0],
            [1395.0, 806.0],
        ] {
            for scale in [0.85, 1.0, 1.55] {
                for anchor in [
                    [0.0, 0.0, 24.0, 24.0],
                    [viewport[0] - 24.0, 0.0, 24.0, 24.0],
                    [0.0, viewport[1] - 24.0, 24.0, 24.0],
                    [viewport[0] - 24.0, viewport[1] - 24.0, 24.0, 24.0],
                ] {
                    for theme in [
                        ThemePreset::Daylight,
                        ThemePreset::Solarized,
                        ThemePreset::DeviceDark,
                        ThemePreset::HighContrast,
                    ] {
                        let overlay = hint_overlay("Refresh microphone and speech permissions; long hints must wrap inside the panel".into(), anchor, viewport, scale, theme).unwrap();
                        let rect = overlay.quads[0].rect;
                        assert!(rect[0] >= 0.0 && rect[1] >= 0.0);
                        assert!(
                            rect[0] + rect[2] <= viewport[0] && rect[1] + rect[3] <= viewport[1]
                        );
                        assert!(
                            rect[1] >= anchor[1] + anchor[3] || rect[1] + rect[3] <= anchor[1],
                            "tooltip covers its own control"
                        );
                        assert!(overlay.quads.iter().all(|quad| quad.color[3] == 1.0));
                        for glyph in &overlay.atlas_glyphs {
                            assert!(glyph.rect[0] >= rect[0] && glyph.rect[1] >= rect[1]);
                            assert!(glyph.rect[0] + glyph.rect[2] <= rect[0] + rect[2] + 0.01);
                            assert!(glyph.rect[1] + glyph.rect[3] <= rect[1] + rect[3] + 0.01);
                        }
                    }
                }
            }
        }
        assert!(
            hint_overlay(
                "hint".into(),
                [0.0; 4],
                [10.0, 10.0],
                1.0,
                ThemePreset::Daylight
            )
            .is_none()
        );
    }

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
        assert_eq!(
            font_face_label(FontFaceChoice::ArialUnicode),
            "Arial Unicode"
        );
        assert_eq!(theme_preset_label(ThemePreset::Daylight), "Daylight");
        assert_eq!(theme_preset_label(ThemePreset::DeviceDark), "Device Dark");
        assert_eq!(
            theme_preset_label(ThemePreset::HighContrast),
            "High Contrast"
        );
    }

    #[test]
    fn panel_hint_temperature_labels_are_readable() {
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Focused),
            "Focused"
        );
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Balanced),
            "Balanced"
        );
        assert_eq!(
            llm_temperature_label(LlmTemperaturePreset::Expressive),
            "Expressive"
        );
    }

    #[test]
    fn panel_hint_llm_model_label() {
        assert_eq!(
            llm_model_label(LlmModelPreset::Configured),
            "Configured service (suzaku_tool model configure)"
        );
    }
}
