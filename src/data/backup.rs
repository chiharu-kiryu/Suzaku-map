//! A versioned, bounded JSON format with fixed logical entries, never archive paths.
use super::{
    files::{self, DataLease, SETTINGS_LIMIT, StagedFile},
    paths::DataPaths,
};
use crate::ime::settings::ImeSettings;
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const BACKUP_LIMIT: usize = 262_144;
const IME_KEYS: &[&str] = &[
    "language",
    "llm_enabled",
    "llm_endpoint",
    "llm_model",
    "llm_timeout_ms",
    "llm_temperature_tenths",
];

#[derive(Debug, Clone)]
pub struct Backup {
    pub ime: Option<Value>,
    pub panel: Option<Map<String, Value>>,
}

fn panel_value_valid(key: &str, value: &str) -> bool {
    match key {
        "text_scale" => matches!(value, "small" | "medium" | "large"),
        "candidate_density" => matches!(value, "compact" | "cozy"),
        "preview_style" => matches!(value, "compact" | "full"),
        "font_face" => matches!(
            value,
            "auto" | "monaco" | "menlo" | "geneva" | "helvetica" | "pingfang" | "arial_unicode"
        ),
        "text_spacing" => matches!(value, "tight" | "normal" | "relaxed"),
        "text_smoothing" => matches!(value, "sharp" | "smooth"),
        "theme_preset" => matches!(
            value,
            "daylight" | "device_dark" | "high_contrast" | "solarized"
        ),
        "voice_auto_insert" | "llm_enabled" => matches!(value, "true" | "false"),
        "llm_model" => value == "llama32_3b",
        "llm_temperature" => {
            matches!(value, "focused" | "balanced" | "expressive")
                || value
                    .strip_prefix("custom:")
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|v| v <= 10)
        }
        "pointer_tap_slop_tenths" | "pointer_tap_max_ms" | "pointer_target_slop_tenths" => {
            value.parse::<u16>().is_ok()
        }
        "window_scale" => value
            .parse::<f32>()
            .is_ok_and(|v| v.is_finite() && v > 0.0 && v <= 10.0),
        _ => false,
    }
}

fn panel_from_text(raw: &str) -> Result<Map<String, Value>, String> {
    let mut fields = Map::new();
    for line in raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (key, value) = line.split_once('=').ok_or("面板设置格式错误")?;
        let (key, value) = (key.trim(), value.trim());
        if !panel_value_valid(key, value) || fields.insert(key.into(), value.into()).is_some() {
            return Err(format!("不支持、重复或无效的面板设置字段：{key}"));
        }
    }
    Ok(fields)
}

fn read_setting(path: &Path) -> Result<Option<String>, String> {
    files::read_optional(path, SETTINGS_LIMIT).map_err(|e| format!("{}: {e}", path.display()))
}

