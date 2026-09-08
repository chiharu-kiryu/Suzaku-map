//! Text-field input is separate from panel commands and input-mode tabs.
use super::{PanelState, PanelWindowKind};
use suzaku_map::ime::gpu::{InteractionKind, PanelChromeState};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::Ime,
    keyboard::{Key, ModifiersState, NamedKey},
};

#[derive(Default)]
pub(super) struct TextInputState {
    allowed: bool,
    preedit: String,
    cursor: usize,
    cursor_area: Option<[f32; 4]>,
}

impl TextInputState {
    pub(super) fn composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.preedit.clear();
        self.cursor = 0;
    }

    fn set_preedit(&mut self, text: String, cursor: Option<(usize, usize)>) {
        // winit supplies a byte offset, while PanelChromeState uses characters.
        let byte_offset = cursor
            .map_or(text.len(), |(start, _)| start)
            .min(text.len());
        self.cursor = text
            .char_indices()
            .take_while(|(offset, _)| *offset < byte_offset)
            .count();
        self.preedit = text;
    }

    pub(super) fn preview(&self, chrome: &mut PanelChromeState, settings: bool) {
        if !self.composing() {
            return;
        }
        if settings {
            chrome.settings_search_query.push_str(&self.preedit);
        } else if chrome.input_focused {
            let start = chrome.caret_index.min(chrome.seed_text.chars().count());
            chrome.insert_text(&self.preedit);
            chrome.caret_index = start + self.cursor;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum EditAction<'a> {
    Insert(&'a str),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Finish,
    Candidates,
    Fold,
    Ignore,
}

pub(super) fn text_modifiers_allowed(modifiers: ModifiersState) -> bool {
    !modifiers.super_key()
        && (!modifiers.control_key() || modifiers.alt_key()) // Windows/Linux AltGr
        && (!modifiers.alt_key() || modifiers.control_key() || cfg!(target_os = "macos"))
}

pub(super) fn primary_shortcut_modifier(modifiers: ModifiersState) -> bool {
    !modifiers.alt_key() && (modifiers.control_key() || modifiers.super_key())
}

fn edit_action<'a>(
    key: &Key,
    text: Option<&'a str>,
    modifiers: ModifiersState,
    repeat: bool,
    composing: bool,
) -> EditAction<'a> {
    if composing || !text_modifiers_allowed(modifiers) {
        return EditAction::Ignore;
    }
    // Modified navigation belongs to the OS/editor, never a panel command.
    if modifiers.control_key() || modifiers.alt_key() || modifiers.super_key() {
        return text.map_or(EditAction::Ignore, EditAction::Insert);
    }
    match key {
        Key::Named(NamedKey::Backspace) => EditAction::Backspace,
        Key::Named(NamedKey::Delete) => EditAction::Delete,
        Key::Named(NamedKey::ArrowLeft) => EditAction::Left,
        Key::Named(NamedKey::ArrowRight) => EditAction::Right,
        Key::Named(NamedKey::Home) => EditAction::Home,
        Key::Named(NamedKey::End) => EditAction::End,
        Key::Named(NamedKey::Enter | NamedKey::Escape) if !repeat => EditAction::Finish,
        Key::Named(NamedKey::ArrowDown) if !repeat => EditAction::Candidates,
        Key::Named(NamedKey::Tab) if !repeat => EditAction::Fold,
        Key::Named(NamedKey::Space) => EditAction::Insert(text.unwrap_or(" ")),
        Key::Character(_) => text.map_or(EditAction::Ignore, EditAction::Insert),
        _ => EditAction::Ignore,
    }
}

impl PanelState {
    pub(super) fn begin_text_editing(&mut self) {
        self.leave_native_view();
        match self
            .text_focus
            .request(&self.window, self.runs_without_window_focus)
        {
            Ok(()) => {
                self.chrome.focus_input();
                self.chrome.move_caret_to_end();
            }
            Err(_) => {
                self.chrome.blur_input();
                self.last_commit_feedback =
                    Some("Could not focus the input field. Click it to retry.".into());
                self.commit_feedback_ticks = 180;
            }
        }
    }

    pub(super) fn finish_text_editing(&mut self) {
        self.text_input.clear();
        self.chrome.blur_input();
        self.window.set_ime_allowed(false);
        self.text_input.allowed = false;
        self.text_input.cursor_area = None;
        self.text_focus.release();
    }

    pub(super) fn sync_text_input_state(&mut self) {
        let settings = self.kind == PanelWindowKind::Settings;
        let allowed = self.is_focused
            && if settings {
                self.chrome.settings_search_focused
            } else {
                self.chrome.input_focused && !self.chrome.compact_mode && !self.chrome.settings_open
            };
        if self.text_input.allowed != allowed {
            self.text_input.allowed = allowed;
            self.text_input.cursor_area = None;
            if !allowed {
                self.text_input.clear();
            }
            self.window.set_ime_allowed(allowed);
        }
        if allowed {
            let target = if settings {
                InteractionKind::SettingsSearchInput
            } else {
                InteractionKind::SeedInput
            };
            if let Some([x, y, w, h]) = self.interaction_rect(target) {
                let area = [x, y, w, h];
                if self.text_input.cursor_area != Some(area) {
                    self.text_input.cursor_area = Some(area);
                    self.window.set_ime_cursor_area(
                        PhysicalPosition::new(x as f64, (y + h) as f64),
                        PhysicalSize::new(w.max(1.0) as u32, 1),
                    );
                }
            }
        }
    }

