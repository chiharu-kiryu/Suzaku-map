use std::fs;
use std::path::PathBuf;

use suzaku_map::ime::gpu::{
    CandidateDensity, DisplayTextScale, FontFaceChoice, LlmModelPreset, LlmTemperaturePreset,
    PanelChromeState, PreviewStyle, TextSmoothing, TextSpacing,
};
use suzaku_map::platform::settings_host::display_settings_path;
use suzaku_map::platform::voice_host::HostSpeechRecognizer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersistedDisplaySettings {
    pub(crate) text_scale: DisplayTextScale,
    pub(crate) candidate_density: CandidateDensity,
    pub(crate) preview_style: PreviewStyle,
    pub(crate) font_face: FontFaceChoice,
    pub(crate) text_spacing: TextSpacing,
    pub(crate) text_smoothing: TextSmoothing,
    pub(crate) llm_enabled: bool,
    pub(crate) llm_model: LlmModelPreset,
    pub(crate) llm_temperature: LlmTemperaturePreset,
}

pub(crate) struct VoiceInputController {
    samples: Vec<&'static str>,
    next_index: usize,
    pub(crate) bridge: Option<HostSpeechRecognizer>,
}

impl VoiceInputController {
    pub(crate) fn new() -> Self {
        Self {
            samples: vec![
                "hello xr panel",
                "tablet ime voice seed",
                "continue this sentence by tap",
                "spatial input feels lighter with speech",
            ],
            next_index: 0,
            bridge: HostSpeechRecognizer::new(),
        }
    }

    pub(crate) fn next_sample(&mut self) -> String {
        let sample = self.samples[self.next_index % self.samples.len()].to_string();
        self.next_index = (self.next_index + 1) % self.samples.len();
        sample
    }
}

impl From<&PanelChromeState> for PersistedDisplaySettings {
    fn from(chrome: &PanelChromeState) -> Self {
        Self {
            text_scale: chrome.text_scale,
            candidate_density: chrome.candidate_density,
            preview_style: chrome.preview_style,
            font_face: chrome.font_face,
            text_spacing: chrome.text_spacing,
            text_smoothing: chrome.text_smoothing,
            llm_enabled: chrome.llm_enabled,
            llm_model: chrome.llm_model,
            llm_temperature: chrome.llm_temperature,
        }
    }
}

pub(crate) fn apply_display_settings(
    chrome: &mut PanelChromeState,
    settings: &PersistedDisplaySettings,
) {
    chrome.text_scale = settings.text_scale;
    chrome.candidate_density = settings.candidate_density;
    chrome.preview_style = settings.preview_style;
    chrome.font_face = settings.font_face;
    chrome.text_spacing = settings.text_spacing;
    chrome.text_smoothing = settings.text_smoothing;
    chrome.llm_enabled = settings.llm_enabled;
    chrome.llm_model = settings.llm_model;
    chrome.llm_temperature = settings.llm_temperature;
}

pub(crate) fn save_display_settings(settings: &PersistedDisplaySettings) -> std::io::Result<()> {
    let contents = format!(
        "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\n",
        encode_text_scale(settings.text_scale),
        encode_candidate_density(settings.candidate_density),
        encode_preview_style(settings.preview_style),
        encode_font_face(settings.font_face),
        encode_text_spacing(settings.text_spacing),
        encode_text_smoothing(settings.text_smoothing),
        if settings.llm_enabled {
            "true"
        } else {
            "false"
        },
        encode_llm_model(settings.llm_model),
        encode_llm_temperature(settings.llm_temperature),
    );
    let path = display_settings_path();
    ensure_settings_parent(&path)?;
    fs::write(path, contents)
}

