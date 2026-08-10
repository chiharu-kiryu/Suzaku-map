use super::{layout_text_block, point_in_rect};

pub const PANEL_SCALE_STEP: f32 = 0.1;
pub const PANEL_SCALE_MIN: f32 = 0.65;
pub const PANEL_SCALE_MAX: f32 = 1.55;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateQuad {
    pub rect: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTarget {
    pub index: usize,
    pub rect: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    HeaderTitle,
    HeaderStatus,
    InputLabel,
    InputValue,
    ToolLabel,
    ToolButton,
    KeyboardKey,
    SettingLabel,
    SettingOption,
    VoiceLabel,
    VoiceButton,
    VoiceTranscript,
    HandwritingLabel,
    HandwritingButton,
    HandwritingCandidate,
    NextTokenLabel,
    NextTokenChip,
    CandidatePrimary,
    CandidateMeta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    VirtualKeyboard,
    Dictation,
    Handwriting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKeyboardKey {
    Character(char),
    Text(&'static str),
    Space,
    Backspace,
    Shift,
    ToggleNumeric,
    ToggleAlphabetic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayTextScale {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateDensity {
    Compact,
    Cozy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewStyle {
    Compact,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFaceChoice {
    Auto,
    Monaco,
    Menlo,
    Geneva,
    Helvetica,
    PingFang,
    ArialUnicode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSpacing {
    Tight,
    Normal,
    Relaxed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSmoothing {
    Sharp,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    Daylight,
    DeviceDark,
    HighContrast,
    Solarized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmModelPreset {
    Llama32_3b,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LlmTemperaturePreset {
    Focused,
    Balanced,
    Expressive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceCaptureState {
    Idle,
    Listening,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoicePermissionState {
    Unknown,
    Ready,
    Pending,
    Denied,
    Error,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanelChromeState {
    pub seed_text: String,
    pub compact_mode: bool,
    pub input_modes_expanded: bool,
    pub active_input_mode: InputMode,
    pub input_focused: bool,
    pub caret_index: usize,
    pub keyboard_shifted: bool,
    pub keyboard_numeric: bool,
    pub settings_open: bool,
    pub text_scale: DisplayTextScale,
    pub candidate_density: CandidateDensity,
    pub preview_style: PreviewStyle,
    pub font_face: FontFaceChoice,
    pub text_spacing: TextSpacing,
    pub text_smoothing: TextSmoothing,
    pub theme_preset: ThemePreset,
    pub voice_state: VoiceCaptureState,
    pub voice_permission: VoicePermissionState,
    pub voice_backend_label: String,
    pub voice_supports_live_capture: bool,
    pub hovered_interaction: Option<InteractionKind>,
    pub pressed_interaction: Option<InteractionKind>,
    pub voice_transcript: String,
    pub voice_visual_phase: u8,
    pub voice_auto_insert: bool,
    pub llm_enabled: bool,
    pub llm_model: LlmModelPreset,
    pub llm_temperature: LlmTemperaturePreset,
    pub pointer_tap_slop_tenths: u16,
    pub pointer_tap_max_ms: u16,
    pub pointer_target_slop_tenths: u16,
    pub window_scale: f32,
    pub composed_tokens: Vec<String>,
    pub next_token_candidates: Vec<String>,
    pub sentence_candidates: Vec<String>,
    pub sentence_candidate_source_indices: Vec<usize>,
    pub handwriting_strokes: Vec<Vec<[f32; 2]>>,
    pub handwriting_candidates: Vec<String>,
    pub handwriting_hint: String,
    pub settings_scroll_offset: f32,
    pub settings_search_query: String,
    pub settings_search_focused: bool,
    pub settings_collapsed_sections: Vec<bool>,
}

impl Default for PanelChromeState {
    fn default() -> Self {
        Self {
            seed_text: String::new(),
            compact_mode: false,
            input_modes_expanded: false,
            active_input_mode: InputMode::VirtualKeyboard,
            input_focused: true,
            caret_index: 0,
            keyboard_shifted: false,
            keyboard_numeric: false,
            settings_open: false,
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Monaco,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::Daylight,
            voice_state: VoiceCaptureState::Idle,
            voice_permission: VoicePermissionState::Unknown,
            voice_backend_label: "Unknown Voice Host".to_string(),
            voice_supports_live_capture: false,
            hovered_interaction: None,
            pressed_interaction: None,
            voice_transcript: String::new(),
            voice_visual_phase: 0,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 100,
            pointer_tap_max_ms: 420,
            pointer_target_slop_tenths: 50,
            window_scale: 1.0,
            composed_tokens: Vec::new(),
            next_token_candidates: Vec::new(),
            sentence_candidates: Vec::new(),
            sentence_candidate_source_indices: Vec::new(),
            handwriting_strokes: Vec::new(),
            handwriting_candidates: Vec::new(),
            handwriting_hint: "Draw a seed word with mouse or touch.".to_string(),
            settings_scroll_offset: 0.0,
            settings_search_query: String::new(),
            settings_search_focused: false,
            settings_collapsed_sections: Vec::new(),
        }
    }
}

impl PanelChromeState {
    pub fn focus_input(&mut self) {
        self.input_focused = true;
        self.caret_index = self.caret_index.min(self.seed_text.chars().count());
    }

    pub fn blur_input(&mut self) {
        self.input_focused = false;
    }

    pub fn set_seed_text(&mut self, text: String) {
        self.seed_text = text;
        self.caret_index = self.caret_index.min(self.seed_text.chars().count());
    }

    pub fn insert_text(&mut self, text: &str) {
        let mut chars: Vec<char> = self.seed_text.chars().collect();
        let insert: Vec<char> = text.chars().collect();
        self.caret_index = self.caret_index.min(chars.len());
        chars.splice(self.caret_index..self.caret_index, insert.iter().copied());
        self.seed_text = chars.into_iter().collect();
        self.caret_index += insert.len();
    }

    pub fn backspace(&mut self) {
        let mut chars: Vec<char> = self.seed_text.chars().collect();
        self.caret_index = self.caret_index.min(chars.len());
        if self.caret_index == 0 {
            return;
        }

        chars.remove(self.caret_index - 1);
        self.seed_text = chars.into_iter().collect();
        self.caret_index -= 1;
    }

    pub fn move_caret_left(&mut self) {
        self.caret_index = self.caret_index.saturating_sub(1);
    }

    pub fn move_caret_right(&mut self) {
        self.caret_index = (self.caret_index + 1).min(self.seed_text.chars().count());
    }

    pub fn move_caret_to_end(&mut self) {
        self.caret_index = self.seed_text.chars().count();
    }

    pub fn toggle_shift(&mut self) {
        self.keyboard_shifted = !self.keyboard_shifted;
    }

    pub fn use_numeric_keyboard(&mut self) {
        self.keyboard_numeric = true;
        self.keyboard_shifted = false;
    }

    pub fn use_alpha_keyboard(&mut self) {
        self.keyboard_numeric = false;
    }

    pub fn can_decrease_window_scale(&self) -> bool {
        self.window_scale > PANEL_SCALE_MIN + 0.0001
    }

    pub fn can_increase_window_scale(&self) -> bool {
        self.window_scale < PANEL_SCALE_MAX - 0.0001
    }

    pub fn can_reset_window_scale(&self) -> bool {
        (self.window_scale - 1.0).abs() > 0.0001
    }

    pub fn display_text(&self) -> String {
        if self.seed_text.is_empty() {
            "Type seed words".to_string()
        } else {
            self.seed_text.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionKind {
    SeedInput,
    ToggleCompactMode,
    InputModesToggle,
    InputModeButton(InputMode),
    VirtualKeyboardKey(VirtualKeyboardKey),
    SettingsToggle,
    SetTextScale(DisplayTextScale),
    SetCandidateDensity(CandidateDensity),
    SetPreviewStyle(PreviewStyle),
    SetFontFace(FontFaceChoice),
    SetTextSpacing(TextSpacing),
    SetTextSmoothing(TextSmoothing),
    SetThemePreset(ThemePreset),
    DecreaseWindowScale,
    IncreaseWindowScale,
    DragWindowScale,
    ResetWindowScale,
    SetLlmEnabled(bool),
    SetPointerTapSlopTenths(u16),
    SetPointerTapMaxMs(u16),
    SetPointerTargetSlopTenths(u16),
    SetVoiceAutoInsert(bool),
    SetLlmModel(LlmModelPreset),
    SetLlmTemperature(LlmTemperaturePreset),
    SettingsScrollTrack,
    SettingsScrollHandle,
    SettingsSearchInput,
    SettingsSearchClear,
    ToggleSettingsSection(usize),
    SettingsOptionTextScroll(InteractionKind),
    SelectNextToken(usize),
    RewindNextToken,
    ToggleVoiceCapture,
    OpenVoiceSettings,
    RefreshVoicePermissions,
    CycleVoiceSample,
    InsertVoiceTranscript,
    ClearVoiceTranscript,
    HandwritingCanvas,
    UndoHandwritingStroke,
    ClearHandwriting,
    UseHandwritingCandidate(usize),
    Candidate(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InteractiveTarget {
    pub kind: InteractionKind,
    pub rect: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsScrollMetadata {
    pub track_rect: [f32; 4],
    pub handle_rect: [f32; 4],
    pub content_height: f32,
    pub visible_height: f32,
    pub max_scroll_offset: f32,
    pub handle_drag_range: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    pub text: String,
    pub origin: [f32; 2],
    pub max_width: f32,
    pub pixel_size: f32,
    pub letter_spacing: f32,
    pub line_gap: f32,
    pub max_lines: usize,
    pub color: [f32; 4],
    pub align: TextAlign,
    pub role: TextRole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    pub quads: Vec<CandidateQuad>,
    pub atlas_glyphs: Vec<AtlasGlyph>,
    pub lines: Vec<String>,
    pub truncated: bool,
    pub bounds: [f32; 4],
    pub role: TextRole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSection {
    pub role: TextRole,
    pub layouts: Vec<TextLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderScene {
    pub quads: Vec<CandidateQuad>,
    pub text_quads: Vec<CandidateQuad>,
    pub atlas_glyphs: Vec<AtlasGlyph>,
    pub text_sections: Vec<TextSection>,
    pub hit_targets: Vec<HitTarget>,
    pub interactive_targets: Vec<InteractiveTarget>,
    pub sentence_candidate_truncated: Vec<usize>,
    pub next_token_candidate_truncated: Vec<usize>,
    pub handwriting_candidate_truncated: Vec<usize>,
    pub settings_option_truncated: Vec<InteractionKind>,
    pub settings_scroll_metadata: Option<SettingsScrollMetadata>,
    pub labels: Vec<String>,
    pub selected_label: Option<String>,
    pub draft_text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AtlasGlyph {
    pub ch: char,
    pub rect: [f32; 4],
    pub color: [f32; 4],
}

impl RenderScene {
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.hit_targets
            .iter()
            .find(|target| point_in_rect(x, y, target.rect))
            .map(|target| target.index)
    }

    pub fn hit_interaction(&self, x: f32, y: f32) -> Option<InteractionKind> {
        self.interactive_targets
            .iter()
            .rev()
            .find(|target| point_in_rect(x, y, target.rect))
            .map(|target| target.kind)
    }
}

impl TextBlock {
    pub fn layout(&self) -> TextLayout {
        layout_text_block(self)
    }
}

pub struct WgpuCandidateRenderer {
    pub scene_width: f32,
    pub scene_height: f32,
}
