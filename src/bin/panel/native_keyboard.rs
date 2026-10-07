//! Ordered host-owned edit operations. Never predict the result of undo or Space.
use std::collections::VecDeque;
use suzaku_map::ime::companion::{MAX_TEXT_BYTES, NativeComposition};

const MAX_QUEUED_EDITS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum KeyboardEdit {
    Text(String),
    Backspace,
    Continue,
}

pub(super) struct NativeKeyboardQueue {
    target: NativeComposition,
    last_public_seed: String,
    edits: VecDeque<KeyboardEdit>,
    text_bytes: usize,
    pub flight: Option<NativeComposition>,
    pub blocked: bool,
}

impl NativeKeyboardQueue {
    pub fn new(frame: &NativeComposition) -> Self {
        Self {
            target: frame.clone(),
            last_public_seed: frame.seed.clone(),
            edits: VecDeque::new(),
            text_bytes: 0,
            flight: None,
            blocked: false,
        }
    }

    pub fn matches(&self, frame: &NativeComposition) -> bool {
        frame.focused
            && !frame.private
            && self.target.host == frame.host
            && self.target.context == frame.context
            && self.target.language == frame.language
    }

    pub fn observe(&mut self, frame: &NativeComposition) {
        if self.matches(frame) {
            self.last_public_seed.clone_from(&frame.seed);
        }
    }

    pub fn recovery_seed(&self) -> &str {
        &self.last_public_seed
    }

    pub fn push(&mut self, edit: KeyboardEdit) -> bool {
        let bytes = match &edit {
            KeyboardEdit::Text(text) => {
                if text.chars().any(char::is_control) {
                    return false;
                }
                text.len()
            }
            _ => 0,
        };
        if self.text_bytes.saturating_add(bytes) > MAX_TEXT_BYTES {
            return false;
        }
        if let KeyboardEdit::Text(text) = &edit
            && let Some(KeyboardEdit::Text(last)) = self.edits.back_mut()
        {
            last.push_str(text);
            self.text_bytes += bytes;
            return true;
        }
        if self.edits.len() >= MAX_QUEUED_EDITS {
            return false;
        }
        self.text_bytes += bytes;
        self.edits.push_back(edit);
        true
    }

    pub fn front(&self) -> Option<&KeyboardEdit> {
        self.edits.front()
    }

    pub fn pop(&mut self) -> Option<KeyboardEdit> {
        let edit = self.edits.pop_front()?;
        if let KeyboardEdit::Text(text) = &edit {
            self.text_bytes -= text.len();
        }
        Some(edit)
    }

    pub fn has_text(&self) -> bool {
        self.text_bytes != 0
    }

    /// Recovery is an explicit local edit, never a replay to the native field.
    /// Keep literal follow-up text, without pretending unknown B/S succeeded.
    pub fn append_recovery_text(&self, text: &mut String) {
        for edit in &self.edits {
            if let KeyboardEdit::Text(suffix) = edit {
                text.push_str(suffix);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> NativeComposition {
        NativeComposition {
            host: "host".into(),
            context: 1,
            revision: 2,
            focused: true,
            private: false,
            cursor: None,
            language: "en".into(),
            seed: "hello".into(),
            selected: 0,
            candidates: vec![],
        }
    }

    #[test]
    fn semantic_order_and_literal_coalescing_are_preserved() {
        let mut queue = NativeKeyboardQueue::new(&frame());
        for edit in [
            KeyboardEdit::Continue,
            KeyboardEdit::Backspace,
            KeyboardEdit::Text("日".into()),
            KeyboardEdit::Text("本".into()),
            KeyboardEdit::Continue,
        ] {
            assert!(queue.push(edit));
        }
        for edit in [
            KeyboardEdit::Continue,
            KeyboardEdit::Backspace,
            KeyboardEdit::Text("日本".into()),
            KeyboardEdit::Continue,
        ] {
            assert_eq!(queue.pop(), Some(edit));
        }
        assert!(queue.front().is_none());
        assert!(!queue.has_text());
    }

    #[test]
    fn bounds_reject_newest_without_dropping_or_truncating_earlier_edits() {
        let mut queue = NativeKeyboardQueue::new(&frame());
        for _ in 0..MAX_QUEUED_EDITS {
            assert!(queue.push(KeyboardEdit::Backspace));
        }
        assert!(!queue.push(KeyboardEdit::Continue));
        for _ in 0..MAX_QUEUED_EDITS {
            assert_eq!(queue.pop(), Some(KeyboardEdit::Backspace));
        }
        let text = "x".repeat(MAX_TEXT_BYTES);
        assert!(queue.push(KeyboardEdit::Text(text.clone())));
        assert!(!queue.push(KeyboardEdit::Text("日".into())));
        assert!(!queue.push(KeyboardEdit::Text("\n".into())));
        assert_eq!(queue.pop(), Some(KeyboardEdit::Text(text)));
        assert!(!queue.has_text());
    }

    #[test]
    fn recovery_keeps_literals_without_guessing_semantic_results_or_crossing_targets() {
        let initial = frame();
        let mut queue = NativeKeyboardQueue::new(&initial);
        queue.push(KeyboardEdit::Backspace);
        queue.push(KeyboardEdit::Text("x".into()));
        queue.push(KeyboardEdit::Continue);
        queue.push(KeyboardEdit::Text("日".into()));
        let mut text = initial.seed.clone();
        queue.append_recovery_text(&mut text);
        assert_eq!(text, "hellox日");
        for boundary in 0..5 {
            let mut next = initial.clone();
            match boundary {
                0 => next.host.push('x'),
                1 => next.context += 1,
                2 => next.language = "ja".into(),
                3 => next.private = true,
                _ => next.focused = false,
            }
            assert!(!queue.matches(&next));
        }
        let mut next = initial;
        next.revision += 1;
        next.seed.clear();
        assert!(queue.matches(&next));
    }

    #[test]
    fn recovery_base_tracks_public_frames_not_the_initial_seed_or_an_uncertain_flight() {
        let mut queue = NativeKeyboardQueue::new(&frame());
        assert_eq!(queue.recovery_seed(), "hello");
        let mut applied = frame();
        applied.revision += 1;
        applied.seed = "hello ".into();
        queue.observe(&applied);
        queue.flight = Some(applied.clone());
        queue.blocked = true;
        queue.flight = None; // The failed ACK must not erase the confirmed base.
        assert_eq!(queue.recovery_seed(), "hello ");
        let mut foreign = applied.clone();
        foreign.context += 1;
        foreign.seed = "other field".into();
        queue.observe(&foreign);
        assert_eq!(queue.recovery_seed(), "hello ");
        applied.private = true;
        applied.seed.clear();
        queue.observe(&applied);
        assert_eq!(queue.recovery_seed(), "hello ");
    }
}
