//! Continue finite authored sentences across editable Han/Pinyin word adoptions.
//! This is not arbitrary sentence generation or a larger Pinyin/model window.
use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};
use suzaku_map::languages::llm::{
    LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn assert_sentence(seed: &str, sentence: &str) {
    let mut ime = engine(seed);
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    assert!(ime.snapshot().committed_text.is_empty());
    let index = ime
        .candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == sentence && c.kind == CandidateKind::Sentence)
        .unwrap_or_else(|| panic!("{seed:?} -> {sentence:?}: {:?}", ime.candidates()));
    ime.select_candidate(index);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(
        ime.commit(CommitOptions { force: true }).text.as_deref(),
        Some(sentence)
    );
}

#[test]
fn adopted_chinese_words_keep_the_sentence_while_typing_and_adopting_the_next_word() {
    for (seed, sentence) in [
        ("发音可以", "发音可以再示范一下吗？"),
        ("发音 可以", "发音 可以再示范一下吗？"),
        ("发音 ke yi", "发音 可以再示范一下吗？"),
        ("发音 ke y", "发音 可以再示范一下吗？"),
        ("发音可以再", "发音可以再示范一下吗？"),
        ("发音 可以 zai", "发音 可以 再示范一下吗？"),
        ("fayin keyi", "发音可以再示范一下吗？"),
        ("这个词是", "这个词是什么意思？"),
        ("这个词 shi", "这个词 是什么意思？"),
        ("会议什么时候", "会议什么时候开始？"),
        ("输入法支持", "输入法支持多种语言。"),
        ("学习一门", "学习一门新的语言。"),
        ("我喜欢北京的", "我喜欢北京的文化。"),
    ] {
        assert_sentence(seed, sentence);
    }
}

#[test]
fn adopted_everyday_adjectives_keep_authored_sentences_and_literal_spaces() {
    for (word, rest) in [("很强", "的学习能力。"), ("很厉害", "的表现。")] {
        for gap in ["", " ", "  "] {
            for tail in ["", "de", "的"] {
                let seed = format!("{word}{gap}{tail}");
                assert_sentence(&seed, &format!("{word}{gap}{rest}"));
            }
            // Existing decoding deliberately requires at least two letters
            // before completing an unfinished reading. A lone d is still
            // literal input, not an instruction to guess the syllable de.
            let seed = format!("{word}{gap}d");
            let ime = engine(&seed);
            assert_eq!(ime.candidates()[0].text, seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| { candidate.text == format!("{word}{gap}{rest}") })
            );
        }
    }
}

#[test]
fn sentence_progress_preserves_every_adopted_space_and_long_prefix() {
    for prefix in [
        String::new(),
        "已确认的前文。".repeat(50),
        "前文 café 😀。  ".repeat(40),
    ] {
        for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}"] {
            for tail in ["", " ", "  ", "\u{3000}"] {
                let seed = format!("{prefix}发音{gap}可以{tail}");
                assert_sentence(&seed, &format!("{seed}再示范一下吗？"));
            }
            for reading in ["ke yi", "ke'yi", "keyi", "KE YI", "ke y"] {
                let seed = format!("{prefix}发音{gap}{reading}");
                assert_sentence(&seed, &format!("{prefix}发音{gap}可以再示范一下吗？"));
            }
        }
    }
}

#[test]
fn sentence_progress_survives_the_256_character_boundary_in_both_directions() {
    for length in [250, 251, 252, 253, 254, 255, 256, 257, 260] {
        let prefix = "你".repeat(length);
        for (seed, sentence) in [
            ("发音 ke yi", "发音 可以再示范一下吗？"),
            ("发音 可以", "发音 可以再示范一下吗？"),
            ("发音 可以 ", "发音 可以 再示范一下吗？"),
            ("发音 可以 zai", "发音 可以 再示范一下吗？"),
        ] {
            assert_sentence(&format!("{prefix}{seed}"), &format!("{prefix}{sentence}"));
        }
    }
}

#[test]
fn sentence_progress_does_not_cross_newlines_identifiers_or_rewrite_another_intention() {
    for prefix in [String::new(), "前文。".repeat(100)] {
        for tail in [
            "https://发音可以",
            "user_发音可以",
            "person@发音可以",
            "dir\\发音可以",
            "发音\n可以",
            "发音\r\n可以",
            "发音\u{2028}可以",
            "发音\u{2029}可以",
            "发音不需要",
            "发音 可以 在",
            "发音。可以",
            "发音可以？",
            "发音可以再示范一下吗？",
            "发音 xyzzy",
            "发音 https://ke",
            "发音 ke_yi",
            "发音 2026",
        ] {
            let seed = format!("{prefix}{tail}");
            let ime = engine(&seed);
            assert!(ime.candidates().iter().any(|c| c.text == seed), "{seed:?}");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|c| c.text.ends_with("再示范一下吗？") && c.text != seed),
                "unrelated sentence for {seed:?}: {:?}",
                ime.candidates()
            );
        }
        let seed = format!("{prefix}发音{}可以", " ".repeat(256));
        assert!(
            !engine(&seed)
                .candidates()
                .iter()
                .any(|c| c.text.ends_with("再示范一下吗？"))
        );
    }
}

#[test]
fn authored_homophones_do_not_replace_the_primary_or_already_adopted_text() {
    let ime = engine("发音 可以 zai");
    assert_eq!(ime.candidates()[0].text, "发音 可以 在");
    assert_sentence("发音 可以 zai", "发音 可以 再示范一下吗？");
    let ime = engine("发音 可以 在");
    assert_eq!(ime.candidates()[0].text, "发音 可以 在");
    assert!(!ime.candidates().iter().any(|c| c.text.contains("可以 再")));
    let ime = engine("zai");
    assert_eq!(ime.candidates()[0].text, "在");
    assert!(
        ime.candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.text == "再" && c.kind == CandidateKind::Word)
    );
}

#[test]
fn failed_model_does_not_discard_local_sentence_progress() {
    struct Unavailable;
    impl LlmCompletionProvider for Unavailable {
        fn provider_id(&self) -> &str {
            "sentence-progress-unavailable"
        }
        fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
            Vec::new()
        }
        fn generate_checked(
            &self,
            _: &LlmCompletionRequest,
        ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
            Err(LlmProviderError::Unavailable)
        }
    }
    for (seed, expected) in [
        ("发音 ke yi", "发音 可以再示范一下吗？"),
        ("发音 可以 zai", "发音 可以 再示范一下吗？"),
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(std::sync::Arc::new(Unavailable)));
        ime.seed(seed);
        let local = ime.candidates().to_vec();
        assert!(local.iter().take(PAGE_SIZE).any(|c| c.text == expected));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while ime.prediction_pending() && std::time::Instant::now() < deadline {
            ime.poll_prediction();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            ime.prediction_status(),
            suzaku_map::ime::PredictionStatus::Unavailable
        );
        assert_eq!(ime.prediction_error(), Some(&LlmProviderError::Unavailable));
        assert_eq!(ime.candidates(), local);
    }
}
