use std::{env, path::PathBuf};

fn absolute(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

/// Empty/relative XDG values are ignored, as required by the base-directory specification.
pub fn xdg_home(name: &str, fallback: &str) -> Option<PathBuf> {
    absolute(env::var_os(name))
        .or_else(|| absolute(env::var_os("HOME")).map(|home| home.join(fallback)))
}

pub fn config_home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    return absolute(env::var_os("APPDATA"));
    #[cfg(target_os = "macos")]
    return absolute(env::var_os("HOME")).map(|home| home.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    xdg_home("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> Option<PathBuf> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    return config_home();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    xdg_home("XDG_DATA_HOME", ".local/share")
}

#[derive(Clone, Debug)]
pub struct DataPaths {
    pub ime: PathBuf,
    pub panel: PathBuf,
    pub backups: PathBuf,
}

impl DataPaths {
    pub fn current() -> Result<Self, String> {
        let cwd = env::current_dir().map_err(|error| error.to_string())?;
        let ime = crate::ime::settings::settings_path().ok_or("无法定位输入法设置")?;
        let panel = crate::platform::settings_host::display_settings_path();
        Ok(Self {
            ime: cwd.join(ime),
            panel: cwd.join(panel),
            backups: data_home()
                .ok_or("无法定位用户数据目录")?
                .join("suzaku/backups"),
        })
    }

    pub fn lock_path(&self) -> Result<PathBuf, String> {
        lock_for_settings(&self.ime)
    }
}

pub fn lock_for_settings(path: &std::path::Path) -> Result<PathBuf, String> {
    let name = path.file_name().ok_or("设置路径必须指向文件")?;
    if name == ".suzaku-data.lock" {
        return Err("设置文件不能使用保留的维护锁名称".into());
    }
    let absolute = env::current_dir().map_err(|e| e.to_string())?.join(path);
    Ok(absolute
        .parent()
        .ok_or("设置路径没有父目录")?
        .join(".suzaku-data.lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_values_must_be_absolute_and_nonempty() {
        assert!(absolute(None).is_none());
        assert!(absolute(Some("".into())).is_none());
        assert!(absolute(Some("relative/path".into())).is_none());
        let path = env::temp_dir().join("suzaku-data-path-test");
        assert_eq!(absolute(Some(path.clone().into_os_string())), Some(path));
    }
}
