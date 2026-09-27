//! Long drafts use bounded local tail work, not a larger model prompt.
use suzaku_map::ime::candidate_mix::{
    CandidateKind, CandidateSource, PREVIEW_CHARS, display_label_for_seed,
};
use suzaku_map::ime::companion::{MAX_FRAME_BYTES, NativeCandidate, NativeComposition};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(language: &str, seed: &str, mixed: bool) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    engine.seed(seed);
    engine
}

#[test]
fn english_long_drafts_keep_word_and_sentence_completions_without_rewriting_prefixes() {
    for prefix in [
        "note ".repeat(60),
        "前文 café 😀  ".repeat(40),
        "note ".repeat(1000),
        "\"\\ ".repeat(1000),
    ] {
        let seed = format!("{prefix}please sen");
        let word = format!("{prefix}please send");
        let sentence = format!("{prefix}please send me the details.");
        for mixed in [false, true] {
            let mut engine = engine("en", &seed, mixed);
            assert!(seed.chars().count() > 256);
            assert_eq!(engine.candidates()[0].text, seed);
            for (text, kind) in [
                (&word, CandidateKind::Word),
                (&sentence, CandidateKind::Sentence),
            ] {
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .take(6)
                        .any(|c| &c.text == text && c.kind == kind),
                    "missing {kind:?}, mixed={mixed}, bytes={}",
                    seed.len()
                );
            }
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.text.starts_with(&seed) && c.source == CandidateSource::Local)
            );
            let index = engine
                .candidates()
                .iter()
                .position(|c| c.text == word)
                .unwrap();
            engine.select_candidate(index);
            assert!(engine.snapshot().committed_text.is_empty());
            engine.seed(format!("{word} me the d"));
            let expected = format!("{word} me the details");
            let index = engine
                .candidates()
                .iter()
                .position(|c| c.text == expected)
                .unwrap();
            engine.select_candidate(index);
            assert_eq!(
                engine.commit(CommitOptions { force: true }).text.as_deref(),
                Some(expected.as_str())
            );
        }
    }
}

#[test]
fn chinese_long_adopted_prefixes_keep_tail_conversion_and_sentence_choices() {
    for prefix in [
        "你".repeat(252),
        format!("  {}\u{3000}", "已确认的前文。".repeat(80)),
        format!("{}中文  ", "👩‍💻 Rust2026 ".repeat(60)),
        "你".repeat(1000),
    ] {
        for mixed in [false, true] {
            let seed = format!("{prefix}ni hao");
            let word = format!("{prefix}你好");
            let sentence = format!("{prefix}你好，很高兴认识你。");
            let mut engine = engine("zh-Hans", &seed, mixed);
            assert_eq!(engine.candidates()[0].text, word);
            assert!(engine.candidates().iter().any(|c| c.text == seed));
            assert!(
                engine
                    .candidates()
                    .iter()
                    .take(6)
                    .any(|c| c.kind == CandidateKind::Sentence && (!mixed || c.text == sentence)),
                "mixed={mixed}, bytes={}, tails={:?}",
                seed.len(),
                engine
                    .candidates()
                    .iter()
                    .map(|c| (&c.text[prefix.len()..], c.kind))
                    .collect::<Vec<_>>()
            );
            assert!(
                engine
                    .candidates()
                    .iter()
                    .all(|c| c.text.starts_with(&prefix) && c.source == CandidateSource::Local)
            );
            // Replacing with the adopted result still leaves the entire draft editable.
            engine.seed(format!("{word} de"));
            let expected = format!("{word} 的");
            assert_eq!(engine.candidates()[0].text, expected);
            assert_eq!(
                engine.commit(CommitOptions { force: true }).text.as_deref(),
                Some(expected.as_str())
            );
        }
    }
}

