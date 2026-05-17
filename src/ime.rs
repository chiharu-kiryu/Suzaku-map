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

fn append_rounded_rect_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    radius: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let radius = radius.min(w * 0.5).min(h * 0.5).max(0.0);
    if radius <= 1.0 {
        quads.push(gpu::CandidateQuad { rect, color });
        return;
    }

    quads.push(gpu::CandidateQuad {
        rect: [x + radius, y, w - radius * 2.0, h],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x, y + radius, radius, h - radius * 2.0],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + w - radius, y + radius, radius, h - radius * 2.0],
        color,
    });

    let steps = 6;
    for step in 0..steps {
        let y0 = step as f32 / steps as f32 * radius;
        let y1 = (step + 1) as f32 / steps as f32 * radius;
        let mid = (y0 + y1) * 0.5;
        let inset = radius - (radius * radius - (radius - mid).powi(2)).sqrt();
        let strip_h = (y1 - y0).max(1.0);
        let strip_w = (radius - inset).max(1.0);

        quads.push(gpu::CandidateQuad {
            rect: [x + inset, y + y0, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + w - radius, y + y0, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + inset, y + h - y1, strip_w, strip_h],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + w - radius, y + h - y1, strip_w, strip_h],
            color,
        });
    }
}

fn append_soft_card_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    fill: [f32; 4],
    outline: [f32; 4],
    shadow: [f32; 4],
    cutout: [f32; 4],
    radius: f32,
) {
    let [x, y, w, h] = rect;
    let radius = radius.min(w * 0.22).min(h * 0.35).max(3.0);
    let border = 1.5_f32.min(w * 0.04).min(h * 0.10).max(1.0);

    let deep_shadow = [shadow[0], shadow[1], shadow[2], shadow[3] * 0.78];
    let ambient_shadow = [shadow[0], shadow[1], shadow[2], shadow[3] * 0.38];
    append_rounded_rect_quads(
        quads,
        [x + 1.0, y + 2.0, w, h],
        ambient_shadow,
        radius + 2.0,
    );
    append_rounded_rect_quads(quads, [x + 3.0, y + 6.0, w, h], deep_shadow, radius + 1.5);
    append_rounded_rect_quads(quads, rect, outline, radius);
    append_rounded_rect_quads(
        quads,
        [x + border, y + border, w - border * 2.0, h - border * 2.0],
        fill,
        (radius - border).max(1.0),
    );
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 1.5,
            y + h - (h * 0.16).max(5.0) - border,
            w - border * 3.0,
            (h * 0.16).max(5.0),
        ],
        color: [0.12, 0.18, 0.28, 0.035],
    });
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 2.0,
            y + border * 2.0,
            w - border * 4.0,
            (h * 0.14).max(4.0),
        ],
        color: [1.0, 1.0, 1.0, 0.11],
    });
    quads.push(gpu::CandidateQuad {
        rect: [
            x + border * 2.2,
            y + border * 1.3,
            w - border * 4.4,
            border.max(1.0),
        ],
        color: [1.0, 1.0, 1.0, 0.15],
    });
    let cut = radius * 0.18;
    append_rounded_rect_quads(quads, [x + border, y + border, cut, cut], fill, cut * 0.6);
    append_rounded_rect_quads(
        quads,
        [x + w - border - cut, y + border, cut, cut],
        fill,
        cut * 0.6,
    );
    append_rounded_rect_quads(
        quads,
        [x + border, y + h - border - cut, cut, cut],
        fill,
        cut * 0.6,
    );
    append_rounded_rect_quads(
        quads,
        [x + w - border - cut, y + h - border - cut, cut, cut],
        fill,
        cut * 0.6,
    );

    let _ = cutout;
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
    let body = [x + unit * 0.28, y + unit * 0.36, unit * 0.28, unit * 0.22];
    let neck = [x + unit * 0.50, y + unit * 0.24, unit * 0.10, unit * 0.16];
    let head = [x + unit * 0.57, y + unit * 0.20, unit * 0.14, unit * 0.14];
    let crest = [x + unit * 0.49, y + unit * 0.18, unit * 0.10, unit * 0.08];
    let wing_upper = [x + unit * 0.22, y + unit * 0.28, unit * 0.24, unit * 0.12];
    let wing_main = [x + unit * 0.18, y + unit * 0.36, unit * 0.34, unit * 0.14];
    let tail_base = [x + unit * 0.44, y + unit * 0.56, unit * 0.22, unit * 0.09];
    let tail_flare = [x + unit * 0.54, y + unit * 0.64, unit * 0.22, unit * 0.08];
    let tail_tip = [x + unit * 0.64, y + unit * 0.72, unit * 0.14, unit * 0.06];
    let beak_rect = [x + unit * 0.70, y + unit * 0.28, unit * 0.12, unit * 0.06];
    let eye_rect = [x + unit * 0.63, y + unit * 0.28, unit * 0.03, unit * 0.03];

    for bird_rect in [
        body, neck, head, crest, wing_upper, wing_main, tail_base, tail_flare, tail_tip,
    ] {
        quads.push(gpu::CandidateQuad {
            rect: bird_rect,
            color: primary,
        });
    }
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.52, unit * 0.20, unit * 0.08],
        color: secondary,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.36, y + unit * 0.60, unit * 0.22, unit * 0.06],
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

fn append_keyboard_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let pad = w.min(h) * 0.22;
    quads.push(gpu::CandidateQuad {
        rect: [x + pad, y + pad * 1.1, w - pad * 2.0, h - pad * 2.0],
        color,
    });
    let key_w = (w - pad * 3.2) / 3.0;
    let key_h = (h - pad * 4.8) / 3.0;
    for row in 0..2 {
        for col in 0..3 {
            quads.push(gpu::CandidateQuad {
                rect: [
                    x + pad * 1.6 + col as f32 * (key_w + pad * 0.4),
                    y + pad * 1.6 + row as f32 * (key_h + pad * 0.5),
                    key_w,
                    key_h,
                ],
                color: [0.95, 0.98, 1.0, 0.95],
            });
        }
    }
    quads.push(gpu::CandidateQuad {
        rect: [x + pad * 1.6, y + h - pad * 2.0, w - pad * 3.2, key_h * 0.9],
        color: [0.95, 0.98, 1.0, 0.95],
    });
}

fn append_mic_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.40, y + unit * 0.22, unit * 0.20, unit * 0.30],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.35, y + unit * 0.54, unit * 0.30, unit * 0.07],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.48, y + unit * 0.61, unit * 0.04, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.37, y + unit * 0.72, unit * 0.26, unit * 0.05],
        color,
    });
}

fn append_pen_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.30, y + unit * 0.52, unit * 0.30, unit * 0.09],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.54, y + unit * 0.36, unit * 0.10, unit * 0.20],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.64, y + unit * 0.28, unit * 0.08, unit * 0.08],
        color,
    });
}

fn append_backspace_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.32, y + unit * 0.34, unit * 0.26, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.32, y + unit * 0.58, unit * 0.26, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.44, unit * 0.14, unit * 0.08],
        color,
    });
}

fn append_refresh_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.26, unit * 0.32, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.52, y + unit * 0.34, unit * 0.08, unit * 0.24],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.40, y + unit * 0.58, unit * 0.22, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.24, y + unit * 0.48, unit * 0.08, unit * 0.20],
        color,
    });
}

fn append_trash_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.28, unit * 0.32, unit * 0.06],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.30, y + unit * 0.36, unit * 0.40, unit * 0.30],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.44, y + unit * 0.20, unit * 0.12, unit * 0.06],
        color,
    });
}

fn append_undo_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.42, unit * 0.30, unit * 0.08],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.22, y + unit * 0.32, unit * 0.08, unit * 0.18],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.44, y + unit * 0.50, unit * 0.18, unit * 0.08],
        color,
    });
}

fn append_seed_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.46, y + unit * 0.24, unit * 0.08, unit * 0.44],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.28, y + unit * 0.42, unit * 0.44, unit * 0.08],
        color,
    });
}

fn append_next_icon_quads(quads: &mut Vec<gpu::CandidateQuad>, rect: [f32; 4], color: [f32; 4]) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.28, unit * 0.12, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.46, y + unit * 0.40, unit * 0.12, unit * 0.12],
        color,
    });
    quads.push(gpu::CandidateQuad {
        rect: [x + unit * 0.34, y + unit * 0.52, unit * 0.12, unit * 0.12],
        color,
    });
}

fn append_chevron_icon_quads(
    quads: &mut Vec<gpu::CandidateQuad>,
    rect: [f32; 4],
    color: [f32; 4],
    expanded: bool,
) {
    let [x, y, w, h] = rect;
    let unit = w.min(h);
    if expanded {
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.28, y + unit * 0.46, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.46, y + unit * 0.34, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.64, y + unit * 0.46, unit * 0.08, unit * 0.08],
            color,
        });
    } else {
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.28, y + unit * 0.34, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.46, y + unit * 0.46, unit * 0.18, unit * 0.08],
            color,
        });
        quads.push(gpu::CandidateQuad {
            rect: [x + unit * 0.64, y + unit * 0.34, unit * 0.08, unit * 0.08],
            color,
        });
    }
}

#[cfg(feature = "gpu")]
pub mod gpu {
    use crate::platform::voice_host::{voice_status_text, voice_transcript_placeholder};

