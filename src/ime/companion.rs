//! Versioned, bounded view of the active native composition. No surrounding text
//! or committed history crosses this channel; private contexts are always blank.
use super::{Candidate, InputSource, Mode, Snapshot};
use crate::languages::BuiltinLanguage;
use serde_json::{Value, json};

pub const MAX_FRAME_BYTES: usize = 65536;
pub const MAX_TEXT_BYTES: usize = 8192;
pub const MAX_CANDIDATES: usize = 32;

#[derive(Clone, Debug)]
pub enum NativeOperation {
    Commit(usize),
    Select(usize),
    Replace(String),
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeCandidate {
    pub text: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeComposition {
    pub host: String,
    pub context: u64,
    pub revision: u64,
    pub focused: bool,
    pub private: bool,
    pub language: String,
    pub seed: String,
    pub selected: usize,
    pub candidates: Vec<NativeCandidate>,
}

impl NativeComposition {
    pub fn visible(&self) -> bool {
        self.focused && !self.private && !self.seed.is_empty()
    }

    pub fn to_json(&self) -> Value {
        json!({"version":1,"host":self.host,"context":self.context,"revision":self.revision,
            "focused":self.focused,"private":self.private,"language":self.language,
            "seed":self.seed,"selected":self.selected,"candidates":self.candidates.iter()
                .map(|c| json!({"text":c.text,"label":c.label})).collect::<Vec<_>>()})
    }

    pub fn parse(raw: &[u8]) -> Result<Self, String> {
        if raw.len() > MAX_FRAME_BYTES {
            return Err("Native frame too large".into());
        }
        let v: Value = serde_json::from_slice(raw).map_err(|_| "Invalid native frame")?;
        let text = |value: &Value| -> Result<String, String> {
            let s = value.as_str().ok_or("Missing native text")?;
            if s.len() > MAX_TEXT_BYTES || s.chars().any(char::is_control) {
                return Err("Invalid native text".into());
            }
            Ok(s.into())
        };
        if v["version"] != 1 {
            return Err("Unsupported native protocol".into());
        }
        let host = text(&v["host"])?;
        if host.len() != 36 || !host.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
            return Err("Invalid host identity".into());
        }
        let language = text(&v["language"])?;
        if BuiltinLanguage::resolve(&language).is_none() {
            return Err("Unsupported native language".into());
        }
        let entries = v["candidates"]
            .as_array()
            .ok_or("Missing native candidates")?;
        if entries.len() > MAX_CANDIDATES {
            return Err("Too many native candidates".into());
        }
        let candidates = entries
            .iter()
            .map(|c| {
                Ok(NativeCandidate {
                    text: text(&c["text"])?,
                    label: text(&c["label"])?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let frame = Self {
            host,
            context: v["context"].as_u64().ok_or("Missing context")?,
            revision: v["revision"].as_u64().ok_or("Missing revision")?,
            focused: v["focused"].as_bool().ok_or("Missing focus")?,
            private: v["private"].as_bool().ok_or("Missing privacy")?,
            language,
            seed: text(&v["seed"])?,
            selected: v["selected"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("Invalid selection")?,
            candidates,
        };
        if (!frame.focused || frame.private || frame.seed.is_empty())
            && (!frame.seed.is_empty() || !frame.candidates.is_empty())
        {
            return Err("Non-public native composition was not redacted".into());
        }
        if frame.selected >= frame.candidates.len().max(1) {
            return Err("Invalid native selection".into());
        }
        Ok(frame)
    }

    pub fn engine_candidates(&self) -> Vec<Candidate> {
        self.candidates
            .iter()
            .map(|c| Candidate {
                text: c.text.clone(),
                label: c.label.clone(),
                score: 1.0,
            })
            .collect()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            mode: if self.visible() {
                Mode::Composing
            } else {
                Mode::Idle
            },
            seed_text: self.seed.clone(),
            active_language: self.language.clone(),
            selected_index: self.selected,
            draft_text: self
                .candidates
                .get(self.selected)
                .map(|c| c.text.clone())
                .unwrap_or_default(),
            committed_text: String::new(),
            active_source: InputSource::HardwareKeyboard,
            candidate_labels: self.candidates.iter().map(|c| c.label.clone()).collect(),
            confidence: 1.0,
            degraded: false,
            warnings: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn sample() -> NativeComposition {
        NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 1,
            revision: 2,
            focused: true,
            private: false,
            language: "en".into(),
            seed: "hel".into(),
            selected: 0,
            candidates: vec![NativeCandidate {
                text: "hello".into(),
                label: "hello · AI".into(),
            }],
        }
    }
    #[test]
    fn native_frame_roundtrip_preserves_full_candidates_and_selection() {
        let frame = sample();
        assert_eq!(
            NativeComposition::parse(frame.to_json().to_string().as_bytes()).unwrap(),
            frame
        );
        assert_eq!(frame.snapshot().candidate_labels, vec!["hello · AI"]);
        assert_eq!(frame.snapshot().committed_text, "");
    }
    #[test]
    fn rejects_unredacted_private_unfocused_and_empty_frames() {
        for change in ["private", "focused", "seed"] {
            let mut value = sample().to_json();
            value[change] = match change {
                "private" => json!(true),
                "focused" => json!(false),
                _ => json!(""),
            };
            assert!(NativeComposition::parse(value.to_string().as_bytes()).is_err());
        }
    }
    #[test]
    fn rejects_invalid_versions_indices_text_and_oversized_frames() {
        for (key, value) in [
            ("version", json!(2)),
            ("selected", json!(22)),
            ("seed", json!("secret\n")),
            ("host", json!("bad")),
            ("language", json!("unknown")),
        ] {
            let mut raw = sample().to_json();
            raw[key] = value;
            assert!(NativeComposition::parse(raw.to_string().as_bytes()).is_err());
        }
        assert!(NativeComposition::parse(&vec![b' '; MAX_FRAME_BYTES + 1]).is_err());
    }
}
