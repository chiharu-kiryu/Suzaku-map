//! Fixed regression cases for reading separators around unfinished Japanese
//! romaji. Horizontal trailing separators do not request a new syllable across
//! tokens; vertical boundaries and the exact literal draft remain significant.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const STUDY: &str = "日本語を勉強しています。";
const READY: &str = "準備ができたら連絡します。";
const CONDITION: &str = "条件が合えば、こちらを選びます。";
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
        default_language: "ja".into(),
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
    let selected = index(ime, text, kind);
    ime.select_candidate(selected);
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
fn trailing_horizontal_separators_keep_complete_and_partial_word_sentence_choices() {
    for (partial, complete, word, sentence) in [
        ("nihong", "nihongo", "日本語", STUDY),
        ("JYUNB", "JYUNBI", "準備", READY),
        ("jouk", "jouken", "条件", CONDITION),
    ] {
        for &separator in HORIZONTAL {
            for reading in [partial, complete] {
                let seed = format!("{reading}{separator}");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                index(&ime, word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
                commit_once_and_undo(&mut ime, word, CandidateKind::Word);
                commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn adopted_kanji_and_katakana_keep_partial_tail_completion_before_a_separator() {
    for (adopted, expected) in [("日本語", "日本語を勉強"), ("ニホンゴ", "ニホンゴを勉強")]
    {
        for &separator in HORIZONTAL {
            let mut ime = engine("nihongo");
            let selected = index(&ime, adopted, CandidateKind::Word);
            ime.select_candidate(selected);
            assert_eq!(ime.selected_completion_text(true), Some(adopted));
            assert!(ime.snapshot().committed_text.is_empty());
            // Editable host draft replacement, not private or physical key delivery.
            ime.seed(adopted);
            let partial = format!("{adopted}wobenky{separator}");
            ime.seed(&partial);
            assert_local_draft(&ime, &partial);
            commit_once_and_undo(&mut ime, expected, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&partial), &partial, CandidateKind::Literal);

            // Finish the final syllable BEFORE restoring the trailing separator.
            // This does not promise that "ky + Space + ou" forms one syllable.
            let complete = format!("{adopted}wobenkyou{separator}");
            ime.seed(&complete);
            assert_local_draft(&ime, &complete);
            let exact = ime
                .candidates()
                .iter()
                .position(|candidate| candidate.text == expected)
                .unwrap_or_else(|| {
                    panic!(
                        "missing complete conversion {expected:?}: {:?}",
                        ime.candidates()
                    )
                });
            assert!(exact < PAGE_SIZE);
            let kind = ime.candidates()[exact].kind;
            commit_once_and_undo(&mut ime, expected, kind);
            commit_once_and_undo(&mut engine(&complete), &complete, CandidateKind::Literal);
        }
    }
}

#[test]
fn choosing_a_word_after_a_separator_still_allows_particle_progress() {
    for (reading, word, continued, sentence) in [
        ("nihong \t", "日本語", "日本語wo", STUDY),
        ("JYUNB\u{a0}", "準備", "準備ga", READY),
        ("jouk\u{3000}", "条件", "条件ga", CONDITION),
    ] {
        let mut ime = engine(reading);
        assert_local_draft(&ime, reading);
        let selected = index(&ime, word, CandidateKind::Word);
        ime.select_candidate(selected);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level draft adoption discards only the explicitly chosen reading.
        ime.seed(word);
        assert_local_draft(&ime, word);
        ime.seed(continued);
        assert_local_draft(&ime, continued);
        commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(continued), continued, CandidateKind::Literal);
    }
}

#[test]
fn adopted_written_words_cannot_lose_their_kana_tail_to_a_new_reading() {
    for seed in ["同じkanji", "同じかんじ"] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "同じ漢字", "{seed:?}");
        assert_ne!(ime.candidates()[0].text, "同時間じ");
        index(&ime, "同じ感じ", CandidateKind::Word);
        commit_once_and_undo(&mut ime, "同じ漢字", CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), "同じ感じ", CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }

    let mut ime = engine("後でnwa");
    assert_local_draft(&ime, "後でnwa");
    // The adopted 後で must survive even though で + nwa resembles 電話's
    // reading. Do not promise conversion of the remaining unknown んわ tail.
    assert!(ime.candidates()[0].text.starts_with("後で"));
    assert_ne!(ime.candidates()[0].text, "後電話");
    commit_once_and_undo(&mut ime, "後でnwa", CandidateKind::Literal);

    let mut ime = engine("別のjouken");
    assert_local_draft(&ime, "別のjouken");
    assert_eq!(ime.candidates()[0].text, "別の条件");
    commit_once_and_undo(&mut ime, "別の条件", CandidateKind::Sentence);
    commit_once_and_undo(
        &mut engine("別のjouken"),
        "別のjouken",
        CandidateKind::Literal,
    );
}

#[test]
fn vertical_boundaries_and_unknown_tails_cannot_be_discarded_for_completions() {
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        for (head, tail) in [
            ("nihong", "o"),
            ("JYUNB", "I"),
            ("jouk", "en"),
            ("日本語wo", "benky"),
            ("ニホンゴwo", "benky"),
        ] {
            let seed = format!("{head}{boundary}{tail} \t");
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            for candidate in ime.candidates() {
                assert!(
                    candidate.text.contains(boundary),
                    "lost line boundary in {seed:?}: {candidate:?}"
                );
                for forbidden in [STUDY, READY, CONDITION, "日本語を勉強", "ニホンゴを勉強"]
                {
                    assert_ne!(candidate.text, forbidden);
                }
            }
            commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }

    for seed in [
        "xyznihong ",
        "🙂JYUNB\t",
        "https://jouk\u{a0}",
        "日本語xyzbenky ",
        "ニホンゴ🙂benky ",
        "準備gaxyz ",
        "条件woga\t",
        "nihongx ",
        "JYUNB! ",
        "jouk🙂 ",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        for candidate in ime.candidates() {
            for sentence in [STUDY, READY, CONDITION] {
                assert!(
                    !candidate.text.ends_with(sentence),
                    "{seed:?} must not manufacture {sentence:?}: {:?}",
                    ime.candidates()
                );
            }
            for word in ["日本語", "準備", "条件", "日本語を勉強", "ニホンゴを勉強"]
            {
                assert_ne!(candidate.text, word, "discarded unknown text in {seed:?}");
            }
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
