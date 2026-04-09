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

fn append_gear_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    cutout: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let teeth = [
        [cx - 1.5, y + 2.0, 3.0, 4.0],
        [cx - 1.5, y + h - 6.0, 3.0, 4.0],
        [x + 2.0, cy - 1.5, 4.0, 3.0],
        [x + w - 6.0, cy - 1.5, 4.0, 3.0],
        [x + 4.0, y + 4.0, 3.0, 3.0],
        [x + w - 7.0, y + 4.0, 3.0, 3.0],
        [x + 4.0, y + h - 7.0, 3.0, 3.0],
        [x + w - 7.0, y + h - 7.0, 3.0, 3.0],
    ];

    for tooth in teeth {
        quads.push(gpu::CandidateQuad { rect: tooth, color });
    }

    quads.push(gpu::CandidateQuad {
        rect: [cx - 4.0, cy - 4.0, 8.0, 8.0],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [cx - 1.5, cy - 1.5, 3.0, 3.0],
        color: cutout,
    });
}

fn append_suzaku_bird_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    primary: [f32; 4],
    secondary: [f32; 4],
    beak: [f32; 4],
    eye: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    let body = [x + unit * 0.22, y + unit * 0.38, unit * 0.30, unit * 0.24];
    let neck = [x + unit * 0.44, y + unit * 0.24, unit * 0.12, unit * 0.18];
    let head = [x + unit * 0.50, y + unit * 0.20, unit * 0.14, unit * 0.14];
    let wing_top = [x + unit * 0.28, y + unit * 0.24, unit * 0.18, unit * 0.14];
    let wing_mid = [x + unit * 0.20, y + unit * 0.32, unit * 0.28, unit * 0.12];
    let tail = [x + unit * 0.14, y + unit * 0.50, unit * 0.14, unit * 0.10];
    let tail_tip = [x + unit * 0.10, y + unit * 0.56, unit * 0.12, unit * 0.08];
    let beak_rect = [x + unit * 0.63, y + unit * 0.24, unit * 0.12, unit * 0.07];
    let eye_rect = [x + unit * 0.56, y + unit * 0.24, unit * 0.03, unit * 0.03];

    for bird_rect in [body, neck, head, wing_top, wing_mid, tail, tail_tip] {
        quads.push(gpu::CandidateQuad {
            rect: bird_rect,
            color: primary,
        });
    }
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.56, unit * 0.22, unit * 0.08],
        color: secondary,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.36, y + unit * 0.64, unit * 0.14, unit * 0.04],
        color: secondary,
    });
    quads.push(gpu::CandidateQuad {
        rect: beak_rect,
        color: beak,
    });
    quads.push(gpu::CandidateQuad {
        rect: eye_rect,
        color: eye,
    });
}

#[cfg(feature = "gpu")]
pub mod gpu {
    use crate::platform::voice_host::{voice_status_text, voice_transcript_placeholder};

