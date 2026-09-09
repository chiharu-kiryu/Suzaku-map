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
pub(crate) const DEFAULT_WINDOW_SCALE: f32 = 1.0;
pub(crate) const FIRST_LAUNCH_WINDOW_SCALE: f32 = 1.0;

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

pub(crate) fn encode_llm_temperature(value: LlmTemperaturePreset) -> String {
    match value {
        LlmTemperaturePreset::Focused => "focused".into(),
        LlmTemperaturePreset::Balanced => "balanced".into(),
        LlmTemperaturePreset::Expressive => "expressive".into(),
        LlmTemperaturePreset::Custom(value) => format!("custom:{value}"),
    }
}

pub(crate) fn decode_llm_temperature(value: &str) -> Option<LlmTemperaturePreset> {
    match value {
        "focused" => Some(LlmTemperaturePreset::Focused),
        "balanced" => Some(LlmTemperaturePreset::Balanced),
        "expressive" => Some(LlmTemperaturePreset::Expressive),
        _ => value
            .strip_prefix("custom:")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value <= 10)
            .map(LlmTemperaturePreset::from_tenths),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_temperature_settings_round_trip_without_rounding() {
        for value in 0..=10 {
            let preset = LlmTemperaturePreset::from_tenths(value);
            assert_eq!(preset.tenths(), value);
            assert_eq!(
                decode_llm_temperature(&encode_llm_temperature(preset)),
                Some(preset)
            );
        }
        assert!(decode_llm_temperature("custom:11").is_none());
    }

    fn assert_float_eq(actual: f32, expected: f32, case: &str) {
        if actual.is_infinite() || expected.is_infinite() {
            assert_eq!(
                actual, expected,
                "{case}: expected {expected}, got {actual}"
            );
            return;
        }
        assert!(
            (actual - expected).abs() < 1e-6,
            "{case}: expected {expected}, got {actual}"
        );
    }

    struct DisplaySettingsCodecRoundTripCase {
        name: &'static str,
        settings: PersistedDisplaySettings,
    }

    fn encode_display_settings(settings: &PersistedDisplaySettings) -> String {
        format!(
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
        )
    }

    fn decode_display_settings_payload(payload: &str) -> PersistedDisplaySettings {
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

        for line in payload.lines() {
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

        decoded
    }

    fn run_display_settings_codec_round_trip_cases(cases: &[DisplaySettingsCodecRoundTripCase]) {
        for case in cases {
            let encoded = encode_display_settings(&case.settings);
            let decoded = decode_display_settings_payload(&encoded);
            assert_eq!(decoded, case.settings, "{}", case.name);
        }
    }

    #[test]
    fn display_settings_round_trip_decision_matrix() {
        let cases = [
            DisplaySettingsCodecRoundTripCase {
                name: "display_settings_round_trip_retains_full_payload",
                settings: PersistedDisplaySettings {
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
                },
            },
            DisplaySettingsCodecRoundTripCase {
                name: "display_settings_round_trip_with_auto_and_sharp_normalization_values",
                settings: PersistedDisplaySettings {
                    text_scale: DisplayTextScale::Medium,
                    candidate_density: CandidateDensity::Cozy,
                    preview_style: PreviewStyle::Compact,
                    font_face: FontFaceChoice::Auto,
                    text_spacing: TextSpacing::Normal,
                    text_smoothing: TextSmoothing::Smooth,
                    theme_preset: ThemePreset::HighContrast,
                    voice_auto_insert: true,
                    llm_enabled: true,
                    llm_model: LlmModelPreset::Llama32_3b,
                    llm_temperature: LlmTemperaturePreset::Balanced,
                    pointer_tap_slop_tenths: 65,
                    pointer_tap_max_ms: 520,
                    pointer_target_slop_tenths: 32,
                    window_scale: 1.55,
                },
            },
            DisplaySettingsCodecRoundTripCase {
                name: "display_settings_round_trip_with_boolean_extremes",
                settings: PersistedDisplaySettings {
                    text_scale: DisplayTextScale::Small,
                    candidate_density: CandidateDensity::Cozy,
                    preview_style: PreviewStyle::Compact,
                    font_face: FontFaceChoice::Monaco,
                    text_spacing: TextSpacing::Tight,
                    text_smoothing: TextSmoothing::Smooth,
                    theme_preset: ThemePreset::Daylight,
                    voice_auto_insert: false,
                    llm_enabled: true,
                    llm_model: LlmModelPreset::Llama32_3b,
                    llm_temperature: LlmTemperaturePreset::Focused,
                    pointer_tap_slop_tenths: 110,
                    pointer_tap_max_ms: 900,
                    pointer_target_slop_tenths: 88,
                    window_scale: 1.55,
                },
            },
        ];

        run_display_settings_codec_round_trip_cases(&cases);
    }

    struct VoiceControllerSampleCase {
        name: &'static str,
        sample_count: usize,
        expected_unique: usize,
        expect_next_against_index: Option<usize>,
    }

    fn count_unique_strings(samples: &[String]) -> usize {
        let mut unique = 0usize;
        for (i, sample) in samples.iter().enumerate() {
            if !samples[..i].iter().any(|existing| existing == sample) {
                unique += 1;
            }
        }
        unique
    }

    fn run_voice_controller_sample_cases(cases: &[VoiceControllerSampleCase]) {
        for case in cases {
            let mut voice = VoiceInputController::new();
            let samples: Vec<String> = (0..case.sample_count)
                .map(|_| voice.next_sample())
                .collect();

            assert_eq!(samples.len(), case.sample_count, "{}", case.name);
            assert!(
                count_unique_strings(&samples) >= case.expected_unique,
                "{}",
                case.name
            );

            for sample in &samples {
                assert!(!sample.is_empty(), "{}", case.name);
            }

            if let Some(repeat_index) = case.expect_next_against_index {
                let next = voice.next_sample();
                assert_eq!(next, samples[repeat_index], "{}", case.name);
            }
        }
    }

    #[test]
    fn voice_controller_cycles_decision_matrix() {
        let cases = [
            VoiceControllerSampleCase {
                name: "voice_controller_two_samples_are_non_empty_and_distinct",
                sample_count: 2,
                expected_unique: 2,
                expect_next_against_index: None,
            },
            VoiceControllerSampleCase {
                name: "voice_controller_full_sample_set_repeats_from_start",
                sample_count: 4,
                expected_unique: 4,
                expect_next_against_index: Some(0),
            },
            VoiceControllerSampleCase {
                name: "voice_controller_cycle_wraps_after_more_than_one_roundtrip",
                sample_count: 9,
                expected_unique: 4,
                expect_next_against_index: Some(1),
            },
        ];

        run_voice_controller_sample_cases(&cases);
    }

    struct NormalizeDisplayPointerSettingsCase {
        name: &'static str,
        pointer_tap_slop_tenths_input: u16,
        pointer_tap_max_ms_input: u16,
        pointer_target_slop_tenths_input: u16,
        expected_tap_slop_tenths: u16,
        expected_tap_max_ms: u16,
        expected_target_slop_tenths: u16,
    }

    fn run_normalize_display_pointer_settings_cases(cases: &[NormalizeDisplayPointerSettingsCase]) {
        for case in cases {
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
                pointer_tap_slop_tenths: case.pointer_tap_slop_tenths_input,
                pointer_tap_max_ms: case.pointer_tap_max_ms_input,
                pointer_target_slop_tenths: case.pointer_target_slop_tenths_input,
                window_scale: 1.55,
            };

            normalize_display_pointer_settings(&mut settings);

            assert_eq!(
                settings.pointer_tap_slop_tenths, case.expected_tap_slop_tenths,
                "{}",
                case.name
            );
            assert_eq!(
                settings.pointer_tap_max_ms, case.expected_tap_max_ms,
                "{}",
                case.name
            );
            assert_eq!(
                settings.pointer_target_slop_tenths, case.expected_target_slop_tenths,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn normalize_display_pointer_settings_decision_matrix() {
        let cases = [
            NormalizeDisplayPointerSettingsCase {
                name: "normalize_extreme_low_and_high_pointer_stability_values",
                pointer_tap_slop_tenths_input: 3,
                pointer_tap_max_ms_input: 20,
                pointer_target_slop_tenths_input: 500,
                expected_tap_slop_tenths: 20,
                expected_tap_max_ms: 120,
                expected_target_slop_tenths: 120,
            },
            NormalizeDisplayPointerSettingsCase {
                name: "normalize_values_within_safe_range_are_preserved",
                pointer_tap_slop_tenths_input: 95,
                pointer_tap_max_ms_input: 240,
                pointer_target_slop_tenths_input: 22,
                expected_tap_slop_tenths: 95,
                expected_tap_max_ms: 240,
                expected_target_slop_tenths: 22,
            },
            NormalizeDisplayPointerSettingsCase {
                name: "normalize_pointer_stability_bottom_boundaries",
                pointer_tap_slop_tenths_input: 20,
                pointer_tap_max_ms_input: 120,
                pointer_target_slop_tenths_input: 10,
                expected_tap_slop_tenths: 20,
                expected_tap_max_ms: 120,
                expected_target_slop_tenths: 10,
            },
            NormalizeDisplayPointerSettingsCase {
                name: "normalize_pointer_stability_top_boundaries",
                pointer_tap_slop_tenths_input: 120,
                pointer_tap_max_ms_input: 1200,
                pointer_target_slop_tenths_input: 120,
                expected_tap_slop_tenths: 120,
                expected_tap_max_ms: 1200,
                expected_target_slop_tenths: 120,
            },
        ];

        run_normalize_display_pointer_settings_cases(&cases);
    }

    struct ApplyDisplaySettingsPointerStabilityCase {
        name: &'static str,
        pointer_tap_slop_tenths_input: u16,
        pointer_tap_max_ms_input: u16,
        pointer_target_slop_tenths_input: u16,
        expected_tap_slop_tenths: u16,
        expected_tap_max_ms: u16,
        expected_target_slop_tenths: u16,
    }

    fn run_apply_display_settings_pointer_stability_cases(
        cases: &[ApplyDisplaySettingsPointerStabilityCase],
    ) {
        let base_settings = PersistedDisplaySettings {
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
            window_scale: 1.0,
        };

        for case in cases {
            let mut chrome = PanelChromeState::default();
            let mut settings = base_settings.clone();
            settings.pointer_tap_slop_tenths = case.pointer_tap_slop_tenths_input;
            settings.pointer_tap_max_ms = case.pointer_tap_max_ms_input;
            settings.pointer_target_slop_tenths = case.pointer_target_slop_tenths_input;

            apply_display_settings(&mut chrome, &settings);

            assert_eq!(
                chrome.pointer_tap_slop_tenths, case.expected_tap_slop_tenths,
                "{}",
                case.name
            );
            assert_eq!(
                chrome.pointer_tap_max_ms, case.expected_tap_max_ms,
                "{}",
                case.name
            );
            assert_eq!(
                chrome.pointer_target_slop_tenths, case.expected_target_slop_tenths,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn apply_display_settings_pointer_stability_decision_matrix() {
        let cases = [
            ApplyDisplaySettingsPointerStabilityCase {
                name: "apply_pointer_stability_clamps_out_of_range_values",
                pointer_tap_slop_tenths_input: 255,
                pointer_tap_max_ms_input: 20,
                pointer_target_slop_tenths_input: 4,
                expected_tap_slop_tenths: 120,
                expected_tap_max_ms: 120,
                expected_target_slop_tenths: 10,
            },
            ApplyDisplaySettingsPointerStabilityCase {
                name: "apply_pointer_stability_preserves_valid_values",
                pointer_tap_slop_tenths_input: 95,
                pointer_tap_max_ms_input: 240,
                pointer_target_slop_tenths_input: 22,
                expected_tap_slop_tenths: 95,
                expected_tap_max_ms: 240,
                expected_target_slop_tenths: 22,
            },
            ApplyDisplaySettingsPointerStabilityCase {
                name: "apply_pointer_stability_forces_all_minimums",
                pointer_tap_slop_tenths_input: 0,
                pointer_tap_max_ms_input: 0,
                pointer_target_slop_tenths_input: 0,
                expected_tap_slop_tenths: 20,
                expected_tap_max_ms: 120,
                expected_target_slop_tenths: 10,
            },
            ApplyDisplaySettingsPointerStabilityCase {
                name: "apply_pointer_stability_forces_all_maximums",
                pointer_tap_slop_tenths_input: 999,
                pointer_tap_max_ms_input: 9999,
                pointer_target_slop_tenths_input: 999,
                expected_tap_slop_tenths: 120,
                expected_tap_max_ms: 1200,
                expected_target_slop_tenths: 120,
            },
        ];

        run_apply_display_settings_pointer_stability_cases(&cases);
    }

    struct ApplyDisplaySettingsWindowScaleCase {
        name: &'static str,
        window_scale_input: f32,
        expected_window_scale: f32,
    }

    fn run_apply_display_settings_window_scale_cases(
        cases: &[ApplyDisplaySettingsWindowScaleCase],
    ) {
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
            window_scale: 1.0,
        };

        for case in cases {
            let mut chrome = PanelChromeState::default();
            let mut test_settings = settings.clone();
            test_settings.window_scale = case.window_scale_input;
            apply_display_settings(&mut chrome, &test_settings);
            assert_float_eq(chrome.window_scale, case.expected_window_scale, case.name);
        }
    }

    #[test]
    fn apply_display_settings_window_scale_decision_matrix() {
        let cases = [
            ApplyDisplaySettingsWindowScaleCase {
                name: "apply_display_settings_preserves_low_window_scale",
                window_scale_input: 0.10,
                expected_window_scale: 0.10,
            },
            ApplyDisplaySettingsWindowScaleCase {
                name: "apply_display_settings_preserves_high_window_scale",
                window_scale_input: 99.9,
                expected_window_scale: 99.9,
            },
            ApplyDisplaySettingsWindowScaleCase {
                name: "apply_display_settings_preserves_normal_window_scale",
                window_scale_input: 1.3,
                expected_window_scale: 1.3,
            },
            ApplyDisplaySettingsWindowScaleCase {
                name: "apply_display_settings_preserves_negative_window_scale",
                window_scale_input: -2.0,
                expected_window_scale: -2.0,
            },
            ApplyDisplaySettingsWindowScaleCase {
                name: "apply_display_settings_preserves_unsafe_window_scale",
                window_scale_input: f32::INFINITY,
                expected_window_scale: f32::INFINITY,
            },
        ];

        run_apply_display_settings_window_scale_cases(&cases);
    }

    struct DecodeWindowScaleCase {
        name: &'static str,
        input: &'static str,
        expected: Option<f32>,
    }

    fn run_decode_window_scale_cases(cases: &[DecodeWindowScaleCase]) {
        for case in cases {
            let result = decode_window_scale(case.input);
            match case.expected {
                Some(expected) => assert_float_eq(result.expect(case.name), expected, case.name),
                None => assert!(result.is_none(), "{}", case.name),
            }
        }
    }

    #[test]
    fn decode_window_scale_decision_matrix() {
        let cases = [
            DecodeWindowScaleCase {
                name: "below_min_is_clamped_to_global_min",
                input: "0.01",
                expected: Some(PANEL_SCALE_MIN),
            },
            DecodeWindowScaleCase {
                name: "above_max_is_clamped_to_global_max",
                input: "9.0",
                expected: Some(PANEL_SCALE_MAX),
            },
            DecodeWindowScaleCase {
                name: "valid_value_is_preserved",
                input: "1.3",
                expected: Some(1.3),
            },
            DecodeWindowScaleCase {
                name: "nan_is_rejected",
                input: "NaN",
                expected: None,
            },
            DecodeWindowScaleCase {
                name: "invalid_text_is_rejected",
                input: "not-a-number",
                expected: None,
            },
            DecodeWindowScaleCase {
                name: "infinity_is_rejected",
                input: "inf",
                expected: None,
            },
            DecodeWindowScaleCase {
                name: "leading_and_trailing_whitespace_is_trimmed",
                input: "  1.3 \n",
                expected: Some(1.3),
            },
        ];

        run_decode_window_scale_cases(&cases);
    }

    struct NormalizeDisplayReadabilityCase {
        name: &'static str,
        font_face_input: FontFaceChoice,
        text_smoothing_input: TextSmoothing,
        expected_font_face: FontFaceChoice,
        expected_text_smoothing: TextSmoothing,
    }

    fn run_normalize_display_readability_cases(cases: &[NormalizeDisplayReadabilityCase]) {
        for case in cases {
            let mut chrome = PanelChromeState {
                font_face: case.font_face_input,
                text_smoothing: case.text_smoothing_input,
                ..PanelChromeState::default()
            };
            normalize_display_readability(&mut chrome);
            assert_eq!(chrome.font_face, case.expected_font_face, "{}", case.name);
            assert_eq!(
                chrome.text_smoothing, case.expected_text_smoothing,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn normalize_display_readability_decision_matrix() {
        let cases = [
            NormalizeDisplayReadabilityCase {
                name: "auto_font_face_switches_to_monaco_and_smooth_switches_to_sharp",
                font_face_input: FontFaceChoice::Auto,
                text_smoothing_input: TextSmoothing::Smooth,
                expected_font_face: FontFaceChoice::Monaco,
                expected_text_smoothing: TextSmoothing::Sharp,
            },
            NormalizeDisplayReadabilityCase {
                name: "non_auto_font_face_keeps_monaco",
                font_face_input: FontFaceChoice::Monaco,
                text_smoothing_input: TextSmoothing::Sharp,
                expected_font_face: FontFaceChoice::Monaco,
                expected_text_smoothing: TextSmoothing::Sharp,
            },
            NormalizeDisplayReadabilityCase {
                name: "monaco_font_is_kept_and_smooth_is_normalized_to_sharp",
                font_face_input: FontFaceChoice::Monaco,
                text_smoothing_input: TextSmoothing::Smooth,
                expected_font_face: FontFaceChoice::Monaco,
                expected_text_smoothing: TextSmoothing::Sharp,
            },
            NormalizeDisplayReadabilityCase {
                name: "auto_font_with_sharp_stays_monaco",
                font_face_input: FontFaceChoice::Auto,
                text_smoothing_input: TextSmoothing::Sharp,
                expected_font_face: FontFaceChoice::Monaco,
                expected_text_smoothing: TextSmoothing::Sharp,
            },
        ];

        run_normalize_display_readability_cases(&cases);
    }

    enum DecodeCodecCaseValue {
        TextScale(DisplayTextScale),
        CandidateDensity(CandidateDensity),
        PreviewStyle(PreviewStyle),
        FontFace(FontFaceChoice),
        TextSpacing(TextSpacing),
        TextSmoothing(TextSmoothing),
        ThemePreset(ThemePreset),
        LlmTemperature(LlmTemperaturePreset),
        LlmModel(LlmModelPreset),
    }

    struct DecodeCodecCase {
        name: &'static str,
        input: &'static str,
        expected: DecodeCodecCaseValue,
    }

    fn run_decode_codec_success_cases(cases: &[DecodeCodecCase]) {
        for case in cases {
            match case.expected {
                DecodeCodecCaseValue::TextScale(expected) => {
                    assert_eq!(
                        decode_text_scale(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::CandidateDensity(expected) => {
                    assert_eq!(
                        decode_candidate_density(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::PreviewStyle(expected) => {
                    assert_eq!(
                        decode_preview_style(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::FontFace(expected) => {
                    assert_eq!(
                        decode_font_face(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::TextSpacing(expected) => {
                    assert_eq!(
                        decode_text_spacing(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::TextSmoothing(expected) => {
                    assert_eq!(
                        decode_text_smoothing(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::ThemePreset(expected) => {
                    assert_eq!(
                        decode_theme_preset(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::LlmTemperature(expected) => {
                    assert_eq!(
                        decode_llm_temperature(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
                DecodeCodecCaseValue::LlmModel(expected) => {
                    assert_eq!(
                        decode_llm_model(case.input),
                        Some(expected),
                        "{}",
                        case.name
                    );
                }
            }
        }
    }

    struct DecodeCodecRejectCase {
        name: &'static str,
        input: &'static str,
        reject_u16: bool,
        reject_text_scale: bool,
        reject_candidate_density: bool,
        reject_preview_style: bool,
        reject_font_face: bool,
        reject_text_spacing: bool,
        reject_theme_preset: bool,
        reject_llm_temperature: bool,
        reject_llm_model: bool,
    }

    fn run_decode_codec_reject_cases(cases: &[DecodeCodecRejectCase]) {
        for case in cases {
            if case.reject_text_scale {
                assert!(decode_text_scale(case.input).is_none(), "{}", case.name);
            }
            if case.reject_candidate_density {
                assert!(
                    decode_candidate_density(case.input).is_none(),
                    "{}",
                    case.name
                );
            }
            if case.reject_preview_style {
                assert!(decode_preview_style(case.input).is_none(), "{}", case.name);
            }
            if case.reject_font_face {
                assert!(decode_font_face(case.input).is_none(), "{}", case.name);
            }
            if case.reject_text_spacing {
                assert!(decode_text_spacing(case.input).is_none(), "{}", case.name);
            }
            if case.reject_theme_preset {
                assert!(decode_theme_preset(case.input).is_none(), "{}", case.name);
            }
            if case.reject_llm_temperature {
                assert!(
                    decode_llm_temperature(case.input).is_none(),
                    "{}",
                    case.name
                );
            }
            if case.reject_llm_model {
                assert!(decode_llm_model(case.input).is_none(), "{}", case.name);
            }
            if case.reject_u16 {
                assert!(decode_u16(case.input).is_none(), "{}", case.name);
            }
        }
    }

    #[test]
    fn codec_round_trip_decision_matrix() {
        let cases = [
            DecodeCodecCase {
                name: "decode_text_scale_small_round_trips",
                input: "small",
                expected: DecodeCodecCaseValue::TextScale(DisplayTextScale::Small),
            },
            DecodeCodecCase {
                name: "decode_text_scale_medium_round_trips",
                input: "medium",
                expected: DecodeCodecCaseValue::TextScale(DisplayTextScale::Medium),
            },
            DecodeCodecCase {
                name: "decode_text_scale_large_round_trips",
                input: "large",
                expected: DecodeCodecCaseValue::TextScale(DisplayTextScale::Large),
            },
            DecodeCodecCase {
                name: "decode_candidate_density_compact_round_trips",
                input: "compact",
                expected: DecodeCodecCaseValue::CandidateDensity(CandidateDensity::Compact),
            },
            DecodeCodecCase {
                name: "decode_candidate_density_cozy_round_trips",
                input: "cozy",
                expected: DecodeCodecCaseValue::CandidateDensity(CandidateDensity::Cozy),
            },
            DecodeCodecCase {
                name: "decode_preview_style_compact_round_trips",
                input: "compact",
                expected: DecodeCodecCaseValue::PreviewStyle(PreviewStyle::Compact),
            },
            DecodeCodecCase {
                name: "decode_preview_style_full_round_trips",
                input: "full",
                expected: DecodeCodecCaseValue::PreviewStyle(PreviewStyle::Full),
            },
            DecodeCodecCase {
                name: "decode_font_face_monaco_round_trips",
                input: "monaco",
                expected: DecodeCodecCaseValue::FontFace(FontFaceChoice::Monaco),
            },
            DecodeCodecCase {
                name: "decode_font_face_arial_unicode_round_trips",
                input: "arial_unicode",
                expected: DecodeCodecCaseValue::FontFace(FontFaceChoice::ArialUnicode),
            },
            DecodeCodecCase {
                name: "decode_text_spacing_tight_round_trips",
                input: "tight",
                expected: DecodeCodecCaseValue::TextSpacing(TextSpacing::Tight),
            },
            DecodeCodecCase {
                name: "decode_text_spacing_relaxed_round_trips",
                input: "relaxed",
                expected: DecodeCodecCaseValue::TextSpacing(TextSpacing::Relaxed),
            },
            DecodeCodecCase {
                name: "decode_text_smoothing_sharp_round_trips",
                input: "sharp",
                expected: DecodeCodecCaseValue::TextSmoothing(TextSmoothing::Sharp),
            },
            DecodeCodecCase {
                name: "decode_text_smoothing_smooth_round_trips",
                input: "smooth",
                expected: DecodeCodecCaseValue::TextSmoothing(TextSmoothing::Smooth),
            },
            DecodeCodecCase {
                name: "decode_theme_preset_daylight_round_trips",
                input: "daylight",
                expected: DecodeCodecCaseValue::ThemePreset(ThemePreset::Daylight),
            },
            DecodeCodecCase {
                name: "decode_theme_preset_solarized_round_trips",
                input: "solarized",
                expected: DecodeCodecCaseValue::ThemePreset(ThemePreset::Solarized),
            },
            DecodeCodecCase {
                name: "decode_llm_temperature_focused_round_trips",
                input: "focused",
                expected: DecodeCodecCaseValue::LlmTemperature(LlmTemperaturePreset::Focused),
            },
            DecodeCodecCase {
                name: "decode_llm_temperature_expressive_round_trips",
                input: "expressive",
                expected: DecodeCodecCaseValue::LlmTemperature(LlmTemperaturePreset::Expressive),
            },
            DecodeCodecCase {
                name: "decode_llm_model_llama_round_trips",
                input: "llama32_3b",
                expected: DecodeCodecCaseValue::LlmModel(LlmModelPreset::Llama32_3b),
            },
        ];

        run_decode_codec_success_cases(&cases);
    }

    #[test]
    fn codec_rejects_non_matching_values_matrix() {
        let cases = [
            DecodeCodecRejectCase {
                name: "reject_text_scale_title_case",
                input: "Small",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: false,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_candidate_density_all_caps",
                input: "COMPACT",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_preview_style_title_case",
                input: "Compact",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_font_face_title_case",
                input: "Monaco",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_text_spacing_title_case",
                input: "Tight",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_theme_preset_all_caps",
                input: "DAYLIGHT",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: false,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_llm_temperature_trailing_space",
                input: "Balanced ",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_u16_negative",
                input: "-1",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_u16_overflow",
                input: "65536",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_u16_hex_like",
                input: "0x10",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_llm_temperature_unknown",
                input: "mild",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
            DecodeCodecRejectCase {
                name: "reject_llm_model_unknown",
                input: "gemini2_0",
                reject_text_scale: true,
                reject_candidate_density: true,
                reject_preview_style: true,
                reject_font_face: true,
                reject_text_spacing: true,
                reject_theme_preset: true,
                reject_llm_temperature: true,
                reject_llm_model: true,
                reject_u16: true,
            },
        ];

        run_decode_codec_reject_cases(&cases);
    }
}
