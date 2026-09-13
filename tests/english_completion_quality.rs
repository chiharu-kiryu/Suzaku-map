//! Project-authored writing scenarios, not a population-level language benchmark.
//! No model, network access or personal typing history is needed.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, merge_model};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};
use suzaku_map::languages::llm::LlmCompletion;

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    engine
}

#[test]
fn everyday_writing_has_relevant_words_and_sentences_on_the_first_page() {
    let cases = [
        ("hel", "hello", "hello, how are you?"),
        ("good m", "good morning", "good morning, how are you?"),
        ("thank you ", "thank you for", "thank you for your help."),
        ("please sen", "please send", "please send me the details."),
        (
            "please send me the d",
            "please send me the details",
            "please send me the details.",
        ),
        (
            "let me know ",
            "let me know what",
            "let me know what you think.",
        ),
        ("hello, h", "hello, how", "hello, how are you?"),
        (
            "  Please  send  ",
            "  Please  send  me",
            "  Please  send  me the details.",
        ),
        (
            "He said 'please sen",
            "He said 'please send",
            "He said 'please send me the details.",
        ),
        (
            "I would like to ",
            "I would like to know",
            "I would like to know more.",
        ),
        ("how can I ", "how can I help", "how can I help you?"),
        (
            "how  can  i  ",
            "how  can  i  help",
            "how  can  i  help you?",
        ),
        (
            "could you p",
            "could you please",
            "could you please check this?",
        ),
        (
            "could you sh",
            "could you share",
            "could you share the link?",
        ),
        ("can you sh", "can you share", "can you share the file?"),
        ("what do you ", "what do you think", "what do you think?"),
        ("do you h", "do you have", "do you have time?"),
        ("I want to ", "I want to know", "I want to know more."),
        (
            "I need to c",
            "I need to check",
            "I need to check the details.",
        ),
        ("I'm ", "I'm happy", "I'm happy to help."),
        ("I'm w", "I'm working", "I'm working on it."),
        ("I’m w", "I’m working", "I’m working on it."),
        ("I'll ch", "I'll check", "I'll check the latest version."),
        ("I’ve a", "I’ve attached", "I’ve attached the file."),
        ("We're ", "We're working", "We're working on it."),
        ("we’re r", "we’re ready", "we’re ready to start."),
        ("you're w", "you're welcome", "you're welcome."),
        ("that's a g", "that's a good", "that's a good idea."),
        ("let’s t", "let’s try", "let’s try it again."),
        (
            "please review the c",
            "please review the changes",
            "please review the changes.",
        ),
        (
            "please update the d",
            "please update the documentation",
            "please update the documentation.",
        ),
        ("open the f", "open the file", "open the file."),
        ("run the t", "run the tests", "run the tests."),
        ("fix the b", "fix the bug", "fix the bug."),
        (
            "check the error m",
            "check the error message",
            "check the error message.",
        ),
        ("we need to t", "we need to test", "we need to test this."),
        (
            "we need to d",
            "we need to discuss",
            "we need to discuss the details.",
        ),
        (
            "thanks for letting me k",
            "thanks for letting me know",
            "thanks for letting me know.",
        ),
        (
            "I would like to s",
            "I would like to schedule",
            "I would like to schedule a meeting.",
        ),
        (
            "please let me know if you ",
            "please let me know if you need",
            "please let me know if you need help.",
        ),
    ];
    let (mut word_hits, mut sentence_hits) = (0, 0);
    let mut misses = Vec::new();
    for (seed, word, sentence) in cases {
        let engine = engine(seed);
        let candidates = engine.candidates();
        let page: Vec<_> = candidates.iter().take(PAGE_SIZE).collect();
        let words: Vec<_> = page
            .iter()
            .filter(|c| c.text != seed && c.kind == CandidateKind::Word)
            .take(3)
            .map(|c| c.text.as_str())
            .collect();
        let word_hit = words.contains(&word);
        let sentence_hit = page
            .iter()
            .any(|c| c.kind == CandidateKind::Sentence && c.text == sentence);
        word_hits += usize::from(word_hit);
        sentence_hits += usize::from(sentence_hit);
        assert_eq!(candidates[0].text, seed);
        assert!(
            candidates
                .iter()
                .all(|c| c.text.starts_with(seed) && c.source == CandidateSource::Local)
        );
        assert_eq!(
            candidates.len(),
            candidates
                .iter()
                .map(|c| &c.text)
                .collect::<HashSet<_>>()
                .len()
        );
        if !word_hit || !sentence_hit {
            misses.push(format!(
                "{seed:?}: word={word_hit}, sentence={sentence_hit}, page={:?}",
                page.iter().map(|c| (&c.text, c.kind)).collect::<Vec<_>>()
            ));
        }
    }
    println!(
        "English curated scenarios: word top-3 {word_hits}/{}, sentence first-page {sentence_hits}/{}",
        cases.len(),
        cases.len()
    );
    assert!(misses.is_empty(), "{}", misses.join("\n"));
}

