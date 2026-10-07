//! Authored everyday clarification, not a corpus or live-model benchmark.
//! Frozen expectations and old-prefix fingerprints are independent of new JSON.
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{WordLayer, builtin},
};

const CLARIFICATION: &[(&str, [(&str, &str); 2])] = &[
    (
        "just to make sure i",
        [
            (
                "understand",
                "just to make sure i understand, you want the first one?",
            ),
            (
                "heard",
                "just to make sure i heard you correctly, we're meeting tomorrow?",
            ),
        ],
    ),
    (
        "are you saying that",
        [
            ("it's", "are you saying that it's ready?"),
            ("we", "are you saying that we should wait?"),
        ],
    ),
    (
        "when you say that, do you mean",
        [
            ("today", "when you say that, do you mean today?"),
            ("tomorrow", "when you say that, do you mean tomorrow?"),
        ],
    ),
    (
        "does that mean we should",
        [
            ("wait", "does that mean we should wait here?"),
            ("start", "does that mean we should start now?"),
        ],
    ),
    (
        "could you put that in",
        [
            ("simpler", "could you put that in simpler words?"),
            ("different", "could you put that in different words?"),
        ],
    ),
    (
        "let me say it",
        [
            ("another", "let me say it another way."),
            ("more", "let me say it more clearly."),
        ],
    ),
    (
        "here's what i was",
        [
            ("trying", "here's what i was trying to say."),
            ("thinking", "here's what i was thinking about."),
        ],
    ),
    (
        "what i wanted to ask was",
        [
            (
                "whether",
                "what i wanted to ask was whether we could try again.",
            ),
            ("when", "what i wanted to ask was when we should start."),
        ],
    ),
    (
        "i'm talking about the",
        [
            ("first", "i'm talking about the first step."),
            ("last", "i'm talking about the last message."),
        ],
    ),
    (
        "i'm asking about the",
        [
            ("time", "i'm asking about the time, not the place."),
            ("place", "i'm asking about the place, not the time."),
        ],
    ),
    (
        "could you be a bit more",
        [
            ("specific", "could you be a bit more specific about that?"),
            (
                "clear",
                "could you be a bit more clear about what you need?",
            ),
        ],
    ),
    (
        "which part should i",
        [
            ("change", "which part should i change first?"),
            ("explain", "which part should i explain again?"),
        ],
    ),
    (
        "do you want me to explain",
        [
            ("this", "do you want me to explain this part?"),
            ("that", "do you want me to explain that again?"),
        ],
    ),
    (
        "just to be clear,",
        [
            ("we're", "just to be clear, we're meeting tomorrow."),
            ("i'm", "just to be clear, i'm asking about this one."),
        ],
    ),
    (
        "let's check that we",
        [
            ("agree", "let's check that we agree on the next step."),
            ("understand", "let's check that we understand each other."),
        ],
    ),
    (
        "is there anything i should",
        [
            ("add", "is there anything i should add to this?"),
            (
                "clarify",
                "is there anything i should clarify before we start?",
            ),
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && candidate.text.starts_with(seed)
    }));
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn candidate_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn assert_page(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_local_draft(&ime, seed);
    candidate_index(&ime, word, CandidateKind::Word);
    candidate_index(&ime, sentence, CandidateKind::Sentence);
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = candidate_index(ime, text, kind);
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert!(second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn clarification_contexts_keep_both_words_and_sentences_on_page_one() {
    assert_eq!(CLARIFICATION.len(), 16);
    for (context, branches) in CLARIFICATION {
        let draft = format!("{context} ");
        let ime = engine(&draft);
        assert_local_draft(&ime, &draft);
        assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            assert_page(&format!("{context} {}", &next[..1]), &word, sentence);
        }
    }
}

#[test]
fn every_clarification_word_and_sentence_commits_once_and_can_be_undone() {
    for (context, branches) in CLARIFICATION {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            commit_once_and_undo(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn clarification_preserves_case_spaces_and_curly_apostrophes() {
    for (seed, word, sentence) in [
        (
            "Just to make sure i un",
            "Just to make sure i understand",
            "Just to make sure i understand, you want the first one?",
        ),
        (
            "  could  you  put  that  in  si",
            "  could  you  put  that  in  simpler",
            "  could  you  put  that  in  simpler words?",
        ),
        (
            "could you be a bit more SP",
            "could you be a bit more SPECIFIC",
            "could you be a bit more SPECIFIC about that?",
        ),
        (
            "Here’s what i was tr",
            "Here’s what i was trying",
            "Here’s what i was trying to say.",
        ),
        (
            "I’m talking about the fi",
            "I’m talking about the first",
            "I’m talking about the first step.",
        ),
        (
            "Let’s check that we ag",
            "Let’s check that we agree",
            "Let’s check that we agree on the next step.",
        ),
        (
            "are you saying that it’",
            "are you saying that it’s",
            "are you saying that it’s ready?",
        ),
        (
            "when  you  say  that,  do  you  mean  to",
            "when  you  say  that,  do  you  mean  today",
            "when  you  say  that,  do  you  mean  today?",
        ),
    ] {
        assert_page(seed, word, sentence);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
}

#[test]
fn clarification_keeps_long_prefixes_literal_and_authored_completions() {
    assert!(suzaku_map::languages::english::is_known_english_word("an"));
    assert!(!suzaku_map::languages::english::is_known_english_word(
        "anoth"
    ));
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}let me say it an");
            let word = format!("{prefix}{separator}let me say it another");
            let sentence = format!("{prefix}{separator}let me say it another way.");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            commit_once_and_undo(&mut engine(&seed), &sentence, CandidateKind::Sentence);
            // A byte-exact raw choice ending in known whole word `an` is Word,
            // not a second same-text Literal. Keep both strict classifications.
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Word);
            let literal = format!("{prefix}{separator}let me say it anoth");
            assert_page(&literal, &word, &sentence);
            commit_once_and_undo(&mut engine(&literal), &literal, CandidateKind::Literal);
        }
    }
}

#[test]
fn adopted_clarification_words_keep_spaces_and_partial_followup_words() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}let me say it an");
    let word = format!("{prefix}let me say it another");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // A host draft replacement, not a simulated Space key or native draft undo.
    // Native adoption/BackSpace delivery is qualified in its own private gate.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}let me say it another  way"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}let me say it another  way."),
        CandidateKind::Sentence,
    );
    let continued = format!("{adopted}  wa");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    candidate_index(
        &ime,
        &format!("{prefix}let me say it another  way"),
        CandidateKind::Word,
    );
    let completed = format!("{prefix}let me say it another  way.");
    commit_once_and_undo(&mut ime, &completed, CandidateKind::Sentence);
}