    pub(super) fn handle_ime_event(&mut self, event: Ime) {
        if !self.text_input.allowed {
            return;
        }
        match event {
            Ime::Preedit(text, cursor) => self.text_input.set_preedit(text, cursor),
            Ime::Commit(text) => {
                self.text_input.clear();
                if self.kind == PanelWindowKind::Settings {
                    self.handle_settings_search_text(&text);
                } else {
                    self.handle_text_input(&text);
                }
            }
            Ime::Disabled => self.text_input.clear(),
            Ime::Enabled => {}
        }
        self.last_scene = None;
        self.window.request_redraw();
    }

    pub(super) fn handle_editing_key(&mut self, key: &Key, text: Option<&str>, repeat: bool) {
        match edit_action(
            key,
            text,
            self.modifiers,
            repeat,
            self.text_input.composing(),
        ) {
            EditAction::Insert(text) => self.handle_text_input(text),
            EditAction::Backspace => self.backspace_seed(),
            EditAction::Delete => {
                if self.chrome.caret_index < self.chrome.seed_text.chars().count() {
                    self.chrome.move_caret_right();
                    self.backspace_seed();
                }
            }
            EditAction::Left => self.chrome.move_caret_left(),
            EditAction::Right => self.chrome.move_caret_right(),
            EditAction::Home => self.chrome.caret_index = 0,
            EditAction::End => self.chrome.move_caret_to_end(),
            EditAction::Finish => self.finish_text_editing(),
            EditAction::Candidates => self.chrome.blur_input(),
            EditAction::Fold => {
                self.chrome.input_modes_expanded = !self.chrome.input_modes_expanded
            }
            EditAction::Ignore => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_characters_digits_and_repeats_are_always_text_in_an_editor() {
        for text in [
            "1", "2", "3", "4", "v", "n", "i", "d", "r", "z", "é", "日", "😀",
        ] {
            for repeat in [false, true] {
                assert_eq!(
                    edit_action(
                        &Key::Character(text.into()),
                        Some(text),
                        ModifiersState::empty(),
                        repeat,
                        false
                    ),
                    EditAction::Insert(text)
                );
            }
        }
        for repeat in [false, true] {
            assert_eq!(
                edit_action(
                    &Key::Named(NamedKey::Space),
                    Some(" "),
                    ModifiersState::empty(),
                    repeat,
                    false
                ),
                EditAction::Insert(" ")
            );
        }
    }

    #[test]
    fn shortcut_chords_do_not_insert_letters_but_altgr_can_insert_text() {
        let key = Key::Character("v".into());
        for modifiers in [
            ModifiersState::CONTROL,
            ModifiersState::SUPER,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        ] {
            assert_eq!(
                edit_action(&key, Some("v"), modifiers, false, false),
                EditAction::Ignore
            );
        }
        assert_eq!(
            edit_action(
                &key,
                Some("@"),
                ModifiersState::CONTROL | ModifiersState::ALT,
                false,
                false
            ),
            EditAction::Insert("@")
        );
    }

    #[test]
    fn altgr_does_not_trigger_zoom_or_quit_shortcuts() {
        assert!(primary_shortcut_modifier(ModifiersState::CONTROL));
        assert!(primary_shortcut_modifier(
            ModifiersState::SUPER | ModifiersState::SHIFT
        ));
        assert!(!primary_shortcut_modifier(
            ModifiersState::CONTROL | ModifiersState::ALT
        ));
    }

    #[test]
    fn composition_owns_keys_and_finish_is_not_repeatable() {
        assert_eq!(
            edit_action(
                &Key::Named(NamedKey::Backspace),
                None,
                ModifiersState::empty(),
                true,
                false
            ),
            EditAction::Backspace
        );
        for named in [
            NamedKey::Enter,
            NamedKey::Escape,
            NamedKey::Space,
            NamedKey::Backspace,
            NamedKey::ArrowLeft,
        ] {
            assert_eq!(
                edit_action(
                    &Key::Named(named),
                    None,
                    ModifiersState::empty(),
                    false,
                    true
                ),
                EditAction::Ignore
            );
        }
        assert_eq!(
            edit_action(
                &Key::Named(NamedKey::Enter),
                Some("\r"),
                ModifiersState::empty(),
                true,
                false
            ),
            EditAction::Ignore
        );
    }

    #[test]
    fn preedit_is_only_a_preview_and_uses_unicode_cursor_offsets() {
        let mut input = TextInputState::default();
        let chrome = PanelChromeState {
            seed_text: "ab".into(),
            caret_index: 1,
            ..Default::default()
        };
        input.set_preedit("日😀本".into(), Some((7, 7)));
        let mut preview = chrome.clone();
        input.preview(&mut preview, false);
        assert_eq!(preview.seed_text, "a日😀本b");
        assert_eq!(preview.caret_index, 3);
        assert_eq!(chrome.seed_text, "ab");
        input.set_preedit("".into(), None);
        let mut preview = chrome.clone();
        input.preview(&mut preview, false);
        assert_eq!(preview, chrome);
    }
}