#[test]
fn adopted_long_chinese_words_keep_authored_sentences_before_and_after_space() {
    for prefix in ["你".repeat(260), "前文 café 😀。  ".repeat(60)] {
        for (reading, word, sentence) in [
            ("ni hao", "你好", "你好，很高兴认识你。"),
            ("hui yi", "会议", "会议什么时候开始？"),
            ("shu ru fa", "输入法", "输入法支持多种语言。"),
            (
                "wo xiang xue xi zhong wen",
                "我想学习中文",
                "我想学习中文，请多指教。",
            ),
        ] {
            let raw = format!("{prefix}{reading}");
            let adopted = format!("{prefix}{word}");
            for mixed in [false, true] {
                let mut engine = engine("zh-Hans", &raw, mixed);
                assert_eq!(engine.candidates()[0].text, adopted);
                // The native host adopts a choice by reseeding the complete editable text.
                for spacing in ["", " ", "  ", "\u{a0}", "\u{3000}"] {
                    let seed = format!("{adopted}{spacing}");
                    let expected = format!("{seed}{}", &sentence[word.len()..]);
                    engine.seed(&seed);
                    assert_eq!(engine.candidates()[0].text, seed);
                    assert!(engine.snapshot().committed_text.is_empty());
                    assert!(engine.candidates().iter().all(|candidate| {
                        candidate.text.starts_with(&seed)
                            && candidate.source == CandidateSource::Local
                    }));
                    let index = engine
                        .candidates()
                        .iter()
                        .take(6)
                        .position(|c| c.text == expected && c.kind == CandidateKind::Sentence)
                        .unwrap_or_else(|| {
                            panic!("missing continuation: {word:?}/{spacing:?}/{mixed}")
                        });
                    engine.select_candidate(index);
                    assert!(engine.snapshot().committed_text.is_empty());
                    assert_eq!(
                        engine.commit(CommitOptions { force: true }).text.as_deref(),
                        Some(expected.as_str())
                    );
                    engine.clear_session_context();
                }
            }
        }
    }
}

#[test]
fn adopted_chinese_sentences_survive_crossing_the_local_window_threshold() {
    for (reading, word, suffix) in [
        ("ni hao", "你好", "，很高兴认识你。"),
        ("hui yi", "会议", "什么时候开始？"),
        ("wo xiang xue xi zhong wen", "我想学习中文", "，请多指教。"),
    ] {
        for length in [254, 255, 256, 257, 258] {
            let prefix = "你".repeat(length - word.chars().count());
            let raw = format!("{prefix}{reading}");
            let adopted = format!("{prefix}{word}");
            assert!(raw.chars().count() > 256);
            let mut engine = engine("zh-Hans", &raw, true);
            assert_eq!(engine.candidates()[0].text, adopted);
            assert!(engine.candidates().iter().take(6).any(|candidate| {
                candidate.text == format!("{adopted}{suffix}")
                    && candidate.kind == CandidateKind::Sentence
            }));
            for spacing in ["", " ", "  ", "\u{3000}"] {
                let draft = format!("{adopted}{spacing}");
                engine.seed(&draft);
                assert_eq!(engine.candidates()[0].text, draft);
                assert!(engine.snapshot().committed_text.is_empty());
                assert!(
                    engine
                        .candidates()
                        .iter()
                        .all(|c| c.text.starts_with(&draft))
                );
                assert!(
                    engine.candidates().iter().take(6).any(|candidate| {
                        candidate.text == format!("{draft}{suffix}")
                            && candidate.kind == CandidateKind::Sentence
                    }),
                    "lost sentence after adoption: {word:?}, length={length}, spacing={spacing:?}"
                );
            }
        }
    }
}

#[test]
fn long_chinese_continuation_does_not_hide_identifiers_or_cross_line_boundaries() {
    let prefix = "前文。".repeat(100);
    for tail in [
        "https://你好",
        "https://你好 ",
        "user_会议",
        "person@中文",
        "dir\\输入法",
        "你好。",
        "你好\n",
        "你好\u{2028}",
        "一个没有续写的词",
    ] {
        let seed = format!("{prefix}{tail}");
        let engine = engine("zh-Hans", &seed, true);
        assert_eq!(
            engine
                .candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed.as_str()],
            "{tail:?}"
        );
    }
    let seed = format!("{prefix}你好{}", " ".repeat(256));
    assert_eq!(engine("zh-Hans", &seed, true).candidates().len(), 1);
}

#[test]
fn adopted_long_english_words_keep_next_words_and_sentences_across_space() {
    let prefix = "Earlier writing. ".repeat(40);
    for spacing in ["", " ", "  ", "\u{3000}"] {
        let seed = format!("{prefix}please send{spacing}");
        let separator = if spacing.is_empty() { " " } else { "" };
        let word = format!("{seed}{separator}me");
        let sentence = format!("{word} the details.");
        let engine = engine("en", &seed, true);
        assert_eq!(engine.candidates()[0].text, seed);
        for (text, kind) in [
            (word, CandidateKind::Word),
            (sentence, CandidateKind::Sentence),
        ] {
            assert!(
                engine
                    .candidates()
                    .iter()
                    .take(6)
                    .any(|c| c.text == text && c.kind == kind)
            );
        }
        assert!(
            engine
                .candidates()
                .iter()
                .all(|c| c.text.starts_with(&seed))
        );
    }
}

