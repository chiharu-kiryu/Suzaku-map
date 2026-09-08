//! User-only native host settings channel. No input text is involved in these requests.
use crate::{ime::settings::ImeSettings, languages::BuiltinLanguage};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeImeStatus {
    pub settings: ImeSettings,
    pub prediction: String,
    pub prediction_error: Option<String>,
}

pub fn status() -> Result<NativeImeStatus, String> {
    request("S")
}
pub fn set_language(language: BuiltinLanguage) -> Result<NativeImeStatus, String> {
    request(&format!("L{}", language.id()))
}
pub fn set_llm_enabled(enabled: bool) -> Result<NativeImeStatus, String> {
    request(if enabled { "P1" } else { "P0" })
}
pub fn reload_settings() -> Result<NativeImeStatus, String> {
    request("R")
}

fn request(command: &str) -> Result<NativeImeStatus, String> {
    let path = std::env::var_os("SUZAKU_LINUX_IME_SOCKET")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|root| std::path::PathBuf::from(root).join("suzaku-ime/host.sock"))
        })
        .ok_or("无法定位本机输入法服务")?;
    let mut stream = UnixStream::connect(path).map_err(|_| "Suzaku 输入法服务未运行")?;
    let timeout = Some(Duration::from_millis(700));
    stream
        .set_read_timeout(timeout)
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(timeout)
        .map_err(|error| error.to_string())?;
    stream
        .write_all(command.as_bytes())
        .map_err(|error| error.to_string())?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream
        .take(8193)
        .read_to_string(&mut response)
        .map_err(|_| "输入法设置请求超时，请重试")?;
    if response.len() > 8192 {
        return Err("输入法服务返回数据过大".into());
    }
    parse_status(&response)
}

fn parse_status(response: &str) -> Result<NativeImeStatus, String> {
    let value: serde_json::Value =
        serde_json::from_str(response).map_err(|_| "本机输入法服务版本过旧，请重新安装宿主")?;
    if value["ok"] != true {
        return Err(value["error"].as_str().unwrap_or("输入法设置未生效").into());
    }
    Ok(NativeImeStatus {
        settings: ImeSettings::from_json(&value["settings"].to_string())?,
        prediction: value["prediction"].as_str().unwrap_or("Disabled").into(),
        prediction_error: value["prediction_error"].as_str().map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_native_settings_and_rejects_legacy_or_failed_acknowledgements() {
        let raw = serde_json::json!({"ok":true,"settings":ImeSettings::default().to_json(),"prediction":"Pending"}).to_string();
        let parsed = parse_status(&raw).unwrap();
        assert_eq!(parsed.settings.language, BuiltinLanguage::English);
        assert_eq!(parsed.prediction, "Pending");
        assert!(parse_status("0").is_err());
        assert!(parse_status("1").is_err());
        assert_eq!(
            parse_status(r#"{"ok":false,"error":"not saved"}"#).unwrap_err(),
            "not saved"
        );
    }
}