impl Backup {
    pub fn collect(paths: &DataPaths) -> Result<Self, String> {
        Ok(Self {
            // Re-serialize only known configuration, not arbitrary extra fields or comments.
            ime: read_setting(&paths.ime)?
                .map(|raw| ImeSettings::from_json(&raw).map(|v| v.to_json()))
                .transpose()?,
            panel: read_setting(&paths.panel)?
                .map(|raw| panel_from_text(&raw))
                .transpose()?,
        })
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > BACKUP_LIMIT {
            return Err("备份文件过大".into());
        }
        let root: Value = serde_json::from_str(raw).map_err(|_| "备份不是有效的 JSON")?;
        let object = root.as_object().ok_or("备份必须是 JSON 对象")?;
        if object.len() != 6
            || object.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "format"
                        | "schema_version"
                        | "app_version"
                        | "created_unix_seconds"
                        | "ime"
                        | "panel"
                )
            })
            || root["format"] != "suzaku-settings"
            || root["schema_version"].as_u64() != Some(1)
            || root["created_unix_seconds"].as_u64().is_none()
            || !root["app_version"]
                .as_str()
                .is_some_and(|v| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control))
        {
            return Err("不支持或不完整的 Suzaku 备份格式（需要 schema_version=1）".into());
        }
        let ime = if root["ime"].is_null() {
            None
        } else {
            let fields = root["ime"].as_object().ok_or("输入法备份必须是对象")?;
            if fields.keys().any(|key| !IME_KEYS.contains(&key.as_str())) {
                return Err("输入法备份包含未知字段".into());
            }
            Some(ImeSettings::from_json(&root["ime"].to_string())?.to_json())
        };
        let panel = if root["panel"].is_null() {
            None
        } else {
            let fields = root["panel"].as_object().ok_or("面板备份必须是对象")?;
            for (key, value) in fields {
                if !value.as_str().is_some_and(|v| panel_value_valid(key, v)) {
                    return Err(format!("面板备份字段无效：{key}"));
                }
            }
            Some(fields.clone())
        };
        let backup = Self { ime, panel };
        for contents in backup.contents()?.into_iter().flatten() {
            if contents.len() > SETTINGS_LIMIT {
                return Err("备份中的设置文件过大".into());
            }
        }
        Ok(backup)
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let raw = files::read_optional(path, BACKUP_LIMIT)
            .map_err(|e| e.to_string())?
            .ok_or("备份文件不存在")?;
        Self::parse(&raw)
    }

    pub fn to_json(&self) -> Value {
        json!({"format": "suzaku-settings", "schema_version": 1,
            "app_version": env!("CARGO_PKG_VERSION"),
            "created_unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            "ime": self.ime, "panel": self.panel})
    }

    pub fn write_new(&self, path: &Path) -> Result<(), String> {
        let raw = serde_json::to_vec_pretty(&self.to_json()).map_err(|e| e.to_string())?;
        StagedFile::new(path, &raw)
            .and_then(StagedFile::create)
            .map_err(|e| format!("无法创建备份（不会覆盖已有文件）：{e}"))
    }

    fn contents(&self) -> Result<[Option<String>; 2], String> {
        Ok([
            self.ime
                .as_ref()
                .map(serde_json::to_string_pretty)
                .transpose()
                .map_err(|e| e.to_string())?,
            self.panel.as_ref().map(|fields| {
                fields
                    .iter()
                    .map(|(k, v)| format!("{k}={}\n", v.as_str().unwrap_or_default()))
                    .collect()
            }),
        ])
    }
}

pub fn backup_now(paths: &DataPaths) -> Result<PathBuf, String> {
    let _lease = DataLease::acquire(&paths.lock_path()?, false)?;
    let path = new_backup_path(paths, "settings");
    Backup::collect(paths)?.write_new(&path)?;
    Ok(path)
}

fn new_backup_path(paths: &DataPaths, prefix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    paths
        .backups
        .join(format!("{prefix}-{timestamp}-{}.json", std::process::id()))
}

pub fn preview(paths: &DataPaths, backup: &Backup) -> Result<Vec<String>, String> {
    let mut report = Vec::new();
    for (path, next) in [&paths.ime, &paths.panel]
        .into_iter()
        .zip(backup.contents()?)
    {
        let before = read_setting(path)?;
        let operation = if before == next {
            "不变"
        } else if next.is_none() {
            "移除设置文件，使用默认值"
        } else {
            "写入设置"
        };
        report.push(format!("{operation}: {}", path.display()));
    }
    Ok(report)
}

/// Linux-only until other native hosts also participate in the maintenance gate.
/// Files are individually atomic; a durable before-restore backup covers crash recovery.
#[cfg(target_os = "linux")]
pub fn restore(paths: &DataPaths, backup: &Backup) -> Result<Option<PathBuf>, String> {
    let _lease = DataLease::acquire(&paths.lock_path()?, true)?;
    refuse_legacy_runtime()?;
    restore_locked(paths, backup)
}

#[cfg(target_os = "linux")]
fn refuse_legacy_runtime() -> Result<(), String> {
    // Old releases do not hold DataLease. Be conservative even for stale sockets; never unlink them.
    let mut sockets = Vec::new();
    if let Some(root) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        let root = PathBuf::from(root);
        sockets.push(root.join("suzaku-ime/host.sock"));
        sockets.push(root.join("suzaku-panel/control.sock"));
    }
    if let Some(socket) = std::env::var_os("SUZAKU_LINUX_IME_SOCKET") {
        sockets.push(socket.into());
    }
    for path in sockets {
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(format!(
                    "请先退出面板并停止 suzaku-ibus.service；运行时端点仍存在：{}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(any(test, target_os = "linux"))]
fn restore_locked(paths: &DataPaths, backup: &Backup) -> Result<Option<PathBuf>, String> {
    restore_with_publish(paths, backup, publish)
}

#[cfg(any(test, target_os = "linux"))]
fn restore_with_publish(
    paths: &DataPaths,
    backup: &Backup,
    mut publish_file: impl FnMut(&Path, Option<StagedFile>) -> std::io::Result<()>,
) -> Result<Option<PathBuf>, String> {
    // Validate again, including values constructed by Rust callers.
    let checked = Backup::parse(&backup.to_json().to_string())?;
    let destinations = [&paths.ime, &paths.panel];
    let lock_path = paths.lock_path()?;
    if paths.panel == lock_path {
        return Err("面板设置不能指向维护锁".into());
    }
    if paths.ime == paths.panel {
        return Err("输入法和面板设置不能指向同一文件".into());
    }
    let previous = [read_setting(&paths.ime)?, read_setting(&paths.panel)?];
    let desired = checked.contents()?;
    if previous == desired {
        return Ok(None);
    }
    let safety = Backup::collect(paths)?;
    // Stage BOTH the new files and rollback copies before changing any settings.
    let stage = |contents: &[Option<String>; 2]| -> Result<Vec<Option<StagedFile>>, String> {
        destinations
            .iter()
            .zip(contents)
            .map(|(path, raw)| {
                raw.as_ref()
                    .map(|raw| StagedFile::new(path, raw.as_bytes()))
                    .transpose()
                    .map_err(|e| e.to_string())
            })
            .collect()
    };
    let new_files = stage(&desired)?;
    let mut rollback_files = stage(&previous)?;
    // Existing aliases, or aliases through symlinked/../ parents, must not let the
    // two logical entries replace one another. Staging has now created the parents.
    let resolved = |path: &Path| {
        fs::canonicalize(path).or_else(|_| {
            fs::canonicalize(path.parent().unwrap_or(Path::new(".")))
                .map(|parent| parent.join(path.file_name().unwrap_or_default()))
        })
    };
    if resolved(&paths.ime)
        .ok()
        .zip(resolved(&paths.panel).ok())
        .is_some_and(|(a, b)| a == b)
    {
        return Err("输入法和面板设置解析到同一文件".into());
    }
    let safety_path = new_backup_path(paths, "before-restore");
    safety.write_new(&safety_path)?;
    for (index, staged) in new_files.into_iter().enumerate() {
        if let Err(error) = publish_file(destinations[index], staged) {
            let mut failures = Vec::new();
            for restored in 0..index {
                if let Err(error) =
                    publish_file(destinations[restored], rollback_files[restored].take())
                {
                    failures.push(error.to_string());
                }
            }
            return Err(format!(
                "恢复失败：{error}；回退{}；恢复前备份：{}",
                if failures.is_empty() {
                    "完成".into()
                } else {
                    format!("失败：{}", failures.join(", "))
                },
                safety_path.display()
            ));
        }
    }
    Ok(Some(safety_path))
}

#[cfg(any(test, target_os = "linux"))]
fn publish(path: &Path, staged: Option<StagedFile>) -> std::io::Result<()> {
    if let Some(staged) = staged {
        staged.replace()
    } else {
        match fs::remove_file(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
