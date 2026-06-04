use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::languages::english::EnglishLanguagePlugin;

use super::{build_combinations, clamp01, tokenize_seed};

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