#[test]
fn longer_context_beats_generic_next_words_without_repeating_committed_text() {
    for (seed, expected, unrelated) in [
        ("how can I ", "how can I help", "how can I am"),
        (
            "I would like to ",
            "I would like to know",
            "I would like to the",
        ),
        ("we need to t", "we need to test", "we need to the"),
        ("how can I", "how can I help", "how can Is"),
    ] {
        let engine = engine(seed);
        assert_eq!(engine.candidates()[1].text, expected);
        assert!(!engine.candidates().iter().any(|c| c.text == unrelated));
    }
    let mut engine = engine("how can");
    engine.commit(CommitOptions { force: true });
    engine.seed("I ");
    assert_eq!(engine.candidates()[1].text, "I help");
    assert!(engine.candidates().iter().all(|c| c.text.starts_with("I ")));
    engine.clear_session_context();
    engine.seed("I ");
    assert_eq!(engine.candidates()[1].text, "I am");
    engine.seed("can");
    assert!(
        engine
            .candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.text == "can't" && c.kind == CandidateKind::Word)
    );
}

#[test]
fn incomplete_identifiers_and_finished_sentences_are_not_fabricated_into_phrases() {
    for seed in [
        "https://exa",
        "src/please",
        "user_nam",
        "unknownword",
        "please-send",
        "v0.4",
        "iPh",
        "don't'please",
        "hello! ",
        "please send me the details.",
        "unknownword ",
    ] {
        let engine = engine(seed);
        assert_eq!(
            engine
                .candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed]
        );
    }
}

#[test]
fn sentence_only_model_replies_also_offer_one_word_without_losing_the_sentence() {
    for (seed, sentence, word) in [
        (
            "please rec",
            "please reconsider the proposal.",
            "please reconsider",
        ),
        ("we need ", "we need reliable backups.", "we need reliable"),
        (
            "  I’ll rec",
            "  I’ll reconsider the proposal.",
            "  I’ll reconsider",
        ),
    ] {
        let local = engine(seed).candidates().to_vec();
        let anchored = local[1].text.clone();
        let (merged, accepted) = merge_model(
            "en",
            seed,
            local,
            vec![
                LlmCompletion {
                    text: sentence.into(),
                    kind: Some(CandidateKind::Sentence),
                    score_bias: 0.6,
                },
                LlmCompletion {
                    text: sentence.into(),
                    kind: None,
                    score_bias: 0.3,
                },
            ],
            12,
        );
        assert!(accepted);
        assert_eq!(merged[0].text, seed);
        assert_eq!(merged[1].text, anchored);
        for (text, kind) in [
            (word, CandidateKind::Word),
            (sentence, CandidateKind::Sentence),
        ] {
            assert!(
                merged.iter().take(PAGE_SIZE).any(|c| c.text == text
                    && c.kind == kind
                    && c.source == CandidateSource::Model),
                "{merged:?}"
            );
        }
        assert_eq!(
            merged.len(),
            merged.iter().map(|c| &c.text).collect::<HashSet<_>>().len()
        );
    }
}
