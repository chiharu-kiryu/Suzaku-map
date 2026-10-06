//! Phrase wrappers must not expose English context inside a protected token.
//! Fixed writing examples exercise the public engine without models or packs.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "en".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.text.starts_with(seed) && candidate.source == CandidateSource::Local
    }));
}

fn page_index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn assert_no_false_phrase(ime: &XRTabletImeEngine, prefix: &str) {
    // `sent` is a normal independent completion of `sen`. A false `please`
    // phrase match suppresses it in favor of `send` and the authored sentences.
    page_index(ime, &format!("{prefix}sent"), CandidateKind::Word);
    for ending in ["send me the details.", "send me a message."] {
        let sentence = format!("{prefix}{ending}");
        assert!(
            ime.candidates()
                .iter()
                .all(|candidate| candidate.text != sentence),
            "protected token supplied phrase context: {sentence:?}"
        );
    }
}

#[test]
fn protected_token_wrappers_do_not_expose_phrases_in_short_or_long_drafts() {
    for history in [String::new(), "Earlier café 😀. ".repeat(40)] {
        for token in [
            "src/", "src\\", "user_", "person@", "example.", "user-", "note:",
        ] {
            for opening in ["(", "[", "{", "\"", "“", "‘", "'", "(['\""] {
                let prefix = format!("{history}{token}{opening}please ");
                let seed = format!("{prefix}sen");
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                assert!(ime.snapshot().committed_text.is_empty());
                assert_no_false_phrase(&ime, &prefix);
            }
        }
    }
}

#[test]
fn protected_token_wrappers_do_not_supply_committed_phrase_context() {
    for history in [String::new(), "Earlier café 😀. ".repeat(40)] {
        for token in ["src/", "user_", "person@", "example."] {
            for opening in ["(", "[", "{", "\"", "“", "‘", "'", "(['\""] {
                let context = format!("{history}{token}{opening}please");
                let mut ime = engine(&context);
                assert_local_draft(&ime, &context);
                let committed = ime.commit(CommitOptions { force: true });
                assert!(committed.ok);
                assert_eq!(committed.text.as_deref(), Some(context.as_str()));
                ime.seed("sen");
                assert_local_draft(&ime, "sen");
                assert_eq!(ime.snapshot().committed_text, context);
                assert_no_false_phrase(&ime, "");
            }
        }
    }
}

#[test]
fn ordinary_opening_wrappers_preserve_words_and_authored_sentences() {
    for history in [String::new(), "Earlier café 😀. ".repeat(40)] {
        for opening in ["(", "[", "{", "\"", "“", "‘", "'", "(['\"", "He said '"] {
            let prefix = format!("{history}{opening}Please  ");
            let seed = format!("{prefix}sen");
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            page_index(&ime, &format!("{prefix}send"), CandidateKind::Word);
            page_index(
                &ime,
                &format!("{prefix}send me the details."),
                CandidateKind::Sentence,
            );
        }
    }
}

#[test]
fn whitespace_after_protected_tokens_allows_a_fresh_wrapped_phrase() {
    for history in [String::new(), "Earlier café 😀. ".repeat(40)] {
        for token in ["src/(opaque", "user_\"opaque", "person@‘opaque"] {
            for gap in [" ", "  ", "\u{a0}", "\u{3000}"] {
                let prefix = format!("{history}{token}{gap}(‘Please  ");
                let seed = format!("{prefix}sen");
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                page_index(&ime, &format!("{prefix}send"), CandidateKind::Word);
                page_index(
                    &ime,
                    &format!("{prefix}send me the details."),
                    CandidateKind::Sentence,
                );
            }
        }
    }
}

#[test]
fn wrapped_words_continue_after_adoption_and_commit_without_rewriting_the_prefix() {
    for history in [String::new(), "Earlier café 😀. ".repeat(40)] {
        let prefix = format!("{history}He said ('Please  ");
        let seed = format!("{prefix}sen");
        let word = format!("{prefix}send");
        let sentence = format!("{word} me the details.");
        let mut ime = engine(&seed);
        let index = page_index(&ime, &word, CandidateKind::Word);
        ime.select_candidate(index);
        let adopted = ime.selected_completion_text(true).unwrap().to_owned();
        ime.seed(&adopted);
        assert!(ime.snapshot().committed_text.is_empty());
        let continued = format!("{adopted} m");
        ime.seed(&continued);
        assert_local_draft(&ime, &continued);
        page_index(&ime, &format!("{word} me"), CandidateKind::Word);
        let index = page_index(&ime, &sentence, CandidateKind::Sentence);
        ime.select_candidate(index);
        assert!(ime.snapshot().committed_text.is_empty());
        let committed = ime.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(sentence.as_str()));
        assert!(ime.snapshot().seed_text.is_empty());
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }
}
