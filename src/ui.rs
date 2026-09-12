//! Application chrome localization. Independent of IME and translation languages.
use std::{borrow::Cow, collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiLanguage {
    #[default]
    System,
    English,
    ChineseSimplified,
    Japanese,
    Korean,
    Spanish,
    French,
    German,
    Portuguese,
}

impl UiLanguage {
    pub const ALL: [Self; 9] = [
        Self::System,
        Self::English,
        Self::ChineseSimplified,
        Self::Japanese,
        Self::Korean,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::Portuguese,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::ChineseSimplified => "zh-Hans",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Portuguese => "pt",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.id() == id)
    }

    /// Native names stay recognizable even after an accidental language switch.
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::English => "English",
            Self::ChineseSimplified => "简体中文",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Portuguese => "Português",
        }
    }

    pub fn from_locale(locale: &str) -> Option<Self> {
        let base = locale
            .split(['_', '-', '.', '@'])
            .next()?
            .to_ascii_lowercase();
        match base.as_str() {
            "en" | "c" | "posix" => Some(Self::English),
            "zh" => Some(Self::ChineseSimplified),
            "ja" => Some(Self::Japanese),
            "ko" => Some(Self::Korean),
            "es" => Some(Self::Spanish),
            "fr" => Some(Self::French),
            "de" => Some(Self::German),
            "pt" => Some(Self::Portuguese),
            _ => None,
        }
    }

    pub fn resolved(self) -> Self {
        if self != Self::System {
            return self;
        }
        static SYSTEM: OnceLock<UiLanguage> = OnceLock::new();
        *SYSTEM.get_or_init(|| {
            // Resolve once outside the render hot path. No settings or model requests.
            let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
                .into_iter()
                .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()));
            locale
                .as_deref()
                .and_then(Self::from_locale)
                .unwrap_or(Self::English)
        })
    }

    pub fn tr<'a>(self, key: &'a str) -> &'a str {
        let index = match self.resolved() {
            Self::ChineseSimplified => 0,
            Self::Japanese => 1,
            Self::Korean => 2,
            Self::Spanish => 3,
            Self::French => 4,
            Self::German => 5,
            Self::Portuguese => 6,
            _ => return key,
        };
        catalog()
            .get(key)
            .map(|values| values[index])
            .filter(|v| !v.is_empty())
            .unwrap_or(key)
    }

    /// Only pass application-owned messages, never drafts, candidates or model output.
    /// A single `{}` slot retains its payload verbatim (paths, model IDs, errors, etc.).
    pub fn message<'a>(self, source: &'a str) -> Cow<'a, str> {
        if catalog().contains_key(source) {
            return Cow::Borrowed(self.tr(source));
        }
        // Prefer the outer message prefix over generic suffix templates. Otherwise
        // e.g. a sent draft ending in " settings" could be mistaken for a tab hint.
        let matched = catalog()
            .keys()
            .filter_map(|&key| {
                let (prefix, suffix) = key.split_once("{}")?;
                let value = source.strip_prefix(prefix)?.strip_suffix(suffix)?;
                Some((key, value, prefix.len(), suffix.len()))
            })
            .max_by_key(|(_, _, prefix, suffix)| (*prefix, *suffix));
        if let Some((key, value, _, _)) = matched {
            return Cow::Owned(self.tr(key).replace("{}", value));
        }
        Cow::Borrowed(source)
    }
}

fn catalog() -> &'static HashMap<&'static str, [&'static str; 7]> {
    static CATALOG: OnceLock<HashMap<&'static str, [&'static str; 7]>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("ui/catalog.tsv")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let mut fields = line.split('\t');
                let key = fields.next().expect("catalog key");
                let values = std::array::from_fn(|_| fields.next().expect("seven translations"));
                assert!(fields.next().is_none(), "extra locale in catalog");
                (key, values)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_resources_cover_program_owned_literal_labels() {
        for source in [
            include_str!("ime/gpu/settings_scene.rs"),
            include_str!("ime/gpu/panel_scene.rs"),
            include_str!("ime/gpu/panel_scene_keyboard_body_block.rs"),
            include_str!("ime/gpu/panel_scene_voice_body_block.rs"),
            include_str!("ime/gpu/panel_scene_handwriting_body_block.rs"),
            include_str!("ime/gpu/panel_scene_next_tokens_block.rs"),
        ] {
            for tail in source.split("ui.tr(\"").skip(1) {
                let key = tail.split('"').next().unwrap();
                assert!(catalog().contains_key(key), "missing UI resource: {key}");
            }
        }
    }
    #[test]
    fn catalog_is_complete_unique_and_preserves_slots() {
        let rows: Vec<_> = include_str!("ui/catalog.tsv")
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(rows.len(), catalog().len(), "duplicate localization key");
        for (&key, values) in catalog() {
            assert!(!key.is_empty());
            for value in values {
                assert!(!value.is_empty(), "missing {key}");
                assert_eq!(
                    key.matches("{}").count(),
                    value.matches("{}").count(),
                    "{key}"
                );
            }
        }
        for language in UiLanguage::ALL {
            assert_eq!(UiLanguage::from_id(language.id()), Some(language));
        }
    }
    #[test]
    fn locale_fallback_and_interpolation_do_not_translate_payloads() {
        for (input, expected) in [
            ("zh_CN.UTF-8", UiLanguage::ChineseSimplified),
            ("ja-JP", UiLanguage::Japanese),
            ("pt_BR", UiLanguage::Portuguese),
            ("C.UTF-8", UiLanguage::English),
        ] {
            assert_eq!(UiLanguage::from_locale(input), Some(expected));
        }
        assert_eq!(UiLanguage::from_locale("ar_EG"), None);
        assert_eq!(UiLanguage::from_id("zh_CN"), None);
        assert_eq!(
            UiLanguage::ChineseSimplified.tr("unknown.key"),
            "unknown.key"
        );
        assert_eq!(
            UiLanguage::ChineseSimplified.message("Font: Settings"),
            "字体：Settings"
        );
        assert_eq!(
            UiLanguage::ChineseSimplified.message("Sent to active app: Font: custom settings"),
            "已发送到当前应用：Font: custom settings"
        );
        assert_eq!(
            UiLanguage::ChineseSimplified.message("Font: custom settings"),
            "字体：custom settings"
        );
    }
}
