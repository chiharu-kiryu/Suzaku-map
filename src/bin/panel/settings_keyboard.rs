//! Focus navigation for the custom-drawn settings window. Moving never edits a preference.
use super::PanelState;
use suzaku_map::ime::gpu::{InteractionKind, InteractiveTarget};
use winit::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Navigation {
    Next,
    Previous,
    Left,
    Right,
    Up,
    Down,
}

fn next_target(
    targets: &[InteractiveTarget],
    focus: Option<InteractionKind>,
    direction: Navigation,
) -> Option<InteractionKind> {
    if targets.is_empty() {
        return None;
    }
    let Some(index) = targets.iter().position(|target| Some(target.kind) == focus) else {
        return Some(if direction == Navigation::Previous {
            targets.last().unwrap().kind
        } else {
            targets[0].kind
        });
    };
    match direction {
        Navigation::Next => return Some(targets[(index + 1) % targets.len()].kind),
        Navigation::Previous => {
            return Some(targets[(index + targets.len() - 1) % targets.len()].kind);
        }
        _ => {}
    }
    let center = |target: &InteractiveTarget| {
        [
            target.rect[0] + target.rect[2] * 0.5,
            target.rect[1] + target.rect[3] * 0.5,
        ]
    };
    let [x, y] = center(&targets[index]);
    targets
        .iter()
        .filter_map(|target| {
            let [tx, ty] = center(target);
            let dx = tx - x;
            let dy = ty - y;
            let valid = match direction {
                Navigation::Left => dy.abs() < 2.0 && dx < -1.0,
                Navigation::Right => dy.abs() < 2.0 && dx > 1.0,
                Navigation::Up => dy < -2.0,
                Navigation::Down => dy > 2.0,
                _ => false,
            };
            valid.then_some((target.kind, dy.abs(), dx.abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|target| target.0)
        .or(focus)
}

fn in_scroll_content(kind: InteractionKind) -> bool {
    !matches!(
        kind,
        InteractionKind::SetSettingsCategory(_)
            | InteractionKind::SettingsSearchInput
            | InteractionKind::SettingsSearchClear
            | InteractionKind::SettingsToggle
            | InteractionKind::RetrySaveSettings
    )
}

impl PanelState {
    fn focus_settings_control(&mut self, kind: Option<InteractionKind>) {
        self.chrome.settings_keyboard_focus = kind;
        self.chrome.settings_search_focused = kind == Some(InteractionKind::SettingsSearchInput);
        self.last_scene = None;
        self.reveal_settings_focus();
    }

    pub(super) fn reveal_settings_focus(&mut self) {
        let Some(kind) = self.chrome.settings_keyboard_focus else {
            return;
        };
        let scene = self.current_scene();
        let Some(target) = scene
            .settings_focus_targets
            .iter()
            .find(|target| target.kind == kind)
        else {
            self.chrome.settings_keyboard_focus = None;
            self.chrome.settings_search_focused = false;
            self.last_scene = None;
            return;
        };
        if in_scroll_content(kind)
            && let Some(scroll) = scene.settings_scroll_metadata
        {
            let top = scroll.track_rect[1];
            let bottom = top + scroll.visible_height;
            let delta = if target.rect[1] < top {
                target.rect[1] - top
            } else if target.rect[1] + target.rect[3] > bottom {
                target.rect[1] + target.rect[3] - bottom
            } else {
                0.0
            };
            if delta != 0.0 && scroll.visible_height > 0.0 {
                self.adjust_settings_scroll(delta);
            }
        }
    }

    pub(super) fn handle_settings_key(&mut self, key: &Key, text: Option<&str>, repeat: bool) {
        if self.text_input.composing() {
            return;
        }
        if matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("f"))
            && (self.modifiers == ModifiersState::CONTROL
                || (cfg!(target_os = "macos") && self.modifiers == ModifiersState::SUPER))
        {
            self.focus_settings_control(Some(InteractionKind::SettingsSearchInput));
            return;
        }
        let navigation = match key {
            Key::Named(NamedKey::Tab) if self.modifiers == ModifiersState::SHIFT => {
                Some(Navigation::Previous)
            }
            Key::Named(NamedKey::Tab) if self.modifiers.is_empty() => Some(Navigation::Next),
            Key::Named(NamedKey::ArrowLeft) if self.modifiers.is_empty() => Some(Navigation::Left),
            Key::Named(NamedKey::ArrowRight) if self.modifiers.is_empty() => {
                Some(Navigation::Right)
            }
            Key::Named(NamedKey::ArrowUp) if self.modifiers.is_empty() => Some(Navigation::Up),
            Key::Named(NamedKey::ArrowDown) if self.modifiers.is_empty() => Some(Navigation::Down),
            _ => None,
        };
        if let Some(direction) = navigation {
            // The append-only search field owns horizontal editing keys.
            if self.chrome.settings_search_focused
                && matches!(direction, Navigation::Left | Navigation::Right)
            {
                return;
            }
            let scene = self.current_scene();
            let focus = if self.chrome.settings_search_focused {
                Some(InteractionKind::SettingsSearchInput)
            } else {
                self.chrome.settings_keyboard_focus
            };
            self.focus_settings_control(next_target(
                &scene.settings_focus_targets,
                focus,
                direction,
            ));
            return;
        }
        if self.modifiers.is_empty() {
            match key {
                Key::Named(NamedKey::Escape) => {
                    if !repeat {
                        self.chrome.settings_open = false;
                    }
                    return;
                }
                Key::Named(NamedKey::Enter | NamedKey::Space)
                    if !self.chrome.settings_search_focused =>
                {
                    if !repeat && let Some(kind) = self.chrome.settings_keyboard_focus {
                        // Rebuild after filtering/resizing/collapsing; stale or read-only items
                        // must never activate, nor may keyboard actions move the mouse cursor.
                        let scene = self.current_scene();
                        if scene
                            .settings_focus_targets
                            .iter()
                            .any(|target| target.kind == kind)
                        {
                            self.activate_interaction(kind, false);
                            if kind == InteractionKind::SettingsSearchClear {
                                self.chrome.settings_keyboard_focus =
                                    Some(InteractionKind::SettingsSearchInput);
                            }
                            self.reveal_settings_focus();
                        }
                    }
                    return;
                }
                Key::Named(
                    NamedKey::PageUp | NamedKey::PageDown | NamedKey::Home | NamedKey::End,
                ) if !self.chrome.settings_search_focused => {
                    let page_height = self
                        .current_scene()
                        .settings_scroll_metadata
                        .map_or(34.0, |scroll| scroll.visible_height.max(34.0));
                    self.chrome.settings_keyboard_focus = None;
                    match key {
                        Key::Named(NamedKey::PageUp) => self.adjust_settings_scroll(-page_height),
                        Key::Named(NamedKey::PageDown) => self.adjust_settings_scroll(page_height),
                        Key::Named(NamedKey::Home) => self.set_settings_scroll_offset(0.0),
                        _ => self.set_settings_scroll_offset(
                            self.interaction.settings_scroll_max_offset,
                        ),
                    }
                    return;
                }
                Key::Named(NamedKey::Backspace) => {
                    if self.chrome.settings_search_focused
                        || self.chrome.settings_keyboard_focus.is_none()
                    {
                        self.backspace_settings_search_text();
                        self.focus_settings_control(Some(InteractionKind::SettingsSearchInput));
                    }
                    return;
                }
                _ => {}
            }
        }
        if super::keyboard::text_modifiers_allowed(self.modifiers)
            && (self.chrome.settings_search_focused || matches!(key, Key::Character(_)))
            && let Some(text) = text
        {
            self.handle_settings_search_text(text);
            self.focus_settings_control(Some(InteractionKind::SettingsSearchInput));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzaku_map::ime::gpu::{DisplayTextScale, SettingsCategory};

    fn targets() -> Vec<InteractiveTarget> {
        vec![
            InteractiveTarget {
                kind: InteractionKind::SetSettingsCategory(SettingsCategory::Appearance),
                rect: [0.0, 0.0, 50.0, 20.0],
            },
            InteractiveTarget {
                kind: InteractionKind::SettingsSearchInput,
                rect: [0.0, 30.0, 150.0, 20.0],
            },
            InteractiveTarget {
                kind: InteractionKind::SetTextScale(DisplayTextScale::Small),
                rect: [0.0, 300.0, 50.0, 20.0],
            },
            InteractiveTarget {
                kind: InteractionKind::SetTextScale(DisplayTextScale::Large),
                rect: [60.0, 300.0, 50.0, 20.0],
            },
        ]
    }

    #[test]
    fn tab_order_wraps_and_stale_focus_restarts_safely() {
        let targets = targets();
        for (index, target) in targets.iter().enumerate() {
            assert_eq!(
                next_target(&targets, Some(target.kind), Navigation::Next),
                Some(targets[(index + 1) % targets.len()].kind)
            );
            assert_eq!(
                next_target(&targets, Some(target.kind), Navigation::Previous),
                Some(targets[(index + targets.len() - 1) % targets.len()].kind)
            );
        }
        assert_eq!(
            next_target(
                &targets,
                Some(InteractionKind::SettingsSearchClear),
                Navigation::Next
            ),
            Some(targets[0].kind)
        );
        assert_eq!(
            next_target(&targets, None, Navigation::Previous),
            Some(targets[3].kind)
        );
        assert_eq!(next_target(&[], None, Navigation::Next), None);
    }

    #[test]
    fn arrows_follow_rows_and_reach_offscreen_controls_without_wrapping() {
        let targets = targets();
        assert_eq!(
            next_target(&targets, Some(targets[1].kind), Navigation::Down),
            Some(targets[3].kind)
        );
        assert_eq!(
            next_target(&targets, Some(targets[3].kind), Navigation::Left),
            Some(targets[2].kind)
        );
        assert_eq!(
            next_target(&targets, Some(targets[3].kind), Navigation::Right),
            Some(targets[3].kind)
        );
        assert_eq!(
            next_target(&targets, Some(targets[3].kind), Navigation::Up),
            Some(targets[1].kind)
        );
    }
}
