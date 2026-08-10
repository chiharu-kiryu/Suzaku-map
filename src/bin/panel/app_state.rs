use std::fs;
use std::path::PathBuf;

use suzaku_map::ime::gpu::{
    CandidateDensity, DisplayTextScale, FontFaceChoice, LlmModelPreset, LlmTemperaturePreset,
    PANEL_SCALE_MAX, PANEL_SCALE_MIN, PanelChromeState, PreviewStyle, TextSmoothing, TextSpacing,
    ThemePreset,
};
use suzaku_map::platform::settings_host::display_settings_path;
use suzaku_map::platform::voice_host::HostSpeechRecognizer;

const MIN_POINTER_TAP_SLOP_TENTHS: u16 = 20;
const MAX_POINTER_TAP_SLOP_TENTHS: u16 = 120;
const MIN_POINTER_TAP_MAX_MS: u16 = 120;
const MAX_POINTER_TAP_MAX_MS: u16 = 1200;
const MIN_POINTER_TARGET_SLOP_TENTHS: u16 = 10;
const MAX_POINTER_TARGET_SLOP_TENTHS: u16 = 120;
pub(crate) const DEFAULT_WINDOW_SCALE: f32 = 1.3;
pub(crate) const FIRST_LAUNCH_WINDOW_SCALE: f32 = 1.35;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PersistedDisplaySettings {
    pub(crate) text_scale: DisplayTextScale,
    pub(crate) candidate_density: CandidateDensity,
    pub(crate) preview_style: PreviewStyle,
    pub(crate) font_face: FontFaceChoice,
    pub(crate) text_spacing: TextSpacing,
    pub(crate) text_smoothing: TextSmoothing,
    pub(crate) theme_preset: ThemePreset,
    pub(crate) voice_auto_insert: bool,
    pub(crate) llm_enabled: bool,
    pub(crate) llm_model: LlmModelPreset,
    pub(crate) llm_temperature: LlmTemperaturePreset,
    pub(crate) pointer_tap_slop_tenths: u16,
    pub(crate) pointer_tap_max_ms: u16,
    pub(crate) pointer_target_slop_tenths: u16,
    pub(crate) window_scale: f32,
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
            theme_preset: chrome.theme_preset,
            voice_auto_insert: chrome.voice_auto_insert,
            llm_enabled: chrome.llm_enabled,
            llm_model: chrome.llm_model,
            llm_temperature: chrome.llm_temperature,
            pointer_tap_slop_tenths: chrome.pointer_tap_slop_tenths,
            pointer_tap_max_ms: chrome.pointer_tap_max_ms,
            pointer_target_slop_tenths: chrome.pointer_target_slop_tenths,
            window_scale: chrome.window_scale,
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
    chrome.theme_preset = settings.theme_preset;
    chrome.voice_auto_insert = settings.voice_auto_insert;
    chrome.llm_enabled = settings.llm_enabled;
    chrome.llm_model = settings.llm_model;
    chrome.llm_temperature = settings.llm_temperature;
    chrome.pointer_tap_slop_tenths = settings.pointer_tap_slop_tenths;
    chrome.pointer_tap_max_ms = settings.pointer_tap_max_ms;
    chrome.pointer_target_slop_tenths = settings.pointer_target_slop_tenths;
    chrome.window_scale = settings.window_scale;
    normalize_pointer_stability_settings(chrome);
    normalize_display_readability(chrome);
}

