use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub mode: Mode,
    pub seed_text: String,
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
    pub lexicon: HashMap<String, Vec<String>>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_candidates: 6,
            initial_text: String::new(),
            lexicon: default_lexicon(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CompositionState {
    mode: Mode,
    seed_text: String,
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
    state: CompositionState,
}

impl XRTabletImeEngine {
    pub fn new(config: EngineConfig) -> Self {
        let committed = config.initial_text.clone();

        Self {
            config,
            state: CompositionState {
                mode: Mode::Idle,
                seed_text: String::new(),
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
        self.state.seed_text = input.as_ref().trim().to_string();
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
        let tokens = tokenize_seed(&self.state.seed_text);
        self.state.expansions = tokens
            .iter()
            .map(|token| expand_token(token, &self.config.lexicon, self.state.degraded))
            .collect();
        self.state.candidates = self.compose_candidates();

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

    fn compose_candidates(&self) -> Vec<Candidate> {
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
            .map(|parts| {
                let text = parts.join(" ");
                let exact_bonus = if text == self.state.seed_text {
                    0.20
                } else {
                    0.0
                };
                let short_bonus = if text.len() <= 12 { 0.08 } else { 0.0 };
                Candidate {
                    label: text.clone(),
                    text,
                    score: self.state.confidence + exact_bonus + short_bonus,
                }
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
}

fn tokenize_seed(seed: &str) -> Vec<String> {
    seed.split_whitespace().map(ToString::to_string).collect()
}

fn expand_token(
    token: &str,
    lexicon: &HashMap<String, Vec<String>>,
    degraded: bool,
) -> Vec<String> {
    let key = token.to_lowercase();
    let mut values = vec![token.to_string()];

    if let Some(extra) = lexicon.get(&key) {
        values.extend(extra.iter().cloned());
    }

    values.sort();
    values.dedup();

    let limit = if degraded { 2 } else { 4 };
    values.into_iter().take(limit).collect()
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

fn default_lexicon() -> HashMap<String, Vec<String>> {
    HashMap::from([
        (
            "ni".into(),
            vec!["ni".into(), "you".into(), "need".into(), "new".into()],
        ),
        (
            "hao".into(),
            vec!["hao".into(), "how".into(), "hello".into()],
        ),
        (
            "nihao".into(),
            vec!["ni hao".into(), "hello".into(), "hi there".into()],
        ),
        (
            "xr".into(),
            vec!["XR".into(), "extended reality".into(), "spatial".into()],
        ),
        (
            "tablet".into(),
            vec!["tablet".into(), "pad".into(), "slate".into()],
        ),
        ("map".into(), vec!["map".into(), "mapping".into()]),
        ("ime".into(), vec!["IME".into(), "input method".into()]),
        ("keyboard".into(), vec!["keyboard".into(), "keypad".into()]),
        ("zen".into(), vec!["zen".into(), "calm".into()]),
        ("wo".into(), vec!["wo".into(), "I".into()]),
        ("ai".into(), vec!["ai".into(), "love".into()]),
        ("women".into(), vec!["women".into(), "we".into()]),
        ("shi".into(), vec!["shi".into(), "is".into(), "are".into()]),
        ("de".into(), vec!["de".into(), "of".into()]),
        ("zhong".into(), vec!["zhong".into(), "middle".into()]),
        ("guo".into(), vec!["guo".into(), "country".into()]),
        ("zhongguo".into(), vec!["zhong guo".into(), "China".into()]),
    ])
}

#[cfg(feature = "gpu")]
pub mod gpu {
    use super::Snapshot;
    use bytemuck::{Pod, Zeroable};

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Pod, Zeroable, PartialEq)]
    pub struct CandidateQuad {
        pub rect: [f32; 4],
        pub color: [f32; 4],
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct RenderScene {
        pub quads: Vec<CandidateQuad>,
        pub labels: Vec<String>,
        pub selected_label: Option<String>,
        pub draft_text: String,
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
            let item_height = 72.0;
            let gap = 12.0;
            let start_y = 24.0;

            let quads = snapshot
                .candidate_labels
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    let y = start_y + index as f32 * (item_height + gap);
                    let selected = index == snapshot.selected_index;
                    CandidateQuad {
                        rect: [24.0, y, self.scene_width - 48.0, item_height],
                        color: if selected {
                            [0.10, 0.65, 0.95, 1.0]
                        } else if snapshot.degraded {
                            [0.25, 0.25, 0.30, 0.95]
                        } else {
                            [0.18, 0.18, 0.22, 0.92]
                        },
                    }
                })
                .collect();

            RenderScene {
                quads,
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
}
