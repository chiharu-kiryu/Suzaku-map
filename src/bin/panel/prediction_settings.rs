//! Reconcile the two panel windows with acknowledged native settings, one write at a time.
use suzaku_map::ime::gpu::{LlmTemperaturePreset, PanelChromeState};
use suzaku_map::ime::settings::{ImeSettings, PanelImeSettingsPatch};
use suzaku_map::ime::shortcuts::ShortcutProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Controls {
    enabled: bool,
    temperature: LlmTemperaturePreset,
    shortcuts: ShortcutProfile,
}

impl Controls {
    fn from_chrome(chrome: &PanelChromeState) -> Self {
        Self {
            enabled: chrome.llm_enabled,
            temperature: chrome.llm_temperature,
            shortcuts: chrome.shortcut_profile,
        }
    }
    fn from_settings(settings: &ImeSettings) -> Self {
        Self {
            enabled: settings.llm_enabled,
            temperature: LlmTemperaturePreset::from_tenths(settings.provider.temperature_tenths),
            shortcuts: settings.shortcut_profile,
        }
    }
}

#[derive(Default)]
pub(super) struct PanelImeSettingsSync {
    confirmed: Option<Controls>,
    in_flight: Option<(Controls, [u64; 3])>,
}

impl PanelImeSettingsSync {
    pub fn observe(&mut self, settings: &ImeSettings, chrome: &mut PanelChromeState) {
        let controls = Controls::from_settings(settings);
        let previous = self.confirmed;
        self.confirmed = Some(controls);
        // A menu refresh is not an acknowledgement of the outstanding write.
        if self.in_flight.is_none() {
            // A status event can arrive before about_to_wait dispatches a UI edit.
            if previous.map_or(chrome.prediction_edit_generation[0] == 0, |old| {
                chrome.llm_enabled == old.enabled
            }) {
                chrome.llm_enabled = controls.enabled;
            }
            if previous.map_or(chrome.prediction_edit_generation[1] == 0, |old| {
                chrome.llm_temperature == old.temperature
            }) {
                chrome.llm_temperature = controls.temperature;
            }
            if previous.map_or(chrome.shortcut_edit_generation == 0, |old| {
                chrome.shortcut_profile == old.shortcuts
            }) {
                chrome.shortcut_profile = controls.shortcuts;
            }
        }
    }

    pub fn next_patch(&mut self, chrome: &PanelChromeState) -> Option<PanelImeSettingsPatch> {
        if self.in_flight.is_some() {
            return None;
        }
        let confirmed = self.confirmed?;
        let desired = Controls::from_chrome(chrome);
        if desired == confirmed {
            return None;
        }
        self.in_flight = Some((
            desired,
            [
                chrome.prediction_edit_generation[0],
                chrome.prediction_edit_generation[1],
                chrome.shortcut_edit_generation,
            ],
        ));
        Some(PanelImeSettingsPatch {
            enabled: (desired.enabled != confirmed.enabled).then_some(desired.enabled),
            temperature_tenths: (desired.temperature != confirmed.temperature)
                .then_some(desired.temperature.tenths()),
            shortcut_profile: (desired.shortcuts != confirmed.shortcuts)
                .then_some(desired.shortcuts),
        })
    }