    use super::{
        Snapshot, append_backspace_icon_quads, append_chevron_icon_quads, append_gear_icon_quads,
        append_keyboard_icon_quads, append_mic_icon_quads, append_next_icon_quads,
        append_pen_icon_quads, append_refresh_icon_quads, append_rounded_rect_quads,
        append_seed_icon_quads, append_soft_card_quads, append_suzaku_bird_icon_quads,
        append_trash_icon_quads, append_undo_icon_quads, measure_text_prefix_width,
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
        pub composed_tokens: Vec<String>,
        pub next_token_candidates: Vec<String>,
        pub sentence_candidates: Vec<String>,
        pub sentence_candidate_source_indices: Vec<usize>,
        pub handwriting_strokes: Vec<Vec<[f32; 2]>>,
        pub handwriting_candidates: Vec<String>,
        pub handwriting_hint: String,
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
                composed_tokens: Vec::new(),
                next_token_candidates: Vec::new(),
                sentence_candidates: Vec::new(),
                sentence_candidate_source_indices: Vec::new(),
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
        SetThemePreset(ThemePreset),
        SetLlmEnabled(bool),
        SetVoiceAutoInsert(bool),
        SetLlmModel(LlmModelPreset),
        SetLlmTemperature(LlmTemperaturePreset),
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

    #[derive(Clone, Copy, Debug)]
    struct PanelTheme {
        page_bg: [f32; 4],
        shell: [f32; 4],
        shell_border: [f32; 4],
        surface: [f32; 4],
        surface_alt: [f32; 4],
        surface_muted: [f32; 4],
        keyboard_surface: [f32; 4],
        keyboard_special_surface: [f32; 4],
        keyboard_text: [f32; 4],
        keyboard_secondary_text: [f32; 4],
        accent: [f32; 4],
        accent_soft: [f32; 4],
        accent_text: [f32; 4],
        text_primary: [f32; 4],
        text_secondary: [f32; 4],
        text_muted: [f32; 4],
        border_dark: [f32; 4],
        soft_shadow: [f32; 4],
    }

    impl PanelTheme {
        fn for_preset(preset: ThemePreset) -> Self {
            match preset {
                ThemePreset::Daylight => Self {
                    page_bg: [0.72, 0.78, 0.87, 1.0],
                    shell: [0.83, 0.88, 0.95, 1.0],
                    shell_border: [0.49, 0.58, 0.71, 1.0],
                    surface: [0.89, 0.93, 0.98, 1.0],
                    surface_alt: [0.81, 0.87, 0.95, 1.0],
                    surface_muted: [0.73, 0.79, 0.88, 1.0],
                    keyboard_surface: [0.90, 0.94, 0.99, 1.0],
                    keyboard_special_surface: [0.74, 0.80, 0.89, 1.0],
                    keyboard_text: [0.06, 0.09, 0.14, 1.0],
                    keyboard_secondary_text: [0.08, 0.12, 0.18, 1.0],
                    accent: [0.13, 0.44, 0.82, 1.0],
                    accent_soft: [0.63, 0.81, 1.0, 1.0],
                    accent_text: [0.04, 0.13, 0.26, 1.0],
                    text_primary: [0.06, 0.10, 0.16, 1.0],
                    text_secondary: [0.10, 0.15, 0.22, 1.0],
                    text_muted: [0.18, 0.25, 0.34, 1.0],
                    border_dark: [0.45, 0.55, 0.68, 1.0],
                    soft_shadow: [0.22, 0.30, 0.43, 0.16],
                },
                ThemePreset::DeviceDark => Self {
                    page_bg: [0.15, 0.18, 0.24, 1.0],
                    shell: [0.19, 0.23, 0.31, 1.0],
                    shell_border: [0.33, 0.40, 0.52, 1.0],
                    surface: [0.23, 0.27, 0.36, 1.0],
                    surface_alt: [0.26, 0.31, 0.41, 1.0],
                    surface_muted: [0.29, 0.34, 0.44, 1.0],
                    keyboard_surface: [0.18, 0.22, 0.30, 1.0],
                    keyboard_special_surface: [0.28, 0.33, 0.43, 1.0],
                    keyboard_text: [0.96, 0.98, 1.0, 1.0],
                    keyboard_secondary_text: [0.88, 0.93, 0.99, 1.0],
                    accent: [0.40, 0.68, 1.0, 1.0],
                    accent_soft: [0.22, 0.41, 0.63, 1.0],
                    accent_text: [0.93, 0.97, 1.0, 1.0],
                    text_primary: [0.92, 0.95, 1.0, 1.0],
                    text_secondary: [0.74, 0.81, 0.90, 1.0],
                    text_muted: [0.56, 0.65, 0.77, 1.0],
                    border_dark: [0.37, 0.46, 0.59, 1.0],
                    soft_shadow: [0.03, 0.05, 0.09, 0.34],
                },
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct PanelSceneMetrics {
        panel_width: f32,
        panel_x: f32,
        panel_y: f32,
        panel_height: f32,
        section_gap: f32,
        input_box_h: f32,
        tools_header_h: f32,
        tool_button_h: f32,
        tool_gap: f32,
        tools_content_h: f32,
        settings_panel_h: f32,
        item_height: f32,
        hero_item_height: f32,
        item_gap: f32,
        chip_section_h: f32,
        sentence_section_gap: f32,
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
            let collapsed_daily_mode = !chrome.input_modes_expanded;
            let scene_margin = (7.0 * responsive_scale).max(6.0);
            let max_panel_width = (scene_width - scene_margin * 2.0).max(320.0);
            let min_panel_width = 380.0_f32.min(max_panel_width);
            let desired_panel_width = scene_width * 0.94;
            let panel_width = desired_panel_width
                .min(max_panel_width)
                .max(min_panel_width);
            let input_box_h = 58.0 * responsive_scale;
            let tools_header_h = 28.0 * responsive_scale;
            let tool_button_h = 22.0 * responsive_scale;
            let tool_gap = 5.0 * responsive_scale;
            let section_gap = 6.0 * responsive_scale;
            let extended_input_panel_h = if chrome.input_modes_expanded {
                match chrome.active_input_mode {
                    InputMode::VirtualKeyboard => 142.0 * responsive_scale,
                    InputMode::Dictation => {
                        if max_panel_width < 620.0 {
                            142.0 * responsive_scale
                        } else {
                            122.0 * responsive_scale
                        }
                    }
                    InputMode::Handwriting => 170.0 * responsive_scale,
                }
            } else {
                0.0
            };
            let tools_content_h = if chrome.input_modes_expanded {
                extended_input_panel_h
            } else {
                0.0
            };
            let settings_panel_h = if chrome.settings_open { 286.0 } else { 0.0 };
            let item_height = match (chrome.candidate_density, chrome.preview_style) {
                (CandidateDensity::Compact, PreviewStyle::Compact) => 62.0,
                (CandidateDensity::Compact, PreviewStyle::Full) => 82.0,
                (CandidateDensity::Cozy, PreviewStyle::Compact) => 72.0,
                (CandidateDensity::Cozy, PreviewStyle::Full) => 92.0,
            } * responsive_scale;
            let hero_item_height = item_height + 12.0 * responsive_scale;
            let item_gap = if chrome.candidate_density == CandidateDensity::Compact {
                5.0
            } else {
                6.0
            } * responsive_scale;
            let stacked_token_header =
                max_panel_width < 560.0 && !chrome.next_token_candidates.is_empty();
            let chip_section_h = if chrome.next_token_candidates.is_empty() {
                0.0
            } else {
                (if stacked_token_header { 74.0 } else { 52.0 }) * responsive_scale
            };
            let sentence_section_gap = if chip_section_h > 0.0 && sentence_count > 0 {
                6.0 * responsive_scale
            } else {
                0.0
            };
            let candidate_columns = if collapsed_daily_mode {
                sentence_count.clamp(1, 4)
            } else if panel_width >= 760.0 && sentence_count > 2 {
                2
            } else {
                1
            };
            let sentence_rows = if collapsed_daily_mode {
                if sentence_count == 0 { 0 } else { 1 }
            } else if sentence_count == 0 {
                0
            } else {
                (sentence_count - 1).div_ceil(candidate_columns)
            };
            let sentence_height = if collapsed_daily_mode {
                if sentence_count == 0 {
                    0.0
                } else {
                    52.0 * responsive_scale
                }
            } else if sentence_rows == 0 {
                if sentence_count == 0 {
                    0.0
                } else {
                    hero_item_height
                }
            } else {
                hero_item_height
                    + item_gap
                    + sentence_rows as f32 * item_height
                    + (sentence_rows as f32 - 1.0) * item_gap
            };
            let panel_height = input_box_h
                + section_gap
                + tools_header_h
                + tools_content_h
                + settings_panel_h
                + section_gap
                + chip_section_h
                + sentence_section_gap
                + sentence_height;
            let panel_x = ((scene_width - panel_width) / 2.0).max(scene_margin);
            let centered_panel_y = (scene_height - panel_height) / 2.0;
            let panel_y = centered_panel_y.max(14.0 * responsive_scale);

            Self {
                panel_width,
                panel_x,
                panel_y,
                panel_height,
                section_gap,
                input_box_h,
                tools_header_h,
                tool_button_h,
                tool_gap,
                tools_content_h,
                settings_panel_h,
                item_height,
                hero_item_height,
                item_gap,
                chip_section_h,
                sentence_section_gap,
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
                sentence_candidate_source_indices: (0..snapshot.candidate_labels.len())
                    .take(4)
                    .collect(),
                ..PanelChromeState::default()
            };
            self.build_panel_scene(snapshot, &chrome)
        }

        pub fn build_compact_scene(
            &self,
            snapshot: &Snapshot,
            chrome: &PanelChromeState,
            hovered: bool,
            pressed: bool,
        ) -> RenderScene {
            let theme = PanelTheme::for_preset(chrome.theme_preset);
            let page_bg = theme.page_bg;
            let halo = if pressed {
                [0.42, 0.70, 0.97, 0.24]
            } else if hovered {
                [0.48, 0.75, 1.0, 0.20]
            } else {
                [0.40, 0.66, 0.95, 0.12]
            };
            let shell = if pressed {
                [0.71, 0.83, 0.96, 1.0]
            } else if hovered {
                [0.78, 0.88, 0.98, 1.0]
            } else {
                [0.73, 0.84, 0.97, 1.0]
            };
            let shell_inner = if pressed {
                [0.87, 0.93, 0.99, 1.0]
            } else {
                [0.91, 0.96, 1.0, 1.0]
            };
            let shell_core = if pressed {
                [0.96, 0.98, 1.0, 1.0]
            } else {
                [0.98, 0.99, 1.0, 1.0]
            };
            let shell_shadow = match chrome.theme_preset {
                ThemePreset::Daylight => [0.19, 0.28, 0.41, 0.24],
                ThemePreset::DeviceDark => [0.01, 0.03, 0.07, 0.42],
            };
            let glass_ring = if pressed {
                [0.90, 0.95, 1.0, 0.22]
            } else if hovered {
                [0.93, 0.97, 1.0, 0.24]
            } else {
                [0.88, 0.94, 1.0, 0.18]
            };
            let contact_shadow = match chrome.theme_preset {
                ThemePreset::Daylight => [0.15, 0.23, 0.35, 0.16],
                ThemePreset::DeviceDark => [0.01, 0.02, 0.05, 0.28],
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
            let orb_size = self.scene_width.min(self.scene_height) - 16.0;
            let orb_size = orb_size.clamp(56.0, 92.0);
            let orb_rect = [
                (self.scene_width - orb_size) / 2.0,
                (self.scene_height - orb_size) / 2.0,
                orb_size,
                orb_size,
            ];
            let halo_rect = [
                orb_rect[0] - orb_size * 0.10,
                orb_rect[1] - orb_size * 0.10,
                orb_size * 1.20,
                orb_size * 1.20,
            ];
            let contact_rect = [
                orb_rect[0] + orb_size * 0.18,
                orb_rect[1] + orb_size * 0.90,
                orb_size * 0.64,
                orb_size * 0.09,
            ];
            let ring_rect = [
                orb_rect[0] + orb_size * 0.08,
                orb_rect[1] + orb_size * 0.08,
                orb_size * 0.84,
                orb_size * 0.84,
            ];
            let inner_rect = [
                orb_rect[0] + orb_size * 0.11,
                orb_rect[1] + orb_size * 0.11,
                orb_size * 0.78,
                orb_size * 0.78,
            ];
            let core_rect = [
                orb_rect[0] + orb_size * 0.24,
                orb_rect[1] + orb_size * 0.24,
                orb_size * 0.52,
                orb_size * 0.52,
            ];
            append_rounded_rect_quads(&mut quads, halo_rect, halo, halo_rect[2] * 0.5);
            append_rounded_rect_quads(
                &mut quads,
                contact_rect,
                contact_shadow,
                contact_rect[3] * 0.5,
            );
            append_soft_card_quads(
                &mut quads,
                orb_rect,
                shell,
                [0.41, 0.55, 0.73, 1.0],
                shell_shadow,
                theme.shell,
                orb_rect[2] * 0.5,
            );
            append_rounded_rect_quads(&mut quads, ring_rect, glass_ring, ring_rect[2] * 0.5);
            append_rounded_rect_quads(
                &mut quads,
                [
                    orb_rect[0] + orb_size * 0.04,
                    orb_rect[1] + orb_size * 0.04,
                    orb_size * 0.92,
                    orb_size * 0.18,
                ],
                [1.0, 1.0, 1.0, 0.10],
                orb_rect[2] * 0.26,
            );
            append_rounded_rect_quads(&mut quads, inner_rect, shell_inner, inner_rect[2] * 0.5);
            append_rounded_rect_quads(
                &mut quads,
                [
                    inner_rect[0] + orb_size * 0.03,
                    inner_rect[1] + orb_size * 0.03,
                    inner_rect[2] - orb_size * 0.06,
                    inner_rect[3] * 0.28,
                ],
                [1.0, 1.0, 1.0, 0.12],
                inner_rect[2] * 0.18,
            );
            append_rounded_rect_quads(&mut quads, core_rect, shell_core, core_rect[2] * 0.5);
            append_rounded_rect_quads(
                &mut quads,
                [
                    orb_rect[0] + orb_size * 0.22,
                    orb_rect[1] + orb_size * 0.80,
                    orb_size * 0.56,
                    orb_size * 0.06,
                ],
                [0.33, 0.63, 0.96, 0.85],
                orb_size * 0.03,
            );
            let mut targets = vec![InteractiveTarget {
                kind: InteractionKind::ToggleCompactMode,
                rect: orb_rect,
            }];
            let icon_rect = [
                orb_rect[0] + orb_size * 0.15,
                orb_rect[1] + orb_size * 0.15,
                orb_size * 0.70,
                orb_size * 0.70,
            ];
            append_suzaku_bird_icon_quads(
                &mut quads,
                icon_rect,
                bird_primary,
                bird_secondary,
                bird_beak,
                bird_eye,
            );
            if !snapshot.candidate_labels.is_empty() {
                append_rounded_rect_quads(
                    &mut quads,
                    [
                        orb_rect[0] + orb_size * 0.74,
                        orb_rect[1] + orb_size * 0.18,
                        orb_size * 0.12,
                        orb_size * 0.12,
                    ],
                    [0.96, 0.33, 0.30, 1.0],
                    orb_size * 0.06,
                );
                append_rounded_rect_quads(
                    &mut quads,
                    [
                        orb_rect[0] + orb_size * 0.78,
                        orb_rect[1] + orb_size * 0.22,
                        orb_size * 0.04,
                        orb_size * 0.04,
                    ],
                    [1.0, 0.95, 0.95, 0.95],
                    orb_size * 0.02,
                );
            }
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
            let theme = PanelTheme::for_preset(chrome.theme_preset);
            let page_bg = theme.page_bg;
            let shell = theme.shell;
            let shell_border = theme.shell_border;
            let surface = theme.surface;
            let surface_alt = theme.surface_alt;
            let accent = theme.accent;
            let surface_muted = theme.surface_muted;
            let accent_soft = theme.accent_soft;
            let accent_text = theme.accent_text;
            let text_primary = theme.text_primary;
            let text_secondary = theme.text_secondary;
            let text_muted = theme.text_muted;
            let keyboard_surface = theme.keyboard_surface;
            let keyboard_special_surface = theme.keyboard_special_surface;
            let keyboard_text = theme.keyboard_text;
            let keyboard_secondary_text = theme.keyboard_secondary_text;
            let border_dark = theme.border_dark;
            let soft_shadow = theme.soft_shadow;
            let surface_bright = match chrome.theme_preset {
                ThemePreset::Daylight => [0.93, 0.96, 0.99, 1.0],
                ThemePreset::DeviceDark => [0.28, 0.33, 0.43, 1.0],
            };
            let input_surface = match chrome.theme_preset {
                ThemePreset::Daylight => [0.92, 0.96, 0.99, 1.0],
                ThemePreset::DeviceDark => [0.25, 0.30, 0.39, 1.0],
            };
            let input_focus_surface = match chrome.theme_preset {
                ThemePreset::Daylight => [0.95, 0.98, 1.0, 1.0],
                ThemePreset::DeviceDark => [0.29, 0.35, 0.45, 1.0],
            };
            let badge_text = match chrome.theme_preset {
                ThemePreset::Daylight => [0.96, 0.99, 1.0, 1.0],
                ThemePreset::DeviceDark => [0.97, 0.99, 1.0, 1.0],
            };
            let selected_meta_text = match chrome.theme_preset {
                ThemePreset::Daylight => [0.14, 0.28, 0.40, 1.0],
                ThemePreset::DeviceDark => [0.90, 0.96, 1.0, 1.0],
            };
            let voice_success_text = match chrome.theme_preset {
                ThemePreset::Daylight => [0.20, 0.44, 0.24, 1.0],
                ThemePreset::DeviceDark => [0.84, 0.96, 0.86, 1.0],
            };
            let voice_error_text = match chrome.theme_preset {
                ThemePreset::Daylight => [0.86, 0.38, 0.38, 1.0],
                ThemePreset::DeviceDark => [1.0, 0.74, 0.74, 1.0],
            };
            let voice_hint_fill = match chrome.theme_preset {
                ThemePreset::Daylight => [0.92, 0.96, 1.0, 1.0],
                ThemePreset::DeviceDark => [0.27, 0.35, 0.48, 1.0],
            };
            let voice_hint_border = match chrome.theme_preset {
                ThemePreset::Daylight => [0.53, 0.68, 0.86, 1.0],
                ThemePreset::DeviceDark => [0.54, 0.72, 0.96, 1.0],
            };
            let voice_listening_fill = match chrome.theme_preset {
                ThemePreset::Daylight => [0.84, 0.95, 0.87, 1.0],
                ThemePreset::DeviceDark => [0.23, 0.39, 0.28, 1.0],
            };
            let voice_listening_border = match chrome.theme_preset {
                ThemePreset::Daylight => [0.32, 0.62, 0.38, 1.0],
                ThemePreset::DeviceDark => [0.47, 0.80, 0.54, 1.0],
            };
            let voice_visual_bar = match chrome.theme_preset {
                ThemePreset::Daylight => [0.36, 0.72, 0.43, 0.95],
                ThemePreset::DeviceDark => [0.58, 0.90, 0.64, 0.95],
            };
            let hover_surface = match chrome.theme_preset {
                ThemePreset::Daylight => [0.93, 0.96, 1.0, 1.0],
                ThemePreset::DeviceDark => [0.28, 0.35, 0.46, 1.0],
            };
            let press_surface = match chrome.theme_preset {
                ThemePreset::Daylight => [0.67, 0.82, 0.97, 1.0],
                ThemePreset::DeviceDark => [0.32, 0.48, 0.68, 1.0],
            };
            let hover_border = match chrome.theme_preset {
                ThemePreset::Daylight => [0.46, 0.62, 0.82, 1.0],
                ThemePreset::DeviceDark => [0.56, 0.76, 1.0, 1.0],
            };
            let badge_fill = match chrome.theme_preset {
                ThemePreset::Daylight => [0.24, 0.56, 0.86, 1.0],
                ThemePreset::DeviceDark => [0.32, 0.60, 0.98, 1.0],
            };
            let responsive_scale = self.responsive_scale();
            let input_value_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.2,
                DisplayTextScale::Medium => 3.25,
                DisplayTextScale::Large => 4.1,
            } * responsive_scale;
            let label_px = match chrome.text_scale {
                DisplayTextScale::Small => 2.1,
                DisplayTextScale::Medium => 2.35,
                DisplayTextScale::Large => 3.0,
            } * responsive_scale;
            let tracking = match chrome.text_spacing {
                TextSpacing::Tight => -0.42,
                TextSpacing::Normal => -0.34,
                TextSpacing::Relaxed => 0.06,
            } * responsive_scale;
            let base_line_gap = match chrome.text_spacing {
                TextSpacing::Tight => 4.0,
                TextSpacing::Normal => 5.0,
                TextSpacing::Relaxed => 7.0,
            } * responsive_scale;
            let ui_tracking = tracking * 0.16 - 0.02 * responsive_scale;
            let heading_tracking = tracking * 0.10 - 0.01 * responsive_scale;
            let visible_sentence_candidates: Vec<(usize, String)> = chrome
                .sentence_candidates
                .iter()
                .take(4)
                .enumerate()
                .map(|(display_index, label)| {
                    (
                        chrome
                            .sentence_candidate_source_indices
                            .get(display_index)
                            .copied()
                            .unwrap_or(display_index),
                        label.clone(),
                    )
                })
                .collect();
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
            let title_px = (label_px * 1.24).max(2.35 * responsive_scale);
            let helper_px = (label_px * 1.02).max(2.0 * responsive_scale);
            let chip_px = (label_px * 1.08).max(2.05 * responsive_scale);
            let hero_px = input_value_px * 1.10;
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
            let interaction_state = |kind: InteractionKind| {
                (
                    chrome.hovered_interaction == Some(kind),
                    chrome.pressed_interaction == Some(kind),
                )
            };
            let animated_rect = |rect: [f32; 4], hovered: bool, pressed: bool| {
                if pressed {
                    [rect[0], rect[1] + 1.5 * responsive_scale, rect[2], rect[3]]
                } else if hovered {
                    [rect[0], rect[1] - 1.5 * responsive_scale, rect[2], rect[3]]
                } else {
                    rect
                }
            };
            let animated_shadow = |shadow: [f32; 4], hovered: bool, pressed: bool| {
                let mut next = shadow;
                next[3] *= if pressed {
                    0.72
                } else if hovered {
                    1.22
                } else {
                    1.0
                };
                next
            };
            quads.push(CandidateQuad {
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
            let panel_shell_y = panel_y - 14.0 * responsive_scale;
            let panel_shell_h = metrics.panel_height + 26.0 * responsive_scale;
            append_soft_card_quads(
                &mut quads,
                [
                    panel_x - 10.0 * responsive_scale,
                    panel_shell_y,
                    panel_width + 20.0 * responsive_scale,
                    panel_shell_h,
                ],
                shell,
                shell_border,
                soft_shadow,
                page_bg,
                12.0 * responsive_scale,
            );
            quads.push(CandidateQuad {
                rect: [
                    panel_x + 8.0 * responsive_scale,
                    panel_shell_y + 8.0 * responsive_scale,
                    panel_width - 16.0 * responsive_scale,
                    2.0 * responsive_scale,
                ],
                color: [1.0, 1.0, 1.0, 0.16],
            });
            append_soft_card_quads(
                &mut quads,
                [panel_x, input_box_y, panel_width, metrics.input_box_h],
                if chrome.input_focused {
                    input_focus_surface
                } else {
                    input_surface
                },
                if chrome.input_focused {
                    accent_soft
                } else {
                    border_dark
                },
                soft_shadow,
                shell,
                10.0 * responsive_scale,
            );
            if chrome.input_focused {
                append_rounded_rect_quads(
                    &mut quads,
                    [
                        panel_x + 3.0 * responsive_scale,
                        input_box_y + 3.0 * responsive_scale,
                        panel_width - 6.0 * responsive_scale,
                        metrics.input_box_h - 6.0 * responsive_scale,
                    ],
                    [0.72, 0.87, 1.0, 0.12],
                    8.0 * responsive_scale,
                );
            }
            if chrome.input_focused {
                append_rounded_rect_quads(
                    &mut quads,
                    [
                        panel_x - 1.0 * responsive_scale,
                        input_box_y - 1.0 * responsive_scale,
                        panel_width + 2.0 * responsive_scale,
                        metrics.input_box_h + 2.0 * responsive_scale,
                    ],
                    [0.56, 0.80, 1.0, 0.12],
                    11.0 * responsive_scale,
                );
                quads.push(CandidateQuad {
                    rect: [
                        panel_x + 12.0 * responsive_scale,
                        input_box_y + metrics.input_box_h - 5.0 * responsive_scale,
                        panel_width - 24.0 * responsive_scale,
                        2.0 * responsive_scale,
                    ],
                    color: [0.38, 0.69, 0.96, 0.30],
                });
            }
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SeedInput,
                rect: [panel_x, input_box_y, panel_width, metrics.input_box_h],
            });

            let header_layouts = vec![
                TextBlock {
                    text: "Seed input".to_string(),
                    origin: [
                        panel_x + 16.0 * responsive_scale,
                        input_box_y + 11.5 * responsive_scale,
                    ],
                    max_width: panel_width - 58.0 * responsive_scale,
                    pixel_size: title_px,
                    letter_spacing: heading_tracking,
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
                        input_box_y + 30.0 * responsive_scale,
                    ],
                    max_width: panel_width - 58.0 * responsive_scale,
                    pixel_size: input_value_px,
                    letter_spacing: heading_tracking,
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
                        input_box_y + 28.0 * responsive_scale,
                        2.0 * responsive_scale,
                        20.0 * responsive_scale,
                    ],
                    color: accent,
                });
            }

            let compact_button_rect = [
                panel_x + panel_width - 34.0 * responsive_scale,
                input_box_y + 8.0 * responsive_scale,
                20.0 * responsive_scale,
                20.0 * responsive_scale,
            ];
            append_soft_card_quads(
                &mut quads,
                compact_button_rect,
                surface_alt,
                border_dark,
                soft_shadow,
                if chrome.input_focused {
                    input_focus_surface
                } else {
                    input_surface
                },
                6.0 * responsive_scale,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::ToggleCompactMode,
                rect: compact_button_rect,
            });
            let compact_icon = TextBlock {
                text: "".to_string(),
                origin: [
                    compact_button_rect[0] + 4.0 * responsive_scale,
                    compact_button_rect[1] + 5.0 * responsive_scale,
                ],
                max_width: compact_button_rect[2] - 8.0 * responsive_scale,
                pixel_size: 2.0 * responsive_scale,
                letter_spacing: ui_tracking,
                line_gap: base_line_gap,
                max_lines: 1,
                color: text_secondary,
                align: TextAlign::Center,
                role: TextRole::ToolButton,
            }
            .layout();
            text_quads.extend(compact_icon.quads.iter().copied());
            atlas_glyphs.extend(compact_icon.atlas_glyphs.iter().cloned());
            append_chevron_icon_quads(&mut quads, compact_button_rect, text_secondary, false);

            let tools_header_rect = [panel_x, tools_y, panel_width, metrics.tools_header_h];
            append_soft_card_quads(
                &mut quads,
                tools_header_rect,
                surface_alt,
                border_dark,
                soft_shadow,
                shell,
                10.0 * responsive_scale,
            );
            let toolbar_button_size = metrics.tool_button_h;
            let toolbar_y = tools_y + (metrics.tools_header_h - toolbar_button_size) * 0.5 + 0.5 * responsive_scale;
            let icon_gap = metrics.tool_gap;
            let icon_x = panel_x + 8.0 * responsive_scale;
            let toolbar_buttons = [
                (
                    InteractionKind::InputModeButton(InputMode::VirtualKeyboard),
                    chrome.active_input_mode == InputMode::VirtualKeyboard
                        && chrome.input_modes_expanded,
                ),
                (
                    InteractionKind::InputModeButton(InputMode::Dictation),
                    chrome.active_input_mode == InputMode::Dictation && chrome.input_modes_expanded,
                ),
                (
                    InteractionKind::InputModeButton(InputMode::Handwriting),
                    chrome.active_input_mode == InputMode::Handwriting
                        && chrome.input_modes_expanded,
                ),
                (InteractionKind::SettingsToggle, chrome.settings_open),
            ];
            for (index, (kind, selected)) in toolbar_buttons.iter().enumerate() {
                let rect = [
                    icon_x + index as f32 * (toolbar_button_size + icon_gap),
                    toolbar_y,
                    toolbar_button_size,
                    toolbar_button_size,
                ];
                let (hovered, pressed) = interaction_state(*kind);
                let visual_rect = animated_rect(rect, hovered, pressed);
                append_soft_card_quads(
                    &mut quads,
                    visual_rect,
                    if *selected {
                        accent_soft
                    } else if hovered {
                        surface_bright
                    } else {
                        surface
                    },
                    if *selected {
                        accent
                    } else if hovered {
                        [0.46, 0.62, 0.82, 1.0]
                    } else {
                        border_dark
                    },
                    animated_shadow(soft_shadow, hovered, pressed),
                    surface,
                    7.0 * responsive_scale,
                );
                interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                let icon_color = if *selected { accent_text } else { text_primary };
                match kind {
                    InteractionKind::InputModeButton(InputMode::VirtualKeyboard) => {
                        append_keyboard_icon_quads(&mut quads, visual_rect, icon_color)
                    }
                    InteractionKind::InputModeButton(InputMode::Dictation) => {
                        append_mic_icon_quads(&mut quads, visual_rect, icon_color)
                    }
                    InteractionKind::InputModeButton(InputMode::Handwriting) => {
                        append_pen_icon_quads(&mut quads, visual_rect, icon_color)
                    }
                    InteractionKind::SettingsToggle => append_gear_icon_quads(
                        &mut quads,
                        visual_rect,
                        icon_color,
                        if *selected { accent_soft } else { surface },
                    ),
                    _ => {}
                }
            }
            if chrome.input_modes_expanded {
                let toggle_rect = [
                    panel_x + panel_width - toolbar_button_size - 8.0 * responsive_scale,
                    toolbar_y,
                    toolbar_button_size,
                    toolbar_button_size,
                ];
                let (toggle_hovered, toggle_pressed) =
                    interaction_state(InteractionKind::InputModesToggle);
                let toggle_visual_rect = animated_rect(toggle_rect, toggle_hovered, toggle_pressed);
                append_soft_card_quads(
                    &mut quads,
                    toggle_visual_rect,
                    accent_soft,
                    accent,
                    animated_shadow(soft_shadow, toggle_hovered, toggle_pressed),
                    surface,
                    7.0 * responsive_scale,
                );
                interactive_targets.push(InteractiveTarget {
                    kind: InteractionKind::InputModesToggle,
                    rect: toggle_rect,
                });
                append_chevron_icon_quads(
                    &mut quads,
                    toggle_visual_rect,
                    accent_text,
                    true,
                );
            }

            let tools_panel_rect = [
                panel_x,
                tools_y + metrics.tools_header_h + 4.0 * responsive_scale,
                panel_width,
                metrics.tools_content_h,
            ];
            let show_expanded_tool_panel = chrome.input_modes_expanded;
            if show_expanded_tool_panel {
                append_soft_card_quads(
                    &mut quads,
                    tools_panel_rect,
                    surface,
                    border_dark,
                    soft_shadow,
                    shell,
                    10.0 * responsive_scale,
                );
                quads.push(CandidateQuad {
                    rect: [
                        tools_panel_rect[0] + 12.0 * responsive_scale,
                        tools_panel_rect[1] + 8.0 * responsive_scale,
                        tools_panel_rect[2] - 24.0 * responsive_scale,
                        2.0 * responsive_scale,
                    ],
                    color: [1.0, 1.0, 1.0, 0.14],
                });
                let drawer_rect = [
                    tools_panel_rect[0] + 10.0 * responsive_scale,
                    tools_panel_rect[1] + 6.0 * responsive_scale,
                    tools_panel_rect[2] - 20.0 * responsive_scale,
                    tools_panel_rect[3] - 10.0 * responsive_scale,
                ];
                append_soft_card_quads(
                    &mut quads,
                    drawer_rect,
                    surface_alt,
                    border_dark,
                    soft_shadow,
                    surface,
                    9.0 * responsive_scale,
                );
                let handle_w = 40.0 * responsive_scale;
                quads.push(CandidateQuad {
                    rect: [
                        drawer_rect[0] + (drawer_rect[2] - handle_w) * 0.5,
                        drawer_rect[1] + 7.0 * responsive_scale,
                        handle_w,
                        3.0 * responsive_scale,
                    ],
                    color: [1.0, 1.0, 1.0, 0.22],
                });
                if chrome.active_input_mode == InputMode::VirtualKeyboard {
                    let keyboard_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let key_gap = 5.0 * responsive_scale;
                    let row_h = 24.0 * responsive_scale;
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
                        let row_width = drawer_rect[2] - inset * 2.0;
                        let key_w = (row_width - key_gap * (key_count - 1.0)) / key_count;

                        for (key_index, key) in keys.iter().enumerate() {
                            let x =
                                drawer_rect[0] + inset + key_index as f32 * (key_w + key_gap);
                            let rect = [x, row_y, key_w, row_h];
                            append_soft_card_quads(
                                &mut quads,
                                rect,
                                match key {
                                    VirtualKeyboardKey::Shift if chrome.keyboard_shifted => {
                                        accent_soft
                                    }
                                    VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                                    | VirtualKeyboardKey::Shift => keyboard_special_surface,
                                    _ => keyboard_surface,
                                },
                                if matches!(key, VirtualKeyboardKey::Shift)
                                    && chrome.keyboard_shifted
                                {
                                    accent
                                } else {
                                    border_dark
                                },
                                soft_shadow,
                                surface,
                                7.0 * responsive_scale,
                            );
                            interactive_targets.push(InteractiveTarget {
                                kind: InteractionKind::VirtualKeyboardKey(*key),
                                rect,
                            });
                            let label = match key {
                                VirtualKeyboardKey::Character(ch) => ch.to_string(),
                                VirtualKeyboardKey::Text(text) => (*text).to_string(),
                                VirtualKeyboardKey::Space => "Space".to_string(),
                                VirtualKeyboardKey::Backspace => "".to_string(),
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
                                    (chip_px * 0.9).max(1.75 * responsive_scale)
                                } else {
                                    (chip_px * 1.1).max(2.2 * responsive_scale)
                                },
                                letter_spacing: ui_tracking,
                                line_gap: base_line_gap,
                                max_lines: 1,
                                color: if matches!(
                                    key,
                                    VirtualKeyboardKey::Backspace
                                        | VirtualKeyboardKey::ToggleNumeric
                                        | VirtualKeyboardKey::ToggleAlphabetic
                                        | VirtualKeyboardKey::Shift
                                ) {
                                    keyboard_secondary_text
                                } else {
                                    keyboard_text
                                },
                                align: TextAlign::Center,
                                role: TextRole::KeyboardKey,
                            }
                            .layout();
                            text_quads.extend(layout.quads.iter().copied());
                            atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                            keyboard_layouts.push(layout);
                            if matches!(key, VirtualKeyboardKey::Backspace) {
                                append_backspace_icon_quads(
                                    &mut quads,
                                    rect,
                                    keyboard_secondary_text,
                                );
                            }
                        }
                    }

                    let action_y = keyboard_y + 3.0 * (row_h + key_gap);
                    let left_w = 70.0 * responsive_scale;
                    let mid_key_w = 36.0 * responsive_scale;
                    let right_w = 104.0 * responsive_scale;
                    let space_w =
                        drawer_rect[2] - left_w - right_w - key_gap * 3.0 - mid_key_w * 2.0;
                    let action_keys = if chrome.keyboard_numeric {
                        vec![
                            (
                                VirtualKeyboardKey::ToggleAlphabetic,
                                "ABC",
                                [drawer_rect[0], action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    drawer_rect[0] + left_w + key_gap,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    drawer_rect[0] + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 3.0
                                        + mid_key_w
                                        + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 4.0
                                        + mid_key_w * 2.0
                                        + space_w,
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
                                [drawer_rect[0], action_y, left_w, row_h],
                            ),
                            (
                                VirtualKeyboardKey::Character(','),
                                ",",
                                [
                                    drawer_rect[0] + left_w + key_gap,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Space,
                                "Space",
                                [
                                    drawer_rect[0] + left_w + key_gap * 2.0 + mid_key_w,
                                    action_y,
                                    space_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Character('.'),
                                ".",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 3.0
                                        + mid_key_w
                                        + space_w,
                                    action_y,
                                    mid_key_w,
                                    row_h,
                                ],
                            ),
                            (
                                VirtualKeyboardKey::Backspace,
                                "",
                                [
                                    drawer_rect[0]
                                        + left_w
                                        + key_gap * 4.0
                                        + mid_key_w * 2.0
                                        + space_w,
                                    action_y,
                                    right_w,
                                    row_h,
                                ],
                            ),
                        ]
                    };

                    for (key, label, rect) in action_keys {
                        append_soft_card_quads(
                            &mut quads,
                            rect,
                            match key {
                                VirtualKeyboardKey::Space => keyboard_surface,
                                VirtualKeyboardKey::ToggleNumeric
                                | VirtualKeyboardKey::ToggleAlphabetic
                                | VirtualKeyboardKey::Backspace => keyboard_special_surface,
                                _ => keyboard_surface,
                            },
                            border_dark,
                            soft_shadow,
                            surface,
                            8.0 * responsive_scale,
                        );
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
                                1.8 * responsive_scale
                            } else {
                                2.2 * responsive_scale
                            },
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: if label.chars().count() > 6 { 2 } else { 1 },
                            color: if matches!(
                                key,
                                VirtualKeyboardKey::ToggleNumeric
                                    | VirtualKeyboardKey::ToggleAlphabetic
                                    | VirtualKeyboardKey::Backspace
                            ) {
                                keyboard_secondary_text
                            } else {
                                keyboard_text
                            },
                            align: TextAlign::Center,
                            role: TextRole::KeyboardKey,
                        }
                        .layout();
                        text_quads.extend(layout.quads.iter().copied());
                        atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                        keyboard_layouts.push(layout);
                        if matches!(key, VirtualKeyboardKey::Backspace) {
                            append_backspace_icon_quads(&mut quads, rect, keyboard_secondary_text);
                        }
                    }

                    text_sections.push(TextSection {
                        role: TextRole::KeyboardKey,
                        layouts: keyboard_layouts,
                    });
                } else if chrome.active_input_mode == InputMode::Dictation {
                    let voice_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let voice_rect = [
                        drawer_rect[0],
                        voice_y,
                        drawer_rect[2],
                        drawer_rect[3] - 24.0 * responsive_scale,
                    ];
                    let voice_content_x = voice_rect[0] + 12.0 * responsive_scale;
                    let voice_header_y = voice_y + 8.0 * responsive_scale;
                    let voice_transcript_y = voice_y + 28.0 * responsive_scale;
                    append_soft_card_quads(
                        &mut quads,
                        voice_rect,
                        [0.97, 0.98, 1.0, 1.0],
                        border_dark,
                        soft_shadow,
                        surface,
                        10.0 * responsive_scale,
                    );

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
                    let live_hint = if chrome.voice_state == VoiceCaptureState::Listening {
                        if chrome.voice_transcript.is_empty() {
                            "Listening now..."
                        } else {
                            "Heard just now"
                        }
                    } else if !chrome.voice_transcript.is_empty() {
                        "Ready to insert"
                    } else {
                        ""
                    };
                    if !live_hint.is_empty() {
                        let hint_rect = [
                            drawer_rect[0] + drawer_rect[2] - 122.0,
                            voice_y + 6.0,
                            110.0,
                            16.0,
                        ];
                        append_soft_card_quads(
                            &mut quads,
                            hint_rect,
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_listening_fill
                            } else {
                                voice_hint_fill
                            },
                            if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_listening_border
                            } else {
                                voice_hint_border
                            },
                            [0.30, 0.40, 0.55, 0.10],
                            [0.97, 0.98, 1.0, 1.0],
                            7.0 * responsive_scale,
                        );
                    }
                    if chrome.voice_state == VoiceCaptureState::Listening {
                        let base_x = drawer_rect[0] + drawer_rect[2] - 122.0;
                        let base_y = voice_y + 24.0;
                        let phase = chrome.voice_visual_phase as f32;
                        for index in 0..4 {
                            let pulse = ((phase + index as f32 * 3.0) % 12.0) / 12.0;
                            let mirrored = if pulse > 0.5 { 1.0 - pulse } else { pulse };
                            let bar_h = 8.0 + mirrored * 18.0;
                            quads.push(CandidateQuad {
                                rect: [
                                    base_x + index as f32 * 12.0,
                                    base_y + (22.0 - bar_h),
                                    7.0,
                                    bar_h,
                                ],
                                color: voice_visual_bar,
                            });
                        }
                    }
                    let voice_layouts = vec![
                        TextBlock {
                            text: status_text,
                            origin: [voice_content_x, voice_header_y],
                            max_width: drawer_rect[2]
                                - if live_hint.is_empty() { 28.0 } else { 168.0 },
                            pixel_size: title_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                accent
                            } else if matches!(
                                chrome.voice_permission,
                                VoicePermissionState::Denied | VoicePermissionState::Error
                            ) {
                                voice_error_text
                            } else {
                                text_secondary
                            },
                            align: TextAlign::Left,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: live_hint.to_string(),
                            origin: [drawer_rect[0] + drawer_rect[2] - 118.0, voice_header_y],
                            max_width: 98.0,
                            pixel_size: helper_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: if chrome.voice_state == VoiceCaptureState::Listening {
                                voice_success_text
                            } else {
                                text_primary
                            },
                            align: TextAlign::Center,
                            role: TextRole::VoiceLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: transcript,
                            origin: [voice_content_x, voice_transcript_y],
                            max_width: drawer_rect[2] - 24.0,
                            pixel_size: if chrome.voice_transcript.is_empty() {
                                input_value_px * 0.72
                            } else {
                                input_value_px * 0.9
                            },
                            letter_spacing: if chrome.voice_transcript.is_empty() {
                                ui_tracking
                            } else {
                                heading_tracking
                            },
                            line_gap: base_line_gap,
                            max_lines: 2,
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
                                "Mic"
                            },
                            78.0,
                            chrome.voice_state == VoiceCaptureState::Listening,
                            chrome.voice_permission != VoicePermissionState::Denied
                                && chrome.voice_permission != VoicePermissionState::Error,
                        ),
                        (
                            InteractionKind::InsertVoiceTranscript,
                            "Seed",
                            88.0,
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
                            "Next",
                            84.0,
                            false,
                            true,
                        ));
                    }
                    if matches!(
                        chrome.voice_permission,
                        VoicePermissionState::Pending | VoicePermissionState::Denied
                    ) {
                        voice_actions.push((
                            InteractionKind::OpenVoiceSettings,
                            "Prefs",
                            86.0,
                            false,
                            true,
                        ));
                        voice_actions.push((
                            InteractionKind::RefreshVoicePermissions,
                            "Sync",
                            72.0,
                            false,
                            true,
                        ));
                    }
                    if !chrome.voice_transcript.is_empty() {
                        voice_actions.push((
                            InteractionKind::ClearVoiceTranscript,
                            "Clear",
                            72.0,
                            false,
                            true,
                        ));
                    }
                    let voice_footer_rows = if panel_width < 620.0 { 2 } else { 1 };
                    let voice_footer_rect = [
                        drawer_rect[0] + 10.0 * responsive_scale,
                        voice_rect[1] + voice_rect[3]
                            - (voice_footer_rows as f32 * 24.0 + 10.0) * responsive_scale,
                        drawer_rect[2] - 20.0 * responsive_scale,
                        (voice_footer_rows as f32 * 24.0 + 4.0) * responsive_scale,
                    ];
                    append_soft_card_quads(
                        &mut quads,
                        voice_footer_rect,
                        surface,
                        border_dark,
                        [0.25, 0.34, 0.48, 0.06],
                        shell,
                        8.0 * responsive_scale,
                    );
                    let mut voice_action_layouts = Vec::new();
                    let mut action_cursor_x = voice_footer_rect[0] + 6.0 * responsive_scale;
                    let mut action_row = 0;
                    for (kind, label, width, emphasized, enabled) in voice_actions {
                        let (hovered, pressed) = interaction_state(kind);
                        if action_cursor_x > voice_footer_rect[0] + 6.0 * responsive_scale
                            && action_cursor_x + width
                                > voice_footer_rect[0] + voice_footer_rect[2]
                        {
                            action_row += 1;
                            action_cursor_x = voice_footer_rect[0] + 6.0 * responsive_scale;
                        }
                        let rect = [
                            action_cursor_x,
                            voice_footer_rect[1]
                                + 4.0 * responsive_scale
                                + action_row as f32 * 24.0 * responsive_scale,
                            width.min(voice_footer_rect[2] - 12.0 * responsive_scale),
                            20.0,
                        ];
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        action_cursor_x += width + 8.0 * responsive_scale;
                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                [0.67, 0.82, 0.97, 1.0]
                            } else if emphasized {
                                accent_soft
                            } else if hovered {
                                [0.93, 0.96, 1.0, 1.0]
                            } else if !enabled {
                                [0.97, 0.98, 1.0, 1.0]
                            } else {
                                [0.92, 0.95, 0.99, 1.0]
                            },
                            if pressed {
                                [0.08, 0.35, 0.68, 1.0]
                            } else if emphasized {
                                accent
                            } else if hovered {
                                [0.46, 0.62, 0.82, 1.0]
                            } else {
                                border_dark
                            },
                            animated_shadow(soft_shadow, hovered, pressed),
                            surface,
                            8.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind, rect });
                        let icon_color = if emphasized {
                            accent_text
                        } else if !enabled {
                            text_muted
                        } else {
                            text_primary
                        };
                        match kind {
                            InteractionKind::ToggleVoiceCapture => {
                                append_mic_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::InsertVoiceTranscript => {
                                append_seed_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::CycleVoiceSample => {
                                append_next_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::OpenVoiceSettings => append_gear_icon_quads(
                                &mut quads,
                                visual_rect,
                                icon_color,
                                [0.0, 0.0, 0.0, 0.0],
                            ),
                            InteractionKind::RefreshVoicePermissions => {
                                append_refresh_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            InteractionKind::ClearVoiceTranscript => {
                                append_trash_icon_quads(&mut quads, visual_rect, icon_color)
                            }
                            _ => {
                                let layout = TextBlock {
                                    text: label.to_string(),
                                    origin: [visual_rect[0] + 8.0, visual_rect[1] + 6.0],
                                    max_width: visual_rect[2] - 16.0,
                                    pixel_size: 2.0,
                                    letter_spacing: ui_tracking,
                                    line_gap: base_line_gap,
                                    max_lines: 1,
                                    color: icon_color,
                                    align: TextAlign::Center,
                                    role: TextRole::VoiceButton,
                                }
                                .layout();
                                text_quads.extend(layout.quads.iter().copied());
                                atlas_glyphs.extend(layout.atlas_glyphs.iter().cloned());
                                voice_action_layouts.push(layout);
                            }
                        }
                    }
                    text_sections.push(TextSection {
                        role: TextRole::VoiceButton,
                        layouts: voice_action_layouts,
                    });
                } else if chrome.active_input_mode == InputMode::Handwriting {
                    let handwriting_y = drawer_rect[1] + 16.0 * responsive_scale;
                    let handwriting_title_y = handwriting_y + 4.0 * responsive_scale;
                    let handwriting_hint_y = handwriting_y + 22.0 * responsive_scale;
                    let canvas_rect = [
                        drawer_rect[0],
                        handwriting_y + 44.0 * responsive_scale,
                        drawer_rect[2],
                        72.0 * responsive_scale,
                    ];
                    append_soft_card_quads(
                        &mut quads,
                        canvas_rect,
                        [0.98, 0.99, 1.0, 1.0],
                        border_dark,
                        soft_shadow,
                        surface,
                        10.0 * responsive_scale,
                    );
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
                            origin: [drawer_rect[0] + 14.0, handwriting_title_y],
                            max_width: drawer_rect[2] - 28.0,
                            pixel_size: title_px,
                            letter_spacing: heading_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
                            color: text_secondary,
                            align: TextAlign::Left,
                            role: TextRole::HandwritingLabel,
                        }
                        .layout(),
                        TextBlock {
                            text: chrome.handwriting_hint.clone(),
                            origin: [drawer_rect[0] + 14.0, handwriting_hint_y],
                            max_width: drawer_rect[2] - 28.0,
                            pixel_size: helper_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 1,
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

                    let handwriting_footer_rect = [
                        drawer_rect[0] + 10.0 * responsive_scale,
                        drawer_rect[1] + drawer_rect[3] - 34.0 * responsive_scale,
                        drawer_rect[2] - 20.0 * responsive_scale,
                        26.0 * responsive_scale,
                    ];
                    append_soft_card_quads(
                        &mut quads,
                        handwriting_footer_rect,
                        surface,
                        border_dark,
                        [0.25, 0.34, 0.48, 0.06],
                        shell,
                        8.0 * responsive_scale,
                    );
                    let undo_rect = [
                        handwriting_footer_rect[0] + 6.0 * responsive_scale,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale,
                        68.0,
                        20.0,
                    ];
                    let undo_enabled = !chrome.handwriting_strokes.is_empty();
                    let (undo_hovered, undo_pressed) =
                        interaction_state(InteractionKind::UndoHandwritingStroke);
                    let undo_visual_rect = animated_rect(undo_rect, undo_hovered, undo_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        undo_visual_rect,
                        if undo_pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if undo_hovered {
                            [0.95, 0.97, 1.0, 1.0]
                        } else if undo_enabled {
                            [0.92, 0.95, 0.99, 1.0]
                        } else {
                            [0.97, 0.98, 1.0, 1.0]
                        },
                        border_dark,
                        animated_shadow(soft_shadow, undo_hovered, undo_pressed),
                        surface,
                        8.0,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::UndoHandwritingStroke,
                        rect: undo_rect,
                    });
                    let clear_rect = [
                        handwriting_footer_rect[0] + 82.0 * responsive_scale,
                        handwriting_footer_rect[1] + 2.0 * responsive_scale,
                        68.0,
                        20.0,
                    ];
                    let (clear_hovered, clear_pressed) =
                        interaction_state(InteractionKind::ClearHandwriting);
                    let clear_visual_rect = animated_rect(clear_rect, clear_hovered, clear_pressed);
                    append_soft_card_quads(
                        &mut quads,
                        clear_visual_rect,
                        if clear_pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if clear_hovered {
                            [0.95, 0.97, 1.0, 1.0]
                        } else {
                            [0.92, 0.95, 0.99, 1.0]
                        },
                        border_dark,
                        animated_shadow(soft_shadow, clear_hovered, clear_pressed),
                        surface,
                        8.0,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::ClearHandwriting,
                        rect: clear_rect,
                    });
                    let mut handwriting_action_layouts = Vec::new();
                    append_undo_icon_quads(
                        &mut quads,
                        undo_visual_rect,
                        if undo_enabled {
                            text_primary
                        } else {
                            text_muted
                        },
                    );
                    append_trash_icon_quads(&mut quads, clear_visual_rect, text_primary);

                    let mut chip_x = handwriting_footer_rect[0] + 158.0 * responsive_scale;
                    for (index, candidate) in
                        chrome.handwriting_candidates.iter().take(3).enumerate()
                    {
                        let kind = InteractionKind::UseHandwritingCandidate(index);
                        let (hovered, pressed) = interaction_state(kind);
                        let chip_w = (candidate.chars().count() as f32 * 9.5).max(52.0) + 16.0;
                        let rect = [
                            chip_x,
                            handwriting_footer_rect[1] + 2.0 * responsive_scale,
                            chip_w,
                            20.0,
                        ];
                        let visual_rect = animated_rect(rect, hovered, pressed);
                        append_soft_card_quads(
                            &mut quads,
                            visual_rect,
                            if pressed {
                                [0.67, 0.82, 0.97, 1.0]
                            } else if index == 0 {
                                accent_soft
                            } else if hovered {
                                [0.93, 0.96, 1.0, 1.0]
                            } else {
                                [0.97, 0.98, 1.0, 1.0]
                            },
                            if pressed {
                                [0.08, 0.35, 0.68, 1.0]
                            } else if index == 0 {
                                accent
                            } else if hovered {
                                [0.46, 0.62, 0.82, 1.0]
                            } else {
                                border_dark
                            },
                            animated_shadow(soft_shadow, hovered, pressed),
                            surface,
                            8.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind, rect });
                        let layout = TextBlock {
                            text: candidate.clone(),
                            origin: [visual_rect[0] + 8.0, visual_rect[1] + 6.0],
                            max_width: visual_rect[2] - 16.0,
                            pixel_size: 2.0,
                            letter_spacing: ui_tracking,
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
                        chip_x += chip_w + 10.0;
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
                        "Theme",
                        [
                            (
                                InteractionKind::SetThemePreset(ThemePreset::Daylight),
                                "Daylight",
                                chrome.theme_preset == ThemePreset::Daylight,
                            ),
                            (
                                InteractionKind::SetThemePreset(ThemePreset::DeviceDark),
                                "Device Dark",
                                chrome.theme_preset == ThemePreset::DeviceDark,
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
                                InteractionKind::SetFontFace(FontFaceChoice::Menlo),
                                "Menlo",
                                chrome.font_face == FontFaceChoice::Menlo,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::Geneva),
                                "Geneva",
                                chrome.font_face == FontFaceChoice::Geneva,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::Helvetica),
                                "Helvetica",
                                chrome.font_face == FontFaceChoice::Helvetica,
                            ),
                            (
                                InteractionKind::SetFontFace(FontFaceChoice::PingFang),
                                "PingFang",
                                chrome.font_face == FontFaceChoice::PingFang,
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

                let mut row_y = settings_rect[1] + 10.0;
                let chip_start_x = panel_x + 86.0;
                let chip_max_x = settings_rect[0] + settings_rect[2] - 12.0;
                for (label, options) in sections.iter() {
                    let label_layout = TextBlock {
                        text: (*label).to_string(),
                        origin: [panel_x + 14.0, row_y + 2.0],
                        max_width: 72.0,
                        pixel_size: title_px,
                        letter_spacing: heading_tracking,
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

                    let mut chip_x = chip_start_x;
                    let mut chip_y = row_y;
                    let mut row_bottom = chip_y + 20.0;
                    for (kind, chip_label, selected) in options {
                        let chip_w = (chip_label.chars().count() as f32 * 10.0).max(34.0) + 12.0;
                        if chip_x + chip_w > chip_max_x {
                            chip_x = chip_start_x;
                            chip_y += 24.0;
                        }
                        let rect = [chip_x, chip_y, chip_w, 20.0];
                        append_soft_card_quads(
                            &mut quads,
                            rect,
                            if *selected { accent_soft } else { surface },
                            if *selected { accent } else { border_dark },
                            soft_shadow,
                            surface,
                            7.0,
                        );
                        interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                        let option_layout = TextBlock {
                            text: (*chip_label).to_string(),
                            origin: [chip_x + 6.0, chip_y + 5.0],
                            max_width: chip_w - 12.0,
                            pixel_size: chip_px,
                            letter_spacing: ui_tracking,
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
                        row_bottom = chip_y + 20.0;
                        chip_x += chip_w + 8.0;
                    }
                    row_y = row_bottom + 4.0;
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
                    chip_section_y - 3.0 * responsive_scale,
                    panel_width,
                    metrics.chip_section_h,
                ];
                append_soft_card_quads(
                    &mut quads,
                    chip_section_rect,
                    surface,
                    border_dark,
                    soft_shadow,
                    shell,
                    10.0 * responsive_scale,
                );
                let next_label_layouts = vec![
                    TextBlock {
                        text: if chrome.composed_tokens.is_empty() {
                            "Next tokens".to_string()
                        } else {
                            format!("Next tokens  |  {}", chrome.composed_tokens.join(" "))
                        },
                        origin: [
                            panel_x + 6.0 * responsive_scale,
                            chip_section_y + 5.0 * responsive_scale,
                        ],
                        max_width: panel_width - 96.0 * responsive_scale,
                        pixel_size: title_px,
                        letter_spacing: heading_tracking,
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
                            panel_x + 2.0 * responsive_scale,
                            chip_section_y + 24.0 * responsive_scale,
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
                    let (hovered, pressed) = interaction_state(InteractionKind::RewindNextToken);
                    let visual_back_rect = animated_rect(back_rect, hovered, pressed);
                    append_soft_card_quads(
                        &mut quads,
                        visual_back_rect,
                        if pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if hovered {
                            surface_bright
                        } else {
                            surface_muted
                        },
                        if pressed {
                            [0.08, 0.35, 0.68, 1.0]
                        } else if hovered {
                            [0.46, 0.62, 0.82, 1.0]
                        } else {
                            border_dark
                        },
                        animated_shadow(soft_shadow, hovered, pressed),
                        surface,
                        7.0 * responsive_scale,
                    );
                    interactive_targets.push(InteractiveTarget {
                        kind: InteractionKind::RewindNextToken,
                        rect: back_rect,
                    });
                    let back_layout = TextBlock {
                        text: "Back".to_string(),
                        origin: [
                            visual_back_rect[0] + 8.0 * responsive_scale,
                            visual_back_rect[1] + 6.0 * responsive_scale,
                        ],
                        max_width: visual_back_rect[2] - 16.0 * responsive_scale,
                        pixel_size: 2.0 * responsive_scale,
                        letter_spacing: ui_tracking,
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
                    chip_section_y + 48.0 * responsive_scale
                } else {
                    chip_section_y + 30.0 * responsive_scale
                };
                let mut chip_x = panel_x + 2.0 * responsive_scale;
                let mut row = 0;
                let mut chip_layouts = Vec::new();
                for (index, token) in chrome.next_token_candidates.iter().take(6).enumerate() {
                    let kind = InteractionKind::SelectNextToken(index);
                    let (hovered, pressed) = interaction_state(kind);
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
                    let visual_rect = animated_rect(rect, hovered, pressed);
                    append_soft_card_quads(
                        &mut quads,
                        visual_rect,
                        if pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if index == 0 {
                            accent_soft
                        } else if hovered {
                            surface_bright
                        } else {
                            surface
                        },
                        if pressed {
                            [0.08, 0.35, 0.68, 1.0]
                        } else if index == 0 {
                            accent
                        } else if hovered {
                            [0.46, 0.62, 0.82, 1.0]
                        } else {
                            border_dark
                        },
                        animated_shadow(soft_shadow, hovered, pressed),
                        surface,
                        8.0 * responsive_scale,
                    );
                    interactive_targets.push(InteractiveTarget { kind, rect });
                    let layout = TextBlock {
                        text: token.clone(),
                        origin: [
                            visual_rect[0] + 8.0 * responsive_scale,
                            visual_rect[1] + 6.0 * responsive_scale,
                        ],
                        max_width: visual_rect[2] - 16.0 * responsive_scale,
                        pixel_size: chip_px,
                        letter_spacing: ui_tracking,
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
                    chip_x += chip_w + 10.0 * responsive_scale;
                }
                if !chip_layouts.is_empty() {
                    text_sections.push(TextSection {
                        role: TextRole::NextTokenChip,
                        layouts: chip_layouts,
                    });
                }
            }

            let sentence_y = suggestions_y + metrics.chip_section_h + metrics.sentence_section_gap;
            let collapsed_daily_mode = !chrome.input_modes_expanded;
            let candidate_columns = metrics.candidate_columns;
            let candidate_gap_x = if collapsed_daily_mode {
                4.0 * responsive_scale
            } else {
                14.0 * responsive_scale
            };
            let collapsed_primary_w = if collapsed_daily_mode {
                (panel_width * 0.34).clamp(140.0 * responsive_scale, 220.0 * responsive_scale)
            } else if candidate_columns == 2 {
                0.0
            } else {
                0.0
            };
            let candidate_card_w = if collapsed_daily_mode {
                let trailing_count = visible_sentence_candidates.len().saturating_sub(1);
                if trailing_count == 0 {
                    panel_width
                } else {
                    (panel_width
                        - collapsed_primary_w
                        - candidate_gap_x * trailing_count as f32)
                        / trailing_count as f32
                }
            } else if candidate_columns == 2 {
                (panel_width - candidate_gap_x) / 2.0
            } else {
                panel_width
            };
            if !visible_sentence_candidates.is_empty() {
                let alternate_count = visible_sentence_candidates.len().saturating_sub(1);
                let alternate_rows = if collapsed_daily_mode {
                    0
                } else if alternate_count == 0 {
                    0
                } else {
                    alternate_count.div_ceil(candidate_columns)
                };
                let sentence_section_h = if collapsed_daily_mode {
                    52.0 * responsive_scale
                } else if alternate_rows == 0 {
                    metrics.hero_item_height
                } else {
                    metrics.hero_item_height
                        + metrics.item_gap
                        + alternate_rows as f32 * metrics.item_height
                        + (alternate_rows as f32 - 1.0).max(0.0) * metrics.item_gap
                };
                append_soft_card_quads(
                    &mut quads,
                    [panel_x, sentence_y, panel_width, sentence_section_h],
                    if collapsed_daily_mode {
                        surface_muted
                    } else {
                        surface
                    },
                    border_dark,
                    soft_shadow,
                    shell,
                    if collapsed_daily_mode {
                        10.0 * responsive_scale
                    } else {
                        12.0 * responsive_scale
                    },
                );
            }
            for (display_index, (source_index, label)) in
                visible_sentence_candidates.iter().enumerate()
            {
                let is_hero = !collapsed_daily_mode && display_index == 0;
                let style_label =
                    crate::panel_support::sentence_candidate_style_label(label, is_hero);
                let (x, y, card_width, card_height) = if is_hero {
                    (panel_x, sentence_y, panel_width, metrics.hero_item_height)
                } else if collapsed_daily_mode {
                    let trailing_index = display_index.saturating_sub(1);
                    (
                        if display_index == 0 {
                            panel_x
                        } else {
                            panel_x
                                + collapsed_primary_w
                                + candidate_gap_x
                                + trailing_index as f32 * (candidate_card_w + candidate_gap_x)
                        },
                        sentence_y,
                        if display_index == 0 {
                            collapsed_primary_w
                        } else {
                            candidate_card_w
                        },
                        42.0 * responsive_scale,
                    )
                } else {
                    let alternate_index = display_index - 1;
                    let column = alternate_index % candidate_columns;
                    let row = alternate_index / candidate_columns;
                    (
                        panel_x + column as f32 * (candidate_card_w + candidate_gap_x),
                        sentence_y
                            + metrics.hero_item_height
                            + metrics.item_gap
                            + row as f32 * (metrics.item_height + metrics.item_gap),
                        candidate_card_w,
                        metrics.item_height,
                    )
                };
                let selected = *source_index == snapshot.selected_index;
                let kind = InteractionKind::Candidate(*source_index);
                let (hovered, pressed) = interaction_state(kind);
                let quad = CandidateQuad {
                    rect: [x, y, card_width, card_height],
                    color: [0.0, 0.0, 0.0, 0.0],
                };
                let visual_rect = animated_rect(quad.rect, hovered, pressed);
                let collapsed_primary = collapsed_daily_mode && display_index == 0;
                append_soft_card_quads(
                    &mut quads,
                    visual_rect,
                    if pressed {
                        press_surface
                    } else if collapsed_primary {
                        accent_soft
                    } else if selected {
                        accent_soft
                    } else if is_hero {
                        surface_bright
                    } else if hovered {
                        hover_surface
                    } else if collapsed_daily_mode {
                        surface_alt
                    } else if snapshot.degraded {
                        surface_muted
                    } else {
                        surface
                    },
                    if pressed {
                        [0.08, 0.35, 0.68, 1.0]
                    } else if collapsed_primary {
                        accent
                    } else if selected || is_hero {
                        accent
                    } else if hovered {
                        hover_border
                    } else {
                        border_dark
                    },
                    animated_shadow(soft_shadow, hovered, pressed),
                    surface,
                    if is_hero {
                        12.0 * responsive_scale
                    } else if collapsed_daily_mode {
                        10.0 * responsive_scale
                    } else {
                        10.0 * responsive_scale
                    },
                );
                if collapsed_primary {
                    append_rounded_rect_quads(
                        &mut quads,
                        [
                            visual_rect[0] + 3.0 * responsive_scale,
                            visual_rect[1] + 3.0 * responsive_scale,
                            visual_rect[2] - 6.0 * responsive_scale,
                            visual_rect[3] - 6.0 * responsive_scale,
                        ],
                        [1.0, 1.0, 1.0, 0.08],
                        8.0 * responsive_scale,
                    );
                } else if collapsed_daily_mode && display_index > 0 {
                    quads.push(CandidateQuad {
                        rect: [
                            visual_rect[0] - candidate_gap_x * 0.5,
                            visual_rect[1] + 7.0 * responsive_scale,
                            1.0,
                            visual_rect[3] - 14.0 * responsive_scale,
                        ],
                        color: [1.0, 1.0, 1.0, 0.10],
                    });
                }
                if is_hero {
                    append_rounded_rect_quads(
                        &mut quads,
                        [
                            visual_rect[0] + 4.0 * responsive_scale,
                            visual_rect[1] + 4.0 * responsive_scale,
                            visual_rect[2] - 8.0 * responsive_scale,
                            visual_rect[3] - 8.0 * responsive_scale,
                        ],
                        [1.0, 1.0, 1.0, 0.08],
                        10.0 * responsive_scale,
                    );
                }
                if is_hero {
                    let badge_rect = [
                        visual_rect[0] + visual_rect[2] - 88.0 * responsive_scale,
                        visual_rect[1] + 10.0 * responsive_scale,
                        68.0 * responsive_scale,
                        18.0 * responsive_scale,
                    ];
                    quads.push(CandidateQuad {
                        rect: badge_rect,
                        color: badge_fill,
                    });
                    let badge_layout = TextBlock {
                        text: style_label.to_string(),
                        origin: [
                            badge_rect[0] + 8.0 * responsive_scale,
                            badge_rect[1] + 4.0 * responsive_scale,
                        ],
                        max_width: badge_rect[2] - 16.0 * responsive_scale,
                        pixel_size: 1.7 * responsive_scale,
                        letter_spacing: ui_tracking * 0.7,
                        line_gap: base_line_gap,
                        max_lines: 1,
                        color: badge_text,
                        align: TextAlign::Center,
                        role: TextRole::CandidateMeta,
                    }
                    .layout();
                    text_quads.extend(badge_layout.quads.iter().copied());
                    atlas_glyphs.extend(badge_layout.atlas_glyphs.iter().cloned());
                    text_sections.push(TextSection {
                        role: TextRole::CandidateMeta,
                        layouts: vec![badge_layout],
                    });
                }
                hit_targets.push(HitTarget {
                    index: *source_index,
                    rect: quad.rect,
                });
                interactive_targets.push(InteractiveTarget {
                    kind,
                    rect: quad.rect,
                });
                let primary_layout = TextBlock {
                    text: if collapsed_daily_mode {
                        format!("{}  {}", display_index + 1, label)
                    } else {
                        label.clone()
                    },
                    origin: [
                        visual_rect[0] + if collapsed_daily_mode {
                            12.0 * responsive_scale
                        } else {
                            18.0 * responsive_scale
                        },
                        visual_rect[1] + if collapsed_daily_mode {
                            11.0 * responsive_scale
                        } else {
                            16.0 * responsive_scale
                        },
                    ],
                    max_width: visual_rect[2]
                        - if collapsed_daily_mode {
                            20.0 * responsive_scale
                        } else {
                            36.0 * responsive_scale
                        },
                    pixel_size: if is_hero {
                        hero_px
                    } else if collapsed_daily_mode {
                        chip_px * 1.02
                    } else {
                        input_value_px * 1.02
                    },
                    letter_spacing: heading_tracking,
                    line_gap: base_line_gap,
                    max_lines: if collapsed_daily_mode {
                        1
                    } else if is_hero {
                        3
                    } else if chrome.preview_style == PreviewStyle::Full {
                        3
                    } else {
                        2
                    },
                    color: if collapsed_primary || selected {
                        accent_text
                    } else if collapsed_daily_mode {
                        text_secondary
                    } else {
                        text_primary
                    },
                    align: TextAlign::Left,
                    role: TextRole::CandidatePrimary,
                }
                .layout();
                let candidate_layouts = if collapsed_daily_mode {
                    vec![primary_layout]
                } else {
                    let meta_y = (primary_layout.bounds[1]
                        + primary_layout.bounds[3]
                        + 10.0 * responsive_scale)
                        .max(visual_rect[1] + 54.0 * responsive_scale);
                    vec![
                        primary_layout,
                        TextBlock {
                            text: if selected && is_hero {
                                "Primary sentence selected".to_string()
                            } else if selected {
                                "Alternate sentence selected".to_string()
                            } else if is_hero {
                                "Best match · tap to commit".to_string()
                            } else {
                                format!("{style_label} phrasing · tap to commit")
                            },
                            origin: [visual_rect[0] + 18.0 * responsive_scale, meta_y],
                            max_width: visual_rect[2] - 36.0 * responsive_scale,
                            pixel_size: helper_px,
                            letter_spacing: ui_tracking,
                            line_gap: base_line_gap,
                            max_lines: 2,
                            color: if selected {
                                selected_meta_text
                            } else {
                                text_secondary
                            },
                            align: TextAlign::Left,
                            role: TextRole::CandidateMeta,
                        }
                        .layout(),
                    ]
                };
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
            let theme = PanelTheme::for_preset(chrome.theme_preset);
            let page_bg = theme.page_bg;
            let shell = theme.shell;
            let shell_border = theme.shell_border;
            let surface = theme.surface;
            let surface_alt = theme.surface_alt;
            let surface_muted = theme.surface_muted;
            let accent_soft = theme.accent_soft;
            let accent = theme.accent;
            let accent_text = theme.accent_text;
            let text_primary = theme.text_primary;
            let text_secondary = theme.text_secondary;
            let border_dark = theme.border_dark;
            let soft_shadow = theme.soft_shadow;
            let tracking = match chrome.text_spacing {
                TextSpacing::Tight => -0.45,
                TextSpacing::Normal => -0.15,
                TextSpacing::Relaxed => 0.24,
            };
            let ui_tracking = tracking * 0.08 - 0.03;
            let heading_tracking = tracking * 0.04 - 0.02;
            let base_line_gap = match chrome.candidate_density {
                CandidateDensity::Compact => 4.0,
                CandidateDensity::Cozy => 6.0,
            };
            let title_px = 3.6;
            let section_px = 2.45;
            let chip_px = 2.2;
            let panel_width = self.scene_width.clamp(520.0, 900.0) - 24.0;
            let panel_x = ((self.scene_width - panel_width) / 2.0).max(8.0);
            let panel_y = 10.0;
            let row_h = 36.0;
            let title_h = 40.0;
            let settings_row_count = 13.0;
            let panel_height = title_h + 12.0 + settings_row_count * row_h + 12.0;

            let mut quads = Vec::new();
            let mut text_quads = Vec::new();
            let mut atlas_glyphs = Vec::new();
            let mut text_sections = Vec::new();
            let hit_targets = Vec::new();
            let mut interactive_targets = Vec::new();
            let interaction_state = |kind: InteractionKind| {
                (
                    chrome.hovered_interaction == Some(kind),
                    chrome.pressed_interaction == Some(kind),
                )
            };
            let animated_rect = |rect: [f32; 4], hovered: bool, pressed: bool| {
                if pressed {
                    [rect[0], rect[1] + 1.0, rect[2], rect[3]]
                } else if hovered {
                    [rect[0], rect[1] - 1.0, rect[2], rect[3]]
                } else {
                    rect
                }
            };
            let animated_shadow = |shadow: [f32; 4], hovered: bool, pressed: bool| {
                let mut next = shadow;
                next[3] *= if pressed {
                    0.76
                } else if hovered {
                    1.18
                } else {
                    1.0
                };
                next
            };

            quads.push(CandidateQuad {
                rect: [0.0, 0.0, self.scene_width, self.scene_height],
                color: page_bg,
            });
            append_soft_card_quads(
                &mut quads,
                [panel_x, panel_y, panel_width, panel_height],
                shell,
                shell_border,
                soft_shadow,
                page_bg,
                12.0,
            );
            quads.push(CandidateQuad {
                rect: [panel_x + 10.0, panel_y + 8.0, panel_width - 20.0, 2.0],
                color: [1.0, 1.0, 1.0, 0.16],
            });

            let close_rect = [panel_x + panel_width - 36.0, panel_y + 8.0, 22.0, 22.0];
            let (close_hovered, close_pressed) = interaction_state(InteractionKind::SettingsToggle);
            let close_visual_rect = animated_rect(close_rect, close_hovered, close_pressed);
            append_soft_card_quads(
                &mut quads,
                close_visual_rect,
                if close_pressed {
                    [0.67, 0.82, 0.97, 1.0]
                } else if close_hovered {
                    [0.93, 0.96, 1.0, 1.0]
                } else {
                    surface_alt
                },
                if close_pressed {
                    [0.08, 0.35, 0.68, 1.0]
                } else if close_hovered {
                    [0.46, 0.62, 0.82, 1.0]
                } else {
                    shell_border
                },
                animated_shadow(soft_shadow, close_hovered, close_pressed),
                shell,
                8.0,
            );
            interactive_targets.push(InteractiveTarget {
                kind: InteractionKind::SettingsToggle,
                rect: close_rect,
            });
            append_gear_icon_quads(&mut quads, close_visual_rect, text_secondary, surface_alt);

            let title_layout = TextBlock {
                text: "Panel Settings".to_string(),
                origin: [panel_x + 16.0, panel_y + 12.0],
                max_width: panel_width - 60.0,
                pixel_size: title_px,
                letter_spacing: heading_tracking,
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
                    "Theme",
                    [
                        (
                            InteractionKind::SetThemePreset(ThemePreset::Daylight),
                            "Daylight",
                            chrome.theme_preset == ThemePreset::Daylight,
                        ),
                        (
                            InteractionKind::SetThemePreset(ThemePreset::DeviceDark),
                            "Device Dark",
                            chrome.theme_preset == ThemePreset::DeviceDark,
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
                            InteractionKind::SetFontFace(FontFaceChoice::Menlo),
                            "Menlo",
                            chrome.font_face == FontFaceChoice::Menlo,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Geneva),
                            "Geneva",
                            chrome.font_face == FontFaceChoice::Geneva,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::Helvetica),
                            "Helvetica",
                            chrome.font_face == FontFaceChoice::Helvetica,
                        ),
                        (
                            InteractionKind::SetFontFace(FontFaceChoice::PingFang),
                            "PingFang",
                            chrome.font_face == FontFaceChoice::PingFang,
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
                    "Tone",
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
            let mut row_y = panel_y + title_h + 10.0;
            let label_col_x = panel_x + 18.0;
            let chip_start_x = panel_x + 132.0;
            let chip_max_x = panel_x + panel_width - 20.0;
            let chip_gap_x = 12.0;
            let chip_gap_y = 10.0;
            for (label, options) in sections.iter() {
                let section_top = row_y - 4.0;
                let estimated_rows = options
                    .iter()
                    .fold((chip_start_x, 1usize), |(cursor_x, rows), (_, chip_label, _)| {
                        let chip_w = (chip_label.chars().count() as f32 * 10.8).max(58.0) + 22.0;
                        if cursor_x + chip_w > chip_max_x {
                            (chip_start_x + chip_w + chip_gap_x, rows + 1)
                        } else {
                            (cursor_x + chip_w + chip_gap_x, rows)
                        }
                    })
                    .1;
                let section_height = 18.0 + estimated_rows as f32 * row_h - (row_h - 26.0);
                append_soft_card_quads(
                    &mut quads,
                    [panel_x + 10.0, section_top, panel_width - 20.0, section_height],
                    surface,
                    [border_dark[0], border_dark[1], border_dark[2], 0.42],
                    [soft_shadow[0], soft_shadow[1], soft_shadow[2], soft_shadow[3] * 0.45],
                    surface_muted,
                    10.0,
                );
                quads.push(CandidateQuad {
                    rect: [
                        panel_x + 22.0,
                        section_top + section_height - 1.0,
                        panel_width - 44.0,
                        1.0,
                    ],
                    color: [1.0, 1.0, 1.0, 0.08],
                });
                let label_layout = TextBlock {
                    text: match *label {
                        "LLM" => "LLM".to_string(),
                        "Voice Auto" => "Voice Auto".to_string(),
                        "Device Dark" => "Device Dark".to_string(),
                        other => other.to_string(),
                    },
                    origin: [label_col_x, row_y + 7.0],
                    max_width: 100.0,
                    pixel_size: section_px,
                    letter_spacing: heading_tracking,
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

                let mut chip_x = chip_start_x;
                let mut chip_y = row_y;
                let mut row_bottom = chip_y + 26.0;
                for (kind, chip_label, selected) in options {
                    let (hovered, pressed) = interaction_state(*kind);
                    let chip_w = (chip_label.chars().count() as f32 * 10.8).max(58.0) + 22.0;
                    if chip_x + chip_w > chip_max_x {
                        chip_x = chip_start_x;
                        chip_y += row_h;
                    }
                    let rect = [chip_x, chip_y, chip_w, 26.0];
                    let visual_rect = animated_rect(rect, hovered, pressed);
                    append_soft_card_quads(
                        &mut quads,
                        visual_rect,
                        if pressed {
                            [0.67, 0.82, 0.97, 1.0]
                        } else if *selected {
                            accent_soft
                        } else if hovered {
                            [0.93, 0.96, 1.0, 1.0]
                        } else {
                            surface_alt
                        },
                        if pressed {
                            [0.08, 0.35, 0.68, 1.0]
                        } else if *selected {
                            accent
                        } else if hovered {
                            [0.46, 0.62, 0.82, 1.0]
                        } else {
                            shell_border
                        },
                        animated_shadow(soft_shadow, hovered, pressed),
                        shell,
                        9.0,
                    );
                    interactive_targets.push(InteractiveTarget { kind: *kind, rect });
                    let option_layout = TextBlock {
                        text: (*chip_label).to_string(),
                        origin: [visual_rect[0] + 10.0, visual_rect[1] + 7.0],
                        max_width: visual_rect[2] - 20.0,
                        pixel_size: chip_px,
                        letter_spacing: ui_tracking,
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
                    row_bottom = chip_y + 26.0;
                    chip_x += chip_w + chip_gap_x;
                }
                row_y = row_bottom + chip_gap_y;
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
