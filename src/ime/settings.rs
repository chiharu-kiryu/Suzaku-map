//! User settings contain configuration only, never typed text or prediction history.

use crate::languages::{
    BuiltinLanguage,
    model::{ModelProtocol, ModelProviderConfig, ModelScope},
};
use serde_json::{Value, json};
use std::{fs, io::Read, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeSettings {
    pub language: BuiltinLanguage,
    pub llm_enabled: bool,
    pub provider: ModelProviderConfig,
}

/// A narrow, atomic update: panel controls cannot overwrite language/model configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PredictionSettingsPatch {
    pub enabled: Option<bool>,
    pub temperature_tenths: Option<u32>,
}

impl PredictionSettingsPatch {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(raw).map_err(|_| "联想设置不是有效的 JSON")?;
        let fields = value.as_object().ok_or("联想设置必须是 JSON 对象")?;
        if fields.is_empty()
            || fields
                .keys()
                .any(|key| !matches!(key.as_str(), "llm_enabled" | "llm_temperature_tenths"))
        {
            return Err("不支持的联想设置字段".into());
        }
        let checked = ImeSettings::from_json(raw)?;
        Ok(Self {
            enabled: fields
                .contains_key("llm_enabled")
                .then_some(checked.llm_enabled),
            temperature_tenths: fields
                .contains_key("llm_temperature_tenths")
                .then_some(checked.provider.temperature_tenths),
        })
    }

    pub fn to_json(self) -> Value {
        let mut value = json!({});
        if let Some(enabled) = self.enabled {
            value["llm_enabled"] = enabled.into();
        }
        if let Some(temperature) = self.temperature_tenths {
            value["llm_temperature_tenths"] = temperature.into();
        }
        value
    }

    pub fn apply(self, settings: &mut ImeSettings) -> Result<(), String> {
        // Validate before changing either field, including patches constructed in Rust.
        let checked = Self::from_json(&self.to_json().to_string())?;
        if let Some(enabled) = checked.enabled {
            settings.llm_enabled = enabled;
        }
        if let Some(temperature) = checked.temperature_tenths {
            settings.provider.temperature_tenths = temperature;
        }
        Ok(())
    }
}

impl Default for ImeSettings {
    fn default() -> Self {
        Self {
            language: BuiltinLanguage::English,
            llm_enabled: false,
            provider: ModelProviderConfig::default(),
        }
    }
}

