//! Authored food, weather wording and plan-change examples, not a language benchmark.
//! Weather sentences are static phrases, not forecasts. No model is configured.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

// Fixed expectations are independent of the production resource at runtime.
const NEEDS: &[(&str, [(&str, &str); 2])] = &[
    (
        "could we get the",
        [
            ("menu", "could we get the menu, please?"),
            ("bill", "could we get the bill, please?"),
        ],
    ),
    (
        "i'll have the",
        [
            ("soup", "i'll have the soup, please."),
            ("salad", "i'll have the salad, please."),
        ],
    ),
    (
        "please make it",
        [
            ("mild", "please make it mild."),
            ("less", "please make it less spicy."),
        ],
    ),
    (
        "could you put the sauce",
        [
            ("on", "could you put the sauce on the side?"),
            ("in", "could you put the sauce in a separate bowl?"),
        ],
    ),
    (
        "can i get some",
        [
            ("water", "can i get some water, please?"),
            ("napkins", "can i get some napkins, please?"),
        ],
    ),
    (
        "i'd like my tea",
        [
            ("without", "i'd like my tea without sugar."),
            ("with", "i'd like my tea with milk."),
        ],
    ),
    (
        "this tastes",
        [
            ("delicious", "this tastes delicious."),
            ("fresh", "this tastes fresh and light."),
        ],
    ),
    (
        "could we order",
        [
            ("dessert", "could we order dessert now?"),
            ("another", "could we order another drink?"),
        ],
    ),
    (
        "it's getting",
        [
            ("cold", "it's getting cold outside."),
            ("windy", "it's getting windy outside."),
        ],
    ),
    (
        "it might",
        [
            ("rain", "it might rain later."),
            ("snow", "it might snow tonight."),
        ],
    ),
    (
        "don't forget your",
        [
            ("umbrella", "don't forget your umbrella."),
            ("jacket", "don't forget your jacket."),
        ],
    ),
    (
        "let's stay",
        [
            ("inside", "let's stay inside until the rain stops."),
            ("home", "let's stay home today."),
        ],
    ),
    (
        "the weather looks",
        [
            ("clear", "the weather looks clear this morning."),
            ("cloudy", "the weather looks cloudy today."),
        ],
    ),
    (
        "we can go outside",
        [
            ("later", "we can go outside later."),
            ("tomorrow", "we can go outside tomorrow."),
        ],
    ),
    (
        "i'll bring my",
        [
            ("coat", "i'll bring my coat."),
            ("umbrella", "i'll bring my umbrella just in case."),
        ],
    ),
    (
        "please drive",
        [
            ("carefully", "please drive carefully in the rain."),
            ("slowly", "please drive slowly on this road."),
        ],
    ),
    (
        "can we change the",
        [
            ("time", "can we change the time?"),
            ("place", "can we change the place?"),
        ],
    ),
    (
        "i need to leave",
        [
            ("early", "i need to leave early today."),
            ("now", "i need to leave now."),
        ],
    ),
    (
        "i can't make it",
        [
            ("today", "i can't make it today."),
            ("tonight", "i can't make it tonight."),
        ],
    ),
    (
        "could we talk",
        [
            ("later", "could we talk later?"),
            ("tomorrow", "could we talk tomorrow?"),
        ],
    ),
    (
        "i'll let you know if",
        [
            ("anything", "i'll let you know if anything changes."),
            ("the", "i'll let you know if the plans change."),
        ],
    ),
    (
        "we may have to",
        [
            ("cancel", "we may have to cancel our plans."),
            ("reschedule", "we may have to reschedule our meeting."),
        ],
    ),
    (
        "let's decide",
        [
            ("later", "let's decide later."),
            ("tomorrow", "let's decide tomorrow morning."),
        ],
    ),
    (
        "i'll send you the",
        [
            ("address", "i'll send you the address."),
            ("details", "i'll send you the details soon."),
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
}

#[test]
fn daily_needs_keep_broad_choices_reachable_and_typed_branches_on_the_first_page() {
    assert_eq!(NEEDS.len(), 24);
    for (context, branches) in NEEDS {
        let seed = format!("{context} ");
        let ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        candidate_index(&ime, branches[0].1, CandidateKind::Sentence, PAGE_SIZE);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            candidate_index(&ime, &word, CandidateKind::Word, PAGE_SIZE);
            // Appending this layer must not evict earlier broad-context phrases.
            candidate_index(&ime, sentence, CandidateKind::Sentence, 12);
            assert_page(&format!("{context} {}", &next[..1]), &word, sentence);
        }
    }
}

#[test]
fn every_daily_needs_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in NEEDS {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_needs_preserve_case_spacing_and_ascii_or_curly_apostrophes() {
    for (seed, word, sentence) in [
        (
            "Please make it mi",
            "Please make it mild",
            "Please make it mild.",
        ),
        (
            "please  make  it  le",
            "please  make  it  less",
            "please  make  it  less spicy.",
        ),
        (
            "It’s getting co",
            "It’s getting cold",
            "It’s getting cold outside.",
        ),
        (
            "don’t forget your u",
            "don’t forget your umbrella",
            "don’t forget your umbrella.",
        ),
        (
            "I’d like my tea wi",
            "I’d like my tea without",
            "I’d like my tea without sugar.",
        ),
        (
            "I’ll let you know if an",
            "I’ll let you know if anything",
            "I’ll let you know if anything changes.",
        ),
        (
            "could we order DE",
            "could we order DESSERT",
            "could we order DESSERT now?",
        ),
        (
            "  can  we  change  the  ti",
            "  can  we  change  the  time",
            "  can  we  change  the  time?",
        ),
        (
            "let’s stay in",
            "let’s stay inside",
            "let’s stay inside until the rain stops.",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_needs_keep_long_literal_prefixes_and_an_adopted_word_editable() {
    let prefix = "Earlier note. ".repeat(12);
    let seed = format!("{prefix}could we order de");
    let word = format!("{prefix}could we order dessert");
    let sentence = format!("{prefix}could we order dessert now?");
    assert_page(&seed, &word, &sentence);
    assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);

    let mut ime = engine("could we order de");
    let index = candidate_index(
        &ime,
        "could we order dessert",
        CandidateKind::Word,
        PAGE_SIZE,
    );
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert!(ime.snapshot().committed_text.is_empty());
    // The host adopts into the current draft, rather than committing the word.
    let continued = format!("{adopted}  n");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    assert_eq!(ime.snapshot().seed_text, "could we order dessert  n");
    candidate_index(
        &ime,
        "could we order dessert  now",
        CandidateKind::Word,
        PAGE_SIZE,
    );
    let index = candidate_index(
        &ime,
        "could we order dessert  now?",
        CandidateKind::Sentence,
        PAGE_SIZE,
    );
    ime.select_candidate(index);
    assert_eq!(
        ime.commit(CommitOptions { force: true }).text.as_deref(),
        Some("could we order dessert  now?")
    );
    assert!(ime.commit(CommitOptions { force: true }).text.is_none());
}
