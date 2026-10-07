//! Committed history must undo exact prefixes without changing editor joins.
use suzaku_map::ime::{CommitOptions, EngineConfig, XRTabletImeEngine};

fn engine(language: &str, initial: &str, mixed: bool) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        initial_text: initial.into(),
        ..Default::default()
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    engine
}

fn select_literal(engine: &mut XRTabletImeEngine, text: &str) {
    engine.seed(text);
    let index = engine
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text)
        .expect("the literal draft must remain available");
    engine.select_candidate(index);
}

fn commit_literal(engine: &mut XRTabletImeEngine, text: &str, expected: &str) {
    select_literal(engine, text);
    let result = engine.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(expected));
    assert_eq!(result.snapshot.committed_text, expected);
    assert_eq!(engine.snapshot().committed_text, expected);
    assert!(result.snapshot.seed_text.is_empty());
}

#[test]
fn multilingual_commits_undo_exact_unicode_and_whitespace_prefixes() {
    for (language, separator) in [("en", " "), ("zh-Hans", ""), ("ja", "")] {
        for mixed in [false, true] {
            for initial in ["", "前文😀\u{3000}"] {
                let mut engine = engine(language, initial, mixed);
                let mut expected = initial.to_owned();
                let mut previous = Vec::new();
                for chunk in ["hello", "世界", " café\u{301} ", "👩‍💻", "\u{3000}  ", "”尾"]
                {
                    previous.push(expected.clone());
                    if !expected.is_empty() {
                        expected.push_str(separator);
                    }
                    expected.push_str(chunk);
                    commit_literal(&mut engine, chunk, &expected);
                    assert!(!engine.commit(CommitOptions { force: true }).ok);
                }
                for expected in previous.into_iter().rev() {
                    assert_eq!(engine.undo().unwrap().committed_text, expected);
                }
                assert_eq!(engine.snapshot().committed_text, initial);
                assert!(engine.undo().is_none());
            }
        }
    }
}

#[test]
fn committing_after_undo_branches_from_the_restored_utf8_prefix() {
    for (language, separator) in [("en", " "), ("zh-Hans", "")] {
        let initial = "前😀";
        let mut engine = engine(language, initial, true);
        let first = format!("{initial}{separator}é");
        let discarded = format!("{first}{separator}旧👩‍💻");
        commit_literal(&mut engine, "é", &first);
        commit_literal(&mut engine, "旧👩‍💻", &discarded);
        assert_eq!(engine.undo().unwrap().committed_text, first);

        let replacement = format!("{first}{separator}新\u{3000}");
        commit_literal(&mut engine, "新\u{3000}", &replacement);
        assert_eq!(engine.undo().unwrap().committed_text, first);
        assert_eq!(engine.undo().unwrap().committed_text, initial);
        assert!(engine.undo().is_none());
    }
}

#[test]
fn failed_commits_and_cleared_context_do_not_leave_stale_undo_boundaries() {
    let mut engine = engine("en", "初😀", true);
    commit_literal(&mut engine, "é", "初😀 é");
    select_literal(&mut engine, "a replacement that requires confirmation");
    assert!(!engine.commit(CommitOptions::default()).ok);
    assert_eq!(engine.undo().unwrap().committed_text, "初😀");
    assert!(engine.undo().is_none());

    commit_literal(&mut engine, "旧内容", "初😀 旧内容");
    engine.clear_prediction_context();
    assert!(engine.snapshot().committed_text.is_empty());
    assert!(engine.undo().is_none());
    commit_literal(&mut engine, "新", "新");
    assert!(engine.undo().unwrap().committed_text.is_empty());
    assert!(engine.undo().is_none());

    commit_literal(&mut engine, "別", "別");
    engine.clear_session_context();
    assert!(engine.snapshot().committed_text.is_empty());
    assert!(engine.undo().is_none());
}

#[test]
fn commit_undo_depth_is_not_capped_by_the_preference_receipt_limit() {
    let initial = "起点😀";
    let mut engine = engine("en", initial, true);
    let mut expected = initial.to_owned();
    let mut previous = Vec::new();
    for index in 0..130 {
        previous.push(expected.clone());
        let chunk = format!("第{index}😀");
        expected.push(' ');
        expected.push_str(&chunk);
        commit_literal(&mut engine, &chunk, &expected);
    }
    for expected in previous.into_iter().rev() {
        assert_eq!(engine.undo().unwrap().committed_text, expected);
    }
    assert_eq!(engine.snapshot().committed_text, initial);
    assert!(engine.undo().is_none());
}