impl ImeSettings {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(raw).map_err(|_| "输入法设置不是有效的 JSON")?;
        if !value.is_object() {
            return Err("输入法设置必须是 JSON 对象".into());
        }
        let mut settings = Self::default();
        if let Some(language) = value.get("language") {
            settings.language = language
                .as_str()
                .and_then(BuiltinLanguage::resolve)
                .ok_or("不支持的输入语言")?;
        }
        if let Some(enabled) = value.get("llm_enabled") {
            settings.llm_enabled = enabled.as_bool().ok_or("llm_enabled 必须是布尔值")?;
        }
        if let Some(endpoint) = value.get("llm_endpoint") {
            settings.provider.endpoint = endpoint.as_str().ok_or("模型地址必须是字符串")?.into();
        }
        if let Some(scope) = value.get("llm_scope") {
            settings.provider.scope = match scope.as_str() {
                Some("local") => ModelScope::Local,
                Some("cloud") => ModelScope::Cloud,
                _ => return Err("llm_scope 必须为 local 或 cloud".into()),
            };
        }
        if let Some(protocol) = value.get("llm_protocol") {
            settings.provider.protocol = match protocol.as_str() {
                Some("auto") => ModelProtocol::Auto,
                Some("ollama") => ModelProtocol::Ollama,
                Some("openai-compatible") => ModelProtocol::OpenAiCompatible,
                _ => return Err("不支持的模型接口协议".into()),
            };
        }
        if let Some(consent) = value.get("llm_cloud_consent") {
            settings.provider.cloud_consent = consent.as_bool().ok_or("云端授权必须是布尔值")?;
        }
        if let Some(key) = value.get("llm_api_key_env").filter(|v| !v.is_null()) {
            settings.provider.api_key_env =
                Some(key.as_str().ok_or("密钥环境变量名必须是字符串")?.into());
        }
        if let Some(model) = value.get("llm_model") {
            let model = model.as_str().ok_or("模型名称必须是字符串")?;
            if model.is_empty() || model.len() > 256 || model.chars().any(char::is_control) {
                return Err("模型名称无效".into());
            }
            settings.provider.model = model.into();
        }
        if let Some(timeout) = value.get("llm_timeout_ms") {
            settings.provider.timeout_ms = timeout
                .as_u64()
                .filter(|value| (20..=5000).contains(value))
                .ok_or("模型超时必须为 20–5000 毫秒")?;
        }
        if let Some(temperature) = value.get("llm_temperature_tenths") {
            settings.provider.temperature_tenths = temperature
                .as_u64()
                .filter(|value| *value <= 10)
                .ok_or("模型温度必须为 0–10")?
                as u32;
        }
        settings
            .provider
            .validate()
            .map_err(|error| error.to_string())?;
        Ok(settings)
    }

    pub fn to_json(&self) -> Value {
        json!({"language": self.language.id(), "llm_enabled": self.llm_enabled,
            "llm_scope": self.provider.scope.id(), "llm_protocol": self.provider.protocol.id(),
            "llm_cloud_consent": self.provider.cloud_consent, "llm_api_key_env": self.provider.api_key_env,
            "llm_endpoint": self.provider.endpoint, "llm_model": self.provider.model,
            "llm_timeout_ms": self.provider.timeout_ms, "llm_temperature_tenths": self.provider.temperature_tenths})
    }

    pub fn load() -> Result<Self, String> {
        let path = settings_path().ok_or("无法定位输入法设置目录")?;
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("无法读取输入法设置：{error}")),
        };
        let mut raw = String::new();
        file.take(65_537)
            .read_to_string(&mut raw)
            .map_err(|error| format!("无法读取输入法设置：{error}"))?;
        if raw.len() > 65_536 {
            return Err("输入法设置文件过大".into());
        }
        Self::from_json(&raw)
    }

    pub fn save(&self) -> Result<(), String> {
        let _lease = crate::data::files::DataLease::current_shared()?;
        let path = settings_path().ok_or("无法定位输入法设置目录")?;
        let validated = Self::from_json(&self.to_json().to_string())?;
        let contents =
            serde_json::to_vec_pretty(&validated.to_json()).map_err(|e| e.to_string())?;
        crate::data::files::atomic_write(&path, &contents)
            .map_err(|error| format!("无法保存输入法设置（未应用修改）：{error}"))
    }
}

