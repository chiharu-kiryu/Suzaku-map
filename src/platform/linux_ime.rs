use super::TargetPlatform;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinuxImeFramework {
    IBus,
    Fcitx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxImeBootstrap {
    pub framework: LinuxImeFramework,
    pub host_platform: TargetPlatform,
    pub daemon_detected: bool,
    pub host_registration_ready: bool,
    pub marked_text_roundtrip_ready: bool,
    pub commit_roundtrip_ready: bool,
    pub native_candidate_window_ready: bool,
    pub recommended_connection_name: String,
}

impl LinuxImeBootstrap {
    pub fn describe(&self) -> String {
        format!(
            "framework: {:?} | platform: {:?} | daemon detected: {} | host registration ready: {} | marked text: {} | commit: {} | native candidates: {} | recommended connection: {}",
            self.framework,
            self.host_platform,
            self.daemon_detected,
            self.host_registration_ready,
            self.marked_text_roundtrip_ready,
            self.commit_roundtrip_ready,
            self.native_candidate_window_ready,
            self.recommended_connection_name
        )
    }
}

pub fn recommended_connection_name() -> String {
    "dev.suzaku.linux.ime".to_string()
}

pub fn bootstrap_status(platform: TargetPlatform) -> LinuxImeBootstrap {
    let framework = detected_framework();
    let recommended_connection_name = recommended_connection_name();
    let daemon_detected = framework_daemon_detected(&framework);
    let host_registration_ready = framework_host_registered(&framework, &recommended_connection_name);
    let roundtrip_capable = host_registration_ready && daemon_detected;
    LinuxImeBootstrap {
        framework,
        host_platform: platform,
        daemon_detected,
        host_registration_ready,
        marked_text_roundtrip_ready: env_flag_override_or("SUZAKU_LINUX_IME_MARKED_TEXT")
            .unwrap_or(roundtrip_capable),
        commit_roundtrip_ready: env_flag_override_or("SUZAKU_LINUX_IME_COMMIT")
            .unwrap_or(roundtrip_capable),
        native_candidate_window_ready: env_flag_override_or(
            "SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW",
        )
        .unwrap_or(roundtrip_capable),
        recommended_connection_name,
    }
}

fn detected_framework() -> LinuxImeFramework {
    if std::env::var("SUZAKU_LINUX_IME_FRAMEWORK")
        .map(|value| value.eq_ignore_ascii_case("fcitx"))
        .unwrap_or(false)
    {
        LinuxImeFramework::Fcitx
    } else {
        LinuxImeFramework::IBus
    }
}

fn framework_daemon_detected(framework: &LinuxImeFramework) -> bool {
    if let Some(value) = env_flag_override("SUZAKU_LINUX_IME_DAEMON_READY") {
        return value;
    }

    match framework {
        LinuxImeFramework::IBus => process_has_name("ibus-daemon")
            || process_has_name("ibus-x11")
            || process_has_name("ibus-portal"),
        LinuxImeFramework::Fcitx => {
            process_has_name("fcitx")
                || process_has_name("fcitx5")
                || process_has_name("fcitx5-qt")
        }
    }
}

fn framework_host_registered(framework: &LinuxImeFramework, connection: &str) -> bool {
    if let Some(override_ready) = env_flag_override("SUZAKU_LINUX_IME_REGISTERED") {
        return override_ready;
    }

    match framework {
        LinuxImeFramework::IBus => ibus_engine_registered(connection),
        LinuxImeFramework::Fcitx => fcitx_engine_registered(connection),
    }
}

fn env_flag_override(key: &str) -> Option<bool> {
    parse_bool_env(std::env::var(key).ok()?)
}

fn env_flag_override_or(key: &str) -> Option<bool> {
    env_flag_override(key)
}

fn parse_bool_env(value: String) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn process_has_name(process_name: &str) -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg(process_name)
        .status()
        .is_ok_and(|status| status.success())
}

