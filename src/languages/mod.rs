pub mod chinese;
pub mod english;
pub mod japanese;
pub mod llama;
pub mod llm;

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
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .filter(|text| !text.is_empty() && seen.insert(text.clone()))
        .enumerate()
        .map(|(index, text)| crate::ime::Candidate {
            label: text.clone(),
            text,
            score: confidence + 1.0 - index as f32 * 0.02,
        })
        .collect()
}