pub(crate) fn save_display_settings(settings: &PersistedDisplaySettings) -> std::io::Result<()> {
    let contents = format!(
        "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\ntheme_preset={}\nvoice_auto_insert={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\npointer_tap_slop_tenths={}\npointer_tap_max_ms={}\npointer_target_slop_tenths={}\nwindow_scale={}\n",
        encode_text_scale(settings.text_scale),
        encode_candidate_density(settings.candidate_density),
        encode_preview_style(settings.preview_style),
        encode_font_face(settings.font_face),
        encode_text_spacing(settings.text_spacing),
        encode_text_smoothing(settings.text_smoothing),
        encode_theme_preset(settings.theme_preset),
        if settings.voice_auto_insert {
            "true"
        } else {
            "false"
        },
        if settings.llm_enabled {
            "true"
        } else {
            "false"
        },
        encode_llm_model(settings.llm_model),
        encode_llm_temperature(settings.llm_temperature),
        settings.pointer_tap_slop_tenths,
        settings.pointer_tap_max_ms,
        settings.pointer_target_slop_tenths,
        settings.window_scale,
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
        font_face: FontFaceChoice::Monaco,
        text_spacing: TextSpacing::Normal,
        text_smoothing: TextSmoothing::Sharp,
        theme_preset: ThemePreset::Daylight,
        voice_auto_insert: true,
        llm_enabled: false,
        llm_model: LlmModelPreset::Llama32_3b,
        llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 100,
            pointer_tap_max_ms: 420,
            pointer_target_slop_tenths: 50,
            window_scale: DEFAULT_WINDOW_SCALE,
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
            "theme_preset" => {
                if let Some(parsed) = decode_theme_preset(value.trim()) {
                    settings.theme_preset = parsed;
                }
            }
            "voice_auto_insert" => settings.voice_auto_insert = value.trim() == "true",
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
            "pointer_tap_slop_tenths" => {
                if let Some(parsed) = decode_u16(value.trim()) {
                    settings.pointer_tap_slop_tenths = parsed;
                }
            }
            "pointer_tap_max_ms" => {
                if let Some(parsed) = decode_u16(value.trim()) {
                    settings.pointer_tap_max_ms = parsed;
                }
            }
            "pointer_target_slop_tenths" => {
                if let Some(parsed) = decode_u16(value.trim()) {
                    settings.pointer_target_slop_tenths = parsed;
                }
            }
            "window_scale" => {
                if let Some(parsed) = decode_window_scale(value.trim()) {
                    settings.window_scale = parsed;
                }
            }
            _ => {}
        }
    }

    normalize_display_pointer_settings(&mut settings);

    Some(settings)
}

pub(crate) fn normalize_pointer_stability_settings(chrome: &mut PanelChromeState) {
    chrome.pointer_tap_slop_tenths = chrome
        .pointer_tap_slop_tenths
        .clamp(MIN_POINTER_TAP_SLOP_TENTHS, MAX_POINTER_TAP_SLOP_TENTHS);
    chrome.pointer_tap_max_ms = chrome
        .pointer_tap_max_ms
        .clamp(MIN_POINTER_TAP_MAX_MS, MAX_POINTER_TAP_MAX_MS);
    chrome.pointer_target_slop_tenths = chrome.pointer_target_slop_tenths.clamp(
        MIN_POINTER_TARGET_SLOP_TENTHS,
        MAX_POINTER_TARGET_SLOP_TENTHS,
    );
}

fn normalize_display_pointer_settings(settings: &mut PersistedDisplaySettings) {
    settings.pointer_tap_slop_tenths = settings
        .pointer_tap_slop_tenths
        .clamp(MIN_POINTER_TAP_SLOP_TENTHS, MAX_POINTER_TAP_SLOP_TENTHS);
    settings.pointer_tap_max_ms = settings
        .pointer_tap_max_ms
        .clamp(MIN_POINTER_TAP_MAX_MS, MAX_POINTER_TAP_MAX_MS);
    settings.pointer_target_slop_tenths = settings.pointer_target_slop_tenths.clamp(
        MIN_POINTER_TARGET_SLOP_TENTHS,
        MAX_POINTER_TARGET_SLOP_TENTHS,
    );
    settings.window_scale = settings
        .window_scale
        .clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX);
}

fn decode_window_scale(value: &str) -> Option<f32> {
    let parsed = value.trim().parse::<f32>().ok()?;
    if !parsed.is_finite() {
        return None;
    }
    Some(parsed.clamp(PANEL_SCALE_MIN, PANEL_SCALE_MAX))
}

