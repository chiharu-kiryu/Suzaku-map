//! Context-local shortcuts. Adapters own modifier, focus, privacy and Compose checks.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShortcutProfile {
    /// Preserve existing shortcuts and leave all additional Alt chords to the app.
    #[default]
    Standard,
    /// Optional QWERTY home-row aliases; never installs a global key grab.
    HomeRow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ShortcutAction {
    Previous = 1,
    Next = 2,
    PreviousPage = 3,
    NextPage = 4,
    Adopt = 5,
}

impl ShortcutProfile {
    pub const ALL: [Self; 2] = [Self::Standard, Self::HomeRow];

    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::HomeRow => "home-row",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard keys",
            Self::HomeRow => "Home row (Alt)",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| profile.id() == id)
    }

    /// Only call for an exact Alt chord in a nonempty, public IBus draft.
    /// Case folding allows Caps Lock, not Shift (checked by the adapter).
    pub fn alt_action(self, key: char) -> Option<ShortcutAction> {
        if self != Self::HomeRow {
            return None;
        }
        match key.to_ascii_lowercase() {
            'k' => Some(ShortcutAction::Previous),
            'j' => Some(ShortcutAction::Next),
            'h' => Some(ShortcutAction::PreviousPage),
            'l' => Some(ShortcutAction::NextPage),
            ';' => Some(ShortcutAction::Adopt),
            _ => None,
        }
    }

    /// Searchable read-only reference, including the unchanged writing-stream keys.
    pub fn reference(self) -> Vec<(&'static str, Vec<&'static str>)> {
        let mut rows = vec![
            ("Scope", vec!["IBus", "Focused input"]),
            ("Switch EN/ZH", vec!["Ctrl+Shift+Space"]),
            ("Previous", vec!["Shift+Tab", "↑"]),
            ("Next choice", vec!["Tab", "↓"]),
            ("Page back", vec!["PageUp"]),
            ("Page forward", vec!["PageDown"]),
            ("Adopt", vec!["1–6", "Shift+Enter"]),
            ("Undo choice", vec!["Backspace"]),
            ("Continue", vec!["Space"]),
            ("Submit", vec!["Enter"]),
            ("Cancel", vec!["Esc"]),
            ("Literal digits", vec!["Alt+0–9", "Numpad"]),
            ("Panel help", vec!["F1"]),
            ("Panel settings", vec!["Ctrl+,"]),
        ];
        if self == Self::HomeRow {
            for ((_, _, key), (_, keys)) in HOME_ROW_BINDINGS.iter().zip(&mut rows[2..7]) {
                keys.push(key);
            }
        }
        rows
    }
}

/// Labels shared by the settings reference and shortcut tests. Keys are not text actions.
pub const HOME_ROW_BINDINGS: [(char, ShortcutAction, &str); 5] = [
    ('k', ShortcutAction::Previous, "Alt+K"),
    ('j', ShortcutAction::Next, "Alt+J"),
    ('h', ShortcutAction::PreviousPage, "Alt+H"),
    ('l', ShortcutAction::NextPage, "Alt+L"),
    (';', ShortcutAction::Adopt, "Alt+;"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_toggle_is_easy_to_find_without_moving_home_row_aliases() {
        for profile in ShortcutProfile::ALL {
            let rows = profile.reference();
            assert_eq!(rows[1], ("Switch EN/ZH", vec!["Ctrl+Shift+Space"]));
            for (label, (_, _, key)) in [
                "Previous",
                "Next choice",
                "Page back",
                "Page forward",
                "Adopt",
            ]
            .into_iter()
            .zip(HOME_ROW_BINDINGS)
            {
                let keys = &rows.iter().find(|(name, _)| *name == label).unwrap().1;
                assert_eq!(keys.contains(&key), profile == ShortcutProfile::HomeRow);
            }
        }
    }

    #[test]
    fn profiles_and_displayed_aliases_match_without_stealing_literal_digits() {
        for profile in ShortcutProfile::ALL {
            assert_eq!(ShortcutProfile::from_id(profile.id()), Some(profile));
        }
        assert_eq!(ShortcutProfile::from_id("vim"), None);
        for (key, action, _) in HOME_ROW_BINDINGS {
            assert_eq!(ShortcutProfile::HomeRow.alt_action(key), Some(action));
            assert_eq!(
                ShortcutProfile::HomeRow.alt_action(key.to_ascii_uppercase()),
                Some(action)
            );
            assert_eq!(ShortcutProfile::Standard.alt_action(key), None);
        }
        for key in "0123456789qacvxz \t\r\u{1b}é中".chars() {
            assert_eq!(ShortcutProfile::HomeRow.alt_action(key), None, "{key:?}");
        }
    }
}
