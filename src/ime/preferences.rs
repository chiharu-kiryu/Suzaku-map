//! Choice lifecycle adapter. The frequency store knows nothing about candidates
//! or dictionaries; this layer gates learning on successful composition commits.
use super::{Candidate, candidate_mix::CandidateKind};
use crate::preferences::{Feedback, PreferenceCache, PreferenceKey};
use std::{collections::VecDeque, time::Instant};

const MAX_PENDING: usize = 32;
const MAX_UNDO: usize = 64;

fn english_word_boundary(tail: &str) -> bool {
    // Periods and colons are provisional: later typing can make them part of
    // a URL/identifier. Check every new draft, including punctuation runs,
    // without discarding ordinary sentence endings or closing punctuation.
    tail.trim_start_matches(['.', ':'])
        .chars()
        .next()
        .is_none_or(|ch| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    ',' | ';'
                        | '!'
                        | '?'
                        | ')'
                        | ']'
                        | '}'
                        | '"'
                        | '”'
                        | '—'
                        | '、'
                        | '，'
                        | '。'
                        | '？'
                        | '！'
                        | '；'
                        | '：'
                        | '）'
                        | '」'
                        | '』'
                )
        })
}

struct PendingChoice {
    key: PreferenceKey,
    prefix: PreferenceKey,
    bytes: usize,
    english_word: bool,
}

pub(super) struct SelectionPreferences {
    cache: PreferenceCache,
    enabled: bool,
    pending: Vec<PendingChoice>,
    undo: VecDeque<Vec<Feedback>>,
}

impl Default for SelectionPreferences {
    fn default() -> Self {
        Self {
            cache: PreferenceCache::default(),
            enabled: true,
            pending: Vec::new(),
            undo: VecDeque::new(),
        }
    }
}

impl SelectionPreferences {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.clear_pending();
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.clear_pending();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn clear_pending(&mut self) {
        self.pending.clear();
        self.undo.clear();
    }

    pub fn retain_draft(&mut self, draft: &str) {
        self.pending.retain(|choice| {
            draft
                .get(..choice.bytes)
                .is_some_and(|prefix| self.cache.fingerprint(&prefix) == choice.prefix)
                && (!choice.english_word || english_word_boundary(&draft[choice.bytes..]))
        });
    }

    pub fn stage(&mut self, language: &str, query: &str, candidate: &Candidate) {
        if !self.enabled
            || candidate.text == query
            || !matches!(
                candidate.kind,
                CandidateKind::Word | CandidateKind::Sentence
            )
        {
            return;
        }
        let Some(key) = self.cache.key(language, query, &candidate.text) else {
            return;
        };
        if self.pending.iter().any(|choice| choice.key == key) {
            return;
        }
        if self.pending.len() == MAX_PENDING {
            self.pending.remove(0);
        }
        self.pending.push(PendingChoice {
            key,
            prefix: self.cache.fingerprint(&candidate.text.as_str()),
            bytes: candidate.text.len(),
            english_word: language == "en" && candidate.kind == CandidateKind::Word,
        });
    }

    pub fn commit(&mut self, text: &str) {
        self.retain_draft(text);
        let now = Instant::now();
        let feedback = self
            .pending
            .drain(..)
            .map(|choice| self.cache.record(choice.key, now))
            .collect();
        if self.undo.len() == MAX_UNDO {
            self.undo.pop_front();
        }
        self.undo.push_back(feedback);
    }

    pub fn undo(&mut self) {
        if let Some(feedback) = self.undo.pop_back() {
            for receipt in feedback.into_iter().rev() {
                self.cache.undo(receipt);
            }
        }
    }

    pub fn bonus(&self, language: &str, query: &str, candidate: &Candidate, now: Instant) -> f32 {
        if !self.enabled
            || !matches!(
                candidate.kind,
                CandidateKind::Word | CandidateKind::Sentence
            )
        {
            return 0.0;
        }
        self.cache
            .key(language, query, &candidate.text)
            .map_or(0.0, |key| self.cache.bonus(key, now))
    }
}
