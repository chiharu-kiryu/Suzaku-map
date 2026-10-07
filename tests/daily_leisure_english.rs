//! Authored hobbies, weekend plans and relaxation, not a language benchmark.
//! Fixed local expectations do not inspect production JSON or contact a model.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const LEISURE: &[(&str, [(&str, &str); 2])] = &[
    (
        "what do you like to",
        [
            ("read", "what do you like to read in your free time?"),
            ("watch", "what do you like to watch on weekends?"),
        ],
    ),
    (
        "what kind of music do you",
        [
            ("like", "what kind of music do you like?"),
            ("enjoy", "what kind of music do you enjoy most?"),
        ],
    ),
    (
        "i like listening to",
        [
            ("music", "i like listening to music while i cook."),
            ("podcasts", "i like listening to podcasts on my way home."),
        ],
    ),
    (
        "i've been reading",
        [
            ("this", "i've been reading this book lately."),
            ("books", "i've been reading books about drawing."),
        ],
    ),
    (
        "have you watched",
        [
            ("this", "have you watched this movie yet?"),
            ("that", "have you watched that new show?"),
        ],
    ),
    (
        "let's watch a",
        [
            ("movie", "let's watch a movie tonight."),
            ("show", "let's watch a show after dinner."),
        ],
    ),
    (
        "we could spend the afternoon",
        [
            ("reading", "we could spend the afternoon reading at home."),
            (
                "walking",
                "we could spend the afternoon walking around the park.",
            ),
        ],
    ),
    (
        "this weekend i'm planning to",
        [
            ("relax", "this weekend i'm planning to relax at home."),
            ("visit", "this weekend i'm planning to visit a friend."),
        ],
    ),
    (
        "i'd like to spend some time",
        [
            ("outside", "i'd like to spend some time outside today."),
            (
                "reading",
                "i'd like to spend some time reading this evening.",
            ),
        ],
    ),
    (
        "we could take a",
        [
            ("walk", "we could take a walk after lunch."),
            ("break", "we could take a break and enjoy the view."),
        ],
    ),
    (
        "i enjoy taking",
        [
            ("photos", "i enjoy taking photos of small things."),
            ("walks", "i enjoy taking walks in the evening."),
        ],
    ),
    (
        "i'm learning to",
        [
            ("draw", "i'm learning to draw in my free time."),
            ("cook", "i'm learning to cook something new."),
        ],
    ),
    (
        "that sounds like a fun",
        [
            ("day", "that sounds like a fun day out."),
            ("idea", "that sounds like a fun idea for the weekend."),
        ],
    ),
    (
        "it feels good to",
        [
            ("relax", "it feels good to relax after a busy week."),
            ("slow", "it feels good to slow down for a while."),
        ],
    ),
    (
        "i'm looking forward to a",
        [
            ("quiet", "i'm looking forward to a quiet evening."),
            ("relaxing", "i'm looking forward to a relaxing weekend."),
        ],
    ),
    (
        "let's keep the weekend",
        [
            ("free", "let's keep the weekend free for now."),
            ("simple", "let's keep the weekend simple and relaxed."),
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
    assert_eq!(ime.snapshot().seed_text, seed);
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

fn candidate_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn assert_page(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    assert_local_draft(&ime, seed);
    candidate_index(&ime, word, CandidateKind::Word);
    candidate_index(&ime, sentence, CandidateKind::Sentence);
}

fn assert_exact_commit(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    assert_local_draft(&ime, seed);
    let index = candidate_index(&ime, text, kind);
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
fn daily_leisure_keeps_both_words_and_sentences_on_the_first_page() {
    assert_eq!(LEISURE.len(), 16);
    for (context, branches) in LEISURE {
        let seed = format!("{context} ");
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            assert_page(&format!("{context} {}", &next[..2]), &word, sentence);
        }
    }
}

#[test]
fn every_daily_leisure_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in LEISURE {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_leisure_preserves_case_spaces_and_smart_apostrophes() {
    for (seed, word, sentence) in [
        (
            "What do you like to re",
            "What do you like to read",
            "What do you like to read in your free time?",
        ),
        (
            "  i  like  listening  to  mu",
            "  i  like  listening  to  music",
            "  i  like  listening  to  music while i cook.",
        ),
        (
            "let's watch a MO",
            "let's watch a MOVIE",
            "let's watch a MOVIE tonight.",
        ),
        (
            "Let’s watch a sh",
            "Let’s watch a show",
            "Let’s watch a show after dinner.",
        ),
        (
            "I’ve been reading th",
            "I’ve been reading this",
            "I’ve been reading this book lately.",
        ),
        (
            "I’d like to spend some time ou",
            "I’d like to spend some time outside",
            "I’d like to spend some time outside today.",
        ),
        (
            "This weekend I’m planning to re",
            "This weekend I’m planning to relax",
            "This weekend I’m planning to relax at home.",
        ),
        (
            "i'm  learning  to  dr",
            "i'm  learning  to  draw",
            "i'm  learning  to  draw in my free time.",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_leisure_preserves_long_prefixes_and_literal_commit() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}what do you like to re");
            let word = format!("{prefix}{separator}what do you like to read");
            let sentence =
                format!("{prefix}{separator}what do you like to read in your free time?");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);
            assert_exact_commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    assert_exact_commit(
        "what do you like to re",
        "what do you like to re",
        CandidateKind::Literal,
    );
}

#[test]
fn daily_leisure_adopted_words_keep_spaces_and_sentence_progress() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}what do you like to re");
    let word = format!("{prefix}what do you like to read");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // Engine-level host draft replacement, not physical key delivery or native
    // draft undo. The native gate checks those independently of this data test.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}what do you like to read  in"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}what do you like to read  in your free time?"),
        CandidateKind::Sentence,
    );

    let continued = format!("{adopted}  in your fr");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    candidate_index(
        &ime,
        &format!("{prefix}what do you like to read  in your free"),
        CandidateKind::Word,
    );
    let completed = format!("{prefix}what do you like to read  in your free time?");
    let index = candidate_index(&ime, &completed, CandidateKind::Sentence);
    ime.select_candidate(index);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(completed.as_str()));
    assert_eq!(ime.snapshot().committed_text, completed);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert!(second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, completed);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
