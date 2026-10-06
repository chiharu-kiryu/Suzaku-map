//! Authored comparisons, preferences and alternatives across daily contexts.
//! Fixed expectations do not inspect production resources or configure a model.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const CHOICES: &[(&str, [(&str, &str); 2])] = &[
    (
        "which one would you",
        [
            ("choose", "which one would you choose?"),
            ("recommend", "which one would you recommend?"),
        ],
    ),
    (
        "i prefer the",
        [
            ("first", "i prefer the first one."),
            ("second", "i prefer the second one."),
        ],
    ),
    (
        "i'd rather",
        [
            ("go", "i'd rather go together."),
            ("stay", "i'd rather stay here."),
        ],
    ),
    (
        "would you rather",
        [
            ("walk", "would you rather walk or take the bus?"),
            ("wait", "would you rather wait or come back later?"),
        ],
    ),
    (
        "either option is",
        [
            ("fine", "either option is fine with me."),
            ("possible", "either option is possible if we plan ahead."),
        ],
    ),
    (
        "both options look",
        [
            ("good", "both options look good to me."),
            ("reasonable", "both options look reasonable to me."),
        ],
    ),
    (
        "this looks",
        [
            ("better", "this looks better than the other one."),
            ("easier", "this looks easier than i expected."),
        ],
    ),
    (
        "that seems a little",
        [
            ("expensive", "that seems a little expensive for me."),
            ("complicated", "that seems a little complicated for now."),
        ],
    ),
    (
        "could we try something",
        [
            ("simpler", "could we try something simpler first?"),
            ("different", "could we try something different this time?"),
        ],
    ),
    (
        "can we compare the",
        [
            ("prices", "can we compare the prices first?"),
            ("sizes", "can we compare the sizes first?"),
        ],
    ),
    (
        "what matters most is",
        [
            ("comfort", "what matters most is comfort for me."),
            ("time", "what matters most is time today."),
        ],
    ),
    (
        "i care more about",
        [
            ("quality", "i care more about quality than color."),
            ("convenience", "i care more about convenience than price."),
        ],
    ),
    (
        "if there's enough",
        [
            ("time", "if there's enough time, we can walk."),
            ("room", "if there's enough room, we can sit together."),
        ],
    ),
    (
        "if it doesn't",
        [
            ("work", "if it doesn't work, we can try again."),
            ("fit", "if it doesn't fit, we can get another one."),
        ],
    ),
    (
        "we could always",
        [
            ("try", "we could always try a different way."),
            ("ask", "we could always ask for help."),
        ],
    ),
    (
        "another option would be",
        [
            ("waiting", "another option would be waiting until tomorrow."),
            (
                "leaving",
                "another option would be leaving a little earlier.",
            ),
        ],
    ),
    (
        "instead of",
        [
            ("going", "instead of going now, we could wait."),
            (
                "waiting",
                "instead of waiting here, we could come back later.",
            ),
        ],
    ),
    (
        "let's keep our",
        [
            ("options", "let's keep our options open."),
            ("plans", "let's keep our plans simple."),
        ],
    ),
    (
        "we don't have to",
        [
            ("decide", "we don't have to decide right now."),
            ("hurry", "we don't have to hurry if you need more time."),
        ],
    ),
    (
        "i'm happy with",
        [
            ("either", "i'm happy with either option."),
            ("this", "i'm happy with this choice."),
        ],
    ),
    (
        "i'm not sure which",
        [
            ("one", "i'm not sure which one to choose."),
            ("way", "i'm not sure which way is easier."),
        ],
    ),
    (
        "it depends on the",
        [
            ("price", "it depends on the price."),
            ("weather", "it depends on the weather."),
        ],
    ),
    (
        "as long as it's",
        [
            ("quiet", "as long as it's quiet, i'm happy."),
            ("nearby", "as long as it's nearby, we can walk."),
        ],
    ),
    (
        "we can choose a",
        [
            ("different", "we can choose a different place."),
            ("simpler", "we can choose a simpler plan."),
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
fn daily_choices_keep_both_words_and_sentences_on_the_first_page() {
    assert_eq!(CHOICES.len(), 24);
    for (context, branches) in CHOICES {
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
fn every_daily_choices_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in CHOICES {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_choices_preserve_case_spaces_and_smart_apostrophes() {
    for (seed, word, sentence) in [
        (
            "Either option is fi",
            "Either option is fine",
            "Either option is fine with me.",
        ),
        (
            "  i  prefer  the  se",
            "  i  prefer  the  second",
            "  i  prefer  the  second one.",
        ),
        (
            "could we try something DI",
            "could we try something DIFFERENT",
            "could we try something DIFFERENT this time?",
        ),
        ("I'd rather st", "I'd rather stay", "I'd rather stay here."),
        ("I’d rather go", "I’d rather go", "I’d rather go together."),
        (
            "If it doesn’t fi",
            "If it doesn’t fit",
            "If it doesn’t fit, we can get another one.",
        ),
        (
            "I’m happy with ei",
            "I’m happy with either",
            "I’m happy with either option.",
        ),
        (
            "as long as it’s ne",
            "as long as it’s nearby",
            "as long as it’s nearby, we can walk.",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_choices_preserve_long_mixed_punctuation_prefixes_and_literal_commit() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}either option is fi");
            let word = format!("{prefix}{separator}either option is fine");
            let sentence = format!("{prefix}{separator}either option is fine with me.");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);
            assert_exact_commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    let seed = "either option is fi";
    assert_exact_commit(seed, seed, CandidateKind::Literal);
}

#[test]
fn daily_choices_adopted_words_keep_spaces_and_sentence_progress() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}either option is fi");
    let word = format!("{prefix}either option is fine");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // This is an engine-level host draft replacement, not physical key delivery.
    // Native IBus is tested separately; no new GTK case is claimed by this test.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}either option is fine  with"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}either option is fine  with me."),
        CandidateKind::Sentence,
    );
    assert_exact_commit(
        &format!("{adopted}  with m"),
        &format!("{prefix}either option is fine  with me."),
        CandidateKind::Sentence,
    );
}
