//! Project-authored progress, retries and everyday corrections, not a benchmark.
//! Fixed expectations exercise local fallback without reading production JSON.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const FOLLOWUP: &[(&str, [(&str, &str); 2])] = &[
    (
        "i haven't finished",
        [
            ("yet", "i haven't finished yet, but i'm getting there."),
            ("reading", "i haven't finished reading the message."),
        ],
    ),
    (
        "i'm still trying to",
        [
            ("understand", "i'm still trying to understand this part."),
            ("finish", "i'm still trying to finish before dinner."),
        ],
    ),
    (
        "i've just",
        [
            ("started", "i've just started, so i need some time."),
            ("finished", "i've just finished the first part."),
        ],
    ),
    (
        "it's taking longer than",
        [
            ("expected", "it's taking longer than expected."),
            ("usual", "it's taking longer than usual today."),
        ],
    ),
    (
        "i'm having trouble",
        [
            ("finding", "i'm having trouble finding the right place."),
            ("starting", "i'm having trouble starting this."),
        ],
    ),
    (
        "i got stuck on",
        [
            ("this", "i got stuck on this part."),
            ("the", "i got stuck on the last step."),
        ],
    ),
    (
        "let's try it",
        [
            ("again", "let's try it again after a short break."),
            ("together", "let's try it together this time."),
        ],
    ),
    (
        "let's start with the",
        [
            ("first", "let's start with the first question."),
            ("easy", "let's start with the easy part."),
        ],
    ),
    (
        "we can take a",
        [
            ("break", "we can take a break if you need one."),
            ("look", "we can take a look together."),
        ],
    ),
    (
        "i'll come back to",
        [
            ("this", "i'll come back to this later."),
            ("that", "i'll come back to that after dinner."),
        ],
    ),
    (
        "can we pick this up",
        [
            ("later", "can we pick this up later?"),
            ("tomorrow", "can we pick this up tomorrow?"),
        ],
    ),
    (
        "i'll let you know when",
        [
            ("it's", "i'll let you know when it's ready."),
            ("we're", "i'll let you know when we're done."),
        ],
    ),
    (
        "i forgot to",
        [
            ("bring", "i forgot to bring the keys."),
            ("mention", "i forgot to mention one thing."),
        ],
    ),
    (
        "sorry i didn't",
        [
            ("understand", "sorry i didn't understand the question."),
            ("notice", "sorry i didn't notice your message."),
        ],
    ),
    (
        "what i meant was",
        [
            ("this", "what i meant was this one."),
            ("that", "what i meant was that we could wait."),
        ],
    ),
    (
        "sorry i got the",
        [
            ("time", "sorry i got the time wrong."),
            ("address", "sorry i got the address wrong."),
        ],
    ),
    (
        "i read that",
        [
            ("wrong", "i read that wrong, sorry."),
            ("too", "i read that too quickly."),
        ],
    ),
    (
        "let me correct",
        [
            ("that", "let me correct that before we continue."),
            ("this", "let me correct this one detail."),
        ],
    ),
    (
        "there's one more",
        [
            ("thing", "there's one more thing i wanted to ask."),
            ("question", "there's one more question before we finish."),
        ],
    ),
    (
        "i think i missed",
        [
            ("something", "i think i missed something important."),
            ("the", "i think i missed the last part."),
        ],
    ),
    (
        "let's go over it",
        [
            ("again", "let's go over it again slowly."),
            ("together", "let's go over it together before we start."),
        ],
    ),
    (
        "please give me a little more",
        [
            ("time", "please give me a little more time."),
            ("information", "please give me a little more information."),
        ],
    ),
    (
        "we're almost",
        [
            ("done", "we're almost done, just one more step."),
            ("there", "we're almost there, let's keep going."),
        ],
    ),
    (
        "i'll finish the rest",
        [
            ("later", "i'll finish the rest later."),
            ("tomorrow", "i'll finish the rest tomorrow morning."),
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
fn daily_followup_keeps_both_words_and_sentences_on_the_first_page() {
    assert_eq!(FOLLOWUP.len(), 24);
    for (context, branches) in FOLLOWUP {
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
fn every_daily_followup_word_and_sentence_commits_exactly_once_offline() {
    for (context, branches) in FOLLOWUP {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..1]);
            assert_exact_commit(&seed, &format!("{context} {next}"), CandidateKind::Word);
            assert_exact_commit(&seed, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn daily_followup_preserves_case_spaces_and_smart_apostrophes() {
    for (seed, word, sentence) in [
        (
            "I haven't finished ye",
            "I haven't finished yet",
            "I haven't finished yet, but i'm getting there.",
        ),
        (
            "I haven’t finished re",
            "I haven’t finished reading",
            "I haven’t finished reading the message.",
        ),
        (
            "  i  forgot  to  br",
            "  i  forgot  to  bring",
            "  i  forgot  to  bring the keys.",
        ),
        (
            "let's try it TO",
            "let's try it TOGETHER",
            "let's try it TOGETHER this time.",
        ),
        (
            "Let’s go over it ag",
            "Let’s go over it again",
            "Let’s go over it again slowly.",
        ),
        (
            "We’re almost do",
            "We’re almost done",
            "We’re almost done, just one more step.",
        ),
        (
            "I’ll come back to th",
            "I’ll come back to this",
            "I’ll come back to this later.",
        ),
        (
            "sorry  i  didn't  no",
            "sorry  i  didn't  notice",
            "sorry  i  didn't  notice your message.",
        ),
    ] {
        assert_page(seed, word, sentence);
        assert_exact_commit(seed, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn daily_followup_preserves_long_prefixes_and_literal_commit() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}i forgot to br");
            let word = format!("{prefix}{separator}i forgot to bring");
            let sentence = format!("{prefix}{separator}i forgot to bring the keys.");
            assert!(seed.chars().count() > 256);
            assert_page(&seed, &word, &sentence);
            assert_exact_commit(&seed, &sentence, CandidateKind::Sentence);
            assert_exact_commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    assert_exact_commit("i forgot to br", "i forgot to br", CandidateKind::Literal);
}

#[test]
fn daily_followup_adopted_words_keep_spaces_and_sentence_progress() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}i forgot to br");
    let word = format!("{prefix}i forgot to bring");
    let mut ime = engine(&seed);
    let index = candidate_index(&ime, &word, CandidateKind::Word);
    ime.select_candidate(index);
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // Engine-level host draft replacement, not simulated physical key delivery.
    // Native IBus has its own gate; Space here must remain uncommitted input.
    let spaced = format!("{adopted}  ");
    ime.seed(&spaced);
    assert_local_draft(&ime, &spaced);
    candidate_index(
        &ime,
        &format!("{prefix}i forgot to bring  the"),
        CandidateKind::Word,
    );
    candidate_index(
        &ime,
        &format!("{prefix}i forgot to bring  the keys."),
        CandidateKind::Sentence,
    );

    let continued = format!("{adopted}  the ke");
    ime.seed(&continued);
    assert_local_draft(&ime, &continued);
    candidate_index(
        &ime,
        &format!("{prefix}i forgot to bring  the keys"),
        CandidateKind::Word,
    );
    let completed = format!("{prefix}i forgot to bring  the keys.");
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
