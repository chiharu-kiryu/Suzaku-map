//! Exact word projections from synthetic provider-neutral sentence responses.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, merge_predictions};
use suzaku_map::ime::{Candidate, EngineConfig, XRTabletImeEngine};
use suzaku_map::prediction::{PredictionCandidate, PredictionKind};

fn projected(seed: &str, sentence: &str) -> Vec<Candidate> {
    let mut engine = XRTabletImeEngine::new(EngineConfig::default());
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    merge_predictions(
        "en",
        seed,
        engine.candidates().to_vec(),
        vec![PredictionCandidate {
            text: sentence.into(),
            score_bias: 0.8,
            kind: Some(PredictionKind::Sentence),
        }],
        12,
    )
    .0
}

#[test]
fn extra_horizontal_spacing_preserves_separate_word_and_sentence_candidates() {
    for seed in [
        "we need ",
        "we need  ",
        "we need\u{a0}",
        "  we need\u{3000}",
    ] {
        for gap in [
            " ", "  ", "\u{a0}", "\u{2002}", "\u{2009}", "\u{202f}", "\u{3000}",
        ] {
            let word = format!("{seed}{gap}reliable");
            let sentence = format!("{word} backups.");
            let candidates = projected(seed, &sentence);
            assert_eq!(candidates[0].text, seed);
            for (text, kind) in [
                (&word, CandidateKind::Word),
                (&sentence, CandidateKind::Sentence),
            ] {
                assert!(
                    candidates.iter().any(|c| &c.text == text
                        && c.kind == kind
                        && c.source == CandidateSource::Model),
                    "missing {kind:?} {text:?}: {candidates:?}"
                );
            }
        }
    }
}

#[test]
fn a_known_complete_word_may_gain_a_unicode_separator_without_rewriting_it() {
    for seed in ["hello", "  Hello", "你好，hello", "I'm"] {
        for gap in [" ", "\u{a0}", "\u{2003}", "\u{3000}"] {
            let word = format!("{seed}{gap}ready");
            let sentence = format!("{word} for it.");
            let candidates = projected(seed, &sentence);
            assert!(
                candidates
                    .iter()
                    .any(|c| c.text == word && c.kind == CandidateKind::Word),
                "missing exact word {word:?}: {candidates:?}"
            );
            assert!(
                candidates
                    .iter()
                    .any(|c| c.text == sentence && c.kind == CandidateKind::Sentence)
            );
        }
    }
}

#[test]
fn separators_never_complete_an_unfinished_word_or_split_technical_tokens() {
    for gap in [
        " ", "\u{a0}", "\u{2009}", "\u{3000}", "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}",
        "\u{2029}",
    ] {
        let sentence = format!("hel{gap}there again.");
        let candidates = projected("hel", &sentence);
        assert!(
            !candidates
                .iter()
                .any(|c| c.text == format!("hel{gap}there") && c.kind == CandidateKind::Word)
        );
    }
    for tail in [
        "reliable.example site",
        "reliable-backups daily",
        "user_name today",
        "reliable@example.test",
        "世界 today",
        "reliable123 next",
    ] {
        let candidates = projected("we need ", &format!("we need  {tail}"));
        assert!(
            !candidates
                .iter()
                .any(|c| c.source == CandidateSource::Model && c.kind == CandidateKind::Word),
            "{tail}: {candidates:?}"
        );
    }
    for gap in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let candidates = projected("we need ", &format!("we need {gap}reliable backups."));
        assert!(
            !candidates
                .iter()
                .any(|c| c.source == CandidateSource::Model && c.kind == CandidateKind::Word),
            "crossed line {gap:?}: {candidates:?}"
        );
    }
}
