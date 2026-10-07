use suzaku_map::ime::candidate_mix::{CandidateKind, PAGE_SIZE};
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.enable_ibus_candidate_mix();
    engine.seed(seed);
    engine
}

fn choose(engine: &mut XRTabletImeEngine, text: &str) {
    let index = engine
        .candidates()
        .iter()
        .position(|c| c.text == text)
        .unwrap_or_else(|| panic!("missing {text}: {:?}", engine.candidates()));
    engine.select_candidate(index);
}

fn confirm(engine: &mut XRTabletImeEngine) {
    assert!(engine.commit(CommitOptions { force: true }).ok);
}

#[test]
fn repeated_chinese_choices_promote_words_without_losing_literal_or_sentences() {
    let mut engine = engine("zh-Hans", "shijian");
    assert_eq!(engine.candidates()[0].text, "时间");
    for _ in 0..8 {
        choose(&mut engine, "实践");
        confirm(&mut engine);
        engine.clear_session_context();
        engine.seed("shijian");
    }
    assert_eq!(engine.preference_count(), 1);
    assert_eq!(engine.candidates()[0].text, "实践");
    assert!(engine.candidates().iter().any(|c| c.text == "shijian"));
    assert!(
        engine
            .candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.kind == CandidateKind::Sentence)
    );
    assert!(
        engine
            .candidates()
            .iter()
            .all(|c| c.score >= 0.0 && c.score <= 100.0)
    );
    engine.seed("shi jian");
    assert_eq!(
        engine.candidates()[0].text,
        "时间",
        "exact spelling scopes the preference"
    );
    engine.seed("shijian");
    let selected = engine.candidates()[0].text.clone();
    engine.select_candidate(0);
    engine.clear_preferences();
    assert_eq!(
        engine.selected_completion_text(true),
        Some(selected.as_str())
    );
    engine.seed("shijian");
    assert_eq!(engine.candidates()[0].text, "时间");
    assert_eq!(engine.preference_count(), 0);
}

#[test]
fn english_learns_completion_but_never_displaces_literal_anchor() {
    let mut engine = engine("en", "hel");
    let target = engine
        .candidates()
        .iter()
        .filter(|c| c.kind == CandidateKind::Word && c.text != "hel")
        .nth(1)
        .unwrap()
        .text
        .clone();
    for _ in 0..8 {
        choose(&mut engine, &target);
        confirm(&mut engine);
        engine.clear_session_context();
        engine.seed("hel");
    }
    assert_eq!(engine.candidates()[0].text, "hel");
    assert_eq!(engine.candidates()[1].text, target);
    assert!(
        engine
            .candidates()
            .iter()
            .take(PAGE_SIZE)
            .any(|c| c.kind == CandidateKind::Sentence)
    );
}

#[test]
fn navigation_failed_commit_literal_and_cancel_do_not_learn() {
    let mut engine = engine("zh-Hans", "shijian");
    choose(&mut engine, "实践");
    assert!(!engine.commit(CommitOptions::default()).ok);
    assert_eq!(engine.preference_count(), 0);
    engine.stage_selected_preference();
    engine.seed("");
    engine.seed("shijian");
    choose(&mut engine, "shijian");
    confirm(&mut engine);
    assert_eq!(engine.preference_count(), 0);
    assert!(!engine.commit(CommitOptions { force: true }).ok);
}

#[test]
fn adopted_choices_learn_only_when_surviving_until_commit_and_undo_reverses_them() {
    let mut engine = engine("zh-Hans", "shijian");
    choose(&mut engine, "实践");
    engine.stage_selected_preference();
    engine.seed("实践");
    assert_eq!(engine.preference_count(), 0);
    engine.seed("shijian"); // native Backspace restores spelling
    choose(&mut engine, "shijian");
    confirm(&mut engine);
    assert_eq!(engine.preference_count(), 0);

    engine.seed("shijian");
    choose(&mut engine, "实践");
    engine.stage_selected_preference();
    engine.seed("实践 ");
    choose(&mut engine, "实践 ");
    confirm(&mut engine);
    assert_eq!(engine.preference_count(), 1);
    engine.undo().unwrap();
    assert_eq!(engine.preference_count(), 0);
}

#[test]
fn edited_adoptions_and_private_or_focus_boundaries_do_not_learn() {
    for reset in ["focus", "language", "private", "edit"] {
        let mut engine = engine("en", "hel");
        choose(&mut engine, "hello");
        engine.stage_selected_preference();
        engine.seed("hello");
        match reset {
            "focus" => {
                engine.clear_session_context();
            }
            "language" => {
                engine.set_language("zh-Hans");
                engine.set_language("en");
            }
            "private" => {
                engine.set_preferences_enabled(false);
            }
            _ => {
                engine.seed("hellos");
            }
        }
        engine.seed("hello");
        choose(&mut engine, "hello");
        confirm(&mut engine);
        assert_eq!(engine.preference_count(), 0, "{reset}");
    }
}

#[test]
fn privacy_suppresses_reading_and_writing_but_retains_public_cache() {
    let mut engine = engine("zh-Hans", "shijian");
    for _ in 0..8 {
        choose(&mut engine, "实践");
        confirm(&mut engine);
        engine.seed("shijian");
    }
    assert_eq!(engine.candidates()[0].text, "实践");
    engine.set_preferences_enabled(false);
    engine.seed("shijian");
    assert_eq!(engine.candidates()[0].text, "时间");
    choose(&mut engine, "事件");
    confirm(&mut engine);
    assert_eq!(engine.preference_count(), 1);
    engine.set_preferences_enabled(true);
    engine.seed("shijian");
    assert_eq!(engine.candidates()[0].text, "实践");
}