pub(crate) fn load_display_settings() -> Option<PersistedDisplaySettings> {
    let contents = fs::read_to_string(display_settings_path()).ok()?;
    let mut settings = PersistedDisplaySettings {
        text_scale: DisplayTextScale::Medium,
        candidate_density: CandidateDensity::Cozy,
        preview_style: PreviewStyle::Compact,
        font_face: FontFaceChoice::Auto,
        text_spacing: TextSpacing::Normal,
        text_smoothing: TextSmoothing::Smooth,
        llm_enabled: false,
        llm_model: LlmModelPreset::Llama32_3b,
        llm_temperature: LlmTemperaturePreset::Balanced,
    };

    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "text_scale" => {
                if let Some(parsed) = decode_text_scale(value.trim()) {
                    settings.text_scale = parsed;
                }
            }
            "candidate_density" => {
                if let Some(parsed) = decode_candidate_density(value.trim()) {
                    settings.candidate_density = parsed;
                }
            }
            "preview_style" => {
                if let Some(parsed) = decode_preview_style(value.trim()) {
                    settings.preview_style = parsed;
                }
            }
            "font_face" => {
                if let Some(parsed) = decode_font_face(value.trim()) {
                    settings.font_face = parsed;
                }
            }
            "text_spacing" => {
                if let Some(parsed) = decode_text_spacing(value.trim()) {
                    settings.text_spacing = parsed;
                }
            }
            "text_smoothing" => {
                if let Some(parsed) = decode_text_smoothing(value.trim()) {
                    settings.text_smoothing = parsed;
                }
            }
            "llm_enabled" => settings.llm_enabled = value.trim() == "true",
            "llm_model" => {
                if let Some(parsed) = decode_llm_model(value.trim()) {
                    settings.llm_model = parsed;
                }
            }
            "llm_temperature" => {
                if let Some(parsed) = decode_llm_temperature(value.trim()) {
                    settings.llm_temperature = parsed;
                }
            }
            _ => {}
        }
    }

    Some(settings)
}

fn ensure_settings_parent(path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(crate) fn encode_text_scale(value: DisplayTextScale) -> &'static str {
    match value {
        DisplayTextScale::Small => "small",
        DisplayTextScale::Medium => "medium",
        DisplayTextScale::Large => "large",
    }
}

pub(crate) fn decode_text_scale(value: &str) -> Option<DisplayTextScale> {
    match value {
        "small" => Some(DisplayTextScale::Small),
        "medium" => Some(DisplayTextScale::Medium),
        "large" => Some(DisplayTextScale::Large),
        _ => None,
    }
}

pub(crate) fn encode_candidate_density(value: CandidateDensity) -> &'static str {
    match value {
        CandidateDensity::Compact => "compact",
        CandidateDensity::Cozy => "cozy",
    }
}

pub(crate) fn decode_candidate_density(value: &str) -> Option<CandidateDensity> {
    match value {
        "compact" => Some(CandidateDensity::Compact),
        "cozy" => Some(CandidateDensity::Cozy),
        _ => None,
    }
}

pub(crate) fn encode_preview_style(value: PreviewStyle) -> &'static str {
    match value {
        PreviewStyle::Compact => "compact",
        PreviewStyle::Full => "full",
    }
}

pub(crate) fn decode_preview_style(value: &str) -> Option<PreviewStyle> {
    match value {
        "compact" => Some(PreviewStyle::Compact),
        "full" => Some(PreviewStyle::Full),
        _ => None,
    }
}

pub(crate) fn encode_font_face(value: FontFaceChoice) -> &'static str {
    match value {
        FontFaceChoice::Auto => "auto",
        FontFaceChoice::Monaco => "monaco",
        FontFaceChoice::Geneva => "geneva",
        FontFaceChoice::ArialUnicode => "arial_unicode",
    }
}

pub(crate) fn decode_font_face(value: &str) -> Option<FontFaceChoice> {
    match value {
        "auto" => Some(FontFaceChoice::Auto),
        "monaco" => Some(FontFaceChoice::Monaco),
        "geneva" => Some(FontFaceChoice::Geneva),
        "arial_unicode" => Some(FontFaceChoice::ArialUnicode),
        _ => None,
    }
}

pub(crate) fn encode_text_spacing(value: TextSpacing) -> &'static str {
    match value {
        TextSpacing::Tight => "tight",
        TextSpacing::Normal => "normal",
        TextSpacing::Relaxed => "relaxed",
    }
}

