//! Conservative authored-sentence progress, not sentence-reading conversion or
//! morphological analysis. All expectations are fixed independently of the
//! production lexicon and exercise only the local mixed-candidate engine.
use std::collections::HashSet;
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine};

const PLAN: &str = "予定が決まったら連絡します。";
const STUDY: &str = "日本語を勉強しています。";
const READY: &str = "準備ができたら連絡します。";

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
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    // Raw spelling remains reachable even when first-page slots are occupied.
    if kind != CandidateKind::Literal {
        assert!(index < PAGE_SIZE, "not on page one: {text:?}");
    }
    index
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let index = index(ime, text, kind);
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
fn particles_after_known_words_keep_matching_authored_sentences_on_page_one() {
    for (seed, expected) in [
        ("予定が", PLAN),
        ("よていが", PLAN),
        ("yoteiga", PLAN),
        ("yotei ga", PLAN),
        ("予定ga", PLAN),
        ("日本語を", STUDY),
        ("にほんごを", STUDY),
        ("nihongowo", STUDY),
        ("nihongo wo", STUDY),
        ("日本語wo", STUDY),
        ("準備が", READY),
        ("じゅんびが", READY),
        ("junbiga", READY),
        ("JYUNBI GA", READY),
        ("準備ga", READY),
        ("予定を", "予定を確認してから返事します。"),
        ("yotei wo", "予定を確認してから返事します。"),
        ("日本語で", "日本語で入力できます。"),
        ("nihongo de", "日本語で入力できます。"),
        ("準備を", "準備を手伝ってもらえますか。"),
        ("JYUNBI WO", "準備を手伝ってもらえますか。"),
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        commit_once_and_undo(&mut ime, expected, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn every_typed_literal_prefix_of_an_authored_sentence_is_preserved() {
    for (trigger, expected) in [("予定", PLAN), ("日本語", STUDY), ("準備", READY)] {
        let mut ime = engine(trigger);
        // Advance through the independent fixed expectation one Unicode scalar
        // at a time, including kanji already supplied by the user. No kanji is
        // inferred from arbitrary incomplete sentence-level romaji here.
        for (end, _) in expected
            .char_indices()
            .filter(|(end, _)| *end > trigger.len())
        {
            let seed = &expected[..end];
            ime.seed(seed);
            assert_local_draft(&ime, seed);
            let candidate = &ime.candidates()[index(&ime, expected, CandidateKind::Sentence)];
            assert!(candidate.text.starts_with(seed));
            commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
        }
        commit_once_and_undo(&mut ime, expected, CandidateKind::Sentence);
        // Once the complete sentence is typed, committing it literally must
        // neither duplicate punctuation nor reinsert an earlier trigger.
        let mut complete = engine(expected);
        assert_local_draft(&complete, expected);
        commit_once_and_undo(&mut complete, expected, CandidateKind::Literal);
    }
}

#[test]
fn adopting_a_word_then_typing_a_particle_keeps_the_draft_editable() {
    for (reading, word, continued, expected) in [
        ("yotei", "予定", "予定が", PLAN),
        ("nihongo", "日本語", "日本語を", STUDY),
        ("JYUNBI", "準備", "準備が", READY),
    ] {
        let mut ime = engine(reading);
        let word_index = index(&ime, word, CandidateKind::Word);
        ime.select_candidate(word_index);
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // This is an editable host draft update, not simulated physical input.
        ime.seed(word);
        assert_local_draft(&ime, word);
        ime.seed(continued);
        assert_local_draft(&ime, continued);
        commit_once_and_undo(&mut ime, expected, CandidateKind::Sentence);
    }
}

#[test]
fn unknown_prefixes_or_conflicting_tails_cannot_be_dropped_to_offer_known_sentences() {
    let unsupported = [
        "予定xyz".to_owned(),
        "予定がxyz".to_owned(),
        "予定をが".to_owned(),
        "予定が決まり".to_owned(),
        "🙂予定が".to_owned(),
        "XYZ予定が".to_owned(),
        "前置き予定が".to_owned(),
        "https://予定が".to_owned(),
        "yoteigakim".to_owned(),
        format!("{}予定が", "前文".repeat(150)),
    ];
    for seed in unsupported {
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        for candidate in ime.candidates() {
            for authored in [
                PLAN,
                STUDY,
                READY,
                "予定を確認してから返事します。",
                "日本語で入力できます。",
                "準備を手伝ってもらえますか。",
            ] {
                assert!(
                    !candidate.text.ends_with(authored),
                    "{seed:?} must not manufacture {authored:?}: {:?}",
                    ime.candidates()
                );
            }
        }
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
}

#[test]
fn romaji_separators_are_consumed_only_for_conversion_and_remain_literal_choices() {
    for (seed, expected) in [
        ("  yotei  ga ", PLAN),
        ("\tyotei\tga\u{3000}", PLAN),
        ("日本語 を", STUDY),
        ("nihongo\u{3000}wo", STUDY),
        ("JYUNBI  GA ", READY),
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        commit_once_and_undo(&mut ime, expected, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn line_boundaries_survive_conversion_and_cannot_join_an_authored_sentence() {
    for (seed, converted, separator) in [
        ("予定\nが", "予定\nが", "\n"),
        ("yotei\nga", "予定\nが", "\n"),
        ("予定\r\nが", "予定\r\nが", "\r\n"),
        ("YOTEI\r\nGA", "予定\r\nが", "\r\n"),
        ("予定\u{2028}が", "予定\u{2028}が", "\u{2028}"),
        ("yotei\u{2028}ga", "予定\u{2028}が", "\u{2028}"),
        ("準備\nができ", "準備\nができ", "\n"),
        ("JYUNBI\nga deki", "準備\nができ", "\n"),
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert!(
            ime.candidates()
                .iter()
                .any(|candidate| candidate.text == converted),
            "missing line-preserving conversion {converted:?}: {:?}",
            ime.candidates()
        );
        for candidate in ime.candidates() {
            assert!(
                candidate.text.contains(separator),
                "lost line boundary in {seed:?}: {candidate:?}"
            );
            assert_ne!(candidate.text, PLAN);
            assert_ne!(candidate.text, READY);
        }
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}