pub fn settings_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SUZAKU_IME_CONFIG") {
        return (!path.is_empty()).then(|| PathBuf::from(path));
    }
    let root = crate::data::paths::config_home();
    root.map(|root| root.join("suzaku-ime/settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_local_auto_but_legacy_model_and_language_are_preserved() {
        let defaults = ImeSettings::from_json("{}").unwrap();
        assert_eq!(defaults.provider.model, "auto");
        assert_eq!(defaults.provider.scope, ModelScope::Local);
        assert!(!defaults.llm_enabled);
        assert!(!defaults.provider.cloud_consent);
        let legacy = ImeSettings::from_json(
            r#"{"language":"ja","llm_model":"llama3.2:3b","llm_enabled":true}"#,
        )
        .unwrap();
        assert_eq!(legacy.provider.model, "llama3.2:3b");
        assert_eq!(legacy.language, BuiltinLanguage::Japanese);
        assert!(legacy.llm_enabled);
    }

    #[test]
    fn cloud_needs_explicit_scope_and_model_and_persists_only_a_key_reference() {
        let settings = ImeSettings::from_json(r#"{"llm_scope":"cloud","llm_protocol":"openai-compatible","llm_endpoint":"https://models.example/v1/chat/completions","llm_model":"any-family","llm_api_key_env":"SUZAKU_MODEL_KEY","llm_enabled":true}"#).unwrap();
        assert!(!settings.provider.cloud_consent);
        assert_eq!(
            ImeSettings::from_json(&settings.to_json().to_string()).unwrap(),
            settings
        );
        let json = settings.to_json();
        assert!(json.get("llm_api_key").is_none());
        assert_eq!(json["llm_api_key_env"], "SUZAKU_MODEL_KEY");
        for (key, value) in [
            ("llm_scope", json!("local")),
            ("llm_model", json!("auto")),
            (
                "llm_endpoint",
                json!("http://remote.example/v1/chat/completions"),
            ),
            ("llm_api_key_env", json!("KEY=secret")),
            ("llm_cloud_consent", json!("true")),
            ("llm_protocol", json!("invented")),
        ] {
            let mut invalid = json.clone();
            invalid[key] = value;
            assert!(
                ImeSettings::from_json(&invalid.to_string()).is_err(),
                "{key}"
            );
        }
    }
    #[test]
    fn prediction_patch_is_validated_atomically_and_preserves_other_settings() {
        let mut settings = ImeSettings::default();
        settings.language = BuiltinLanguage::Japanese;
        settings.provider.model = "llama3.2:1b".into();
        let before = settings.clone();
        PredictionSettingsPatch::from_json(r#"{"llm_temperature_tenths":7}"#)
            .unwrap()
            .apply(&mut settings)
            .unwrap();
        assert_eq!(settings.provider.temperature_tenths, 7);
        let mut expected = before;
        expected.provider.temperature_tenths = 7;
        assert_eq!(settings, expected);
        for raw in [
            "{}",
            "null",
            r#"{"llm_model":"other"}"#,
            r#"{"llm_enabled":true,"llm_temperature_tenths":11}"#,
            r#"{"llm_temperature_tenths":-1}"#,
            r#"{"llm_temperature_tenths":"7"}"#,
        ] {
            assert!(PredictionSettingsPatch::from_json(raw).is_err(), "{raw}");
        }
        assert!(
            PredictionSettingsPatch {
                enabled: Some(true),
                temperature_tenths: Some(11)
            }
            .apply(&mut settings)
            .is_err()
        );
        assert_eq!(settings, expected);
    }
    #[test]
    fn english_is_the_default_without_overwriting_saved_language_choices() {
        assert_eq!(ImeSettings::default().language, BuiltinLanguage::English);
        assert_eq!(
            ImeSettings::from_json("{}").unwrap().language,
            BuiltinLanguage::English
        );
        for language in BuiltinLanguage::ALL {
            let raw = json!({"language":language.id()}).to_string();
            assert_eq!(ImeSettings::from_json(&raw).unwrap().language, language);
        }
    }
    #[test]
    fn settings_roundtrip_all_languages_without_input_history() {
        for language in BuiltinLanguage::ALL {
            let settings = ImeSettings {
                language,
                llm_enabled: true,
                ..Default::default()
            };
            let json = settings.to_json().to_string();
            assert_eq!(ImeSettings::from_json(&json).unwrap(), settings);
            assert!(!json.contains("context"));
        }
    }
    #[test]
    fn bad_language_remote_endpoint_and_invalid_types_are_rejected() {
        for value in [
            r#"{"language":"zh-Hant"}"#,
            r#"{"llm_enabled":"true"}"#,
            r#"{"llm_endpoint":"http://example.com"}"#,
            r#"{"llm_timeout_ms":999999}"#,
            r#"{"llm_model":""}"#,
        ] {
            assert!(ImeSettings::from_json(value).is_err());
        }
    }
}
