pub mod chinese;
pub mod english;
pub mod japanese;
pub mod llama;
pub mod llm;
pub mod model;
pub mod translation;

/// Language selection is independent of the prediction provider and host platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinLanguage {
    ChineseSimplified,
    English,
    Japanese,
}

impl BuiltinLanguage {
    pub const ALL: [Self; 3] = [Self::ChineseSimplified, Self::English, Self::Japanese];

    pub fn id(self) -> &'static str {
        match self {
            Self::ChineseSimplified => "zh-Hans",
            Self::English => "en",
            Self::Japanese => "ja",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ChineseSimplified => "中文（简体拼音）",
            Self::English => "English",
            Self::Japanese => "日本語（ローマ字）",
        }
    }

    pub fn resolve(id: &str) -> Option<Self> {
        match id.replace('_', "-").to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh-sg" | "zh-hans" | "zh-hans-cn" => Some(Self::ChineseSimplified),
            "en" | "en-us" | "en-gb" | "en-au" | "en-ca" => Some(Self::English),
            "ja" | "ja-jp" => Some(Self::Japanese),
            _ => None,
        }
    }
}

pub(crate) fn ranked_candidates(
    values: impl IntoIterator<Item = String>,
    confidence: f32,
) -> Vec<crate::ime::Candidate> {
    ranked_typed_candidates(
        values.into_iter().map(|text| (text, Default::default())),
        confidence,
    )
}

pub(crate) fn ranked_typed_candidates(
    values: impl IntoIterator<Item = (String, crate::ime::candidate_mix::CandidateKind)>,
    confidence: f32,
) -> Vec<crate::ime::Candidate> {
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .filter(|(text, _)| !text.is_empty() && seen.insert(text.clone()))
        .enumerate()
        .map(|(index, (text, kind))| crate::ime::Candidate {
            label: text.clone(),
            text,
            score: confidence + 1.0 - index as f32 * 0.02,
            kind,
            ..Default::default()
        })
        .collect()
}
