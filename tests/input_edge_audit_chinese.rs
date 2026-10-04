//! Chinese input edge audit: punctuation around tone-marked Pinyin must not
//! leak tone digits into a commit, while numeric literal syntax stays intact.
use suzaku_map::ime::candidate_mix::CandidateSource;
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn assert_exact_conversion(seed: &str, expected: &str) {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(
        ime.candidates()[0].text,
        expected,
        "incorrect primary conversion for {seed:?}: {:?}",
        ime.candidates()
    );
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert!(ime.candidates().len() <= 12);
    assert!(ime.snapshot().committed_text.is_empty());
    let first = ime.commit(CommitOptions { force: true });
    assert!(first.ok);
    assert_eq!(first.text.as_deref(), Some(expected));
    assert_eq!(ime.snapshot().committed_text, expected);
    assert!(ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert_eq!(ime.snapshot().committed_text, expected);
    assert!(ime.undo().unwrap().committed_text.is_empty());
}

#[test]
fn tone_digits_end_before_paired_aside_punctuation() {
    for (open, close) in [
        ("(", ")"),
        ("[", "]"),
        ("{", "}"),
        ("（", "）"),
        ("【", "】"),
        ("〔", "〕"),
        ("〈", "〉"),
        ("《", "》"),
        ("「", "」"),
        ("『", "』"),
        ("“", "”"),
        ("‘", "’"),
    ] {
        let seed = format!("ni3hao3{open}shi4jie4{close}");
        let expected = format!("你好{open}世界{close}");
        assert_exact_conversion(&seed, &expected);
    }
}

#[test]
fn chinese_em_dash_separates_tone_marked_clauses_without_leaking_digits() {
    for dash in ["—", "——"] {
        for (prefix, gap) in [("", ""), ("前文。", " "), ("前文。", "\u{3000}")] {
            let seed = format!("{prefix}ni3hao3{gap}{dash}shi4jie4");
            let expected = format!("{prefix}你好{gap}{dash}世界");
            assert_exact_conversion(&seed, &expected);
        }
    }
}

#[test]
fn numeric_syntax_after_pinyin_is_not_treated_as_tone_punctuation() {
    for (seed, expected) in [
        ("hao3.5", "好3.5"),
        ("hao3.5.1", "好3.5.1"),
        ("hao3:30", "好3:30"),
        ("hao3,000", "好3,000"),
        ("hao3/4", "好3/4"),
        ("hao3\\4", "好3\\4"),
        ("hao3-4", "好3-4"),
        ("hao3_4", "好3_4"),
        ("hao3–4", "好3–4"),
        ("hao3+4", "好3+4"),
        ("hao3=4", "好3=4"),
        ("hao3%", "好3%"),
        ("hao30!", "好30!"),
        ("hao6!", "好6!"),
        ("hao0!", "好0!"),
    ] {
        assert_exact_conversion(seed, expected);
    }
}
