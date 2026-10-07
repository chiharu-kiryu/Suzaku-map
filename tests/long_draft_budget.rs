//! Verify work budgets by observing plugin calls, not machine-dependent timings.
use std::sync::{Arc, Mutex};

use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{Candidate, CommitOptions, EngineConfig, LanguagePlugin, XRTabletImeEngine};
use suzaku_map::languages::{chinese::ChineseLanguagePlugin, english::EnglishLanguagePlugin};

type Calls = Arc<Mutex<Vec<(&'static str, String)>>>;

struct RecordingPlugin {
    inner: Box<dyn LanguagePlugin>,
    calls: Calls,
}

impl LanguagePlugin for RecordingPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn normalize_seed(&self, input: &str) -> String {
        self.inner.normalize_seed(input)
    }

    fn expand_token(&self, token: &str, degraded: bool) -> Vec<String> {
        self.inner.expand_token(token, degraded)
    }

    fn direct_candidates_with_context(
        &self,
        seed: &str,
        context: &str,
        confidence: f32,
    ) -> Option<Vec<Candidate>> {
        self.calls.lock().unwrap().push(("direct", seed.into()));
        self.inner
            .direct_candidates_with_context(seed, context, confidence)
    }

    fn build_candidates(&self, parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        self.calls.lock().unwrap().push(("build", seed.into()));
        self.inner.build_candidates(parts, seed, confidence)
    }
}

/// Exercise both a direct decoder and the generic token-combination fallback.
struct JapaneseProbe {
    direct: bool,
}

impl LanguagePlugin for JapaneseProbe {
    fn id(&self) -> &str {
        "ja"
    }

    fn normalize_seed(&self, input: &str) -> String {
        input.into()
    }

    fn expand_token(&self, token: &str, _degraded: bool) -> Vec<String> {
        vec![token.into()]
    }

    fn direct_candidates(&self, seed: &str, confidence: f32) -> Option<Vec<Candidate>> {
        self.direct
            .then(|| self.build_candidates(&[], seed, confidence))
    }

    fn build_candidates(&self, _parts: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        let text = format!("{seed}候");
        vec![Candidate {
            label: text.clone(),
            text,
            score: confidence,
            kind: CandidateKind::Word,
            source: CandidateSource::Local,
        }]
    }
}