struct UnavailableProvider(LlmProviderError);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "clarification-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        Err(self.0.clone())
    }
}

#[test]
fn clarification_local_choices_survive_typed_provider_errors() {
    for error in [
        LlmProviderError::NoLocalModel,
        LlmProviderError::Unavailable,
        LlmProviderError::Timeout,
        LlmProviderError::HttpStatus(503),
    ] {
        for (context, branches) in CLARIFICATION.iter().step_by(4) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..1]);
            let word = format!("{context} {next}");
            let mut ime = engine(&seed);
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(&seed);
            assert_local_draft(&ime, &seed);
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(ime.prediction_error(), Some(&error));
            assert_eq!(ime.candidates(), local);
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}

fn hash_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a [String])>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for (context, values) in pairs {
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash = (hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    hash
}

// Match the documented English projection order without accessing model or
// provider internals. Every position, including older duplicate anchors, counts.
fn word_priority_positions(layers: &[WordLayer]) -> BTreeMap<String, usize> {
    let projected = layers.iter().flat_map(|layer| {
        layer
            .words
            .iter()
            .map(String::as_str)
            .chain(
                layer
                    .next_words
                    .iter()
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(layer.sentences.iter().flat_map(|sentence| {
                sentence
                    .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’')))
                    .filter(|word| !word.is_empty())
            }))
    });
    let mut positions = BTreeMap::new();
    for (rank, word) in projected.enumerate() {
        positions
            .entry(word.replace('’', "'").to_ascii_lowercase())
            .or_insert(rank);
    }
    positions
}

#[test]
fn clarification_preserves_all_nineteen_existing_layer_priorities_and_wording() {
    // Captured before daily_clarification; all older independent fingerprints
    // stay in lexicon_resources and the English implementation regression suite.
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..19];
    assert_eq!(
        hash_pairs(
            previous
                .iter()
                .map(|layer| (layer.id.as_str(), layer.words.as_slice()))
        ),
        0x2bd5eb2999b49be8
    );
    assert_eq!(hash_pairs(en.next_words().take(561)), 0x9ced62df3c2eafcd);
    assert_eq!(
        hash_pairs(en.sentences().take(1086).map(|text| (text, &[][..]))),
        0x13734868a1c28842
    );
    let previous_positions = word_priority_positions(previous);
    let current_positions = word_priority_positions(en.word_layers());
    assert_eq!(previous_positions.len(), 6134);
    let mut hash = 0xcbf29ce484222325_u64;
    for (word, rank) in previous_positions {
        assert_eq!(
            current_positions[&word], rank,
            "changed priority for {word}"
        );
        for byte in word.bytes().chain([0]).chain((rank as u64).to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(hash, 0xefe0b415ade6b8b5);
    assert!(current_positions.len() < 6144);
}
