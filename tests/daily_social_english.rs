//! Authored social check-ins, invitations and polite declines, not a benchmark.
//! Keep offline fallback independent of any model and preserve the literal draft.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

// Fixed expectations do not inspect the production resource at runtime.
const SOCIAL: &[(&str, [(&str, &str); 2])] = &[
    (
        "it's good to",
        [
            ("see", "it's good to see you again."),
            ("hear", "it's good to hear from you."),
        ],
    ),
    (
        "i haven't seen you",
        [
            ("lately", "i haven't seen you lately."),
            ("before", "i haven't seen you before."),
        ],
    ),
    (
        "how have you been",
        [
            ("lately", "how have you been lately?"),
            ("feeling", "how have you been feeling?"),
        ],
    ),
    (
        "did you have a",
        [
            ("good", "did you have a good weekend?"),
            ("nice", "did you have a nice trip?"),
        ],
    ),
    (
        "i hope you're",
        [
            ("doing", "i hope you're doing well."),
            ("feeling", "i hope you're feeling better."),
        ],
    ),
    (
        "are you feeling",
        [
            ("better", "are you feeling better?"),
            ("okay", "are you feeling okay?"),
        ],
    ),
    (
        "have you been sleeping",
        [
            ("well", "have you been sleeping well?"),
            ("better", "have you been sleeping better?"),
        ],
    ),
    (
        "remember to take a",
        [
            ("break", "remember to take a break."),
            ("moment", "remember to take a moment for yourself."),
        ],
    ),
    (
        "you don't have to",
        [
            ("rush", "you don't have to rush."),
            ("apologize", "you don't have to apologize."),
        ],
    ),
    (
        "i'm here if you",
        [
            ("need", "i'm here if you need me."),
            ("want", "i'm here if you want to talk."),
        ],
    ),
    (
        "i'm sorry to hear",
        [
            ("that", "i'm sorry to hear that."),
            ("about", "i'm sorry to hear about your bad day."),
        ],
    ),
    (
        "that must have been",
        [
            ("hard", "that must have been hard."),
            ("tiring", "that must have been tiring."),
        ],
    ),
    (
        "do you want to grab",
        [
            ("lunch", "do you want to grab lunch?"),
            ("coffee", "do you want to grab coffee?"),
        ],
    ),
    (
        "would you like to join",
        [
            ("us", "would you like to join us for dinner?"),
            ("me", "would you like to join me for a walk?"),
        ],
    ),
    (
        "are you doing anything",
        [
            ("tonight", "are you doing anything tonight?"),
            ("tomorrow", "are you doing anything tomorrow?"),
        ],
    ),
    (
        "let's do something",
        [
            ("fun", "let's do something fun this weekend."),
            ("quiet", "let's do something quiet tonight."),
        ],
    ),
    (
        "i know a nice",
        [
            ("cafe", "i know a nice cafe nearby."),
            ("place", "i know a nice place to eat."),
        ],
    ),
    (
        "shall we meet",
        [
            ("outside", "shall we meet outside the cafe?"),
            ("there", "shall we meet there at six?"),
        ],
    ),
    (
        "i'd love to come",
        [
            ("along", "i'd love to come along."),
            ("over", "i'd love to come over for dinner."),
        ],
    ),
    (
        "thanks for inviting",
        [
            ("me", "thanks for inviting me."),
            ("us", "thanks for inviting us."),
        ],
    ),
    (
        "i already have",
        [
            ("plans", "i already have plans tonight."),
            ("something", "i already have something planned."),
        ],
    ),
    (
        "maybe we could try",
        [
            ("another", "maybe we could try another day?"),
            ("next", "maybe we could try next week?"),
        ],
    ),
    (
        "i'll have to pass",
        [
            ("today", "i'll have to pass today, but thank you."),
            ("tonight", "i'll have to pass tonight, but thank you."),
        ],
    ),
    (
        "please enjoy the",
        [
            ("evening", "please enjoy the evening."),
            ("party", "please enjoy the party."),
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
fn explicit_social_word_forms_complete_without_a_sentence_context() {
    for (seed, word) in [
        ("tiri", "tiring"),
        ("quie", "quiet"),
        ("caf", "cafe"),
        ("shal", "shall"),
    ] {
        assert_exact_commit(seed, word, CandidateKind::Word);
    }
}

#[test]
fn daily_social_keeps_both_choices_and_typed_branches_on_the_first_page() {
    assert_eq!(SOCIAL.len(), 24);
    for (context, branches) in SOCIAL {
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
fn every_daily_social_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in SOCIAL {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_social_preserves_case_spacing_and_apostrophe_variants() {
    for (seed, word, sentence) in [
        (
            "Do you want to grab co",
            "Do you want to grab coffee",
            "Do you want to grab coffee?",
        ),
        (
            "remember  to  take  a  br",
            "remember  to  take  a  break",
            "remember  to  take  a  break.",
        ),
        (
            "I hope you're fe",
            "I hope you're feeling",
            "I hope you're feeling better.",
        ),
        (
            "I hope you’re do",
            "I hope you’re doing",
            "I hope you’re doing well.",
        ),
        (
            "I’m here if you wa",
            "I’m here if you want",
            "I’m here if you want to talk.",
        ),
        (
            "I’d love to come ov",
            "I’d love to come over",
            "I’d love to come over for dinner.",
        ),
        (
            "you don't have to AP",
            "you don't have to APOLOGIZE",
            "you don't have to APOLOGIZE.",
        ),
        (
            "  would  you  like  to  join  us",
            "  would  you  like  to  join  us for",
            "  would  you  like  to  join  us for dinner?",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_social_keeps_long_drafts_and_spaces_after_word_adoption() {
    let prefix = "Earlier note. ".repeat(40);
    assert!(
        prefix.chars().count() > 256,
        "exercise the bounded long-draft route"
    );
    let seed = format!("{prefix}i'm here if you wa");
    let word = format!("{prefix}i'm here if you want");
    let sentence = format!("{prefix}i'm here if you want to talk.");
    assert_page(&seed, &word, &sentence);
    assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);

    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word, PAGE_SIZE);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert!(ime.snapshot().committed_text.is_empty());
    // Word selection adopts a draft. Literal spaces do not commit it; the
    // isolated GTK gate separately checks real key dispatch through IBus.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    assert_eq!(ime.snapshot().seed_text, spaced);
    candidate_index(
        &ime,
        &format!("{prefix}i'm here if you want  to"),
        CandidateKind::Word,
        PAGE_SIZE,
    );
    candidate_index(
        &ime,
        &format!("{prefix}i'm here if you want  to talk."),
        CandidateKind::Sentence,
        PAGE_SIZE,
    );

    let continued = format!("{adopted}  to ta");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    assert_eq!(ime.snapshot().seed_text, continued);
    candidate_index(
        &ime,
        &format!("{prefix}i'm here if you want  to talk"),
        CandidateKind::Word,
        PAGE_SIZE,
    );
    let completed = format!("{prefix}i'm here if you want  to talk.");
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