#[test]
fn long_unbroken_readings_and_identifiers_are_not_split_into_invented_words() {
    for (language, seed) in [
        ("zh-Hans", format!("{}n", "ni".repeat(128))),
        ("zh-Hans", format!("{}https://bei", "你".repeat(260))),
        ("zh-Hans", format!("{}user_name", "你".repeat(260))),
        (
            "zh-Hans",
            format!("{}https://中文nihao", "前文 ".repeat(90)),
        ),
        ("zh-Hans", format!("{}user_中文nihao", "前文 ".repeat(90))),
        ("zh-Hans", format!("{}foo@中文nihao", "前文 ".repeat(90))),
        ("zh-Hans", format!("{} nihao", "x".repeat(270))),
        ("en", format!("{}https://hel", "note ".repeat(60))),
        ("en", format!("{}user_hel", "note ".repeat(60))),
        ("en", format!("{}hel", "x".repeat(270))),
        ("ja", format!("{}nihongo", "文".repeat(260))),
    ] {
        let engine = engine(language, &seed, true);
        assert_eq!(
            engine
                .candidates()
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            [seed.as_str()],
            "{language}"
        );
    }
    let mut engine = engine("zh-Hans", &format!("{}n", "ni".repeat(128)), true);
    engine.seed("ni".repeat(128));
    assert_eq!(engine.candidates()[0].text, "你".repeat(128));
}

#[test]
fn long_candidate_payloads_fit_the_existing_native_frame_without_losing_literal_input() {
    for (language, seed) in [
        ("en", format!("{}please sen", "note ".repeat(1600))),
        ("en", format!("{}please sen", "\"\\ ".repeat(2700))),
        ("en", format!("{}hel", " ".repeat(8189))),
        ("zh-Hans", format!("{}nihao", "你".repeat(2729))),
        ("zh-Hans", format!("{}你好 ", "前文。".repeat(880))),
        ("zh-Hans", format!("{}会议", "你".repeat(2728))),
    ] {
        let engine = engine(language, &seed, true);
        assert!(engine.candidates().iter().any(|c| c.text == seed));
        let frame = NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 1,
            revision: 1,
            focused: true,
            private: false,
            language: language.into(),
            seed: seed.clone(),
            selected: 0,
            candidates: engine
                .candidates()
                .iter()
                .map(|c| NativeCandidate {
                    text: c.text.clone(),
                    label: c.label.clone(),
                    kind: c.kind,
                    source: c.source,
                    weight: c.score.clamp(0.0, 100.0) as u8,
                })
                .collect(),
        };
        let raw = frame.to_json().to_string();
        assert!(raw.len() < MAX_FRAME_BYTES, "{language}: {}", raw.len());
        assert_eq!(NativeComposition::parse(raw.as_bytes()).unwrap(), frame);
    }
}

#[test]
fn long_tail_candidate_limits_and_256_character_transitions_remain_lossless() {
    for length in [250, 251, 252, 253] {
        let prefix = "你".repeat(length);
        let seed = format!("{prefix}nihao");
        assert_eq!(
            engine("zh-Hans", &seed, true).candidates()[0].text,
            format!("{prefix}你好")
        );
    }
    for limit in [0, 1, 2, 3, 6] {
        for language in ["en", "zh-Hans"] {
            let seed = if language == "en" {
                format!("{}hel", "note ".repeat(60))
            } else {
                format!("{}nihao", "你".repeat(260))
            };
            let mut engine = XRTabletImeEngine::new(EngineConfig {
                default_language: language.into(),
                max_candidates: limit,
                ..Default::default()
            });
            engine.seed(&seed);
            assert!(engine.candidates().len() <= limit.max(1));
            assert!(engine.candidates().iter().any(|c| c.text == seed));
        }
    }
}

#[test]
fn chinese_long_candidate_labels_reveal_differences_but_commit_full_text() {
    let prefix = "已经确认的文字".repeat(40);
    let seed = format!("{prefix}bei j");
    let labels: Vec<_> = ["北京", "背景"]
        .map(|tail| {
            display_label_for_seed(
                "zh-Hans",
                &seed,
                &format!("{prefix}{tail}"),
                CandidateKind::Word,
                CandidateSource::Local,
                92,
            )
        })
        .into();
    assert_ne!(labels[0], labels[1]);
    assert!(labels[0].starts_with('…') && labels[0].contains("北京"));
    assert!(labels[1].contains("背景"));
    assert!(
        labels
            .iter()
            .all(|label| label.chars().count() <= PREVIEW_CHARS)
    );
    let rewritten = "这是完全不同的开头".repeat(20);
    assert!(
        display_label_for_seed(
            "zh-Hans",
            &seed,
            &rewritten,
            CandidateKind::Sentence,
            CandidateSource::Model,
            90
        )
        .starts_with("这是完全不同的开头")
    );
    let mut engine = engine("zh-Hans", &seed, true);
    assert_eq!(
        engine.commit(CommitOptions { force: true }).text.as_deref(),
        Some(format!("{prefix}北京").as_str())
    );
}
