use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::languages::english::EnglishLanguagePlugin;

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub mode: Mode,
    pub seed_text: String,
    pub active_language: String,
    pub selected_index: usize,
    pub draft_text: String,
    pub committed_text: String,
    pub active_source: InputSource,
    pub candidate_labels: Vec<String>,
    pub confidence: f32,
    pub degraded: bool,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Idle,
    Composing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
    Unknown,
    GazeDwell,
    HandTracking,
    Stylus,
    HardwareKeyboard,
    OnScreenPanel,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    LowPointerPrecision,
    UnstableGaze,
    DegradedMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalState {
    pub pointer_precision: f32,
    pub gaze_stability: f32,
    pub host_intent_weight: f32,
    pub source_confidence: f32,
}

impl Default for SignalState {
    fn default() -> Self {
        Self {
            pointer_precision: 1.0,
            gaze_stability: 1.0,
            host_intent_weight: 0.5,
            source_confidence: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub label: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitResult {
    pub ok: bool,
    pub reason: CommitReason,
    pub text: Option<String>,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitReason {
    Committed,
    NoCandidate,
    ConfirmationRequired,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitOptions {
    pub force: bool,
}

impl Default for CommitOptions {
    fn default() -> Self {
        Self { force: false }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    pub max_candidates: usize,
    pub initial_text: String,
    pub default_language: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_candidates: 6,
            initial_text: String::new(),
            default_language: "en".to_string(),
        }
    }
}

pub trait LanguagePlugin: Send + Sync {
    fn id(&self) -> &str;

    fn display_name(&self) -> &str {
        self.id()
    }

    fn normalize_seed(&self, input: &str) -> String {
        input.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn expand_token(&self, token: &str, degraded: bool) -> Vec<String>;

    fn build_candidates(
        &self,
        parts: &[String],
        seed_text: &str,
        confidence: f32,
    ) -> Vec<Candidate>;
}

#[derive(Clone, Default)]
pub struct LanguageRegistry {
    plugins: HashMap<String, Arc<dyn LanguagePlugin>>,
}

impl LanguageRegistry {
    pub fn with_default_plugins() -> Self {
        let mut registry = Self {
            plugins: HashMap::new(),
        };
        registry.register(EnglishLanguagePlugin::default());
        registry
    }

    pub fn register<P>(&mut self, plugin: P)
    where
        P: LanguagePlugin + 'static,
    {
        self.plugins.insert(
            plugin.id().to_string(),
            Arc::new(plugin) as Arc<dyn LanguagePlugin>,
        );
    }

    pub fn get(&self, language_id: &str) -> Option<Arc<dyn LanguagePlugin>> {
        self.plugins.get(language_id).cloned()
    }

    pub fn contains(&self, language_id: &str) -> bool {
        self.plugins.contains_key(language_id)
    }

    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.plugins.keys().cloned().collect();
        ids.sort();
        ids
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CompositionState {
    mode: Mode,
    seed_text: String,
    active_language: String,
    expansions: Vec<Vec<String>>,
    candidates: Vec<Candidate>,
    selected_index: usize,
    draft_text: String,
    committed_text: String,
    active_source: InputSource,
    signal: SignalState,
    confidence: f32,
    degraded: bool,
    warnings: Vec<Warning>,
    history: Vec<String>,
}

pub struct XRTabletImeEngine {
    config: EngineConfig,
    registry: LanguageRegistry,
    state: CompositionState,
}

impl XRTabletImeEngine {
    pub fn new(config: EngineConfig) -> Self {
        let committed = config.initial_text.clone();
        let registry = LanguageRegistry::with_default_plugins();
        let default_language = if registry.contains(&config.default_language) {
            config.default_language.clone()
        } else {
            "en".to_string()
        };

        Self {
            config,
            registry,
            state: CompositionState {
                mode: Mode::Idle,
                seed_text: String::new(),
                active_language: default_language,
                expansions: Vec::new(),
                candidates: Vec::new(),
                selected_index: 0,
                draft_text: String::new(),
                committed_text: committed,
                active_source: InputSource::Unknown,
                signal: SignalState::default(),
                confidence: 1.0,
                degraded: false,
                warnings: Vec::new(),
                history: Vec::new(),
            },
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            mode: self.state.mode,
            seed_text: self.state.seed_text.clone(),
            active_language: self.state.active_language.clone(),
            selected_index: self.state.selected_index,
            draft_text: self.state.draft_text.clone(),
            committed_text: self.state.committed_text.clone(),
            active_source: self.state.active_source.clone(),
            candidate_labels: self
                .state
                .candidates
                .iter()
                .map(|candidate| candidate.label.clone())
                .collect(),
            confidence: self.state.confidence,
            degraded: self.state.degraded,
            warnings: self.state.warnings.clone(),
        }
    }

    pub fn register_language_plugin<P>(&mut self, plugin: P)
    where
        P: LanguagePlugin + 'static,
    {
        self.registry.register(plugin);
    }

    pub fn available_languages(&self) -> Vec<String> {
        self.registry.ids()
    }

    pub fn set_language(&mut self, language_id: impl AsRef<str>) -> Snapshot {
        let language_id = language_id.as_ref();
        if self.registry.contains(language_id) {
            self.state.active_language = language_id.to_string();
            self.rebuild();
        }
        self.snapshot()
    }

    pub fn set_source(&mut self, source: InputSource) -> Snapshot {
        self.state.active_source = source;
        self.snapshot()
    }

    pub fn update_signal(&mut self, signal: SignalState) -> Snapshot {
        self.state.signal = signal;
        self.state.confidence = self.compute_confidence();
        self.state.degraded = self.state.confidence < 0.45;
        self.state.warnings.clear();

        if self.state.signal.pointer_precision < 0.4 {
            self.state.warnings.push(Warning::LowPointerPrecision);
        }

        if self.state.signal.gaze_stability < 0.35 {
            self.state.warnings.push(Warning::UnstableGaze);
        }

        if self.state.degraded {
            self.state.warnings.push(Warning::DegradedMode);
        }

        self.rebuild();
        self.snapshot()
    }

    pub fn seed(&mut self, input: impl AsRef<str>) -> Snapshot {
        self.state.mode = Mode::Composing;
        self.state.seed_text = self.active_plugin().normalize_seed(input.as_ref());
        self.rebuild();
        self.snapshot()
    }

    pub fn append_seed(&mut self, fragment: impl AsRef<str>) -> Snapshot {
        let fragment = fragment.as_ref().trim();

        self.state.seed_text = if self.state.seed_text.is_empty() {
            fragment.to_string()
        } else if fragment.is_empty() {
            self.state.seed_text.clone()
        } else {
            format!("{} {}", self.state.seed_text, fragment)
        };

        self.state.mode = Mode::Composing;
        self.state.seed_text = self.active_plugin().normalize_seed(&self.state.seed_text);
        self.rebuild();
        self.snapshot()
    }

    pub fn move_selection(&mut self, delta: isize) -> Snapshot {
        if self.state.candidates.is_empty() {
            return self.snapshot();
        }

        let max_index = self.state.candidates.len().saturating_sub(1) as isize;
        let next = (self.state.selected_index as isize + delta).clamp(0, max_index);
        self.state.selected_index = next as usize;
        self.render_draft();
        self.snapshot()
    }

    pub fn select_candidate(&mut self, index: usize) -> Snapshot {
        if index < self.state.candidates.len() {
            self.state.selected_index = index;
            self.render_draft();
        }

        self.snapshot()
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.state.candidates
    }

    pub fn commit(&mut self, options: CommitOptions) -> CommitResult {
        let snapshot = self.snapshot();
        let Some(candidate) = self
            .state
            .candidates
            .get(self.state.selected_index)
            .cloned()
        else {
            return CommitResult {
                ok: false,
                reason: CommitReason::NoCandidate,
                text: None,
                snapshot,
            };
        };

        if !self.passes_control_gate(&candidate, &options) {
            return CommitResult {
                ok: false,
                reason: CommitReason::ConfirmationRequired,
                text: None,
                snapshot,
            };
        }

        self.state.history.push(self.state.committed_text.clone());

        if self.state.committed_text.is_empty() {
            self.state.committed_text = candidate.text;
        } else {
            self.state.committed_text = format!("{} {}", self.state.committed_text, candidate.text);
        }

        self.state.mode = Mode::Idle;
        self.state.seed_text.clear();
        self.state.expansions.clear();
        self.state.candidates.clear();
        self.state.selected_index = 0;
        self.state.draft_text.clear();

        CommitResult {
            ok: true,
            reason: CommitReason::Committed,
            text: Some(self.state.committed_text.clone()),
            snapshot: self.snapshot(),
        }
    }

    pub fn undo(&mut self) -> Option<Snapshot> {
        let previous = self.state.history.pop()?;
        self.state.committed_text = previous;
        Some(self.snapshot())
    }

    fn rebuild(&mut self) {
        let plugin = self.active_plugin();
        self.state.seed_text = plugin.normalize_seed(&self.state.seed_text);
        let tokens = tokenize_seed(&self.state.seed_text);
        self.state.expansions = tokens
            .iter()
            .map(|token| plugin.expand_token(token, self.state.degraded))
            .collect();
        self.state.candidates = self.compose_candidates_with_plugin(plugin.as_ref());

        if self.state.selected_index >= self.state.candidates.len() {
            self.state.selected_index = 0;
        }

        self.render_draft();
    }

    fn compute_confidence(&self) -> f32 {
        clamp01(
            self.state.signal.pointer_precision * 0.35
                + self.state.signal.gaze_stability * 0.25
                + self.state.signal.host_intent_weight * 0.20
                + self.state.signal.source_confidence * 0.20,
        )
    }

    fn compose_candidates_with_plugin(&self, plugin: &dyn LanguagePlugin) -> Vec<Candidate> {
        if self.state.expansions.is_empty() {
            return Vec::new();
        }

        let max_pool = self.config.max_candidates * 3;
        let mut combinations = Vec::new();
        let mut current = Vec::new();
        build_combinations(
            &self.state.expansions,
            0,
            &mut current,
            &mut combinations,
            max_pool,
        );

        let mut candidates: Vec<Candidate> = combinations
            .into_iter()
            .flat_map(|parts| {
                plugin.build_candidates(&parts, &self.state.seed_text, self.state.confidence)
            })
            .collect();

        candidates.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.text.len().cmp(&right.text.len()))
        });

        let mut unique = Vec::new();
        let mut seen = BTreeSet::new();
        let limit = if self.state.degraded {
            3
        } else {
            self.config.max_candidates
        };

        for candidate in candidates {
            if seen.insert(candidate.text.clone()) {
                unique.push(candidate);
            }

            if unique.len() >= limit {
                break;
            }
        }

        if self.state.degraded {
            if let Some(first) = unique.first_mut() {
                first.label = format!("{} [stable]", first.label);
            }
        }

        unique
    }

    fn render_draft(&mut self) {
        self.state.draft_text = self
            .state
            .candidates
            .get(self.state.selected_index)
            .map(|candidate| candidate.text.clone())
            .unwrap_or_default();
    }

    fn passes_control_gate(&self, candidate: &Candidate, options: &CommitOptions) -> bool {
        if options.force {
            return true;
        }

        let long_replacement = candidate.text.len() >= 18;
        let low_confidence = self.state.confidence < 0.55;
        let ambiguous = self.state.candidates.len() > 1 && self.state.selected_index > 0;

        !(long_replacement || low_confidence || ambiguous)
    }

    fn active_plugin(&self) -> Arc<dyn LanguagePlugin> {
        self.registry
            .get(&self.state.active_language)
            .or_else(|| self.registry.get("en"))
            .expect("default English language plugin must be registered")
    }
}

fn tokenize_seed(seed: &str) -> Vec<String> {
    seed.split_whitespace().map(ToString::to_string).collect()
}

pub(crate) fn expand_token_with_lexicon(
    token: &str,
    lexicon: &HashMap<String, Vec<String>>,
    degraded: bool,
) -> Vec<String> {
    let key = token.to_lowercase();
    let mut values = vec![token.to_string()];

    if let Some(extra) = lexicon.get(&key) {
        values.extend(extra.iter().cloned());
    }

    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for value in values {
        if seen.insert(value.clone()) {
            ordered.push(value);
        }
    }

    let limit = if degraded { 2 } else { 4 };
    ordered.into_iter().take(limit).collect()
}

fn build_combinations(
    expansions: &[Vec<String>],
    index: usize,
    current: &mut Vec<String>,
    output: &mut Vec<Vec<String>>,
    max_pool: usize,
) {
    if output.len() >= max_pool {
        return;
    }

    if index == expansions.len() {
        output.push(current.clone());
        return;
    }

    for option in &expansions[index] {
        current.push(option.clone());
        build_combinations(expansions, index + 1, current, output, max_pool);
        current.pop();

        if output.len() >= max_pool {
            return;
        }
    }
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

pub(crate) fn build_sentence_candidates_for_variants<F>(
    parts: &[String],
    seed_text: &str,
    confidence: f32,
    variant_builder: F,
) -> Vec<Candidate>
where
    F: Fn(&str) -> Vec<String>,
{
    let phrase = parts.join(" ").replace("  ", " ").trim().to_string();
    let mut variants = variant_builder(&phrase);

    if variants.is_empty() {
        variants.push(phrase.clone());
    }

    variants
        .into_iter()
        .map(|text| {
            let word_count = text.split_whitespace().count();
            let exact_bonus = if text == seed_text { 0.20 } else { 0.0 };
            let sentence_bonus = if word_count >= 4 {
                0.22
            } else if word_count >= 2 {
                0.12
            } else {
                0.02
            };
            let continuation_bonus = if text.starts_with(&phrase) && text.len() > phrase.len() {
                0.18
            } else {
                0.0
            };
            Candidate {
                label: text.clone(),
                text,
                score: confidence + exact_bonus + sentence_bonus + continuation_bonus,
            }
        })
        .collect()
}

pub(crate) fn contains_all(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| haystack.contains(needle))
}

fn display_candidate_continuation(seed_text: &str, candidate: &str) -> String {
    let seed_tokens: Vec<&str> = seed_text.split_whitespace().collect();
    let candidate_tokens: Vec<&str> = candidate.split_whitespace().collect();

    let mut prefix_len = 0;
    while prefix_len < seed_tokens.len()
        && prefix_len < candidate_tokens.len()
        && seed_tokens[prefix_len].eq_ignore_ascii_case(candidate_tokens[prefix_len])
    {
        prefix_len += 1;
    }

    if prefix_len > 0 && prefix_len < candidate_tokens.len() {
        return candidate_tokens[prefix_len..].join(" ");
    }

    if candidate_tokens.len() > 3 {
        return candidate_tokens[candidate_tokens.len().saturating_sub(3)..].join(" ");
    }

    candidate.to_string()
}

fn compact_phrase(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= 4 {
        return text.to_string();
    }

    format!(
        "{} ... {}",
        words[..2].join(" "),
        words[words.len() - 2..].join(" ")
    )
}

fn measure_text_prefix_width(
    text: &str,
    char_count: usize,
    pixel_size: f32,
    letter_spacing: f32,
) -> f32 {
    text.chars().take(char_count).fold(0.0, |acc, ch| {
        acc + if ch == ' ' {
            pixel_size * 4.0
        } else {
            pixel_size * 6.5 + letter_spacing
        }
    })
}

#[cfg(feature = "gpu")]
pub mod gpu {
    use super::{
        Snapshot, compact_phrase, display_candidate_continuation, measure_text_prefix_width,
    };
    use bytemuck::{Pod, Zeroable};

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
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
        Geneva,
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
        pub voice_state: VoiceCaptureState,
        pub voice_permission: VoicePermissionState,
        pub voice_transcript: String,
        pub llm_enabled: bool,
        pub llm_model: LlmModelPreset,
        pub llm_temperature: LlmTemperaturePreset,
        pub composed_tokens: Vec<String>,
        pub next_token_candidates: Vec<String>,
        pub sentence_candidates: Vec<String>,
        pub handwriting_strokes: Vec<Vec<[f32; 2]>>,
        pub handwriting_candidates: Vec<String>,
        pub handwriting_hint: String,
    }

    impl Default for PanelChromeState {
        fn default() -> Self {
            Self {
                seed_text: String::new(),
                input_modes_expanded: true,
                active_input_mode: InputMode::VirtualKeyboard,
                input_focused: true,
                caret_index: 0,
                keyboard_shifted: false,
                keyboard_numeric: false,
                settings_open: false,
                text_scale: DisplayTextScale::Medium,
                candidate_density: CandidateDensity::Cozy,
                preview_style: PreviewStyle::Compact,
                font_face: FontFaceChoice::Auto,
                text_spacing: TextSpacing::Normal,
                text_smoothing: TextSmoothing::Smooth,
                voice_state: VoiceCaptureState::Idle,
                voice_permission: VoicePermissionState::Unknown,
                voice_transcript: String::new(),
                llm_enabled: true,
                llm_model: LlmModelPreset::Llama32_3b,
                llm_temperature: LlmTemperaturePreset::Balanced,
                composed_tokens: Vec::new(),
                next_token_candidates: Vec::new(),
                sentence_candidates: Vec::new(),
                handwriting_strokes: Vec::new(),
                handwriting_candidates: Vec::new(),
                handwriting_hint: "Draw a seed word with mouse or touch.".to_string(),
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
        SetLlmEnabled(bool),
        SetLlmModel(LlmModelPreset),
        SetLlmTemperature(LlmTemperaturePreset),
        SelectNextToken(usize),
        RewindNextToken,
        ToggleVoiceCapture,
        CycleVoiceSample,
        InsertVoiceTranscript,
        ClearVoiceTranscript,
        HandwritingCanvas,
        ClearHandwriting,
        UseHandwritingCandidate(usize),
        Candidate(usize),
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct InteractiveTarget {
        pub kind: InteractionKind,
        pub rect: [f32; 4],
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

    impl WgpuCandidateRenderer {
        pub fn new(scene_width: f32, scene_height: f32) -> Self {
            Self {
                scene_width,
                scene_height,
            }
        }

        pub fn build_scene(&self, snapshot: &Snapshot) -> RenderScene {
            let chrome = PanelChromeState {
                seed_text: snapshot.seed_text.clone(),
                sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
                ..PanelChromeState::default()
            };
            self.build_panel_scene(snapshot, &chrome)
        }

        pub fn build_panel_scene(
            &self,
            snapshot: &Snapshot,
            chrome: &PanelChromeState,
        ) -> RenderScene {
            let panel_width = self.scene_width.clamp(360.0, 520.0) - 32.0;
            let input_value_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.0,
                DisplayTextScale::Medium => 3.0,
                DisplayTextScale::Large => 4.0,
            };
            let label_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.0,
                DisplayTextScale::Medium => 2.0,
                DisplayTextScale::Large => 3.0,
            };
            let tracking = match chrome.text_spacing {
                TextSpacing::Tight => -0.4,
                TextSpacing::Normal => 0.0,
                TextSpacing::Relaxed => 0.8,
            };
            let base_line_gap = match chrome.text_spacing {
                TextSpacing::Tight => 4.0,
                TextSpacing::Normal => 6.0,
                TextSpacing::Relaxed => 9.0,
            };
            let input_box_h = 52.0;
            let tools_header_h = 28.0;
            let tool_button_h = 28.0;
            let tool_gap = 8.0;
            let extended_input_panel_h = if chrome.input_modes_expanded {
                match chrome.active_input_mode {
                    InputMode::VirtualKeyboard => 170.0,
                    InputMode::Dictation => 116.0,
                    InputMode::Handwriting => 196.0,
                }
            } else {
                0.0
            };
            let tools_content_h = if chrome.input_modes_expanded {
                tool_button_h + 10.0 + extended_input_panel_h
            } else {
                0.0
            };
            let settings_panel_h = if chrome.settings_open { 208.0 } else { 0.0 };
            let item_height = match (chrome.candidate_density, chrome.preview_style) {
                (CandidateDensity::Compact, PreviewStyle::Compact) => 48.0,
                (CandidateDensity::Compact, PreviewStyle::Full) => 62.0,
                (CandidateDensity::Cozy, PreviewStyle::Compact) => 56.0,
                (CandidateDensity::Cozy, PreviewStyle::Full) => 76.0,
            };
            let gap = if chrome.candidate_density == CandidateDensity::Compact {
                6.0
            } else {
                10.0
            };
            let visible_sentence_candidates: Vec<String> =
                chrome.sentence_candidates.iter().take(4).cloned().collect();
            let sentence_count = visible_sentence_candidates.len() as f32;
            let sentence_height = if sentence_count == 0.0 {
                0.0
            } else {
                sentence_count * item_height + (sentence_count - 1.0) * gap
            };
            let chip_rows = if chrome.next_token_candidates.is_empty() {
                0.0
            } else {
                2.0
            };
            let chip_section_h = if chip_rows == 0.0 {
                0.0
            } else {
                82.0
            };
            let panel_height = input_box_h
                + 12.0
                + tools_header_h
                + tools_content_h
                + settings_panel_h
                + 16.0
                + chip_section_h
                + sentence_height;
            let panel_x = ((self.scene_width - panel_width) / 2.0).max(12.0);
            let panel_y = ((self.scene_height - panel_height) / 2.0).max(12.0);
            let input_box_y = panel_y;
            let tools_y = input_box_y + input_box_h + 12.0;
            let settings_y = tools_y + tools_header_h + tools_content_h;
            let suggestions_y = settings_y + settings_panel_h + 16.0;
            let mut quads = Vec::with_capacity(snapshot.candidate_labels.len() + 6);
            let mut text_quads = Vec::new();
            let mut atlas_glyphs = Vec::new();
            let mut text_sections = Vec::new();
            let mut hit_targets = Vec::with_capacity(snapshot.candidate_labels.len());
            let mut interactive_targets = Vec::new();
            quads.push(CandidateQuad {
                rect: [panel_x, input_box_y, panel_width, input_box_h],
                color: if chrome.input_focused {
                    [0.11, 0.16, 0.24, 0.99]
                } else {
                    [0.10, 0.14, 0.22, 0.95]
                },
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SeedInput,
                rect: [panel_x, input_box_y, panel_width, input_box_h],
            });

            let header_layouts = vec![
                TextBlock {
                    text: "Seed input".to_string(),
                    origin: [panel_x + 16.0, input_box_y + 8.0],
                    max_width: panel_width - 36.0,
                    pixel_size: label_px,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: [0.66, 0.76, 0.90, 1.0],
                    align: TextAlign::Left,
                    role: TextRole::InputLabel,
                }
                .layout(),
                TextBlock {
                    text: if chrome.seed_text.is_empty() {
                        "Type a seed word or phrase".to_string()
                    } else {
                        compact_phrase(&chrome.display_text())
                    },
                    origin: [panel_x + 16.0, input_box_y + 24.0],
                    max_width: panel_width - 36.0,
                    pixel_size: input_value_px,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: if chrome.seed_text.is_empty() {
                        [0.58, 0.66, 0.78, 1.0]
                    } else {
                        [0.92, 0.96, 1.0, 1.0]
                    },
                    align: TextAlign::Left,
                    role: TextRole::InputValue,
                }
                .layout(),
            ];
            for layout in &header_layouts {
                text_quads.extend(layout.quads.iter().copied());
                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            }
            text_sections.push(TextSection {
                role: TextRole::InputLabel,
                layouts: header_layouts,
            });
            if chrome.input_focused {
                let caret_x = panel_x
                    + 16.0
                    + measure_text_prefix_width(
                        &chrome.seed_text,
                        chrome.caret_index,
                        input_value_px,
                        tracking,
                    );
                quads.push(CandidateQuad {
                    rect: [caret_x, input_box_y + 22.0, 2.0, 22.0],
                    color: [0.72, 0.88, 1.0, 1.0],
                });
            }

            let tools_header_rect = [panel_x, tools_y, panel_width, tools_header_h];
            quads.push(CandidateQuad {
                rect: tools_header_rect,
                color: [0.12, 0.16, 0.24, 0.96],
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::InputModesToggle,
                rect: tools_header_rect,
            });
            let display_button_rect = [panel_x + panel_width - 94.0, tools_y + 3.0, 78.0, 22.0];
            quads.push(CandidateQuad {
                rect: display_button_rect,
                color: if chrome.settings_open {
                    [0.40, 0.77, 0.96, 1.0]
                } else {
                    [0.18, 0.23, 0.31, 0.98]
                },
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsToggle,
                rect: display_button_rect,
            });
            let tool_header_layout = vec![
                TextBlock {
                    text: if chrome.input_modes_expanded {
                        "Input methods  hide".to_string()
                    } else {
                        "Input methods  show".to_string()
                    },
                    origin: [panel_x + 16.0, tools_y + 7.0],
                    max_width: panel_width - 140.0,
                    pixel_size: 2.0,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: [0.78, 0.86, 0.96, 1.0],
                    align: TextAlign::Left,
                    role: TextRole::ToolLabel,
                }
                .layout(),
            ];
            for layout in &tool_header_layout {
                text_quads.extend(layout.quads.iter().copied());
                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            }
            let display_layout = vec![
                TextBlock {
                    text: "Display".to_string(),
                    origin: [display_button_rect[0] + 8.0, display_button_rect[1] + 6.0],
                    max_width: display_button_rect[2] - 16.0,
                    pixel_size: 2.0,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: if chrome.settings_open {
                        [0.01, 0.10, 0.16, 1.0]
                    } else {
                        [0.90, 0.95, 1.0, 1.0]
                    },
                    align: TextAlign::Center,
                    role: TextRole::SettingOption,
                }
                .layout(),
            ];
            for layout in &display_layout {
                text_quads.extend(layout.quads.iter().copied());
                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            }
            text_sections.push(TextSection {
                role: TextRole::ToolLabel,
                layouts: tool_header_layout,
            });
            text_sections.push(TextSection {
                role: TextRole::SettingOption,
                layouts: display_layout,
            });

            if chrome.input_modes_expanded {
                let button_y = tools_y + tools_header_h + 8.0;
                let button_w = (panel_width - 16.0 - tool_gap * 2.0) / 3.0;
                let buttons = [
                    (InputMode::VirtualKeyboard, "Keyboard"),
                    (InputMode::Dictation, "Voice"),
                    (InputMode::Handwriting, "Handwrite"),
                ];

                let mut tool_layouts = Vec::new();
                for (index, (mode, label)) in buttons.iter().enumerate() {
                    let x = panel_x + index as f32 * (button_w + tool_gap);
                    let selected = *mode == chrome.active_input_mode;
                    let rect = [x, button_y, button_w, tool_button_h];
                    quads.push(CandidateQuad {
                        rect,
                        color: if selected {
                            [0.40, 0.77, 0.96, 1.0]
                        } else {
                            [0.16, 0.20, 0.28, 0.96]
                        },
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::InputModeButton(*mode),
                        rect,
                    });
                    let layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [x + 10.0, button_y + 8.0],
                        max_width: button_w - 20.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: if selected {
                            [0.01, 0.10, 0.16, 1.0]
                        } else {
                            [0.90, 0.95, 1.0, 1.0]
                        },
                        align: TextAlign::Center,
                        role: TextRole::ToolButton,
                    }
                    .layout();
                    text_quads.extend(layout.quads.iter().copied());
                    atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    tool_layouts.push(layout);
                }
                text_sections.push(TextSection {
                    role: TextRole::ToolButton,
                    layouts: tool_layouts,
                });

                if chrome.active_input_mode == InputMode::VirtualKeyboard {
                    let keyboard_y = button_y + tool_button_h + 10.0;
                    let key_gap = 6.0;
                    let row_h = 28.0;
                    let mut keyboard_layouts = Vec::new();
                    let key_rows = if chrome.keyboard_numeric {
                        vec![
                            "1234567890"
                                .chars()
                                .map(VirtualKeyboardKey::Character)
                                .collect::<Vec<_>>(),
                            vec![
                                VirtualKeyboardKey::Character('-'),
                                VirtualKeyboardKey::Character('/'),
                                VirtualKeyboardKey::Character(':'),
                                VirtualKeyboardKey::Character(';'),
                                VirtualKeyboardKey::Character('('),
                                VirtualKeyboardKey::Character(')'),
                                VirtualKeyboardKey::Character('$'),
                                VirtualKeyboardKey::Character('&'),
                                VirtualKeyboardKey::Character('@'),
                            ],
                            vec![
                                VirtualKeyboardKey::ToggleAlphabetic,
                                VirtualKeyboardKey::Character('.'),
                                VirtualKeyboardKey::Character(','),
                                VirtualKeyboardKey::Character('?'),
                                VirtualKeyboardKey::Character('!'),
                                VirtualKeyboardKey::Character('\''),
                                VirtualKeyboardKey::Backspace,
                            ],
                        ]
                    } else {
                        vec![
                            "qwertyuiop"
                                .chars()
                                .map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                })
                                .collect::<Vec<_>>(),
                            "asdfghjkl"
                                .chars()
                                .map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                })
                                .collect::<Vec<_>>(),
                            {
                                let mut row = vec![VirtualKeyboardKey::Shift];
                                row.extend("zxcvbnm".chars().map(|ch| {
                                    let resolved = if chrome.keyboard_shifted {
                                        ch.to_ascii_uppercase()
                                    } else {
                                        ch
                                    };
                                    VirtualKeyboardKey::Character(resolved)
                                }));
                                row.push(VirtualKeyboardKey::Backspace);
                                row
                            },
                        ]
                    };

                    for (row_index, keys) in key_rows.iter().enumerate() {
                        let key_count = keys.len() as f32;
                        let row_y = keyboard_y + row_index as f32 * (row_h + key_gap);
                        let inset = if chrome.keyboard_numeric {
                            if row_index == 1 { 12.0 } else { 0.0 }
                        } else if row_index == 1 {
                            18.0
                        } else if row_index == 2 {
                            8.0
                        } else {
                            0.0
                        };
                        let row_width = panel_width - inset * 2.0;
                        let key_w = (row_width - key_gap * (key_count - 1.0)) / key_count;

                        for (key_index, key) in keys.iter().enumerate() {
                            let x = panel_x + inset + key_index as f32 * (key_w + key_gap);
                            let rect = [x, row_y, key_w, row_h];
                            quads.push(CandidateQuad {
                                rect,
                                color: match key {
                                    VirtualKeyboardKey::Shift if chrome.keyboard_shifted => {
                                        [0.40, 0.77, 0.96, 1.0]
                                    }
                                    VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                                    | VirtualKeyboardKey::Shift => [0.22, 0.24, 0.32, 0.98],
                                    _ => [0.14, 0.18, 0.26, 0.96],
                                },
                            });
                            interactive_targets.push(InteractiveTarget {
                                kind: InteractionKind::VirtualKeyboardKey(*key),
                                rect,
                            });
                            let label = match key {
                                VirtualKeyboardKey::Character(ch) => ch.to_string(),
                                VirtualKeyboardKey::Text(text) => (*text).to_string(),
                                VirtualKeyboardKey::Space => "Space".to_string(),
                                VirtualKeyboardKey::Backspace => "Back".to_string(),
                                VirtualKeyboardKey::Shift => "Shift".to_string(),
                                VirtualKeyboardKey::ToggleNumeric => "?123".to_string(),
                                VirtualKeyboardKey::ToggleAlphabetic => "ABC".to_string(),
                            };
                            let layout = TextBlock {
                                text: label,
                                origin: [x + 8.0, row_y + 8.0],
                                max_width: key_w - 16.0,
                                pixel_size: 2.0,
                                letter_spacing: tracking,
                                line_gap: base_line_gap,
                                max_lines: 1,
                                color: [0.90, 0.95, 1.0, 1.0],
                                align: TextAlign::Center,
                                role: TextRole::KeyboardKey,
                            }
                            .layout();
                            text_quads.extend(layout.quads.iter().copied());
                            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                            keyboard_layouts.push(layout);
                        }
                    }

                    let action_y = keyboard_y + 3.0 * (row_h + key_gap);
                    let left_w = 64.0;
                    let mid_key_w = 44.0;
                    let right_w = 86.0;
                    let space_w = panel_width - left_w - right_w - key_gap * 3.0 - mid_key_w * 2.0;
                    let action_keys = if chrome.keyboard_numeric {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleAlphabetic,
                                "ABC",
                                [panel_x, action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [panel_x + left_w + key_gap, action_y, mid_key_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    panel_x + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    panel_x + left_w + key_gap * 3.0 + mid_key_w + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "Backspace",
                                [
                                    panel_x + left_w + key_gap * 4.0 + mid_key_w * 2.0 + space_w,
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    } else {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleNumeric,
                                "?123",
                                [panel_x, action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [panel_x + left_w + key_gap, action_y, mid_key_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    panel_x + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    panel_x + left_w + key_gap * 3.0 + mid_key_w + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "Backspace",
                                [
                                    panel_x + left_w + key_gap * 4.0 + mid_key_w * 2.0 + space_w,
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    };

                    for (key, label, rect) in action_keys {
                        quads.push(CandidateQuad {
                            rect,
                            color: match key {
                                VirtualKeyboardKey::Space => [0.18, 0.24, 0.34, 0.98],
                                VirtualKeyboardKey::ToggleNumeric
                                | VirtualKeyboardKey::ToggleAlphabetic
                                | VirtualKeyboardKey::Backspace => [0.24, 0.19, 0.24, 0.98],
                                _ => [0.15, 0.18, 0.26, 0.98],
                            },
                        });
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::VirtualKeyboardKey(key),
                            rect,
                        });
                        let layout = TextBlock {
                            text: label.to_string(),
                            origin: [rect[0] + 10.0, rect[1] + 8.0],
                            max_width: rect[2] - 20.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: [0.94, 0.97, 1.0, 1.0],
                            align: TextAlign::Center,
                            role: TextRole::KeyboardKey,
                        }
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        keyboard_layouts.push(layout);
                    }

                    text_sections.push(TextSection {
                        role: TextRole::KeyboardKey,
                        layouts: keyboard_layouts,
                    });
                } else if chrome.active_input_mode == InputMode::Dictation {
                    let voice_y = button_y + tool_button_h + 10.0;
                    let voice_rect = [panel_x, voice_y, panel_width, 108.0];
                    quads.push(CandidateQuad {
                        rect: voice_rect,
                        color: [0.10, 0.14, 0.22, 0.96],
                    });

                    let transcript = if chrome.voice_transcript.is_empty() {
                        match chrome.voice_permission {
                            VoicePermissionState::Pending => {
                                "Waiting for microphone and speech permission".to_string()
                            }
                            VoicePermissionState::Denied => {
                                "Microphone or speech permission denied in macOS".to_string()
                            }
                            VoicePermissionState::Error => {
                                "Speech recognition hit an error. Stop and try again.".to_string()
                            }
                            VoicePermissionState::Unavailable => {
                                "Speech framework unavailable. Using local fallback samples."
                                    .to_string()
                            }
                            _ => "Tap Listen to capture a voice seed".to_string(),
                        }
                    } else {
                        chrome.voice_transcript.clone()
                    };
                    let status_text = if chrome.voice_state == VoiceCaptureState::Listening {
                        "Voice listening"
                    } else {
                        match chrome.voice_permission {
                            VoicePermissionState::Pending => "Voice permission pending",
                            VoicePermissionState::Denied => "Voice permission denied",
                            VoicePermissionState::Error => "Voice recognition error",
                            VoicePermissionState::Unavailable => "Voice fallback mode",
                            VoicePermissionState::Ready => {
                                if chrome.voice_transcript.is_empty() {
                                    "Voice ready"
                                } else {
                                    "Transcript ready"
                                }
                            }
                            VoicePermissionState::Unknown => "Voice setup",
                        }
                    };
                    let voice_layouts = vec![
                        TextBlock {
                            text: status_text.to_string(),
                            origin: [panel_x + 14.0, voice_y + 10.0],
                            max_width: panel_width - 28.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                [0.49, 0.84, 0.98, 1.0]
                            } else if matches!(
                                chrome.voice_permission,
                                VoicePermissionState::Denied | VoicePermissionState::Error
                            ) {
                                [0.98, 0.62, 0.62, 1.0]
                            } else {
                                [0.76, 0.84, 0.94, 1.0]
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: transcript,
                            origin: [panel_x + 14.0, voice_y + 28.0],
                            max_width: panel_width - 28.0,
                            pixel_size: input_value_px,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 2,
                            color: if chrome.voice_transcript.is_empty() {
                                [0.58, 0.66, 0.78, 1.0]
                            } else {
                                [0.93, 0.97, 1.0, 1.0]
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceTranscript,
                        }
                        .layout(),
                    ];
                    for layout in &voice_layouts {
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    }
                    text_sections.push(TextSection {
                        role: TextRole::VoiceTranscript,
                        layouts: voice_layouts,
                    });

                    let voice_actions = [
                        (
                            InteractionKind::ToggleVoiceCapture,
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                "Stop"
                            } else if chrome.voice_permission == VoicePermissionState::Pending {
                                "Waiting"
                            } else if chrome.voice_permission == VoicePermissionState::Denied {
                                "Denied"
                            } else {
                                "Listen"
                            },
                            [panel_x, voice_y + 74.0, 72.0, 24.0],
                            chrome.voice_state == VoiceCaptureState::Listening,
                        ),
                        (
                            InteractionKind::CycleVoiceSample,
                            "Next",
                            [panel_x + 80.0, voice_y + 74.0, 64.0, 24.0],
                            false,
                        ),
                        (
                            InteractionKind::InsertVoiceTranscript,
                            "Use Seed",
                            [panel_x + 152.0, voice_y + 74.0, 92.0, 24.0],
                            false,
                        ),
                        (
                            InteractionKind::ClearVoiceTranscript,
                            "Clear",
                            [panel_x + 252.0, voice_y + 74.0, 64.0, 24.0],
                            false,
                        ),
                    ];
                    let mut voice_action_layouts = Vec::new();
                    for (kind, label, rect, emphasized) in voice_actions {
                        quads.push(CandidateQuad {
                            rect,
                            color: if emphasized {
                                [0.40, 0.77, 0.96, 1.0]
                            } else {
                                [0.18, 0.22, 0.30, 0.98]
                            },
                        });
                        interactive_targets.push(InteractiveTarget { kind, rect });
                        let layout = TextBlock {
                            text: label.to_string(),
                            origin: [rect[0] + 8.0, rect[1] + 6.0],
                            max_width: rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if emphasized {
                                [0.01, 0.10, 0.16, 1.0]
                            } else {
                                [0.92, 0.96, 1.0, 1.0]
                            },
                            align: TextAlign::Center,
                            role: TextRole::VoiceButton,
                        }
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        voice_action_layouts.push(layout);
                    }
                    text_sections.push(TextSection {
                        role: TextRole::VoiceButton,
                        layouts: voice_action_layouts,
                    });
                } else if chrome.active_input_mode == InputMode::Handwriting {
                    let handwriting_y = button_y + tool_button_h + 10.0;
                    let canvas_rect = [panel_x, handwriting_y + 22.0, panel_width, 108.0];
                    quads.push(CandidateQuad {
                        rect: canvas_rect,
                        color: [0.10, 0.14, 0.22, 0.96],
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::HandwritingCanvas,
                        rect: canvas_rect,
                    });

                    for stroke in &chrome.handwriting_strokes {
                        for point in sample_stroke_points(stroke) {
                            quads.push(CandidateQuad {
                                rect: [point[0] - 2.5, point[1] - 2.5, 5.0, 5.0],
                                color: [0.52, 0.88, 0.98, 0.98],
                            });
                        }
                    }

                    let handwriting_layouts = vec![
                        TextBlock {
                            text: "Trace handwriting".to_string(),
                            origin: [panel_x + 14.0, handwriting_y + 2.0],
                            max_width: panel_width - 28.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: [0.78, 0.86, 0.96, 1.0],
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: chrome.handwriting_hint.clone(),
                            origin: [panel_x + 14.0, handwriting_y + 36.0],
                            max_width: panel_width - 28.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 2,
                            color: [0.75, 0.82, 0.92, 1.0],
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                    ];
                    for layout in &handwriting_layouts {
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    }
                    text_sections.push(TextSection {
                        role: TextRole::HandwritingLabel,
                        layouts: handwriting_layouts,
                    });

                    let clear_rect = [panel_x, handwriting_y + 140.0, 74.0, 24.0];
                    quads.push(CandidateQuad {
                        rect: clear_rect,
                        color: [0.18, 0.22, 0.30, 0.98],
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ClearHandwriting,
                        rect: clear_rect,
                    });
                    let mut handwriting_action_layouts = vec![
                        TextBlock {
                            text: "Clear".to_string(),
                            origin: [clear_rect[0] + 8.0, clear_rect[1] + 6.0],
                            max_width: clear_rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: [0.92, 0.96, 1.0, 1.0],
                            align: TextAlign::Center,
                            role: TextRole::HandwritingButton,
                        }
                        .layout(),
                    ];

                    let mut chip_x = panel_x + 84.0;
                    for (index, candidate) in chrome.handwriting_candidates.iter().take(3).enumerate()
                    {
                        let chip_w = (candidate.chars().count() as f32 * 9.5).max(52.0) + 16.0;
                        let rect = [chip_x, handwriting_y + 140.0, chip_w, 24.0];
                        quads.push(CandidateQuad {
                            rect,
                            color: if index == 0 {
                                [0.40, 0.77, 0.96, 1.0]
                            } else {
                                [0.16, 0.20, 0.28, 0.96]
                            },
                        });
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::UseHandwritingCandidate(index),
                            rect,
                        });
                        let layout = TextBlock {
                            text: candidate.clone(),
                            origin: [rect[0] + 8.0, rect[1] + 6.0],
                            max_width: rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if index == 0 {
                                [0.01, 0.10, 0.16, 1.0]
                            } else {
                                [0.92, 0.96, 1.0, 1.0]
                            },
                            align: TextAlign::Center,
                            role: TextRole::HandwritingCandidate,
                        }
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        handwriting_action_layouts.push(layout);
                        chip_x += chip_w + 8.0;
                    }
                    text_sections.push(TextSection {
                        role: TextRole::HandwritingButton,
                        layouts: handwriting_action_layouts,
                    });
                }
            }

            if chrome.settings_open {
                let settings_rect = [
                    panel_x,
                    settings_y + 8.0,
                    panel_width,
                    settings_panel_h - 8.0,
                ];
                quads.push(CandidateQuad {
                    rect: settings_rect,
                    color: [0.09, 0.13, 0.20, 0.97],
                });
                let mut settings_layouts = Vec::new();
                let mut option_layouts = Vec::new();

                let sections = [
                    (
                        "Text",
                        [
                            (
                                InteractionKind::SetTextScale(DisplayTextScale::Small),
                                "S",
                                chrome.text_scale == DisplayTextScale::Small,
                            ),
                            (
                                InteractionKind::SetTextScale(DisplayTextScale::Medium),
                                "M",
                                chrome.text_scale == DisplayTextScale::Medium,
                            ),
                            (
                                InteractionKind::SetTextScale(DisplayTextScale::Large),
                                "L",
                                chrome.text_scale == DisplayTextScale::Large,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Font",
                        [
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::Auto),
                                "Auto",
                                chrome.font_face == FontFaceChoice::Auto,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::Monaco),
                                "Monaco",
                                chrome.font_face == FontFaceChoice::Monaco,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::Geneva),
                                "Geneva",
                                chrome.font_face == FontFaceChoice::Geneva,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::ArialUnicode),
                                "Arial",
                                chrome.font_face == FontFaceChoice::ArialUnicode,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Density",
                        [
                            (
                                InteractionKind::SetCandidateDensity(CandidateDensity::Compact),
                                "Compact",
                                chrome.candidate_density == CandidateDensity::Compact,
                            ),
                            (
                                InteractionKind::SetCandidateDensity(CandidateDensity::Cozy),
                                "Cozy",
                                chrome.candidate_density == CandidateDensity::Cozy,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Spacing",
                        [
                            (
                                InteractionKind::SetTextSpacing(TextSpacing::Tight),
                                "Tight",
                                chrome.text_spacing == TextSpacing::Tight,
                            ),
                            (
                                InteractionKind::SetTextSpacing(TextSpacing::Normal),
                                "Normal",
                                chrome.text_spacing == TextSpacing::Normal,
                            ),
                            (
                                InteractionKind::SetTextSpacing(TextSpacing::Relaxed),
                                "Relaxed",
                                chrome.text_spacing == TextSpacing::Relaxed,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Smooth",
                        [
                            (
                                InteractionKind::SetTextSmoothing(TextSmoothing::Sharp),
                                "Sharp",
                                chrome.text_smoothing == TextSmoothing::Sharp,
                            ),
                            (
                                InteractionKind::SetTextSmoothing(TextSmoothing::Smooth),
                                "Smooth",
                                chrome.text_smoothing == TextSmoothing::Smooth,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Preview",
                        [
                            (
                                InteractionKind::SetPreviewStyle(PreviewStyle::Compact),
                                "Trim",
                                chrome.preview_style == PreviewStyle::Compact,
                            ),
                            (
                                InteractionKind::SetPreviewStyle(PreviewStyle::Full),
                                "Full",
                                chrome.preview_style == PreviewStyle::Full,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "LLM",
                        [
                            (
                                InteractionKind::SetLlmEnabled(true),
                                "On",
                                chrome.llm_enabled,
                            ),
                            (
                                InteractionKind::SetLlmEnabled(false),
                                "Off",
                                !chrome.llm_enabled,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Model",
                        [
                            (
                                InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                                "Llama3.2 3B",
                                chrome.llm_model == LlmModelPreset::Llama32_3b,
                            ),
                        ]
                        .to_vec(),
                    ),
                    (
                        "Heat",
                        [
                            (
                                InteractionKind::SetLlmTemperature(
                                    LlmTemperaturePreset::Focused,
                                ),
                                "Focused",
                                chrome.llm_temperature == LlmTemperaturePreset::Focused,
                            ),
                            (
                                InteractionKind::SetLlmTemperature(
                                    LlmTemperaturePreset::Balanced,
                                ),
                                "Balanced",
                                chrome.llm_temperature == LlmTemperaturePreset::Balanced,
                            ),
                            (
                                InteractionKind::SetLlmTemperature(
                                    LlmTemperaturePreset::Expressive,
                                ),
                                "Expressive",
                                chrome.llm_temperature == LlmTemperaturePreset::Expressive,
                            ),
                        ]
                        .to_vec(),
                    ),
                ];

                for (section_index, (label, options)) in sections.iter().enumerate() {
                    let row_y = settings_rect[1] + 10.0 + section_index as f32 * 24.0;
                    let label_layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [panel_x + 14.0, row_y + 2.0],
                        max_width: 72.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: [0.70, 0.80, 0.92, 1.0],
                        align: TextAlign::Left,
                        role: TextRole::SettingLabel,
                    }
                    .layout();
                    text_quads.extend(label_layout.quads.iter().copied());
                    atlas_glyphs.extend(label_layout.atlas_glyphs.iter().cloned());
                    settings_layouts.push(label_layout);

                    let mut chip_x = panel_x + 86.0;
                    for (kind, chip_label, selected) in options {
                        let chip_w = (chip_label.chars().count() as f32 * 10.0).max(34.0) + 12.0;
                        let rect = [chip_x, row_y, chip_w, 20.0];
                        quads.push(CandidateQuad {
                            rect,
                            color: if *selected {
                                [0.40, 0.77, 0.96, 1.0]
                            } else {
                                [0.16, 0.20, 0.28, 0.96]
                            },
                        });
                        interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                        let option_layout = TextBlock {
                            text: (*chip_label).to_string(),
                            origin: [chip_x + 6.0, row_y + 5.0],
                            max_width: chip_w - 12.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if *selected {
                                [0.01, 0.10, 0.16, 1.0]
                            } else {
                                [0.90, 0.95, 1.0, 1.0]
                            },
                            align: TextAlign::Center,
                            role: TextRole::SettingOption,
                        }
                        .layout();
                        text_quads.extend(option_layout.quads.iter().copied());
                        atlas_glyphs.extend(option_layout.atlas_glyphs.iter().cloned());
                        option_layouts.push(option_layout);
                        chip_x += chip_w + 8.0;
                    }
                }

                text_sections.push(TextSection {
                    role: TextRole::SettingLabel,
                    layouts: settings_layouts,
                });
                text_sections.push(TextSection {
                    role: TextRole::SettingOption,
                    layouts: option_layouts,
                });
            }

            let chip_section_y = suggestions_y;
            if !chrome.next_token_candidates.is_empty() {
                let next_label_layouts = vec![
                    TextBlock {
                        text: if chrome.composed_tokens.is_empty() {
                            "Next tokens".to_string()
                        } else {
                            format!("Next tokens  |  {}", chrome.composed_tokens.join(" "))
                        },
                        origin: [panel_x + 2.0, chip_section_y],
                        max_width: panel_width - 96.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: [0.78, 0.86, 0.96, 1.0],
                        align: TextAlign::Left,
                        role: TextRole::NextTokenLabel,
                    }
                    .layout(),
                ];
                for layout in &next_label_layouts {
                    text_quads.extend(layout.quads.iter().copied());
                    atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                }
                text_sections.push(TextSection {
                    role: TextRole::NextTokenLabel,
                    layouts: next_label_layouts,
                });

                if !chrome.composed_tokens.is_empty() {
                    let back_rect = [panel_x + panel_width - 78.0, chip_section_y - 2.0, 78.0, 22.0];
                    quads.push(CandidateQuad {
                        rect: back_rect,
                        color: [0.22, 0.19, 0.24, 0.98],
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::RewindNextToken,
                        rect: back_rect,
                    });
                    let back_layout = TextBlock {
                        text: "Back".to_string(),
                        origin: [back_rect[0] + 8.0, back_rect[1] + 6.0],
                        max_width: back_rect[2] - 16.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: [0.92, 0.96, 1.0, 1.0],
                        align: TextAlign::Center,
                        role: TextRole::NextTokenChip,
                    }
                    .layout();
                    text_quads.extend(back_layout.quads.iter().copied());
                    atlas_glyphs.extend(back_layout.atlas_glyphs.iter().cloned());
                    text_sections.push(TextSection {
                        role: TextRole::NextTokenChip,
                        layouts: vec![back_layout],
                    });
                }

                let chip_y = chip_section_y + 24.0;
                let mut chip_x = panel_x;
                let mut row = 0;
                let mut chip_layouts = Vec::new();
                for (index, token) in chrome.next_token_candidates.iter().take(6).enumerate() {
                    let chip_w = (token.chars().count() as f32 * 10.0).max(52.0) + 18.0;
                    if chip_x + chip_w > panel_x + panel_width {
                        row += 1;
                        chip_x = panel_x;
                    }
                    if row >= 2 {
                        break;
                    }
                    let rect = [chip_x, chip_y + row as f32 * 30.0, chip_w, 24.0];
                    quads.push(CandidateQuad {
                        rect,
                        color: if index == 0 {
                            [0.40, 0.77, 0.96, 1.0]
                        } else {
                            [0.15, 0.19, 0.28, 0.96]
                        },
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::SelectNextToken(index),
                        rect,
                    });
                    let layout = TextBlock {
                        text: token.clone(),
                        origin: [rect[0] + 8.0, rect[1] + 6.0],
                        max_width: rect[2] - 16.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: if index == 0 {
                            [0.01, 0.10, 0.16, 1.0]
                        } else {
                            [0.92, 0.96, 1.0, 1.0]
                        },
                        align: TextAlign::Center,
                        role: TextRole::NextTokenChip,
                    }
                    .layout();
                    text_quads.extend(layout.quads.iter().copied());
                    atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    chip_layouts.push(layout);
                    chip_x += chip_w + 8.0;
                }
                if !chip_layouts.is_empty() {
                    text_sections.push(TextSection {
                        role: TextRole::NextTokenChip,
                        layouts: chip_layouts,
                    });
                }
            }

            let sentence_y = suggestions_y + chip_section_h;
            for (index, label) in visible_sentence_candidates.iter().enumerate() {
                let y = sentence_y + index as f32 * (item_height + gap);
                let selected = index == snapshot.selected_index;
                let continuation = display_candidate_continuation(&snapshot.seed_text, label);
                let quad = CandidateQuad {
                    rect: [panel_x, y, panel_width, item_height],
                    color: if selected {
                        [0.39, 0.78, 0.96, 1.0]
                    } else if snapshot.degraded {
                        [0.22, 0.24, 0.30, 0.95]
                    } else {
                        [0.16, 0.18, 0.22, 0.94]
                    },
                };
                quads.push(quad);
                hit_targets.push(HitTarget {
                    index,
                    rect: quad.rect,
                });
                interactive_targets.push(InteractiveTarget {
                    kind: InteractionKind::Candidate(index),
                    rect: quad.rect,
                });
                let candidate_layouts = vec![
                    TextBlock {
                        text: continuation,
                        origin: [panel_x + 18.0, y + 13.0],
                        max_width: panel_width - 36.0,
                        pixel_size: input_value_px,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: if selected {
                            [0.01, 0.10, 0.16, 1.0]
                        } else {
                            [0.94, 0.97, 1.0, 1.0]
                        },
                        align: TextAlign::Left,
                        role: TextRole::CandidatePrimary,
                    }
                    .layout(),
                    TextBlock {
                        text: format!(
                            "{}  |  {}",
                            if selected { "selected" } else { "tap" },
                            if chrome.preview_style == PreviewStyle::Full {
                                label.to_string()
                            } else {
                                compact_phrase(label)
                            }
                        ),
                        origin: [
                            panel_x + 18.0,
                            y + if chrome.preview_style == PreviewStyle::Full {
                                35.0
                            } else {
                                34.0
                            },
                        ],
                        max_width: panel_width - 36.0,
                        pixel_size: 2.0,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: if chrome.preview_style == PreviewStyle::Full {
                            2
                        } else {
                            1
                        },
                        color: if selected {
                            [0.02, 0.18, 0.26, 1.0]
                        } else {
                            [0.70, 0.78, 0.86, 1.0]
                        },
                        align: TextAlign::Left,
                        role: TextRole::CandidateMeta,
                    }
                    .layout(),
                ];
                for layout in &candidate_layouts {
                    text_quads.extend(layout.quads.iter().copied());
                    atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                }
                text_sections.push(TextSection {
                    role: TextRole::CandidatePrimary,
                    layouts: candidate_layouts,
                });
            }

            RenderScene {
                quads,
                text_quads,
                atlas_glyphs,
                text_sections,
                hit_targets,
                interactive_targets,
                labels: snapshot.candidate_labels.clone(),
                selected_label: snapshot
                    .candidate_labels
                    .get(snapshot.selected_index)
                    .cloned(),
                draft_text: snapshot.draft_text.clone(),
            }
        }

        pub async fn request_adapter() -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
        }
    }

    fn point_in_rect(x: f32, y: f32, rect: [f32; 4]) -> bool {
        let [rx, ry, rw, rh] = rect;
        x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
    }

    fn sample_stroke_points(stroke: &[[f32; 2]]) -> Vec<[f32; 2]> {
        if stroke.is_empty() {
            return Vec::new();
        }
        if stroke.len() == 1 {
            return vec![stroke[0]];
        }

        let mut points = Vec::with_capacity(stroke.len() * 4);
        for window in stroke.windows(2) {
            let start = window[0];
            let end = window[1];
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let distance = dx.hypot(dy).max(1.0);
            let steps = (distance / 4.0).ceil() as usize;
            for step in 0..=steps {
                let t = step as f32 / steps.max(1) as f32;
                points.push([start[0] + dx * t, start[1] + dy * t]);
            }
        }
        points
    }

    fn layout_text_block(block: &TextBlock) -> TextLayout {
        let glyph_advance =
            (block.pixel_size * 6.5 + block.letter_spacing).max(block.pixel_size * 4.0);
        let line_height = block.pixel_size * 7.0 + block.line_gap;
        let max_chars_per_line = ((block.max_width / glyph_advance).floor() as usize).max(1);
        let mut lines = wrap_text(&block.text, max_chars_per_line);
        let mut truncated = false;

        if lines.len() > block.max_lines {
            lines.truncate(block.max_lines);
            if let Some(last) = lines.last_mut() {
                *last = ellipsize(last, max_chars_per_line, true);
            }
            truncated = true;
        } else if lines
            .last()
            .map(|line| line.chars().count() > max_chars_per_line)
            .unwrap_or(false)
        {
            if let Some(last) = lines.last_mut() {
                *last = ellipsize(last, max_chars_per_line, false);
            }
            truncated = true;
        }

        let mut quads = Vec::new();
        let mut atlas_glyphs = Vec::new();
        let mut max_line_width: f32 = 0.0;

        for (line_index, line) in lines.iter().enumerate() {
            let line_width = line.chars().fold(0.0_f32, |acc, ch| {
                acc + if ch == ' ' {
                    block.pixel_size * 4.0
                } else {
                    glyph_advance
                }
            });
            max_line_width = max_line_width.max(line_width);
            let offset_x = match block.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => ((block.max_width - line_width) / 2.0).max(0.0),
            };
            let mut cursor_x = block.origin[0] + offset_x;
            let cursor_y = block.origin[1] + line_index as f32 * line_height;

            for ch in line.chars() {
                if ch == ' ' {
                    cursor_x += block.pixel_size * 4.0;
                    continue;
                }

                atlas_glyphs.push(AtlasGlyph {
                    ch,
                    rect: [
                        cursor_x,
                        cursor_y,
                        block.pixel_size * 5.0,
                        block.pixel_size * 7.0,
                    ],
                    color: block.color,
                });

                for (row, pattern) in glyph_bitmap(ch).iter().enumerate() {
                    for col in 0..5 {
                        if (pattern >> (4 - col)) & 1 == 1 {
                            quads.push(CandidateQuad {
                                rect: [
                                    cursor_x + col as f32 * block.pixel_size,
                                    cursor_y + row as f32 * block.pixel_size,
                                    block.pixel_size,
                                    block.pixel_size,
                                ],
                                color: block.color,
                            });
                        }
                    }
                }

                cursor_x += glyph_advance;
            }
        }

        let height = if lines.is_empty() {
            0.0
        } else {
            (lines.len() as f32 - 1.0) * line_height + block.pixel_size * 7.0
        };

        TextLayout {
            quads,
            atlas_glyphs,
            lines,
            truncated,
            bounds: [block.origin[0], block.origin[1], max_line_width, height],
            role: block.role,
        }
    }

    fn wrap_text(text: &str, max_chars_per_line: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();

        for word in text.split_whitespace() {
            let pending_len = if current.is_empty() {
                word.chars().count()
            } else {
                current.chars().count() + 1 + word.chars().count()
            };

            if pending_len <= max_chars_per_line {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
                continue;
            }

            if !current.is_empty() {
                lines.push(current.clone());
                current.clear();
            }

            if word.chars().count() <= max_chars_per_line {
                current.push_str(word);
                continue;
            }

            let mut chunk = String::new();
            for ch in word.chars() {
                if chunk.chars().count() >= max_chars_per_line {
                    lines.push(chunk.clone());
                    chunk.clear();
                }
                chunk.push(ch);
            }
            current = chunk;
        }

        if !current.is_empty() {
            lines.push(current);
        }

        if lines.is_empty() {
            lines.push(String::new());
        }

        lines
    }

    fn ellipsize(text: &str, max_chars: usize, force_suffix: bool) -> String {
        if text.chars().count() <= max_chars && !force_suffix {
            return text.to_string();
        }

        if max_chars <= 1 {
            return "…".to_string();
        }

        let keep = if force_suffix {
            max_chars.saturating_sub(1).min(text.chars().count())
        } else {
            max_chars.saturating_sub(1)
        };
        let mut result = text.chars().take(keep).collect::<String>();
        if result.ends_with(' ') {
            result.pop();
        }
        result.push('…');
        result
    }

    pub fn glyph_bitmap(ch: char) -> [u8; 7] {
        match ch.to_ascii_lowercase() {
            'a' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'b' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
            ],
            'c' => [
                0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
            ],
            'd' => [
                0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
            ],
            'e' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            'f' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'g' => [
                0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
            ],
            'h' => [
                0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'i' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
            ],
            'j' => [
                0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
            ],
            'k' => [
                0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
            ],
            'l' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
            ],
            'm' => [
                0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
            ],
            'n' => [
                0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
            ],
            'o' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'p' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            'q' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
            ],
            'r' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
            's' => [
                0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            't' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'u' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'v' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
            ],
            'w' => [
                0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
            ],
            'x' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
            ],
            'y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'z' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
            ],
            '0' => [
                0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
            ],
            '1' => [
                0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            '2' => [
                0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
            ],
            '3' => [
                0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            ':' => [
                0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
            ],
            '[' => [
                0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
            ],
            ']' => [
                0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
            ],
            '-' => [
                0b00000, 0b00000, 0b00000, 0b01110, 0b00000, 0b00000, 0b00000,
            ],
            _ => [
                0b11111, 0b10001, 0b00100, 0b00100, 0b00100, 0b10001, 0b11111,
            ],
        }
    }
}
