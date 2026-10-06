//! Fixed authored examples for unfinished Pinyin before horizontal spacing.
//! Choosing a conversion may consume reading separators; the literal draft,
//! adopted Han spacing, internal syllable boundaries and quotes remain exact.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const CHINESE: &str = "中文输入很方便。";
const INPUT: &str = "输入法支持多种语言。";
const STUDY: &str = "我想学习中文，请多指教。";
const PRONUNCIATION: &str = "发音 可以再示范一下吗？";
const HORIZONTAL: &[&str] = &[
    " ",
    "  ",
    "\t",
    "\u{a0}",
    "\u{3000}",
    "\u{2003}",
    " \t\u{a0}\u{3000}",
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let found = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if kind != CandidateKind::Literal {
        assert!(found < PAGE_SIZE, "{text:?} was not on page one");
    }
    found
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
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
fn trailing_horizontal_spacing_keeps_partial_word_and_sentence_choices() {
    for (reading, word, sentence) in [
        ("zhongw", "中文", CHINESE),
        ("zhong w", "中文", CHINESE),
        ("ZHONG1 W", "中文", CHINESE),
        ("zhong wen", "中文", CHINESE),
        ("zhong1 wen2", "中文", CHINESE),
        ("shu ru f", "输入法", INPUT),
        ("SHU1 RU4 F", "输入法", INPUT),
        ("shu ru fa", "输入法", INPUT),
    ] {
        // The empty suffix separately checks that these are already supported
        // readings, not requests for new vocabulary or tone-sensitive ranking.
        for separator in std::iter::once("").chain(HORIZONTAL.iter().copied()) {
            let seed = format!("{reading}{separator}");
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            index(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_han_prefix_keeps_partial_tail_completion_and_authored_sentence() {
    for separator in HORIZONTAL {
        for reading in ["zhongw", "zhong w", "ZHONG1 W"] {
            let seed = format!("我想学习{reading}{separator}");
            let ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            index(&ime, "我想学习中文", CandidateKind::Word);
            index(&ime, STUDY, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_words_keep_literal_spacing_while_partial_readings_end_with_space() {
    for separator in HORIZONTAL {
        for gap in [" ", "  ", "\u{3000}"] {
            let mut ime = engine("fa yin");
            ime.select_candidate(index(&ime, "发音", CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some("发音"));
            assert!(ime.snapshot().committed_text.is_empty());
            // The host replaces its editable draft after adoption; this is an
            // engine workflow, not a claim about physical keyboard delivery.
            ime.seed("发音");
            let seed = format!("发音{gap}ke y{separator}");
            ime.seed(&seed);
            assert_local_draft(&ime, &seed);
            let word = format!("发音{gap}可以");
            let sentence = format!("发音{gap}可以再示范一下吗？");
            index(&ime, &word, CandidateKind::Word);
            index(&ime, &sentence, CandidateKind::Sentence);

            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            let continued = format!("{word} zai");
            ime.seed(&continued);
            assert_local_draft(&ime, &continued);
            let sentence = format!("{word} 再示范一下吗？");
            commit_once_and_undo(&mut ime, &sentence, CandidateKind::Sentence);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn partial_space_choices_commit_exactly_once_and_undo_without_implicit_submission() {
    for (seed, word, sentence) in [
        ("zhongw ", "中文", CHINESE),
        ("shu ru f\u{3000}", "输入法", INPUT),
        ("我想学习zhong w\t", "我想学习中文", STUDY),
        ("发音 ke y  ", "发音 可以", PRONUNCIATION),
    ] {
        for (text, kind) in [
            (word, CandidateKind::Word),
            (sentence, CandidateKind::Sentence),
            (seed, CandidateKind::Literal),
        ] {
            let mut ime = engine(seed);
            assert_local_draft(&ime, seed);
            commit_once_and_undo(&mut ime, text, kind);
        }
    }
}

#[test]
fn partial_sentence_progress_survives_short_and_long_draft_routing() {
    for prefix in [
        "你".repeat(247),
        "你".repeat(248),
        "你".repeat(249),
        "前文 café 😀。  ".repeat(40),
    ] {
        for reading in ["ke y", "KE3 Y"] {
            for separator in [" ", "\u{3000}"] {
                let seed = format!("{prefix}发音 {reading}{separator}");
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                index(
                    &ime,
                    &format!("{prefix}{PRONUNCIATION}"),
                    CandidateKind::Sentence,
                );
            }
        }
    }
}

#[test]
fn vertical_boundaries_and_internal_syllable_gaps_are_not_completion_whitespace() {
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for reading in ["zhongw", "shu ru f", "我想学习zhong w", "发音 ke y"] {
            let seed = format!("{reading}{boundary} ");
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            assert!(
                ime.candidates()
                    .iter()
                    .all(|candidate| candidate.text.contains(boundary)),
                "lost vertical boundary in {seed:?}: {:?}",
                ime.candidates()
            );
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }

    for seed in [
        "zhongw en ",
        "zh ong wen ",
        "shu r u fa ",
        "我想学习zhongw en ",
        "发音 k e yi ",
        "unknownzhongw ",
        "https://zhongw ",
        "user_zhongw ",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for candidate in ime.candidates() {
            for forbidden in [
                "中文",
                "输入法",
                "我想学习中文",
                "发音 可以",
                CHINESE,
                INPUT,
                STUDY,
                PRONUNCIATION,
            ] {
                assert_ne!(candidate.text, forbidden, "invalid completion for {seed:?}");
            }
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}

#[test]
fn closing_apostrophes_are_literal_quotes_not_discardable_trailing_spacing() {
    for seed in ["zhongw'", "shu ru f' ", "我想学习zhong w'", "发音 ke y'  "] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert!(
            ime.candidates()
                .iter()
                .all(|candidate| candidate.text.contains('\'')),
            "lost quote in {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }

    let seed = "zhong wen'";
    let mut ime = engine(seed);
    assert_local_draft(&ime, seed);
    assert_eq!(ime.candidates()[0].text, "中文'");
    commit_once_and_undo(&mut ime, "中文'", CandidateKind::Sentence);
}
