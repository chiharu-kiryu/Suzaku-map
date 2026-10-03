//! Project-authored daily chat scenarios, not a general language benchmark.
//! The plain engine has no model provider, network or personal typing history.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

// Fixed expectations are intentionally independent of the production JSON.
const DAILY: &[(&str, [(&str, &str); 2])] = &[
    (
        "that's really",
        [
            ("impressive", "that's really impressive."),
            ("kind", "that's really kind of you."),
        ],
    ),
    (
        "you did",
        [
            ("great", "you did great today."),
            ("well", "you did well on this."),
        ],
    ),
    (
        "it looks",
        [
            ("great", "it looks great on you."),
            ("nice", "it looks nice and comfortable."),
        ],
    ),
    (
        "i like your",
        [
            ("idea", "i like your idea."),
            ("style", "i like your style."),
        ],
    ),
    (
        "i'm happy for",
        [
            ("you", "i'm happy for you."),
            ("them", "i'm happy for them."),
        ],
    ),
    (
        "don't worry about",
        [
            ("it", "don't worry about it."),
            ("the", "don't worry about the delay."),
        ],
    ),
    (
        "no need to",
        [("rush", "no need to rush."), ("worry", "no need to worry.")],
    ),
    (
        "you made my",
        [
            ("day", "you made my day."),
            ("morning", "you made my morning better."),
        ],
    ),
    (
        "i'm almost",
        [
            ("there", "i'm almost there."),
            ("ready", "i'm almost ready to leave."),
        ],
    ),
    (
        "i'm just",
        [
            ("leaving", "i'm just leaving now."),
            ("finishing", "i'm just finishing my work."),
        ],
    ),
    (
        "let's meet at",
        [
            ("noon", "let's meet at noon."),
            ("six", "let's meet at six."),
        ],
    ),
    (
        "i'll wait for",
        [
            ("you", "i'll wait for you outside."),
            ("the", "i'll wait for the others."),
        ],
    ),
    (
        "i'll be home",
        [
            ("soon", "i'll be home soon."),
            ("late", "i'll be home late tonight."),
        ],
    ),
    (
        "are you home",
        [
            ("yet", "are you home yet?"),
            ("today", "are you home today?"),
        ],
    ),
    (
        "what are you",
        [
            ("doing", "what are you doing tonight?"),
            ("planning", "what are you planning for tomorrow?"),
        ],
    ),
    (
        "how was your",
        [
            ("day", "how was your day?"),
            ("weekend", "how was your weekend?"),
        ],
    ),
    (
        "let's take a",
        [
            ("break", "let's take a break."),
            ("walk", "let's take a walk outside."),
        ],
    ),
    (
        "i need some",
        [
            ("rest", "i need some rest."),
            ("water", "i need some water."),
        ],
    ),
    (
        "time for a",
        [
            ("break", "time for a break."),
            ("coffee", "time for a coffee."),
        ],
    ),
    (
        "i feel much",
        [
            ("better", "i feel much better now."),
            ("calmer", "i feel much calmer now."),
        ],
    ),
    (
        "i'm getting",
        [
            ("tired", "i'm getting tired."),
            ("hungry", "i'm getting hungry."),
        ],
    ),
    (
        "have you had",
        [
            ("breakfast", "have you had breakfast?"),
            ("lunch", "have you had lunch?"),
        ],
    ),
    (
        "let's get some",
        [
            ("fresh", "let's get some fresh air."),
            ("coffee", "let's get some coffee."),
        ],
    ),
    (
        "hope you",
        [
            ("sleep", "hope you sleep well."),
            ("feel", "hope you feel better soon."),
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_page(seed: &str, word: &str, sentence: &str) {
    assert_candidates(seed, word, sentence, PAGE_SIZE);
}

fn assert_candidates(seed: &str, word: &str, sentence: &str, limit: usize) {
    let ime = engine(seed);
    let candidates = ime.candidates();
    assert_eq!(candidates[0].text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(candidates.iter().all(|candidate| {
        candidate.source == CandidateSource::Local && candidate.text.starts_with(seed)
    }));
    assert_eq!(
        candidates.len(),
        candidates
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
    assert!(candidates.len() <= 12);
    for (expected, kind) in [
        (word, CandidateKind::Word),
        (sentence, CandidateKind::Sentence),
    ] {
        assert!(
            candidates
                .iter()
                .take(limit)
                .any(|candidate| { candidate.text == expected && candidate.kind == kind }),
            "missing {kind:?} {expected:?} for {seed:?}: {candidates:?}"
        );
    }
}

fn assert_exact_commit(seed: &str, expected: &str) {
    let mut ime = engine(seed);
    let index = ime
        .candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == expected)
        .unwrap_or_else(|| panic!("missing {expected:?} for {seed:?}: {:?}", ime.candidates()));
    ime.select_candidate(index);
    assert_eq!(ime.selected_completion_text(true), Some(expected));
    assert_eq!(
        ime.commit(CommitOptions { force: true }).text.as_deref(),
        Some(expected)
    );
    assert_eq!(ime.snapshot().committed_text, expected);
    assert!(ime.candidates().is_empty());
    assert!(ime.commit(CommitOptions { force: true }).text.is_none());
    assert_eq!(ime.snapshot().committed_text, expected);
    assert!(ime.undo().unwrap().committed_text.is_empty());
}

#[test]
fn daily_chat_offers_a_first_page_pair_and_promotes_each_typed_branch_offline() {
    assert_eq!(DAILY.len(), 24);
    for (context, continuations) in DAILY {
        let seed = format!("{context} ");
        let (first_word, first_sentence) = continuations[0];
        assert_page(&seed, &format!("{context} {first_word}"), first_sentence);
        for (next, sentence) in continuations {
            let word = format!("{context} {next}");
            // Broad contexts retain older phrase ranks (e.g. "take a look").
            // Both new alternatives remain reachable; spelling a branch brings
            // its word and sentence onto the first page without evicting history.
            assert_candidates(&seed, &word, sentence, 12);
            assert_page(&format!("{context} {}", &next[..1]), &word, sentence);
        }
    }
}

#[test]
fn daily_word_and_sentence_choices_commit_the_exact_text_only_once() {
    for (context, continuations) in DAILY {
        for (next, sentence) in continuations {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"));
            assert_exact_commit(&seed, sentence);
        }
    }
}

#[test]
fn daily_chat_preserves_typed_case_spacing_and_apostrophe_variants() {
    for (seed, word, sentence) in [
        ("I'm almost ", "I'm almost there", "I'm almost there."),
        (
            "I’m almost r",
            "I’m almost ready",
            "I’m almost ready to leave.",
        ),
        (
            "that’s really i",
            "that’s really impressive",
            "that’s really impressive.",
        ),
        (
            "LET’S meet at si",
            "LET’S meet at six",
            "LET’S meet at six.",
        ),
        (
            "that's really IM",
            "that's really IMPRESSIVE",
            "that's really IMPRESSIVE.",
        ),
        (
            "  You  did  g",
            "  You  did  great",
            "  You  did  great today.",
        ),
        (
            "Don’t  worry  about  ",
            "Don’t  worry  about  it",
            "Don’t  worry  about  it.",
        ),
        (
            "I’ll be home l",
            "I’ll be home late",
            "I’ll be home late tonight.",
        ),
        (
            "how was your w",
            "how was your weekend",
            "how was your weekend?",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence);
    }
}

#[test]
fn daily_chat_keeps_a_long_literal_prefix_and_continues_after_word_adoption() {
    let prefix = "Earlier note. ".repeat(12);
    let seed = format!("{prefix}I'm almost r");
    let word = format!("{prefix}I'm almost ready");
    let sentence = format!("{prefix}I'm almost ready to leave.");
    assert_page(&seed, &word, &sentence);
    assert_exact_commit(&seed, &sentence);

    let mut ime = engine("I'm almost r");
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == "I'm almost ready")
        .unwrap();
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    // Adoption replaces the ongoing draft; it must not commit before Enter/click.
    ime.seed(format!("{adopted}  to l"));
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.snapshot().seed_text, "I'm almost ready  to l");
    assert_page(
        "I'm almost ready  to l",
        "I'm almost ready  to leave",
        "I'm almost ready  to leave.",
    );
    let index = ime
        .candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == "I'm almost ready  to leave.")
        .unwrap();
    ime.select_candidate(index);
    assert_eq!(
        ime.commit(CommitOptions { force: true }).text.as_deref(),
        Some("I'm almost ready  to leave.")
    );
}