fn normalize_display_readability(chrome: &mut PanelChromeState) {
    if chrome.font_face == FontFaceChoice::Auto {
        chrome.font_face = FontFaceChoice::Monaco;
    }
    if chrome.text_smoothing == TextSmoothing::Smooth {
        chrome.text_smoothing = TextSmoothing::Sharp;
    }
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
        FontFaceChoice::Menlo => "menlo",
        FontFaceChoice::Geneva => "geneva",
        FontFaceChoice::Helvetica => "helvetica",
        FontFaceChoice::PingFang => "pingfang",
        FontFaceChoice::ArialUnicode => "arial_unicode",
    }
}

pub(crate) fn decode_font_face(value: &str) -> Option<FontFaceChoice> {
    match value {
        "auto" => Some(FontFaceChoice::Auto),
        "monaco" => Some(FontFaceChoice::Monaco),
        "menlo" => Some(FontFaceChoice::Menlo),
        "geneva" => Some(FontFaceChoice::Geneva),
        "helvetica" => Some(FontFaceChoice::Helvetica),
        "pingfang" => Some(FontFaceChoice::PingFang),
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

pub(crate) fn encode_theme_preset(value: ThemePreset) -> &'static str {
    match value {
        ThemePreset::Daylight => "daylight",
        ThemePreset::DeviceDark => "device_dark",
        ThemePreset::HighContrast => "high_contrast",
        ThemePreset::Solarized => "solarized",
    }
}

pub(crate) fn decode_theme_preset(value: &str) -> Option<ThemePreset> {
    match value {
        "daylight" => Some(ThemePreset::Daylight),
        "device_dark" => Some(ThemePreset::DeviceDark),
        "high_contrast" => Some(ThemePreset::HighContrast),
        "solarized" => Some(ThemePreset::Solarized),
        _ => None,
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

fn decode_u16(value: &str) -> Option<u16> {
    value.trim().parse::<u16>().ok()
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
            font_face: FontFaceChoice::PingFang,
            text_spacing: TextSpacing::Relaxed,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::DeviceDark,
            voice_auto_insert: false,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Expressive,
            pointer_tap_slop_tenths: 95,
            pointer_tap_max_ms: 240,
            pointer_target_slop_tenths: 22,
            window_scale: 1.2,
        };

        let encoded = format!(
            "text_scale={}\ncandidate_density={}\npreview_style={}\nfont_face={}\ntext_spacing={}\ntext_smoothing={}\ntheme_preset={}\nvoice_auto_insert={}\nllm_enabled={}\nllm_model={}\nllm_temperature={}\npointer_tap_slop_tenths={}\npointer_tap_max_ms={}\npointer_target_slop_tenths={}\nwindow_scale={}\n",
            encode_text_scale(settings.text_scale),
            encode_candidate_density(settings.candidate_density),
            encode_preview_style(settings.preview_style),
            encode_font_face(settings.font_face),
            encode_text_spacing(settings.text_spacing),
            encode_text_smoothing(settings.text_smoothing),
            encode_theme_preset(settings.theme_preset),
            if settings.voice_auto_insert {
                "true"
            } else {
                "false"
            },
            if settings.llm_enabled {
                "true"
            } else {
                "false"
            },
            encode_llm_model(settings.llm_model),
            encode_llm_temperature(settings.llm_temperature),
            settings.pointer_tap_slop_tenths,
            settings.pointer_tap_max_ms,
            settings.pointer_target_slop_tenths,
            settings.window_scale,
        );

        let mut decoded = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Auto,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::Daylight,
            voice_auto_insert: true,
            llm_enabled: true,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 100,
            pointer_tap_max_ms: 420,
            pointer_target_slop_tenths: 50,
            window_scale: 1.0,
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
                "theme_preset" => decoded.theme_preset = decode_theme_preset(value).expect("theme"),
                "voice_auto_insert" => decoded.voice_auto_insert = value == "true",
                "llm_enabled" => decoded.llm_enabled = value == "true",
                "llm_model" => decoded.llm_model = decode_llm_model(value).expect("llm model"),
                "llm_temperature" => {
                    decoded.llm_temperature = decode_llm_temperature(value).expect("llm temp")
                }
                "pointer_tap_slop_tenths" => {
                    decoded.pointer_tap_slop_tenths = decode_u16(value).expect("tap slop")
                }
                "pointer_tap_max_ms" => {
                    decoded.pointer_tap_max_ms = decode_u16(value).expect("tap max")
                }
                "pointer_target_slop_tenths" => {
                    decoded.pointer_target_slop_tenths = decode_u16(value).expect("target slop")
                }
                "window_scale" => {
                    decoded.window_scale = decode_window_scale(value).expect("window scale")
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

    #[test]
    fn normalize_extreme_pointer_stability_values() {
        let mut settings = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Monaco,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::Daylight,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 3,
            pointer_tap_max_ms: 20,
            pointer_target_slop_tenths: 500,
            window_scale: 1.55,
        };

        normalize_display_pointer_settings(&mut settings);

        assert_eq!(settings.pointer_tap_slop_tenths, 20);
        assert_eq!(settings.pointer_tap_max_ms, 120);
        assert_eq!(settings.pointer_target_slop_tenths, 120);
    }

    #[test]
    fn apply_display_settings_clamps_pointer_stability() {
        let mut chrome = PanelChromeState::default();
        let settings = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Monaco,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::Daylight,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 255,
            pointer_tap_max_ms: 20,
            pointer_target_slop_tenths: 4,
            window_scale: 1.0,
        };

        apply_display_settings(&mut chrome, &settings);

        assert_eq!(chrome.pointer_tap_slop_tenths, 120);
        assert_eq!(chrome.pointer_tap_max_ms, 120);
        assert_eq!(chrome.pointer_target_slop_tenths, 10);
    }

    #[test]
    fn apply_display_settings_clamps_window_scale_to_safe_range() {
        let mut chrome = PanelChromeState::default();
        let settings = PersistedDisplaySettings {
            text_scale: DisplayTextScale::Medium,
            candidate_density: CandidateDensity::Cozy,
            preview_style: PreviewStyle::Compact,
            font_face: FontFaceChoice::Monaco,
            text_spacing: TextSpacing::Normal,
            text_smoothing: TextSmoothing::Sharp,
            theme_preset: ThemePreset::Daylight,
            voice_auto_insert: true,
            llm_enabled: false,
            llm_model: LlmModelPreset::Llama32_3b,
            llm_temperature: LlmTemperaturePreset::Balanced,
            pointer_tap_slop_tenths: 100,
            pointer_tap_max_ms: 420,
            pointer_target_slop_tenths: 50,
            window_scale: 0.10,
        };

        apply_display_settings(&mut chrome, &settings);
        assert_eq!(chrome.window_scale, PANEL_SCALE_MIN);

        let settings = PersistedDisplaySettings {
            window_scale: 99.9,
            ..settings
        };

        apply_display_settings(&mut chrome, &settings);
        assert_eq!(chrome.window_scale, PANEL_SCALE_MAX);
    }

    #[test]
    fn decode_window_scale_clamps_and_rejects_invalid_values() {
        assert_eq!(decode_window_scale("0.01"), Some(PANEL_SCALE_MIN));
        assert_eq!(decode_window_scale("9.0"), Some(PANEL_SCALE_MAX));
        assert_eq!(decode_window_scale("1.3"), Some(1.3));
        assert_eq!(decode_window_scale("NaN"), None);
        assert_eq!(decode_window_scale("not-a-number"), None);
    }

    #[test]
    fn voice_controller_cycles_full_sample_set() {
        let mut voice = VoiceInputController::new();
        let samples: Vec<String> = (0..4).map(|_| voice.next_sample()).collect();
        let next = voice.next_sample();

        assert_eq!(samples.len(), 4);
        assert_eq!(next, samples[0]);
        assert!(!samples[0].is_empty());
        assert!(!samples[1].is_empty());
        assert_ne!(samples[0], samples[1]);
    }
}
