//! Coalesce unsent keyboard edits, never replay an uncertain write or cross fields.
use suzaku_map::ime::companion::{MAX_TEXT_BYTES, NativeComposition};

pub(super) struct NativeTyping {
    host: String,
    context: u64,
    language: String,
    base: String,
    pub draft: String,
    flight: Option<Flight>,
    pub blocked: bool,
}

struct Flight {
    revision: u64,
    text: String,
    acknowledged: bool,
}

impl NativeTyping {
    pub fn new(frame: &NativeComposition) -> Self {
        Self {
            host: frame.host.clone(),
            context: frame.context,
            language: frame.language.clone(),
            base: frame.seed.clone(),
            draft: frame.seed.clone(),
            flight: None,
            blocked: false,
        }
    }

    pub fn matches(&self, frame: &NativeComposition) -> bool {
        frame.focused
            && !frame.private
            && self.host == frame.host
            && self.context == frame.context
            && self.language == frame.language
    }

    pub fn edit(&mut self, text: Option<&str>) -> bool {
        if let Some(text) = text {
            if self.draft.len().saturating_add(text.len()) > MAX_TEXT_BYTES
                || text.chars().any(char::is_control)
            {
                return false;
            }
            self.draft.push_str(text);
        } else {
            self.draft.pop();
        }
        true
    }

    pub fn observe(&mut self, frame: &NativeComposition) {
        if self.blocked {
            return;
        }
        if let Some(flight) = &self.flight {
            if frame.revision > flight.revision && frame.seed == flight.text {
                if flight.acknowledged {
                    self.base = flight.text.clone();
                    self.flight = None;
                }
            } else if frame.seed != self.base
                || (flight.acknowledged && frame.revision > flight.revision)
            {
                // A successful replacement must have a matching newer state.
                // The latest-only mailbox can skip that state if a physical
                // edit restores `base`. Keep recovery text, never wait forever
                // or replay it over the external edit.
                self.blocked = true;
            }
        } else if frame.seed != self.base {
            self.blocked = true;
        }
    }

    pub fn acknowledge(&mut self, revision: u64, applied: bool) -> bool {
        let Some(flight) = &mut self.flight else {
            return false;
        };
        if flight.revision != revision {
            return false;
        }
        flight.acknowledged = applied;
        self.blocked |= !applied;
        true
    }

    pub fn ready(&self, frame: &NativeComposition) -> bool {
        self.matches(frame)
            && !self.blocked
            && self.flight.is_none()
            && self.base == frame.seed
            && self.draft != self.base
    }

    pub fn sent(&mut self, revision: u64) {
        self.flight = Some(Flight {
            revision,
            text: self.draft.clone(),
            acknowledged: false,
        });
    }

    pub fn settled(&self) -> bool {
        !self.blocked && self.flight.is_none() && self.draft == self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(text: &str, revision: u64) -> NativeComposition {
        NativeComposition {
            host: "same host".into(),
            context: 3,
            revision,
            focused: true,
            private: false,
            language: "en".into(),
            seed: text.into(),
            selected: 0,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn unsent_edits_wait_for_both_ack_and_matching_frame_in_either_order() {
        for frame_first in [true, false] {
            let mut typing = NativeTyping::new(&frame("hel", 10));
            assert!(typing.edit(Some("l")));
            typing.sent(10);
            assert!(typing.edit(Some("o")));
            assert_eq!(typing.draft, "hello");
            let next = frame("hell", 11);
            if frame_first {
                typing.observe(&next);
                assert!(!typing.ready(&next));
            }
            assert!(typing.acknowledge(10, true));
            assert!(!typing.ready(&next));
            typing.observe(&next);
            assert!(typing.ready(&next));
            typing.sent(11);
            assert!(typing.acknowledge(11, true));
            typing.observe(&frame("hello", 12));
            assert!(typing.settled());
        }
    }

    #[test]
    fn acknowledged_edit_waits_for_a_newer_frame_without_replaying_or_blocking_on_the_old_one() {
        let initial = frame("hel", 10);
        let mut typing = NativeTyping::new(&initial);
        typing.edit(Some("l"));
        typing.sent(10);
        typing.edit(Some("o"));
        assert!(typing.acknowledge(10, true));
        typing.observe(&initial);
        assert!(!typing.blocked);
        assert!(!typing.ready(&initial));
        assert!(!typing.settled());
        let applied = frame("hell", 11);
        typing.observe(&applied);
        assert!(typing.ready(&applied));
        assert_eq!(typing.draft, "hello");
    }

    #[test]
    fn deletion_and_unicode_are_lossless_and_uncertain_or_diverged_edits_never_retry() {
        let mut typing = NativeTyping::new(&frame("hello", 4));
        assert!(typing.edit(Some(" 日本😀")));
        assert!(typing.edit(None));
        assert_eq!(typing.draft, "hello 日本");
        assert!(!typing.edit(Some(&"x".repeat(MAX_TEXT_BYTES))));
        assert!(!typing.edit(Some("\n")));
        typing.sent(4);
        assert!(!typing.acknowledge(3, true));
        assert!(typing.acknowledge(4, false));
        typing.observe(&frame("hello 日本", 5));
        assert!(typing.blocked);
        assert!(!typing.ready(&frame("hello 日本", 5)));
        assert_eq!(typing.draft, "hello 日本");
        let mut typing = NativeTyping::new(&frame("hel", 10));
        typing.edit(Some("l"));
        typing.sent(10);
        typing.observe(&frame("changed by physical keyboard", 11));
        assert!(typing.blocked);
        let mut other = frame("hel", 12);
        other.context += 1;
        assert!(!typing.matches(&other));
        other.context -= 1;
        other.private = true;
        assert!(!typing.matches(&other));
    }
}
