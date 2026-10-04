//! Authored everyday belongings and quantities, not a language benchmark.
//! Expectations are fixed independently of production JSON and require no model.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const OBJECTS: &[(&str, [(&str, &str); 2])] = &[
    (
        "have you seen my",
        [
            ("keys", "have you seen my keys?"),
            ("phone", "have you seen my phone?"),
        ],
    ),
    (
        "where did you put the",
        [
            ("scissors", "where did you put the scissors?"),
            ("tape", "where did you put the tape?"),
        ],
    ),
    (
        "is it in the",
        [
            ("drawer", "is it in the drawer?"),
            ("bag", "is it in the bag?"),
        ],
    ),
    (
        "i put it on the",
        [
            ("table", "i put it on the table."),
            ("shelf", "i put it on the shelf."),
        ],
    ),
    (
        "it's next to the",
        [
            ("door", "it's next to the door."),
            ("window", "it's next to the window."),
        ],
    ),
    (
        "check under the",
        [
            ("chair", "check under the chair."),
            ("bed", "check under the bed."),
        ],
    ),
    (
        "could you pass me the",
        [
            ("salt", "could you pass me the salt?"),
            ("pepper", "could you pass me the pepper?"),
        ],
    ),
    (
        "could you bring the",
        [
            ("cups", "could you bring the cups?"),
            ("plates", "could you bring the plates?"),
        ],
    ),
    (
        "can you move this",
        [
            ("box", "can you move this box?"),
            ("chair", "can you move this chair?"),
        ],
    ),
    (
        "please put it back on the",
        [
            ("shelf", "please put it back on the shelf."),
            ("table", "please put it back on the table."),
        ],
    ),
    (
        "can i borrow your",
        [
            ("charger", "can i borrow your charger?"),
            ("umbrella", "can i borrow your umbrella?"),
        ],
    ),
    (
        "i'll give it back",
        [
            ("tomorrow", "i'll give it back tomorrow."),
            ("tonight", "i'll give it back tonight."),
        ],
    ),
    (
        "have you returned the",
        [
            ("book", "have you returned the book?"),
            ("keys", "have you returned the keys?"),
        ],
    ),
    (
        "you can keep the",
        [
            ("bag", "you can keep the bag."),
            ("box", "you can keep the box."),
        ],
    ),
    (
        "does this belong to",
        [
            ("you", "does this belong to you?"),
            ("someone", "does this belong to someone else?"),
        ],
    ),
    (
        "i think this is",
        [
            ("yours", "i think this is yours."),
            ("mine", "i think this is mine."),
        ],
    ),
    (
        "do we have enough",
        [
            ("cups", "do we have enough cups?"),
            ("plates", "do we have enough plates?"),
        ],
    ),
    (
        "we need two more",
        [
            ("chairs", "we need two more chairs."),
            ("towels", "we need two more towels."),
        ],
    ),
    (
        "i only need one",
        [
            ("spoon", "i only need one spoon."),
            ("fork", "i only need one fork."),
        ],
    ),
    (
        "could i get a smaller",
        [
            ("bag", "could i get a smaller bag?"),
            ("box", "could i get a smaller box?"),
        ],
    ),
    (
        "this one is too",
        [
            ("big", "this one is too big."),
            ("small", "this one is too small."),
        ],
    ),
    (
        "i'd like the same",
        [
            ("size", "i'd like the same size."),
            ("color", "i'd like the same color."),
        ],
    ),
    (
        "there's a spare",
        [
            ("key", "there's a spare key in the drawer."),
            ("charger", "there's a spare charger on the table."),
        ],
    ),
    (
        "please keep them in the",
        [
            ("drawer", "please keep them in the drawer."),
            ("cupboard", "please keep them in the cupboard."),
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
fn daily_objects_keep_both_words_and_sentences_on_the_first_page() {
    assert_eq!(OBJECTS.len(), 24);
    for (context, branches) in OBJECTS {
        let seed = format!("{context} ");
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word);
            candidate_index(&ime, sentence, CandidateKind::Sentence);
            assert_page(&format!("{context} {}", &next[..1]), &word, sentence);
        }
    }
}

#[test]
fn every_daily_objects_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in OBJECTS {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_objects_preserve_case_spaces_and_smart_apostrophes() {
    for (seed, word, sentence) in [
        (
            "Have you seen my ke",
            "Have you seen my keys",
            "Have you seen my keys?",
        ),
        (
            "  can  i  borrow  your  ch",
            "  can  i  borrow  your  charger",
            "  can  i  borrow  your  charger?",
        ),
        (
            "could you pass me the SA",
            "could you pass me the SALT",
            "could you pass me the SALT?",
        ),
        (
            "It's next to the wi",
            "It's next to the window",
            "It's next to the window.",
        ),
        (
            "It’s next to the do",
            "It’s next to the door",
            "It’s next to the door.",
        ),
        (
            "I’ll give it back to",
            "I’ll give it back tomorrow",
            "I’ll give it back tomorrow.",
        ),
        (
            "I’d like the same si",
            "I’d like the same size",
            "I’d like the same size.",
        ),
        (
            "There’s a spare ch",
            "There’s a spare charger",
            "There’s a spare charger on the table.",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_objects_keep_long_mixed_punctuation_prefixes_and_literal_choice() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}have you seen my ke");
            let word = format!("{prefix}{separator}have you seen my keys");
            let sentence = format!("{prefix}{separator}have you seen my keys?");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);
            assert_exact_commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    let seed = "have you seen my ke";
    assert_exact_commit(seed, seed, CandidateKind::Literal);
}

#[test]
fn daily_objects_word_adoption_keeps_space_and_sentence_continuation() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}there's a spare ke");
    let word = format!("{prefix}there's a spare key");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // Host Space remains part of the draft; physical key delivery is covered
    // by the private GTK gate, not by this model-independent resource test.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}there's a spare key  in"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}there's a spare key  in the drawer."),
        CandidateKind::Sentence,
    );
    assert_exact_commit(
        &format!("{adopted}  in the dr"),
        &format!("{prefix}there's a spare key  in the drawer."),
        CandidateKind::Sentence,
    );
}