    pub fn finish(&mut self, settings: Option<&ImeSettings>, chrome: &mut PanelChromeState) {
        if let Some(settings) = settings {
            self.confirmed = Some(Controls::from_settings(settings));
        }
        let Some((sent, edits)) = self.in_flight.take() else {
            return;
        };
        if let Some(confirmed) = self.confirmed {
            // Keep edits made after dispatch; roll back only unchanged controls on failure.
            if chrome.llm_enabled == sent.enabled
                && chrome.prediction_edit_generation[0] == edits[0]
            {
                chrome.llm_enabled = confirmed.enabled;
            }
            if chrome.llm_temperature == sent.temperature
                && chrome.prediction_edit_generation[1] == edits[1]
            {
                chrome.llm_temperature = confirmed.temperature;
            }
            if chrome.shortcut_profile == sent.shortcuts
                && chrome.shortcut_edit_generation == edits[2]
            {
                chrome.shortcut_profile = confirmed.shortcuts;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_edits_reconcile_failures_late_acks_and_unsent_choices_independently() {
        let mut native = ImeSettings::default();
        let mut chrome = PanelChromeState::default();
        let mut sync = PanelImeSettingsSync::default();
        chrome.shortcut_profile = ShortcutProfile::HomeRow;
        chrome.shortcut_edit_generation = 1;
        sync.observe(&native, &mut chrome);
        assert_eq!(chrome.shortcut_profile, ShortcutProfile::HomeRow);
        let patch = sync.next_patch(&chrome).unwrap();
        assert_eq!(
            patch.to_json(),
            serde_json::json!({"shortcut_profile":"home-row"})
        );
        assert!(sync.next_patch(&chrome).is_none());
        // A newer explicit choice wins over an old successful acknowledgement.
        chrome.shortcut_profile = ShortcutProfile::Standard;
        chrome.shortcut_edit_generation += 1;
        patch.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        assert_eq!(chrome.shortcut_profile, ShortcutProfile::Standard);
        let patch = sync.next_patch(&chrome).unwrap();
        patch.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        assert!(sync.next_patch(&chrome).is_none());
        chrome.shortcut_profile = ShortcutProfile::HomeRow;
        chrome.shortcut_edit_generation += 1;
        sync.observe(&native, &mut chrome);
        sync.next_patch(&chrome).unwrap();
        sync.finish(None, &mut chrome);
        assert_eq!(chrome.shortcut_profile, ShortcutProfile::Standard);
        assert!(
            sync.next_patch(&chrome).is_none(),
            "failed writes must not retry forever"
        );
        native.shortcut_profile = ShortcutProfile::HomeRow;
        sync.observe(&native, &mut chrome);
        assert_eq!(chrome.shortcut_profile, ShortcutProfile::HomeRow);
        assert!(sync.next_patch(&chrome).is_none());
    }

    #[test]
    fn tone_is_sent_once_and_reload_preserves_exact_custom_temperature() {
        let mut sync = PanelImeSettingsSync::default();
        let mut chrome = PanelChromeState::default();
        let mut native = ImeSettings::default();
        sync.observe(&native, &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Expressive;
        let patch = sync.next_patch(&chrome).unwrap();
        assert_eq!(
            patch,
            PanelImeSettingsPatch {
                enabled: None,
                temperature_tenths: Some(7),
                shortcut_profile: None,
            }
        );
        assert!(sync.next_patch(&chrome).is_none());
        patch.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        assert!(sync.next_patch(&chrome).is_none());
        native.provider.temperature_tenths = 3;
        sync.observe(&native, &mut chrome);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Custom(3));
        assert!(sync.next_patch(&chrome).is_none());
        chrome.llm_enabled = true;
        assert_eq!(sync.next_patch(&chrome).unwrap().temperature_tenths, None);
    }
    #[test]
    fn late_ack_and_refresh_do_not_erase_a_newer_tone_selection() {
        let mut sync = PanelImeSettingsSync::default();
        let mut chrome = PanelChromeState::default();
        let mut native = ImeSettings::default();
        sync.observe(&native, &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Focused;
        let first = sync.next_patch(&chrome).unwrap();
        chrome.llm_temperature = LlmTemperaturePreset::Expressive;
        chrome.llm_enabled = true;
        sync.observe(&native, &mut chrome);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Expressive);
        first.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        let second = sync.next_patch(&chrome).unwrap();
        assert_eq!(
            second,
            PanelImeSettingsPatch {
                enabled: Some(true),
                temperature_tenths: Some(7),
                shortcut_profile: None,
            }
        );
        second.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        assert!(sync.next_patch(&chrome).is_none());
    }
    #[test]
    fn failed_write_rolls_back_without_retry_loop() {
        let mut sync = PanelImeSettingsSync::default();
        let mut chrome = PanelChromeState::default();
        sync.observe(&ImeSettings::default(), &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Focused;
        sync.next_patch(&chrome).unwrap();
        sync.finish(None, &mut chrome);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Balanced);
        assert!(sync.next_patch(&chrome).is_none());
    }

    #[test]
    fn refresh_before_dispatch_preserves_unsent_edits() {
        let mut sync = PanelImeSettingsSync::default();
        let mut chrome = PanelChromeState::default();
        let native = ImeSettings::default();
        sync.observe(&native, &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Expressive;
        sync.observe(&native, &mut chrome);
        assert_eq!(
            sync.next_patch(&chrome).unwrap().temperature_tenths,
            Some(7)
        );
    }

    #[test]
    fn first_status_keeps_explicit_choices_but_not_stale_persisted_controls() {
        let mut native = ImeSettings {
            llm_enabled: true,
            ..Default::default()
        };
        native.provider.temperature_tenths = 7;
        let mut chrome = PanelChromeState::default();
        chrome.prediction_edit_generation[0] = 1; // Explicit Off, even though it matches the default.
        let mut sync = PanelImeSettingsSync::default();
        sync.observe(&native, &mut chrome);
        assert!(!chrome.llm_enabled);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Expressive);
        assert_eq!(
            sync.next_patch(&chrome).unwrap(),
            PanelImeSettingsPatch {
                enabled: Some(false),
                temperature_tenths: None,
                shortcut_profile: None,
            }
        );

        let mut chrome = PanelChromeState::default();
        chrome.prediction_edit_generation[1] = 1; // Explicit Balanced, same as the initial value.
        let mut sync = PanelImeSettingsSync::default();
        sync.observe(&native, &mut chrome);
        assert!(chrome.llm_enabled);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Balanced);
        assert_eq!(
            sync.next_patch(&chrome).unwrap().temperature_tenths,
            Some(4)
        );

        let mut chrome = PanelChromeState {
            llm_enabled: true,
            llm_temperature: LlmTemperaturePreset::Expressive,
            ..Default::default()
        };
        let mut sync = PanelImeSettingsSync::default();
        sync.observe(&ImeSettings::default(), &mut chrome);
        assert!(
            !chrome.llm_enabled,
            "loading stale preferences is not an explicit opt-in"
        );
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Balanced);
        assert!(sync.next_patch(&chrome).is_none());
    }

    #[test]
    fn late_failure_does_not_swallow_a_reselected_value() {
        let mut chrome = PanelChromeState::default();
        let mut sync = PanelImeSettingsSync::default();
        sync.observe(&ImeSettings::default(), &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Focused;
        chrome.prediction_edit_generation[1] = 1;
        sync.next_patch(&chrome).unwrap();
        chrome.prediction_edit_generation[1] = 3; // Expressive then Focused again while waiting.
        sync.finish(None, &mut chrome);
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Focused);
        assert_eq!(
            sync.next_patch(&chrome).unwrap().temperature_tenths,
            Some(2)
        );
        sync.finish(None, &mut chrome); // No further edit: failure must stop retrying.
        assert_eq!(chrome.llm_temperature, LlmTemperaturePreset::Balanced);
        assert!(sync.next_patch(&chrome).is_none());
    }
}