pub(crate) fn decode_text_spacing(value: &str) -> Option<TextSpacing> {
    match value {
        "tight" => Some(TextSpacing::Tight),
        "normal" => Some(TextSpacing::Normal),
        "relaxed" => Some(TextSpacing::Relaxed),
        _ => None,
    }
}

pub(crate) fn encode_text_smoothing(value: TextSmoothing) -> &'static str {
    match value {
        TextSmoothing::Sharp => "sharp",
        TextSmoothing::Smooth => "smooth",
    }
}

pub(crate) fn decode_text_smoothing(value: &str) -> Option<TextSmoothing> {
    match value {
        "sharp" => Some(TextSmoothing::Sharp),
        "smooth" => Some(TextSmoothing::Smooth),
        _ => None,
    }
}

pub(crate) fn encode_llm_model(value: LlmModelPreset) -> &'static str {
    match value {
        LlmModelPreset::Llama32_3b => "llama32_3b",
    }
}

pub(crate) fn decode_llm_model(value: &str) -> Option<LlmModelPreset> {
    match value {
        "llama32_3b" => Some(LlmModelPreset::Llama32_3b),
        _ => None,
    }
}

pub(crate) fn encode_llm_temperature(value: LlmTemperaturePreset) -> &'static str {
    match value {
        LlmTemperaturePreset::Focused => "focused",
        LlmTemperaturePreset::Balanced => "balanced",
        LlmTemperaturePreset::Expressive => "expressive",
    }
}

pub(crate) fn decode_llm_temperature(value: &str) -> Option<LlmTemperaturePreset> {
    match value {
        "focused" => Some(LlmTemperaturePreset::Focused),
        "balanced" => Some(LlmTemperaturePreset::Balanced),
        "expressive" => Some(LlmTemperaturePreset::Expressive),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_settings_round_trip_codec() {
        let settings = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Large,
            candidate_density: CandidateDensity::Compact,
            preview_style: PreviewStyle::Full,
            font_face: FontFaceChoice::Geneva,
            text_spacing: TextSpacing::Relaxed,
            text_smoothing: TextSmoothing::Sharp,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Expressive,
        };

        let encoded = format!(
            "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\n",
            encode_text_scale(settings.text_scale),
            encode_candidate_density(settings.candidate_density),
            encode_preview_style(settings.preview_style),
            encode_font_face(settings.font_face),
            encode_text_spacing(settings.text_spacing),
            encode_text_smoothing(settings.text_smoothing),
            if settings.llm_enabled {
                "true"
            } else {
                "false"
            },
            encode_llm_model(settings.llm_model),
            encode_llm_temperature(settings.llm_temperature),
        );

        let mut decoded = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Smooth,
            llm_enabled: true,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
        };

        for line in encoded.lines() {
            let (key, value) = line.split_once('=').expect("kv");
            match key {
                "text_scale" => decoded.text_scale = decode_text_scale(value).expect("scale"),
                "candidate_density" => {
                    decoded.candidate_density = decode_candidate_density(value).expect("density")
                }
                "preview_style" => {
                    decoded.preview_style = decode_preview_style(value).expect("preview")
                }
                "font_face" => decoded.font_face = decode_font_face(value).expect("font"),
                "text_spacing" => {
                    decoded.text_spacing = decode_text_spacing(value).expect("spacing")
                }
                "text_smoothing" => {
                    decoded.text_smoothing = decode_text_smoothing(value).expect("smooth")
                }
                "llm_enabled" => decoded.llm_enabled = value == "true",
                "llm_model" => decoded.llm_model = decode_llm_model(value).expect("llm model"),
                "llm_temperature" => {
                    decoded.llm_temperature = decode_llm_temperature(value).expect("llm temp")
                }
                _ => {}
            }
        }

        assert_eq!(decoded, settings);
    }

    #[test]
    fn voice_controller_cycles_samples() {
        let mut voice = VoiceInputController::new();
        let first = voice.next_sample();
        let second = voice.next_sample();
        assert_ne!(first, second);
        assert!(!first.is_empty());
        assert!(!second.is_empty());
    }
}
