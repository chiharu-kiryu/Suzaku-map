//! Reconcile the two panel windows with acknowledged native settings, one write at a time.
use suzaku_map::ime::gpu::{LlmTemperaturePreset, PanelChromeState};
use suzaku_map::ime::settings::{ImeSettings, PredictionSettingsPatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Controls {
    enabled: bool,
    temperature: LlmTemperaturePreset,
}

impl Controls {
    fn from_chrome(chrome: &PanelChromeState) -> Self {
        Self {
            enabled: chrome.llm_enabled,
            temperature: chrome.llm_temperature,
        }
    }
    fn from_settings(settings: &ImeSettings) -> Self {
        Self {
            enabled: settings.llm_enabled,
            temperature: LlmTemperaturePreset::from_tenths(settings.provider.temperature_tenths),
        }
    }
}

#[derive(Default)]
pub(super) struct PredictionSettingsSync {
    confirmed: Option<Controls>,
    in_flight: Option<Controls>,
}

impl PredictionSettingsSync {
    pub fn observe(&mut self, settings: &ImeSettings, chrome: &mut PanelChromeState) {
        let controls = Controls::from_settings(settings);
        let previous = self.confirmed;
        self.confirmed = Some(controls);
        // A menu refresh is not an acknowledgement of the outstanding write.
        if self.in_flight.is_none() {
            // A status event can arrive before about_to_wait dispatches a UI edit.
            if previous.is_none_or(|old| chrome.llm_enabled == old.enabled) {
                chrome.llm_enabled = controls.enabled;
            }
            if previous.is_none_or(|old| chrome.llm_temperature == old.temperature) {
                chrome.llm_temperature = controls.temperature;
            }
        }
    }

    pub fn next_patch(&mut self, chrome: &PanelChromeState) -> Option<PredictionSettingsPatch> {
        if self.in_flight.is_some() {
            return None;
        }
        let confirmed = self.confirmed?;
        let desired = Controls::from_chrome(chrome);
        if desired == confirmed {
            return None;
        }
        self.in_flight = Some(desired);
        Some(PredictionSettingsPatch {
            enabled: (desired.enabled != confirmed.enabled).then_some(desired.enabled),
            temperature_tenths: (desired.temperature != confirmed.temperature)
                .then_some(desired.temperature.tenths()),
        })
    }

    pub fn finish(&mut self, settings: Option<&ImeSettings>, chrome: &mut PanelChromeState) {
        if let Some(settings) = settings {
            self.confirmed = Some(Controls::from_settings(settings));
        }
        let Some(sent) = self.in_flight.take() else {
            return;
        };
        if let Some(confirmed) = self.confirmed {
            // Keep edits made after dispatch; roll back only unchanged controls on failure.
            if chrome.llm_enabled == sent.enabled {
                chrome.llm_enabled = confirmed.enabled;
            }
            if chrome.llm_temperature == sent.temperature {
                chrome.llm_temperature = confirmed.temperature;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tone_is_sent_once_and_reload_preserves_exact_custom_temperature() {
        let mut sync = PredictionSettingsSync::default();
        let mut chrome = PanelChromeState::default();
        let mut native = ImeSettings::default();
        sync.observe(&native, &mut chrome);
        chrome.llm_temperature = LlmTemperaturePreset::Expressive;
        let patch = sync.next_patch(&chrome).unwrap();
        assert_eq!(
            patch,
            PredictionSettingsPatch {
                enabled: None,
                temperature_tenths: Some(7)
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
        let mut sync = PredictionSettingsSync::default();
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
            PredictionSettingsPatch {
                enabled: Some(true),
                temperature_tenths: Some(7)
            }
        );
        second.apply(&mut native).unwrap();
        sync.finish(Some(&native), &mut chrome);
        assert!(sync.next_patch(&chrome).is_none());
    }
    #[test]
    fn failed_write_rolls_back_without_retry_loop() {
        let mut sync = PredictionSettingsSync::default();
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
        let mut sync = PredictionSettingsSync::default();
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
}