#[test]
fn unselected_default_does_not_self_reinforce_and_new_engines_start_empty() {
    let mut ime = engine("zh-Hans", "shijian");
    for _ in 0..8 {
        confirm(&mut ime);
        ime.seed("shijian");
    }
    assert_eq!(ime.preference_count(), 0);
    choose(&mut ime, "实践");
    confirm(&mut ime);
    assert_eq!(ime.preference_count(), 1);
    assert_eq!(engine("zh-Hans", "shijian").preference_count(), 0);
}

#[test]
fn sentences_learn_separately_and_long_drafts_are_not_cached() {
    let mut ime = engine("en", "hel");
    let target = ime
        .candidates()
        .iter()
        .filter(|c| c.kind == CandidateKind::Sentence)
        .nth(1)
        .unwrap()
        .text
        .clone();
    for _ in 0..8 {
        choose(&mut ime, &target);
        confirm(&mut ime);
        ime.clear_session_context();
        ime.seed("hel");
    }
    assert_eq!(ime.candidates()[0].text, "hel");
    assert_eq!(ime.candidates()[1].kind, CandidateKind::Word);
    assert_eq!(
        ime.candidates()
            .iter()
            .find(|c| c.kind == CandidateKind::Sentence)
            .unwrap()
            .text,
        target
    );
    ime.clear_preferences();
    let seed = format!("{} hel", "a ".repeat(600));
    ime.seed(&seed);
    let selected = format!("{}hello", &seed[..seed.len() - 3]);
    choose(&mut ime, &selected);
    confirm(&mut ime);
    assert_eq!(ime.preference_count(), 0);
}

#[test]
fn english_adoption_requires_a_word_boundary_at_commit() {
    for (text, learned) in [
        ("hello2", false),
        ("hello_name", false),
        ("hello\u{301}", false),
        ("hello-world", false),
        ("hello's", false),
        ("hello ", true),
        ("hello!", true),
        ("hello, world", true),
        ("hello。", true),
    ] {
        let mut ime = engine("en", "hel");
        choose(&mut ime, "hello");
        ime.stage_selected_preference();
        ime.seed("hello");
        ime.seed(text);
        choose(&mut ime, text);
        confirm(&mut ime);
        assert_eq!(ime.preference_count(), usize::from(learned), "{text}");
    }
}

fn assert_incremental_punctuation_learning(suffix: &str, learned: bool) {
    let mut ime = engine("en", "hel");
    choose(&mut ime, "hello");
    assert_eq!(
        ime.candidates()[ime.snapshot().selected_index].kind,
        CandidateKind::Word
    );
    ime.stage_selected_preference();
    assert_eq!(ime.preference_count(), 0, "adoption is not confirmation");

    let mut draft = String::from("hello");
    ime.seed(&draft);
    for ch in suffix.chars() {
        draft.push(ch);
        ime.seed(&draft);
        assert_eq!(
            ime.preference_count(),
            0,
            "typing {draft:?} must not confirm the pending choice"
        );
        assert!(ime.snapshot().committed_text.is_empty());
    }

    // Confirm the final literal draft, not another offered completion. A dot
    // or colon may be a sentence boundary until the following character turns
    // it into part of the same URL/identifier token.
    choose(&mut ime, &draft);
    assert_eq!(ime.preference_count(), 0);
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(draft.as_str()));
    assert_eq!(ime.preference_count(), usize::from(learned), "{draft:?}");
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert_eq!(
        ime.preference_count(),
        0,
        "undo must reverse learned feedback"
    );
}

#[test]
fn adopted_english_words_extended_into_tokens_do_not_learn() {
    for suffix in [
        ".com", ":world", "..world", "::world", "://world", ".0", ".\u{301}", "-world", "–world",
        "'s", "’s", "'re", "’re",
    ] {
        assert_incremental_punctuation_learning(suffix, false);
    }
}

#[test]
fn adopted_english_words_with_real_punctuation_boundaries_still_learn() {
    for suffix in [
        ".",
        ". world",
        ":",
        ": world",
        "...",
        "... world",
        "!",
        "!\"",
        "!\" world",
        ".\") world",
        "。world",
        "—world",
        "——继续",
        "，world",
        "：world",
    ] {
        assert_incremental_punctuation_learning(suffix, true);
    }
}

#[test]
fn adopted_english_words_before_smart_closing_double_quotes_still_learn() {
    for suffix in [
        "\"",
        "\" world",
        "”",
        "” world",
        ".”",
        ".” world",
        "!”",
        "!” world",
    ] {
        assert_incremental_punctuation_learning(suffix, true);
    }
}

#[test]
fn discarded_english_adoptions_do_not_revive_when_the_spelling_is_restored() {
    for drafts in [
        ["hello.", "hello.c", "hello."],
        ["hello:", "hello:w", "hello:"],
        ["hello", "hel", "hello."], // Undo adoption, then type the same spelling.
    ] {
        let mut ime = engine("en", "hel");
        choose(&mut ime, "hello");
        ime.stage_selected_preference();
        ime.seed("hello");
        for draft in drafts {
            ime.seed(draft);
            assert_eq!(ime.preference_count(), 0);
            assert!(ime.snapshot().committed_text.is_empty());
        }
        choose(&mut ime, drafts[2]);
        let result = ime.commit(CommitOptions { force: true });
        assert!(result.ok);
        assert_eq!(result.text.as_deref(), Some(drafts[2]));
        assert_eq!(ime.preference_count(), 0, "{drafts:?}");
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert_eq!(ime.preference_count(), 0);
    }
}