    use super::{
        Snapshot, append_gear_icon_quads, append_suzaku_bird_icon_quads,
        display_candidate_continuation, measure_text_prefix_width,
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
        pub voice_state: VoiceCaptureState,
        pub voice_permission: VoicePermissionState,
        pub voice_backend_label: String,
        pub voice_supports_live_capture: bool,
        pub voice_transcript: String,
        pub voice_auto_insert: bool,
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
                compact_mode: false,
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
                voice_backend_label: "Unknown Voice Host".to_string(),
                voice_supports_live_capture: false,
                voice_transcript: String::new(),
                voice_auto_insert: true,
                llm_enabled: false,
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
        SetLlmEnabled(bool),
        SetVoiceAutoInsert(bool),
        SetLlmModel(LlmModelPreset),
        SetLlmTemperature(LlmTemperaturePreset),
        SelectNextToken(usize),
        RewindNextToken,
        ToggleVoiceCapture,
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

    #[derive(Debug, Clone, Copy)]
    struct PanelSceneMetrics {
        panel_width: f32,
        panel_x: f32,
        panel_y: f32,
        section_gap: f32,
        input_box_h: f32,
        tools_header_h: f32,
        tool_button_h: f32,
        tool_gap: f32,
        tools_content_h: f32,
        settings_panel_h: f32,
        item_height: f32,
        item_gap: f32,
        chip_section_h: f32,
        candidate_columns: usize,
        stacked_token_header: bool,
    }

    impl PanelSceneMetrics {
        fn new(
            scene_width: f32,
            scene_height: f32,
            responsive_scale: f32,
            chrome: &PanelChromeState,
            sentence_count: usize,
        ) -> Self {
            let scene_margin = (10.0 * responsive_scale).max(8.0);
            let max_panel_width = (scene_width - scene_margin * 2.0).max(320.0);
            let min_panel_width = 380.0_f32.min(max_panel_width);
            let desired_panel_width = scene_width * 0.90;
            let panel_width = desired_panel_width
                .min(max_panel_width)
                .max(min_panel_width);
            let input_box_h = 68.0 * responsive_scale;
            let tools_header_h = 22.0 * responsive_scale;
            let tool_button_h = 28.0 * responsive_scale;
            let tool_gap = 6.0 * responsive_scale;
            let section_gap = 10.0 * responsive_scale;
            let extended_input_panel_h = if chrome.input_modes_expanded {
                match chrome.active_input_mode {
                    InputMode::VirtualKeyboard => 146.0 * responsive_scale,
                    InputMode::Dictation => {
                        if max_panel_width < 620.0 {
                            136.0 * responsive_scale
                        } else {
                            124.0 * responsive_scale
                        }
                    }
                    InputMode::Handwriting => 170.0 * responsive_scale,
                }
            } else {
                0.0
            };
            let tools_content_h = if chrome.input_modes_expanded {
                tool_button_h + 6.0 * responsive_scale + extended_input_panel_h
            } else {
                0.0
            };
            let settings_panel_h = if chrome.settings_open { 220.0 } else { 0.0 };
            let item_height = match (chrome.candidate_density, chrome.preview_style) {
                (CandidateDensity::Compact, PreviewStyle::Compact) => 70.0,
                (CandidateDensity::Compact, PreviewStyle::Full) => 90.0,
                (CandidateDensity::Cozy, PreviewStyle::Compact) => 80.0,
                (CandidateDensity::Cozy, PreviewStyle::Full) => 102.0,
            } * responsive_scale;
            let item_gap = if chrome.candidate_density == CandidateDensity::Compact {
                6.0
            } else {
                8.0
            } * responsive_scale;
            let stacked_token_header =
                max_panel_width < 560.0 && !chrome.next_token_candidates.is_empty();
            let chip_section_h = if chrome.next_token_candidates.is_empty() {
                0.0
            } else {
                (if stacked_token_header { 84.0 } else { 60.0 }) * responsive_scale
            };
            let candidate_columns = if panel_width >= 760.0 && sentence_count > 2 {
                2
            } else {
                1
            };
            let sentence_rows = if sentence_count == 0 {
                0
            } else {
                sentence_count.div_ceil(candidate_columns)
            };
            let sentence_height = if sentence_rows == 0 {
                0.0
            } else {
                sentence_rows as f32 * item_height + (sentence_rows as f32 - 1.0) * item_gap
            };
            let panel_height = input_box_h
                + section_gap
                + tools_header_h
                + tools_content_h
                + settings_panel_h
                + section_gap
                + chip_section_h
                + sentence_height;
            let panel_x = ((scene_width - panel_width) / 2.0).max(scene_margin);
            let panel_y = ((scene_height - panel_height) / 2.0).max(10.0 * responsive_scale);

            Self {
                panel_width,
                panel_x,
                panel_y,
                section_gap,
                input_box_h,
                tools_header_h,
                tool_button_h,
                tool_gap,
                tools_content_h,
                settings_panel_h,
                item_height,
                item_gap,
                chip_section_h,
                candidate_columns,
                stacked_token_header,
            }
        }
    }

    impl WgpuCandidateRenderer {
        pub fn new(scene_width: f32, scene_height: f32) -> Self {
            Self {
                scene_width,
                scene_height,
            }
        }

        fn responsive_scale(&self) -> f32 {
            let width_factor = self.scene_width / 900.0;
            let height_factor = self.scene_height / 780.0;
            (width_factor * 0.65 + height_factor * 0.35).clamp(0.92, 1.28)
        }

        pub fn build_scene(&self, snapshot: &Snapshot) -> RenderScene {
            let chrome = PanelChromeState {
                seed_text: snapshot.seed_text.clone(),
                sentence_candidates: snapshot.candidate_labels.iter().take(4).cloned().collect(),
                ..PanelChromeState::default()
            };
            self.build_panel_scene(snapshot, &chrome)
        }

        pub fn build_compact_scene(
            &self,
            snapshot: &Snapshot,
            _chrome: &PanelChromeState,
            hovered: bool,
            pressed: bool,
        ) -> RenderScene {
            let page_bg = [0.93, 0.95, 0.98, 0.98];
            let shell = if pressed {
                [0.98, 0.86, 0.87, 1.0]
            } else if hovered {
                [0.99, 0.90, 0.90, 1.0]
            } else {
                [0.99, 0.93, 0.93, 1.0]
            };
            let shell_inner = if pressed {
                [1.0, 0.94, 0.94, 1.0]
            } else {
                [1.0, 0.97, 0.97, 1.0]
            };
            let bird_primary = if pressed {
                [0.77, 0.16, 0.16, 1.0]
            } else if hovered {
                [0.84, 0.19, 0.19, 1.0]
            } else {
                [0.85, 0.23, 0.23, 1.0]
            };
            let bird_secondary = [0.95, 0.47, 0.40, 1.0];
            let bird_beak = [0.96, 0.67, 0.30, 1.0];
            let bird_eye = [0.44, 0.09, 0.09, 1.0];
            let mut quads = Vec::new();
            let text_quads = Vec::new();
            let atlas_glyphs = Vec::new();
            let text_sections = Vec::new();
            let hit_targets = Vec::new();
            quads.push(CandidateQuad {
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
            let orb_size = self.scene_width.min(self.scene_height) - 20.0;
            let orb_size = orb_size.clamp(48.0, 92.0);
            let orb_rect = [
                (self.scene_width - orb_size) / 2.0,
                (self.scene_height - orb_size) / 2.0,
                orb_size,
                orb_size,
            ];
            let inner_rect = [
                orb_rect[0] + orb_size * 0.10,
                orb_rect[1] + orb_size * 0.10,
                orb_size * 0.80,
                orb_size * 0.80,
            ];
            let core_rect = [
                orb_rect[0] + orb_size * 0.24,
                orb_rect[1] + orb_size * 0.24,
                orb_size * 0.52,
                orb_size * 0.52,
            ];
            quads.push(CandidateQuad {
                rect: orb_rect,
                color: shell,
            });
            quads.push(CandidateQuad {
                rect: inner_rect,
                color: shell_inner,
            });
            quads.push(CandidateQuad {
                rect: core_rect,
                color: [1.0, 1.0, 1.0, 1.0],
            });
            let mut targets = vec![InteractiveTarget {
                kind: InteractionKind::ToggleCompactMode,
                rect: orb_rect,
            }];
            let icon_rect = [
                orb_rect[0] + orb_size * 0.18,
                orb_rect[1] + orb_size * 0.18,
                orb_size * 0.64,
                orb_size * 0.64,
            ];
            append_suzaku_bird_icon_quads(
                &mut quads,
                icon_rect,
                bird_primary,
                bird_secondary,
                bird_beak,
                bird_eye,
            );
            RenderScene {
                quads,
                text_quads,
                atlas_glyphs,
                text_sections,
                hit_targets,
                interactive_targets: std::mem::take(&mut targets),
                labels: snapshot.candidate_labels.clone(),
                selected_label: snapshot
                    .candidate_labels
                    .get(snapshot.selected_index)
                    .cloned(),
                draft_text: snapshot.draft_text.clone(),
            }
        }

        pub fn build_panel_scene(
            &self,
            snapshot: &Snapshot,
            chrome: &PanelChromeState,
        ) -> RenderScene {
            if chrome.compact_mode {
                return self.build_compact_scene(snapshot, chrome, false, false);
            }
            let page_bg = [0.93, 0.95, 0.98, 0.98];
            let surface = [0.86, 0.89, 0.94, 0.98];
            let surface_alt = [0.82, 0.86, 0.92, 0.98];
            let accent = [0.43, 0.69, 0.92, 1.0];
            let surface_muted = [0.78, 0.82, 0.88, 0.98];
            let accent_soft = [0.73, 0.88, 0.98, 1.0];
            let accent_text = [0.12, 0.23, 0.36, 1.0];
            let text_primary = [0.22, 0.28, 0.38, 1.0];
            let text_secondary = [0.39, 0.47, 0.58, 1.0];
            let text_muted = [0.53, 0.60, 0.70, 1.0];
            let border_dark = [0.70, 0.77, 0.86, 0.98];
            let responsive_scale = self.responsive_scale();
            let input_value_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.0,
                DisplayTextScale::Medium => 3.0,
                DisplayTextScale::Large => 4.0,
            } * responsive_scale;
            let label_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.0,
                DisplayTextScale::Medium => 2.0,
                DisplayTextScale::Large => 3.0,
            } * responsive_scale;
            let tracking = match chrome.text_spacing {
                TextSpacing::Tight => -0.4,
                TextSpacing::Normal => 0.0,
                TextSpacing::Relaxed => 0.8,
            } * responsive_scale;
            let base_line_gap = match chrome.text_spacing {
                TextSpacing::Tight => 4.0,
                TextSpacing::Normal => 6.0,
                TextSpacing::Relaxed => 9.0,
            } * responsive_scale;
            let visible_sentence_candidates: Vec<String> =
                chrome.sentence_candidates.iter().take(4).cloned().collect();
            let metrics = PanelSceneMetrics::new(
                self.scene_width,
                self.scene_height,
                responsive_scale,
                chrome,
                visible_sentence_candidates.len(),
            );
            let narrow_layout_scale = if metrics.panel_width < 680.0 {
                0.90
            } else if metrics.panel_width < 760.0 {
                0.96
            } else {
                1.0
            };
            let input_value_px = input_value_px * narrow_layout_scale;
            let label_px = label_px * narrow_layout_scale;
            let tracking = tracking * if narrow_layout_scale < 1.0 { 0.75 } else { 1.0 };
            let base_line_gap =
                (base_line_gap * if narrow_layout_scale < 1.0 { 0.88 } else { 1.0 }).max(3.0);
            let panel_width = metrics.panel_width;
            let panel_x = metrics.panel_x;
            let panel_y = metrics.panel_y;
            let input_box_y = panel_y;
            let tools_y = input_box_y + metrics.input_box_h + metrics.section_gap;
            let settings_y = tools_y + metrics.tools_header_h + metrics.tools_content_h;
            let suggestions_y = settings_y + metrics.settings_panel_h + metrics.section_gap;
            let mut quads = Vec::with_capacity(snapshot.candidate_labels.len() + 6);
            let mut text_quads = Vec::new();
            let mut atlas_glyphs = Vec::new();
            let mut text_sections = Vec::new();
            let mut hit_targets = Vec::with_capacity(snapshot.candidate_labels.len());
            let mut interactive_targets = Vec::new();
            quads.push(CandidateQuad {
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
            quads.push(CandidateQuad {
                rect: [panel_x, input_box_y, panel_width, metrics.input_box_h],
                color: if chrome.input_focused {
                    [0.90, 0.93, 0.98, 1.0]
                } else {
                    surface
                },
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SeedInput,
                rect: [panel_x, input_box_y, panel_width, metrics.input_box_h],
            });

            let header_layouts = vec![
                TextBlock {
                    text: "Seed input".to_string(),
                    origin: [
                        panel_x + 16.0 * responsive_scale,
                        input_box_y + 8.0 * responsive_scale,
                    ],
                    max_width: panel_width - 36.0 * responsive_scale,
                    pixel_size: label_px,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Left,
                    role: TextRole::InputLabel,
                }
                .layout(),
                TextBlock {
                    text: if chrome.seed_text.is_empty() {
                        "Type a seed word or phrase".to_string()
                    } else {
                        chrome.display_text()
                    },
                    origin: [
                        panel_x + 16.0 * responsive_scale,
                        input_box_y + 24.0 * responsive_scale,
                    ],
                    max_width: panel_width - 54.0 * responsive_scale,
                    pixel_size: input_value_px,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 3,
                    color: if chrome.seed_text.is_empty() {
                        text_muted
                    } else {
                        text_primary
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
                    + 16.0 * responsive_scale
                    + measure_text_prefix_width(
                        &chrome.seed_text,
                        chrome.caret_index,
                        input_value_px,
                        tracking,
                    );
                quads.push(CandidateQuad {
                    rect: [
                        caret_x,
                        input_box_y + 22.0 * responsive_scale,
                        2.0 * responsive_scale,
                        22.0 * responsive_scale,
                    ],
                    color: accent,
                });
            }

            let settings_button_rect = [
                panel_x + panel_width - 34.0 * responsive_scale,
                input_box_y + 8.0 * responsive_scale,
                20.0 * responsive_scale,
                20.0 * responsive_scale,
            ];
            let compact_button_rect = [
                panel_x + panel_width - 58.0 * responsive_scale,
                input_box_y + 8.0 * responsive_scale,
                18.0 * responsive_scale,
                20.0 * responsive_scale,
            ];
            quads.push(CandidateQuad {
                rect: compact_button_rect,
                color: surface_alt,
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::ToggleCompactMode,
                rect: compact_button_rect,
            });
            let compact_icon = TextBlock {
                text: "o".to_string(),
                origin: [
                    compact_button_rect[0] + 4.0 * responsive_scale,
                    compact_button_rect[1] + 5.0 * responsive_scale,
                ],
                max_width: compact_button_rect[2] - 8.0 * responsive_scale,
                pixel_size: 2.0 * responsive_scale,
                letter_spacing: tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Center,
                role: TextRole::ToolButton,
            }
            .layout();
            text_quads.extend(compact_icon.quads.iter().copied());
            atlas_glyphs.extend(compact_icon.atlas_glyphs.iter().cloned());
            quads.push(CandidateQuad {
                rect: settings_button_rect,
                color: if chrome.settings_open {
                    accent
                } else {
                    surface_alt
                },
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsToggle,
                rect: settings_button_rect,
            });
            append_gear_icon_quads(
                &mut quads,
                settings_button_rect,
                if chrome.settings_open {
                    [0.96, 0.98, 1.0, 1.0]
                } else {
                    text_secondary
                },
                if chrome.settings_open {
                    accent
                } else {
                    surface_alt
                },
            );

            let tools_header_rect = [panel_x, tools_y, panel_width, metrics.tools_header_h];
            quads.push(CandidateQuad {
                rect: tools_header_rect,
                color: surface_alt,
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::InputModesToggle,
                rect: tools_header_rect,
            });
            let tool_header_layout = vec![
                TextBlock {
                    text: if chrome.input_modes_expanded {
                        "Input methods  hide".to_string()
                    } else {
                        "Input methods  show".to_string()
                    },
                    origin: [
                        panel_x + 16.0 * responsive_scale,
                        tools_y + 7.0 * responsive_scale,
                    ],
                    max_width: panel_width - 32.0 * responsive_scale,
                    pixel_size: 2.0 * responsive_scale,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Left,
                    role: TextRole::ToolLabel,
                }
                .layout(),
            ];
            for layout in &tool_header_layout {
                text_quads.extend(layout.quads.iter().copied());
                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
            }
            text_sections.push(TextSection {
                role: TextRole::ToolLabel,
                layouts: tool_header_layout,
            });

            if chrome.input_modes_expanded {
                let tools_panel_rect = [
                    panel_x,
                    tools_y + metrics.tools_header_h + 4.0 * responsive_scale,
                    panel_width,
                    metrics.tools_content_h,
                ];
                quads.push(CandidateQuad {
                    rect: tools_panel_rect,
                    color: surface,
                });
                let button_y = tools_y + metrics.tools_header_h + 6.0 * responsive_scale;
                let button_w =
                    (panel_width - 16.0 * responsive_scale - metrics.tool_gap * 2.0) / 3.0;
                let buttons = [
                    (InputMode::VirtualKeyboard, "Keyboard"),
                    (InputMode::Dictation, "Voice"),
                    (InputMode::Handwriting, "Handwrite"),
                ];

                let mut tool_layouts = Vec::new();
                for (index, (mode, label)) in buttons.iter().enumerate() {
                    let x = panel_x + index as f32 * (button_w + metrics.tool_gap);
                    let selected = *mode == chrome.active_input_mode;
                    let rect = [x, button_y, button_w, metrics.tool_button_h];
                    quads.push(CandidateQuad {
                        rect,
                        color: if selected {
                            accent_soft
                        } else {
                            [0.93, 0.95, 0.98, 1.0]
                        },
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::InputModeButton(*mode),
                        rect,
                    });
                    let layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [
                            x + 10.0 * responsive_scale,
                            button_y + 8.0 * responsive_scale,
                        ],
                        max_width: button_w - 20.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: if selected { accent_text } else { text_primary },
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
                    let keyboard_y = button_y + metrics.tool_button_h + 8.0 * responsive_scale;
                    let key_gap = 6.0 * responsive_scale;
                    let row_h = 28.0 * responsive_scale;
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
                            if row_index == 1 {
                                12.0 * responsive_scale
                            } else {
                                0.0
                            }
                        } else if row_index == 1 {
                            18.0 * responsive_scale
                        } else if row_index == 2 {
                            8.0 * responsive_scale
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
                                        accent_soft
                                    }
                                    VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                                    | VirtualKeyboardKey::Shift => surface_muted,
                                    _ => [0.95, 0.97, 0.99, 1.0],
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
                                VirtualKeyboardKey::ToggleNumeric => "123".to_string(),
                                VirtualKeyboardKey::ToggleAlphabetic => "ABC".to_string(),
                            };
                            let layout = TextBlock {
                                text: label,
                                origin: [
                                    x + 8.0 * responsive_scale,
                                    row_y + 8.0 * responsive_scale,
                                ],
                                max_width: key_w - 16.0 * responsive_scale,
                                pixel_size: if matches!(
                                    key,
                                    VirtualKeyboardKey::Backspace
                                        | VirtualKeyboardKey::ToggleNumeric
                                        | VirtualKeyboardKey::ToggleAlphabetic
                                ) {
                                    1.5 * responsive_scale
                                } else {
                                    2.0 * responsive_scale
                                },
                                letter_spacing: tracking,
                                line_gap: base_line_gap,
                                max_lines: 1,
                                color: text_primary,
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
                    let left_w = 78.0 * responsive_scale;
                    let mid_key_w = 40.0 * responsive_scale;
                    let right_w = 118.0 * responsive_scale;
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
                                "Back",
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
                                "123",
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
                                "Back",
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
                                VirtualKeyboardKey::Space => [0.92, 0.95, 0.99, 1.0],
                                VirtualKeyboardKey::ToggleNumeric
                                | VirtualKeyboardKey::ToggleAlphabetic
                                | VirtualKeyboardKey::Backspace => surface_muted,
                                _ => [0.95, 0.97, 0.99, 1.0],
                            },
                        });
                        interactive_targets.push(InteractiveTarget {
                            kind: InteractionKind::VirtualKeyboardKey(key),
                            rect,
                        });
                        let layout = TextBlock {
                            text: label.to_string(),
                            origin: [
                                rect[0] + 10.0 * responsive_scale,
                                rect[1] + 8.0 * responsive_scale,
                            ],
                            max_width: rect[2] - 20.0 * responsive_scale,
                            pixel_size: if label.chars().count() > 6 {
                                1.5 * responsive_scale
                            } else {
                                2.0 * responsive_scale
                            },
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: if label.chars().count() > 6 { 2 } else { 1 },
                            color: text_primary,
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
                    let voice_y = button_y + metrics.tool_button_h + 8.0 * responsive_scale;
                    let voice_rect = [
                        panel_x,
                        voice_y,
                        panel_width,
                        if panel_width < 620.0 { 162.0 } else { 132.0 },
                    ];
                    quads.push(CandidateQuad {
                        rect: voice_rect,
                        color: [0.94, 0.96, 0.99, 1.0],
                    });

                    let transcript = if chrome.voice_transcript.is_empty() {
                        voice_transcript_placeholder(
                            chrome.voice_permission,
                            &chrome.voice_backend_label,
                            chrome.voice_supports_live_capture,
                        )
                    } else {
                        chrome.voice_transcript.clone()
                    };
                    let status_text = voice_status_text(
                        chrome.voice_state,
                        chrome.voice_permission,
                        !chrome.voice_transcript.is_empty(),
                        &chrome.voice_backend_label,
                    );
                    let voice_layouts = vec![
                        TextBlock {
                            text: status_text,
                            origin: [panel_x + 14.0, voice_y + 10.0],
                            max_width: panel_width - 28.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                accent
                            } else if matches!(
                                chrome.voice_permission,
                                VoicePermissionState::Denied | VoicePermissionState::Error
                            ) {
                                [0.86, 0.38, 0.38, 1.0]
                            } else {
                                text_secondary
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: transcript,
                            origin: [panel_x + 14.0, voice_y + 28.0],
                            max_width: panel_width - 28.0,
                            pixel_size: if chrome.voice_transcript.is_empty() {
                                2.4
                            } else {
                                input_value_px
                            },
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: if chrome.voice_transcript.is_empty() { 3 } else { 2 },
                            color: if chrome.voice_transcript.is_empty() {
                                text_muted
                            } else {
                                text_primary
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

                    let mut voice_actions = vec![
                        (
                            InteractionKind::ToggleVoiceCapture,
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                "Stop"
                            } else if chrome.voice_permission == VoicePermissionState::Denied {
                                "Denied"
                            } else {
                                "Listen"
                            },
                            92.0,
                            chrome.voice_state == VoiceCaptureState::Listening,
                            chrome.voice_permission != VoicePermissionState::Denied
                                && chrome.voice_permission != VoicePermissionState::Error,
                        ),
                        (
                            InteractionKind::InsertVoiceTranscript,
                            "Use Seed",
                            116.0,
                            !chrome.voice_transcript.is_empty()
                                && chrome.voice_state != VoiceCaptureState::Listening,
                            !chrome.voice_transcript.is_empty()
                                && chrome.voice_state != VoiceCaptureState::Listening,
                        ),
                    ];
                    if !chrome.voice_supports_live_capture
                        || chrome.voice_permission == VoicePermissionState::Unavailable
                    {
                        voice_actions.push((
                            InteractionKind::CycleVoiceSample,
                            "Next Sample",
                            118.0,
                            false,
                            true,
                        ));
                    }
                    if !chrome.voice_transcript.is_empty() {
                        voice_actions.push((
                            InteractionKind::ClearVoiceTranscript,
                            "Clear",
                            84.0,
                            false,
                            true,
                        ));
                    }
                    let mut voice_action_layouts = Vec::new();
                    let mut action_cursor_x = panel_x;
                    let mut action_row = 0;
                    for (kind, label, width, emphasized, enabled) in voice_actions {
                        if action_cursor_x > panel_x
                            && action_cursor_x + width > panel_x + panel_width
                        {
                            action_row += 1;
                            action_cursor_x = panel_x;
                        }
                        let rect = [
                            action_cursor_x,
                            voice_y + 96.0 + action_row as f32 * 30.0,
                            width.min(panel_width),
                            26.0,
                        ];
                        action_cursor_x += width + 10.0;
                        quads.push(CandidateQuad {
                            rect,
                            color: if emphasized {
                                accent_soft
                            } else if !enabled {
                                [0.95, 0.96, 0.98, 1.0]
                            } else {
                                [0.90, 0.94, 0.98, 1.0]
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
                                accent_text
                            } else if !enabled {
                                text_muted
                            } else {
                                text_primary
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
                    let handwriting_y = button_y + metrics.tool_button_h + 8.0 * responsive_scale;
                    let canvas_rect = [panel_x, handwriting_y + 22.0, panel_width, 108.0];
                    quads.push(CandidateQuad {
                        rect: canvas_rect,
                        color: [0.95, 0.97, 1.0, 1.0],
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::HandwritingCanvas,
                        rect: canvas_rect,
                    });

                    for stroke in &chrome.handwriting_strokes {
                        for point in sample_stroke_points(stroke) {
                            quads.push(CandidateQuad {
                                rect: [point[0] - 2.5, point[1] - 2.5, 5.0, 5.0],
                                color: accent,
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
                            color: text_secondary,
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
                            color: text_secondary,
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

                    let undo_rect = [panel_x, handwriting_y + 140.0, 74.0, 24.0];
                    let undo_enabled = !chrome.handwriting_strokes.is_empty();
                    quads.push(CandidateQuad {
                        rect: undo_rect,
                        color: if undo_enabled {
                            [0.90, 0.94, 0.98, 1.0]
                        } else {
                            [0.95, 0.96, 0.98, 1.0]
                        },
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::UndoHandwritingStroke,
                        rect: undo_rect,
                    });
                    let clear_rect = [panel_x + 82.0, handwriting_y + 140.0, 74.0, 24.0];
                    quads.push(CandidateQuad {
                        rect: clear_rect,
                        color: [0.90, 0.94, 0.98, 1.0],
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ClearHandwriting,
                        rect: clear_rect,
                    });
                    let mut handwriting_action_layouts = vec![
                        TextBlock {
                            text: "Undo".to_string(),
                            origin: [undo_rect[0] + 8.0, undo_rect[1] + 6.0],
                            max_width: undo_rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if undo_enabled {
                                text_primary
                            } else {
                                text_muted
                            },
                            align: TextAlign::Center,
                            role: TextRole::HandwritingButton,
                        }
                        .layout(),
                        TextBlock {
                            text: "Clear".to_string(),
                            origin: [clear_rect[0] + 8.0, clear_rect[1] + 6.0],
                            max_width: clear_rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: text_primary,
                            align: TextAlign::Center,
                            role: TextRole::HandwritingButton,
                        }
                        .layout(),
                    ];

                    let mut chip_x = panel_x + 166.0;
                    for (index, candidate) in
                        chrome.handwriting_candidates.iter().take(3).enumerate()
                    {
                        let chip_w = (candidate.chars().count() as f32 * 9.5).max(52.0) + 16.0;
                        let rect = [chip_x, handwriting_y + 140.0, chip_w, 24.0];
                        quads.push(CandidateQuad {
                            rect,
                            color: if index == 0 {
                                accent_soft
                            } else {
                                [0.92, 0.95, 0.99, 1.0]
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
                                accent_text
                            } else {
                                text_primary
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
                    metrics.settings_panel_h - 8.0,
                ];
                quads.push(CandidateQuad {
                    rect: settings_rect,
                    color: surface,
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
                        "Voice Auto",
                        [
                            (
                                InteractionKind::SetVoiceAutoInsert(true),
                                "On",
                                chrome.voice_auto_insert,
                            ),
                            (
                                InteractionKind::SetVoiceAutoInsert(false),
                                "Off",
                                !chrome.voice_auto_insert,
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
                        [(
                            InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                            "Llama3.2 3B",
                            chrome.llm_model == LlmModelPreset::Llama32_3b,
                        )]
                        .to_vec(),
                    ),
                    (
                        "Heat",
                        [
                            (
                                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Focused),
                                "Focused",
                                chrome.llm_temperature == LlmTemperaturePreset::Focused,
                            ),
                            (
                                InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Balanced),
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
                        color: text_secondary,
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
                                accent_soft
                            } else {
                                [0.92, 0.95, 0.99, 1.0]
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
                            color: if *selected { accent_text } else { text_primary },
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
                let chip_section_rect = [
                    panel_x,
                    chip_section_y - 4.0 * responsive_scale,
                    panel_width,
                    metrics.chip_section_h,
                ];
                quads.push(CandidateQuad {
                    rect: chip_section_rect,
                    color: surface,
                });
                let next_label_layouts = vec![
                    TextBlock {
                        text: if chrome.composed_tokens.is_empty() {
                            "Next tokens".to_string()
                        } else {
                            format!("Next tokens  |  {}", chrome.composed_tokens.join(" "))
                        },
                        origin: [panel_x + 2.0 * responsive_scale, chip_section_y],
                        max_width: panel_width - 96.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: text_secondary,
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
                    let back_rect = if metrics.stacked_token_header {
                        [
                            panel_x,
                            chip_section_y + 20.0 * responsive_scale,
                            78.0 * responsive_scale,
                            22.0 * responsive_scale,
                        ]
                    } else {
                        [
                            panel_x + panel_width - 78.0 * responsive_scale,
                            chip_section_y - 2.0 * responsive_scale,
                            78.0 * responsive_scale,
                            22.0 * responsive_scale,
                        ]
                    };
                    quads.push(CandidateQuad {
                        rect: back_rect,
                        color: surface_muted,
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::RewindNextToken,
                        rect: back_rect,
                    });
                    let back_layout = TextBlock {
                        text: "Back".to_string(),
                        origin: [
                            back_rect[0] + 8.0 * responsive_scale,
                            back_rect[1] + 6.0 * responsive_scale,
                        ],
                        max_width: back_rect[2] - 16.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: text_primary,
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

                let chip_y = if metrics.stacked_token_header {
                    chip_section_y + 52.0 * responsive_scale
                } else {
                    chip_section_y + 24.0 * responsive_scale
                };
                let mut chip_x = panel_x;
                let mut row = 0;
                let mut chip_layouts = Vec::new();
                for (index, token) in chrome.next_token_candidates.iter().take(6).enumerate() {
                    let chip_w =
                        ((token.chars().count() as f32 * 12.0).max(60.0) + 20.0) * responsive_scale;
                    if chip_x + chip_w > panel_x + panel_width {
                        row += 1;
                        chip_x = panel_x;
                    }
                    if row >= 2 {
                        break;
                    }
                    let rect = [
                        chip_x,
                        chip_y + row as f32 * 30.0 * responsive_scale,
                        chip_w,
                        24.0 * responsive_scale,
                    ];
                    quads.push(CandidateQuad {
                        rect,
                        color: if index == 0 {
                            accent_soft
                        } else {
                            [0.94, 0.96, 0.99, 1.0]
                        },
                    });
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::SelectNextToken(index),
                        rect,
                    });
                    let layout = TextBlock {
                        text: token.clone(),
                        origin: [
                            rect[0] + 8.0 * responsive_scale,
                            rect[1] + 6.0 * responsive_scale,
                        ],
                        max_width: rect[2] - 16.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: if index == 0 {
                            accent_text
                        } else {
                            text_primary
                        },
                        align: TextAlign::Center,
                        role: TextRole::NextTokenChip,
                    }
                    .layout();
                    text_quads.extend(layout.quads.iter().copied());
                    atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                    chip_layouts.push(layout);
                    chip_x += chip_w + 8.0 * responsive_scale;
                }
                if !chip_layouts.is_empty() {
                    text_sections.push(TextSection {
                        role: TextRole::NextTokenChip,
                        layouts: chip_layouts,
                    });
                }
            }

            let sentence_y = suggestions_y + metrics.chip_section_h;
            let candidate_columns = metrics.candidate_columns;
            let candidate_gap_x = 12.0 * responsive_scale;
                let candidate_card_w = if candidate_columns == 2 {
                    (panel_width - candidate_gap_x) / 2.0
                } else {
                    panel_width
                };
            if !visible_sentence_candidates.is_empty() {
                let sentence_rows = visible_sentence_candidates
                    .len()
                    .div_ceil(candidate_columns);
                let sentence_section_h = sentence_rows as f32 * metrics.item_height
                    + (sentence_rows as f32 - 1.0).max(0.0) * metrics.item_gap;
                quads.push(CandidateQuad {
                    rect: [panel_x, sentence_y, panel_width, sentence_section_h],
                    color: surface,
                });
            }
            for (index, label) in visible_sentence_candidates.iter().enumerate() {
                let column = index % candidate_columns;
                let row = index / candidate_columns;
                let x = panel_x + column as f32 * (candidate_card_w + candidate_gap_x);
                let y = sentence_y + row as f32 * (metrics.item_height + metrics.item_gap);
                let selected = index == snapshot.selected_index;
                let continuation = display_candidate_continuation(&snapshot.seed_text, label);
                let quad = CandidateQuad {
                    rect: [x, y, candidate_card_w, metrics.item_height],
                    color: if selected {
                        accent_soft
                    } else if snapshot.degraded {
                        border_dark
                    } else {
                        [0.95, 0.97, 0.99, 1.0]
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
                let primary_layout = TextBlock {
                    text: continuation,
                    origin: [x + 18.0 * responsive_scale, y + 13.0 * responsive_scale],
                    max_width: candidate_card_w - 36.0 * responsive_scale,
                    pixel_size: input_value_px,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: if chrome.preview_style == PreviewStyle::Full {
                        3
                    } else {
                        2
                    },
                    color: if selected { accent_text } else { text_primary },
                    align: TextAlign::Left,
                    role: TextRole::CandidatePrimary,
                }
                .layout();
                let meta_y =
                    (primary_layout.bounds[1] + primary_layout.bounds[3] + 8.0 * responsive_scale)
                        .max(y + 48.0 * responsive_scale);
                let candidate_layouts = vec![
                    primary_layout,
                    TextBlock {
                        text: if selected {
                            "Selected sentence".to_string()
                        } else {
                            "Tap to use this sentence".to_string()
                        },
                        origin: [x + 18.0 * responsive_scale, meta_y],
                        max_width: candidate_card_w - 36.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: tracking,
                        line_gap: base_line_gap,
                        max_lines: 2,
                        color: if selected {
                            [0.20, 0.36, 0.50, 1.0]
                        } else {
                            text_secondary
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

        pub fn build_settings_scene(&self, chrome: &PanelChromeState) -> RenderScene {
            let page_bg = [0.93, 0.95, 0.98, 0.98];
            let surface = [0.87, 0.90, 0.95, 1.0];
            let surface_alt = [0.92, 0.95, 0.99, 1.0];
            let accent_soft = [0.73, 0.88, 0.98, 1.0];
            let accent_text = [0.12, 0.23, 0.36, 1.0];
            let text_primary = [0.22, 0.28, 0.38, 1.0];
            let text_secondary = [0.39, 0.47, 0.58, 1.0];
            let tracking = match chrome.text_spacing {
                TextSpacing::Tight => -0.3,
                TextSpacing::Normal => 0.0,
                TextSpacing::Relaxed => 0.8,
            };
            let base_line_gap = match chrome.candidate_density {
                CandidateDensity::Compact => 4.0,
                CandidateDensity::Cozy => 6.0,
            };
            let panel_width = self.scene_width.clamp(320.0, 520.0) - 28.0;
            let panel_x = ((self.scene_width - panel_width) / 2.0).max(12.0);
            let panel_y = 14.0;
            let row_h = 24.0;
            let title_h = 34.0;
            let settings_row_count = 10.0;
            let panel_height = title_h + 18.0 + settings_row_count * row_h + 18.0;

            let mut quads = Vec::new();
            let mut text_quads = Vec::new();
            let mut atlas_glyphs = Vec::new();
            let mut text_sections = Vec::new();
            let hit_targets = Vec::new();
            let mut interactive_targets = Vec::new();

            quads.push(CandidateQuad {
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
            quads.push(CandidateQuad {
                rect: [panel_x, panel_y, panel_width, panel_height],
                color: surface,
            });

            let close_rect = [panel_x + panel_width - 32.0, panel_y + 8.0, 18.0, 18.0];
            quads.push(CandidateQuad {
                rect: close_rect,
                color: surface_alt,
            });
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsToggle,
                rect: close_rect,
            });
            append_gear_icon_quads(&mut quads, close_rect, text_secondary, surface_alt);

            let title_layout = TextBlock {
                text: "Panel settings".to_string(),
                origin: [panel_x + 14.0, panel_y + 10.0],
                max_width: panel_width - 56.0,
                pixel_size: 3.0,
                letter_spacing: tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_primary,
                align: TextAlign::Left,
                role: TextRole::HeaderTitle,
            }
            .layout();
            text_quads.extend(title_layout.quads.iter().copied());
            atlas_glyphs.extend(title_layout.atlas_glyphs.iter().cloned());
            text_sections.push(TextSection {
                role: TextRole::HeaderTitle,
                layouts: vec![title_layout],
            });

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
                    "Voice Auto",
                    [
                        (
                            InteractionKind::SetVoiceAutoInsert(true),
                            "On",
                            chrome.voice_auto_insert,
                        ),
                        (
                            InteractionKind::SetVoiceAutoInsert(false),
                            "Off",
                            !chrome.voice_auto_insert,
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
                    [(
                        InteractionKind::SetLlmModel(LlmModelPreset::Llama32_3b),
                        "Llama3.2 3B",
                        chrome.llm_model == LlmModelPreset::Llama32_3b,
                    )]
                    .to_vec(),
                ),
                (
                    "Heat",
                    [
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Focused),
                            "Focused",
                            chrome.llm_temperature == LlmTemperaturePreset::Focused,
                        ),
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Balanced),
                            "Balanced",
                            chrome.llm_temperature == LlmTemperaturePreset::Balanced,
                        ),
                        (
                            InteractionKind::SetLlmTemperature(LlmTemperaturePreset::Expressive),
                            "Expressive",
                            chrome.llm_temperature == LlmTemperaturePreset::Expressive,
                        ),
                    ]
                    .to_vec(),
                ),
            ];

            let mut label_layouts = Vec::new();
            let mut option_layouts = Vec::new();
            for (section_index, (label, options)) in sections.iter().enumerate() {
                let row_y = panel_y + title_h + 10.0 + section_index as f32 * row_h;
                let label_layout = TextBlock {
                    text: (*label).to_string(),
                    origin: [panel_x + 14.0, row_y + 2.0],
                    max_width: 72.0,
                    pixel_size: 2.0,
                    letter_spacing: tracking,
                    line_gap: base_line_gap,
                    max_lines: 1,
                    color: text_secondary,
                    align: TextAlign::Left,
                    role: TextRole::SettingLabel,
                }
                .layout();
                text_quads.extend(label_layout.quads.iter().copied());
                atlas_glyphs.extend(label_layout.atlas_glyphs.iter().cloned());
                label_layouts.push(label_layout);

                let mut chip_x = panel_x + 86.0;
                for (kind, chip_label, selected) in options {
                    let chip_w = (chip_label.chars().count() as f32 * 10.0).max(34.0) + 12.0;
                    let rect = [chip_x, row_y, chip_w, 20.0];
                    quads.push(CandidateQuad {
                        rect,
                        color: if *selected { accent_soft } else { surface_alt },
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
                        color: if *selected { accent_text } else { text_primary },
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
                layouts: label_layouts,
            });
            text_sections.push(TextSection {
                role: TextRole::SettingOption,
                layouts: option_layouts,
            });

            RenderScene {
                quads,
                text_quads,
                atlas_glyphs,
                text_sections,
                hit_targets,
                interactive_targets,
                labels: Vec::new(),
                selected_label: None,
                draft_text: String::new(),
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