fn ibus_engine_registered(connection_name: &str) -> bool {
    let output = std::process::Command::new("ibus")
        .arg("list-engine")
        .output();

    if let Ok(response) = &output
        && response.status.success()
    {
        if let Ok(stdout) = String::from_utf8(response.stdout.clone()) {
            if stdout
                .lines()
                .any(|line| line.trim().split_whitespace().next().unwrap_or("") == connection_name)
            {
                return true;
            }
        }
    }

    has_ibus_component_marker(connection_name)
}

fn has_ibus_component_marker(connection_name: &str) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let base = std::path::Path::new(&home).join(".local/share/ibus/component");
    let entries = match std::fs::read_dir(base) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("xml") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        if contents.contains(connection_name) {
            return true;
        }
    }

    false
}

#[allow(clippy::unused_io_amount)]
fn _output_contains_connection(response: std::process::Output, connection_name: &str) -> bool {
    String::from_utf8(response.stdout)
        .ok()
        .is_some_and(|stdout| {
            stdout
                .lines()
                .any(|line| line.trim().split_whitespace().next().unwrap_or("") == connection_name)
        })
}

fn fcitx_engine_registered(connection_name: &str) -> bool {
    if has_fcitx_config_containing(".local/share/fcitx5/inputmethod")
        || has_fcitx_config_containing(".config/fcitx")
        || has_fcitx_config_containing(".config/fcitx5/inputmethod")
    {
        return true;
    }

    has_fcitx_config_file_named(connection_name)
}

fn has_fcitx_config_containing(relative_path: &str) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let base = std::path::Path::new(&home).join(relative_path);
    let entries = match std::fs::read_dir(base) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("conf") {
            continue;
        }

        if let Ok(contents) = std::fs::read_to_string(&path) {
            if contents.contains("suzaku") || contents.contains("Suzaku") {
                return true;
            }
        }
    }

    false
}

fn has_fcitx_config_file_named(connection_name: &str) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let filename = format!("{}.conf", connection_name.replace('.', "_"));
    let paths = [
        std::path::Path::new(&home).join(".local/share/fcitx5/inputmethod").join(filename.as_str()),
        std::path::Path::new(&home).join(".config/fcitx").join("inputmethod").join(filename.as_str()),
        std::path::Path::new(&home)
            .join(".config/fcitx5/inputmethod")
            .join(filename.as_str()),
    ];

    paths.iter().any(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::{LinuxImeFramework, bootstrap_status, recommended_connection_name};
    use crate::platform::TargetPlatform;
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;

    #[test]
    fn linux_ime_connection_name_stays_stable() {
        assert_eq!(recommended_connection_name(), "dev.suzaku.linux.ime");
    }

    #[test]
    fn linux_ime_bootstrap_defaults_to_ibus() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.remove_var("SUZAKU_LINUX_IME_FRAMEWORK");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert_eq!(bootstrap.framework, LinuxImeFramework::IBus);
        });
    }

    #[test]
    fn linux_ime_bootstrap_reads_registration_override() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_IME_REGISTERED", "1");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert!(bootstrap.host_registration_ready);
        });
    }

    #[test]
    fn linux_ime_bootstrap_reports_fcitx_when_explicit() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_IME_FRAMEWORK", "fcitx");
            env.set_var("SUZAKU_LINUX_IME_REGISTERED", "0");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert_eq!(bootstrap.framework, LinuxImeFramework::Fcitx);
            assert!(!bootstrap.host_registration_ready);
        });
    }

    #[test]
    fn linux_ime_bootstrap_can_report_roundtrip_capabilities_with_overrides() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_IME_REGISTERED", "1");
            env.set_var("SUZAKU_LINUX_IME_DAEMON_READY", "1");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert!(bootstrap.marked_text_roundtrip_ready);
            assert!(bootstrap.commit_roundtrip_ready);
            assert!(bootstrap.native_candidate_window_ready);

            env.set_var("SUZAKU_LINUX_IME_MARKED_TEXT", "0");
            env.set_var("SUZAKU_LINUX_IME_COMMIT", "0");
            env.set_var("SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW", "0");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert!(!bootstrap.marked_text_roundtrip_ready);
            assert!(!bootstrap.commit_roundtrip_ready);
            assert!(!bootstrap.native_candidate_window_ready);
        });
    }
}