fn engine(inner: Box<dyn LanguagePlugin>, mixed: bool) -> (XRTabletImeEngine, Calls) {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: inner.id().into(),
        ..Default::default()
    });
    let calls = Calls::default();
    engine.register_language_plugin(RecordingPlugin {
        inner,
        calls: calls.clone(),
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    calls.lock().unwrap().clear();
    (engine, calls)
}

fn assert_literal(engine: &XRTabletImeEngine, expected: &str) {
    assert_eq!(engine.candidates().len(), 1);
    let candidate = &engine.candidates()[0];
    assert_eq!(candidate.text, expected);
    assert_eq!(candidate.kind, CandidateKind::Literal);
    assert_eq!(candidate.source, CandidateSource::Local);
    assert_eq!(candidate.score, 100.0);
    assert_eq!(engine.snapshot().seed_text, expected);
    assert_eq!(engine.snapshot().draft_text, expected);
    assert!(engine.snapshot().committed_text.is_empty());
}

fn assert_probe_calls(calls: &Calls, seed: &str, direct: bool) {
    let expected = if direct {
        vec![("direct", seed.to_owned())]
    } else {
        vec![("direct", seed.to_owned()), ("build", seed.to_owned())]
    };
    assert_eq!(*calls.lock().unwrap(), expected);
}

#[test]
fn mixed_over_budget_drafts_skip_discarded_direct_and_combinatorial_work() {
    for direct in [false, true] {
        for seed in [
            "a".repeat(257),
            "字".repeat(257),
            "😀".repeat(257),
            format!("  {}\u{3000}", "😀字a".repeat(400)),
        ] {
            let (mut engine, calls) = engine(Box::new(JapaneseProbe { direct }), true);
            engine.seed(&seed);
            assert_literal(&engine, &seed);
            assert!(
                calls.lock().unwrap().is_empty(),
                "over-budget candidates are discarded; direct={direct}, chars={}",
                seed.chars().count()
            );
        }
    }
}

#[test]
fn mixed_budget_counts_unicode_scalars_and_includes_the_exact_boundary() {
    for direct in [false, true] {
        for length in [255, 256] {
            let seed = "😀".repeat(length);
            assert!(seed.len() > 256);
            let (mut engine, calls) = engine(Box::new(JapaneseProbe { direct }), true);
            engine.seed(&seed);
            assert_probe_calls(&calls, &seed, direct);
            assert_eq!(engine.candidates()[0].text, format!("{seed}候"));
            assert_eq!(engine.snapshot().seed_text, seed);
            assert!(engine.snapshot().committed_text.is_empty());
        }
    }
}

#[test]
fn adoption_and_deletion_cross_the_budget_without_losing_editability_or_exact_commit() {
    let short = "😀".repeat(256);
    for direct in [false, true] {
        let (mut engine, calls) = engine(Box::new(JapaneseProbe { direct }), true);
        engine.seed(&short);
        engine.select_candidate(0);
        let adopted = engine.selected_completion_text(true).unwrap().to_owned();
        assert_eq!(adopted, format!("{short}候"));
        assert!(engine.snapshot().committed_text.is_empty());
        calls.lock().unwrap().clear();

        // Native adoption and deletion replace the editable seed; neither commits it.
        engine.seed(&adopted);
        assert_literal(&engine, &adopted);
        assert!(calls.lock().unwrap().is_empty());
        let mut deleted = adopted.clone();
        assert_eq!(deleted.pop(), Some('候'));
        engine.seed(&deleted);
        assert_eq!(deleted, short);
        assert_probe_calls(&calls, &short, direct);
        assert_eq!(engine.candidates()[0].text, adopted);
        assert!(engine.snapshot().committed_text.is_empty());

        calls.lock().unwrap().clear();
        engine.seed(&adopted);
        engine.select_candidate(0);
        let result = engine.commit(CommitOptions { force: true });
        assert!(result.ok);
        assert_eq!(result.text.as_deref(), Some(adopted.as_str()));
        assert_eq!(result.snapshot.committed_text, adopted);
        assert!(result.snapshot.seed_text.is_empty());
        assert!(calls.lock().unwrap().is_empty());
        assert!(engine.undo().unwrap().committed_text.is_empty());
        assert!(engine.undo().is_none());
    }
}

#[test]
fn non_mixed_hosts_keep_full_draft_plugin_semantics() {
    let seed = "😀".repeat(257);
    for direct in [false, true] {
        let (mut engine, calls) = engine(Box::new(JapaneseProbe { direct }), false);
        engine.seed(&seed);
        assert_probe_calls(&calls, &seed, direct);
        let expected = format!("{seed}候");
        assert_eq!(engine.candidates()[0].text, expected);
        assert_eq!(engine.snapshot().seed_text, seed);
        assert_eq!(
            engine.commit(CommitOptions { force: true }).text.as_deref(),
            Some(expected.as_str())
        );
    }
}

#[test]
fn english_and_chinese_long_drafts_still_invoke_bounded_tail_decoders() {
    type TailCase = (
        Box<dyn LanguagePlugin>,
        String,
        &'static str,
        &'static str,
        &'static str,
    );
    let cases: Vec<TailCase> = vec![
        (
            Box::new(EnglishLanguagePlugin),
            "前文😀 ".repeat(80),
            "please sen",
            "please send",
            "please send me the details.",
        ),
        (
            Box::new(ChineseLanguagePlugin),
            "前文😀。".repeat(80),
            "ni hao",
            "你好",
            "你好，很高兴认识你。",
        ),
    ];
    for (plugin, prefix, tail, word, sentence) in cases {
        let seed = format!("{prefix}{tail}");
        let (mut engine, calls) = engine(plugin, true);
        engine.seed(&seed);
        let recorded = calls.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "direct");
        assert!(recorded[0].1.chars().count() <= 256);
        assert!(recorded[0].1.ends_with(tail));
        assert_ne!(recorded[0].1, seed);
        for (ending, kind) in [
            (word, CandidateKind::Word),
            (sentence, CandidateKind::Sentence),
        ] {
            assert!(engine.candidates().iter().take(6).any(|candidate| {
                candidate.text == format!("{prefix}{ending}") && candidate.kind == kind
            }));
        }
        assert!(
            engine
                .candidates()
                .iter()
                .any(|candidate| candidate.text == seed)
        );
        assert!(engine.snapshot().committed_text.is_empty());
    }
}

#[test]
fn empty_and_whitespace_drafts_keep_existing_pre_plugin_handling() {
    let (mut engine, calls) = engine(Box::new(JapaneseProbe { direct: true }), true);
    for seed in [" ".repeat(257), "\u{3000}".repeat(257)] {
        engine.seed(&seed);
        assert_literal(&engine, &seed);
        assert!(calls.lock().unwrap().is_empty());
    }
    for seed in [String::new(), "\t".repeat(257)] {
        engine.seed(&seed);
        assert_eq!(engine.snapshot().seed_text, seed);
        assert!(engine.candidates().is_empty());
        assert!(calls.lock().unwrap().is_empty());
        assert!(!engine.commit(CommitOptions { force: true }).ok);
    }
}
