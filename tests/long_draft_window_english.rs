//! The local window includes a complete word at its exact 256-scalar boundary.
use suzaku_map::ime::candidate_mix::CandidateKind;
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(seed: &str, mixed: bool) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: "en".into(),
        ..Default::default()
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    engine.seed(seed);
    engine
}

fn padded_tail(chars: usize) -> String {
    let tail = format!("please{}sen", " ".repeat(chars - 9));
    assert_eq!(tail.chars().count(), chars);
    tail
}

fn assert_exact_commit_and_undo(engine: &mut XRTabletImeEngine, text: &str) {
    let index = engine
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text)
        .unwrap_or_else(|| panic!("missing exact candidate {text:?}"));
    engine.select_candidate(index);
    assert!(engine.snapshot().committed_text.is_empty());
    assert_eq!(engine.preference_count(), 0);
    let result = engine.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(result.snapshot.committed_text, text);
    assert!(result.snapshot.seed_text.is_empty());
    let undone = engine.undo().expect("successful commit has undo history");
    assert!(undone.committed_text.is_empty());
    assert_eq!(engine.preference_count(), 0);
    assert!(engine.undo().is_none());
}

#[test]
fn short_padded_english_phrases_keep_authored_sentences_at_the_window_limit() {
    for chars in [255, 256] {
        let seed = padded_tail(chars);
        let sentence = format!("{seed}d me the details.");
        let mut engine = engine(&seed, true);
        assert_eq!(engine.candidates()[0].text, seed);
        assert!(engine.candidates().iter().any(|candidate| {
            candidate.text == sentence && candidate.kind == CandidateKind::Sentence
        }));
        assert_exact_commit_and_undo(&mut engine, &sentence);
        engine.seed(&seed);
        assert_exact_commit_and_undo(&mut engine, &seed);
    }
}

#[test]
fn long_english_windows_keep_complete_words_at_the_exact_scalar_budget() {
    for chars in [255, 256] {
        for prefix in ["x ", "前文😀\t", "Earlier writing.\n"] {
            let seed = format!("{prefix}{}", padded_tail(chars));
            let word = format!("{seed}d");
            let sentence = format!("{word} me the details.");
            assert!(seed.chars().count() > 256);
            for mixed in [false, true] {
                let mut engine = engine(&seed, mixed);
                assert_eq!(engine.candidates()[0].text, seed);
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .all(|candidate| { candidate.text.starts_with(&seed) })
                );
                for (text, kind) in [
                    (&word, CandidateKind::Word),
                    (&sentence, CandidateKind::Sentence),
                ] {
                    assert!(
                        engine
                            .candidates()
                            .iter()
                            .take(6)
                            .any(|candidate| { &candidate.text == text && candidate.kind == kind }),
                        "missing {kind:?}: tail chars={chars}, prefix={prefix:?}, mixed={mixed}"
                    );
                }
                assert_exact_commit_and_undo(&mut engine, &sentence);
                engine.seed(&seed);
                assert_exact_commit_and_undo(&mut engine, &seed);
            }
        }
    }
}

#[test]
fn over_budget_english_context_stays_frozen_instead_of_expanding_the_window() {
    for prefix in ["x ", "前文😀\t", "Earlier writing.\n"] {
        let seed = format!("{prefix}{}", padded_tail(257));
        let word = format!("{seed}d");
        let outside_window_sentence = format!("{word} me the details.");
        for mixed in [false, true] {
            let mut engine = engine(&seed, mixed);
            assert_eq!(engine.candidates()[0].text, seed);
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|candidate| { candidate.text.starts_with(&seed) })
            );
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|candidate| candidate.text != outside_window_sentence),
                "the frozen `please` must not become local context: {prefix:?}, mixed={mixed}"
            );
            assert!(engine.candidates().iter().any(|candidate| {
                candidate.text == word && candidate.kind == CandidateKind::Word
            }));
            assert_exact_commit_and_undo(&mut engine, &word);
            engine.seed(&seed);
            assert_exact_commit_and_undo(&mut engine, &seed);
        }
    }
}

#[test]
fn bilingual_prose_boundaries_preserve_word_and_sentence_candidates_in_long_drafts() {
    for prefix in ["前文".to_owned(), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            for (tail, word_tail, sentence_tail) in [
                ("hel", "hello", "hello, how are you?"),
                ("Please SEN", "Please SEND", "Please SEND me the details."),
                (
                    "please send ",
                    "please send me",
                    "please send me the details.",
                ),
            ] {
                let seed = format!("{prefix}{separator}{tail}");
                let word = format!("{prefix}{separator}{word_tail}");
                let sentence = format!("{prefix}{separator}{sentence_tail}");
                for mixed in [false, true] {
                    let mut engine = engine(&seed, mixed);
                    // The legacy short-draft API returns untyped word choices;
                    // IBus and the bounded long-draft path mix typed sentences.
                    let typed_mix = mixed || seed.chars().count() > 256;
                    assert_eq!(engine.candidates()[0].text, seed);
                    assert!(
                        engine
                            .candidates()
                            .iter()
                            .all(|candidate| candidate.text.starts_with(&seed))
                    );
                    let mut expected = vec![(
                        &word,
                        if typed_mix {
                            CandidateKind::Word
                        } else {
                            CandidateKind::Unspecified
                        },
                    )];
                    if typed_mix {
                        expected.push((&sentence, CandidateKind::Sentence));
                    }
                    for (text, kind) in expected {
                        assert!(
                            engine.candidates().iter().take(6).any(|candidate| {
                                &candidate.text == text && candidate.kind == kind
                            }),
                            "missing {kind:?}: separator={separator:?}, tail={tail:?}, mixed={mixed}, chars={}",
                            seed.chars().count()
                        );
                    }
                    assert_exact_commit_and_undo(
                        &mut engine,
                        if typed_mix { &sentence } else { &word },
                    );
                    engine.seed(&seed);
                    assert_exact_commit_and_undo(&mut engine, &seed);
                }
            }
        }
    }
}

#[test]
fn long_prose_window_does_not_hide_url_path_or_identifier_syntax() {
    let prefix = "前文".repeat(150);
    for token in [
        "https://host/",
        "src/",
        "src\\",
        "user_",
        "person@",
        "example.com",
        "user-name",
    ] {
        let seed = format!("{token}{prefix}，hel");
        for mixed in [false, true] {
            let engine = engine(&seed, mixed);
            assert_eq!(
                engine
                    .candidates()
                    .iter()
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>(),
                [seed.as_str()],
                "{token:?}/{mixed}"
            );
        }
    }
}

#[test]
fn prose_boundaries_respect_the_exact_local_window_budget() {
    for prefix in ["前文，".to_owned(), format!("{}—", "前文".repeat(150))] {
        for chars in [255, 256, 257] {
            let seed = format!("{prefix}{}", padded_tail(chars));
            let word = format!("{seed}d");
            let sentence = format!("{word} me the details.");
            for mixed in [false, true] {
                let engine = engine(&seed, mixed);
                assert_eq!(engine.candidates()[0].text, seed);
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .all(|candidate| candidate.text.starts_with(&seed))
                );
                assert!(engine.candidates().iter().any(
                    |candidate| candidate.text == word && candidate.kind == CandidateKind::Word
                ));
                assert_eq!(
                    engine
                        .candidates()
                        .iter()
                        .any(|candidate| candidate.text == sentence
                            && candidate.kind == CandidateKind::Sentence),
                    chars <= 256,
                    "chars={chars}, mixed={mixed}"
                );
            }
        }
    }
}
