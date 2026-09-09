use std::env;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use crate::platform::linux;
#[cfg(target_os = "windows")]
use crate::platform::windows;

const SETTINGS_FILE_NAME: &str = "panel-settings.toml";

pub fn display_settings_path() -> PathBuf {
    settings_directory().join(SETTINGS_FILE_NAME)
}

fn settings_directory() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("SuzakuPanel");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata).join(windows::settings_directory_name());
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(config_home) = crate::data::paths::config_home() {
            return config_home.join(linux::ubuntu_settings_directory_name());
        }
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".suzaku-panel")
}

#[cfg(test)]
mod tests {
    use super::display_settings_path;

    #[test]
    fn settings_path_ends_with_named_settings_file() {
        let path = display_settings_path();

        assert!(path.ends_with("panel-settings.toml"));
    }
}
