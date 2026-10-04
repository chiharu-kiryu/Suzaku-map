//! Authored everyday coordination examples, not a language benchmark.
//! Keep fallback behavior model-independent and preserve the user's literal draft.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

// Fixed expectations do not inspect the production resource at runtime.
const COORDINATION: &[(&str, [(&str, &str); 2])] = &[
    (
        "did you get my",
        [
            ("message", "did you get my message?"),
            ("email", "did you get my email?"),
        ],
    ),
    (
        "have you checked the",
        [
            ("time", "have you checked the time?"),
            ("address", "have you checked the address?"),
        ],
    ),
    (
        "are we still meeting",
        [
            ("today", "are we still meeting today?"),
            ("tomorrow", "are we still meeting tomorrow?"),
        ],
    ),
    (
        "does that work for",
        [
            ("you", "does that work for you?"),
            ("everyone", "does that work for everyone?"),
        ],
    ),
    (
        "can you let me know",
        [
            ("today", "can you let me know today?"),
            ("tomorrow", "can you let me know tomorrow?"),
        ],
    ),
    (
        "could you tell me",
        [
            ("when", "could you tell me when to come?"),
            ("where", "could you tell me where to wait?"),
        ],
    ),
    (
        "could you say it",
        [
            ("again", "could you say it again, please?"),
            ("slowly", "could you say it slowly, please?"),
        ],
    ),
    (
        "i didn't hear",
        [
            ("you", "i didn't hear you."),
            ("the", "i didn't hear the last part."),
        ],
    ),
    (
        "do you mean",
        [
            ("this", "do you mean this one?"),
            ("that", "do you mean that one?"),
        ],
    ),
    (
        "i meant the",
        [
            ("other", "i meant the other one."),
            ("first", "i meant the first one."),
        ],
    ),
    (
        "can we confirm the",
        [
            ("time", "can we confirm the time?"),
            ("place", "can we confirm the place?"),
        ],
    ),
    (
        "i'll check it",
        [
            ("now", "i'll check it now."),
            ("again", "i'll check it again."),
        ],
    ),
    (
        "please tell me what",
        [
            ("happened", "please tell me what happened."),
            ("changed", "please tell me what changed."),
        ],
    ),
    (
        "thanks for getting",
        [
            ("back", "thanks for getting back to me."),
            ("in", "thanks for getting in touch."),
        ],
    ),
    (
        "sorry i missed your",
        [
            ("call", "sorry i missed your call."),
            ("message", "sorry i missed your message."),
        ],
    ),
    (
        "i was away from my",
        [
            ("phone", "i was away from my phone."),
            ("desk", "i was away from my desk."),
        ],
    ),
    (
        "i'm in the middle of",
        [
            ("something", "i'm in the middle of something."),
            ("dinner", "i'm in the middle of dinner."),
        ],
    ),
    (
        "can i call you",
        [
            ("now", "can i call you now?"),
            ("later", "can i call you later?"),
        ],
    ),
    (
        "please leave me a",
        [
            ("message", "please leave me a message."),
            ("note", "please leave me a note."),
        ],
    ),
    (
        "could you hold",
        [
            ("this", "could you hold this for a moment?"),
            ("that", "could you hold that for a moment?"),
        ],
    ),
    (
        "could you give me a",
        [
            ("hand", "could you give me a hand?"),
            ("minute", "could you give me a minute?"),
        ],
    ),
    (
        "would you like me to",
        [
            ("wait", "would you like me to wait here?"),
            ("help", "would you like me to help with that?"),
        ],
    ),
    (
        "we can do it",
        [
            ("together", "we can do it together."),
            ("tomorrow", "we can do it tomorrow."),
        ],
    ),
    (
        "i'll take care of",
        [
            ("it", "i'll take care of it."),
            ("this", "i'll take care of this."),
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
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
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
    assert!(ime.candidates().len() <= 12);
}

fn candidate_index(
    ime: &XRTabletImeEngine,
    text: &str,
    kind: CandidateKind,
    limit: usize,
) -> usize {
    ime.candidates()
        .iter()
        .take(limit)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn assert_page(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    assert_local_draft(&ime, seed);
    candidate_index(&ime, word, CandidateKind::Word, PAGE_SIZE);
    candidate_index(&ime, sentence, CandidateKind::Sentence, PAGE_SIZE);
}

fn assert_exact_commit(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    assert_local_draft(&ime, seed);
    let index = candidate_index(&ime, text, kind, PAGE_SIZE);
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
fn daily_coordination_keeps_both_choices_and_typed_branches_on_the_first_page() {
    assert_eq!(COORDINATION.len(), 24);
    for (context, branches) in COORDINATION {
        let seed = format!("{context} ");
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word, PAGE_SIZE);
            candidate_index(&ime, sentence, CandidateKind::Sentence, PAGE_SIZE);
            assert_page(&format!("{context} {}", &next[..1]), &word, sentence);
        }
    }
}

#[test]
fn every_daily_coordination_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in COORDINATION {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_coordination_preserves_case_spacing_and_apostrophe_variants() {
    for (seed, word, sentence) in [
        (
            "Did you get my me",
            "Did you get my message",
            "Did you get my message?",
        ),
        (
            "could  you  give  me  a  ha",
            "could  you  give  me  a  hand",
            "could  you  give  me  a  hand?",
        ),
        (
            "I didn't hear yo",
            "I didn't hear you",
            "I didn't hear you.",
        ),
        (
            "I didn’t hear th",
            "I didn’t hear the",
            "I didn’t hear the last part.",
        ),
        (
            "I’ll check it ag",
            "I’ll check it again",
            "I’ll check it again.",
        ),
        (
            "I’m in the middle of so",
            "I’m in the middle of something",
            "I’m in the middle of something.",
        ),
        (
            "thanks for getting BA",
            "thanks for getting BACK",
            "thanks for getting BACK to me.",
        ),
        (
            "  can  we  confirm  the  ti",
            "  can  we  confirm  the  time",
            "  can  we  confirm  the  time?",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_coordination_keeps_long_drafts_and_spaces_after_word_adoption() {
    let prefix = "Earlier note. ".repeat(40);
    assert!(
        prefix.chars().count() > 256,
        "exercise the bounded long-draft route"
    );
    let seed = format!("{prefix}thanks for getting ba");
    let word = format!("{prefix}thanks for getting back");
    let sentence = format!("{prefix}thanks for getting back to me.");
    assert_page(&seed, &word, &sentence);
    assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);

    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word, PAGE_SIZE);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert!(ime.snapshot().committed_text.is_empty());
    // Selecting a word adopts it into the draft. A host Space is still literal
    // input, not a commit; the private GTK gate covers the physical key route.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    assert_eq!(ime.snapshot().seed_text, spaced);
    candidate_index(
        &ime,
        &format!("{prefix}thanks for getting back  to"),
        CandidateKind::Word,
        PAGE_SIZE,
    );
    candidate_index(
        &ime,
        &format!("{prefix}thanks for getting back  to me."),
        CandidateKind::Sentence,
        PAGE_SIZE,
    );

    let continued = format!("{adopted}  to m");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    assert_eq!(ime.snapshot().seed_text, continued);
    candidate_index(
        &ime,
        &format!("{prefix}thanks for getting back  to me"),
        CandidateKind::Word,
        PAGE_SIZE,
    );
    let completed = format!("{prefix}thanks for getting back  to me.");
    let index = candidate_index(&ime, &completed, CandidateKind::Sentence, PAGE_SIZE);
    ime.select_candidate(index);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(completed.as_str()));
    assert_eq!(ime.snapshot().committed_text, completed);
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert_eq!(ime.snapshot().committed_text, completed);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
