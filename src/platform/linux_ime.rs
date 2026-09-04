use super::TargetPlatform;

#[cfg(not(test))]
use std::sync::{Mutex, OnceLock};
#[cfg(not(test))]
use std::time::{Duration, Instant};

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
    pub runtime_engine_visible: bool,
    pub engine_active: bool,
    pub host_service_ready: bool,
    pub marked_text_roundtrip_ready: bool,
    pub commit_roundtrip_ready: bool,
    pub native_candidate_window_ready: bool,
    pub recommended_connection_name: String,
}

impl LinuxImeBootstrap {
    pub fn describe(&self) -> String {
        format!(
            "framework: {:?} | platform: {:?} | daemon detected: {} | host registration ready: {} | runtime visible: {} | engine active: {} | host service ready: {} | marked text: {} | commit: {} | native candidates: {} | recommended connection: {}",
            self.framework,
            self.host_platform,
            self.daemon_detected,
            self.host_registration_ready,
            self.runtime_engine_visible,
            self.engine_active,
            self.host_service_ready,
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

pub fn recommended_component_name() -> String {
    "org.freedesktop.IBus.Suzaku".to_string()
}

#[cfg(test)]
pub fn bootstrap_status(platform: TargetPlatform) -> LinuxImeBootstrap {
    bootstrap_status_uncached(platform)
}

#[cfg(not(test))]
pub fn bootstrap_status(platform: TargetPlatform) -> LinuxImeBootstrap {
    cached_bootstrap_status(platform)
}

fn bootstrap_status_uncached(platform: TargetPlatform) -> LinuxImeBootstrap {
    let framework = detected_framework();
    let recommended_connection_name = recommended_connection_name();
    let daemon_detected = framework_daemon_detected(&framework);
    let runtime_engine_visible = env_flag_override("SUZAKU_LINUX_IME_RUNTIME_VISIBLE")
        .unwrap_or_else(|| {
            framework_runtime_engine_visible(&framework, &recommended_connection_name)
        });
    let engine_active = env_flag_override("SUZAKU_LINUX_IME_ACTIVE")
        .unwrap_or_else(|| framework_engine_active(&framework, &recommended_connection_name));
    let host_registration_ready =
        env_flag_override("SUZAKU_LINUX_IME_REGISTERED").unwrap_or_else(|| {
            runtime_engine_visible
                || framework_registration_marker(&framework, &recommended_connection_name)
        });
    let host_service_ready = env_flag_override("SUZAKU_LINUX_IME_HOST_READY")
        .unwrap_or_else(|| framework_host_service_detected(&framework));
    let roundtrip_capable =
        daemon_detected && host_registration_ready && runtime_engine_visible && host_service_ready;
    LinuxImeBootstrap {
        framework,
        host_platform: platform,
        daemon_detected,
        host_registration_ready,
        runtime_engine_visible,
        engine_active,
        host_service_ready,
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

#[cfg(not(test))]
fn cached_bootstrap_status(platform: TargetPlatform) -> LinuxImeBootstrap {
    const CACHE_LIFETIME: Duration = Duration::from_secs(3);
    type CacheEntry = (Instant, TargetPlatform, LinuxImeBootstrap);
    static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((checked_at, cached_platform, bootstrap)) = guard.as_ref()
        && *cached_platform == platform
        && checked_at.elapsed() < CACHE_LIFETIME
    {
        return bootstrap.clone();
    }

    let bootstrap = bootstrap_status_uncached(platform);
    *guard = Some((Instant::now(), platform, bootstrap.clone()));
    bootstrap
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
        LinuxImeFramework::IBus => {
            process_has_name("ibus-daemon")
                || process_has_name("ibus-x11")
                || process_has_name("ibus-portal")
        }
        LinuxImeFramework::Fcitx => {
            process_has_name("fcitx") || process_has_name("fcitx5") || process_has_name("fcitx5-qt")
        }
    }
}

fn framework_registration_marker(framework: &LinuxImeFramework, connection: &str) -> bool {
    match framework {
        LinuxImeFramework::IBus => has_ibus_component_marker(connection),
        LinuxImeFramework::Fcitx => has_fcitx_registration_marker(connection),
    }
}

fn framework_runtime_engine_visible(framework: &LinuxImeFramework, connection: &str) -> bool {
    match framework {
        LinuxImeFramework::IBus => ibus_runtime_has_engine(connection),
        LinuxImeFramework::Fcitx => fcitx_active_engine(connection),
    }
}

fn framework_engine_active(framework: &LinuxImeFramework, connection: &str) -> bool {
    match framework {
        LinuxImeFramework::IBus => ibus_active_engine(connection),
        LinuxImeFramework::Fcitx => fcitx_active_engine(connection),
    }
}

fn framework_host_service_detected(framework: &LinuxImeFramework) -> bool {
    match framework {
        LinuxImeFramework::IBus => process_has_name("linux_ime_host"),
        LinuxImeFramework::Fcitx => false,
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
        .output()
        .is_ok_and(|output| output.status.success())
}

fn ibus_runtime_has_engine(connection_name: &str) -> bool {
    command_stdout("ibus", &["list-engine"]).is_some_and(|stdout| {
        stdout
            .lines()
            .any(|line| line.split_whitespace().next() == Some(connection_name))
    }) || ibus_dynamic_registry_has_engine(connection_name)
}

fn ibus_dynamic_registry_has_engine(connection_name: &str) -> bool {
    let Some(address) = command_stdout("ibus", &["address"]) else {
        return false;
    };
    let address = address.trim();
    if address.is_empty() {
        return false;
    }
    command_stdout(
        "gdbus",
        &[
            "call",
            "--address",
            address,
            "--dest",
            "org.freedesktop.IBus",
            "--object-path",
            "/org/freedesktop/IBus",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.IBus",
            "ActiveEngines",
        ],
    )
    .is_some_and(|stdout| stdout.contains(connection_name))
}

fn ibus_active_engine(connection_name: &str) -> bool {
    command_stdout("ibus", &["engine"]).is_some_and(|stdout| stdout.trim() == connection_name)
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
        if contents.contains(connection_name)
            && contents.contains(&recommended_component_name())
            && contents.contains("<exec>")
            && contents.contains("</exec>")
        {
            return true;
        }
    }

    false
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let response = std::process::Command::new(command)
        .args(args)
        .output()
        .ok()?;
    if !response.status.success() {
        return None;
    }
    String::from_utf8(response.stdout).ok()
}

fn has_fcitx_registration_marker(connection_name: &str) -> bool {
    if has_fcitx_config_containing(".local/share/fcitx5/inputmethod")
        || has_fcitx_config_containing(".config/fcitx")
        || has_fcitx_config_containing(".config/fcitx5/inputmethod")
    {
        return true;
    }

    has_fcitx_config_file_named(connection_name)
}

fn fcitx_active_engine(connection_name: &str) -> bool {
    command_stdout("fcitx5-remote", &["-n"]).is_some_and(|stdout| {
        let active = stdout.trim();
        active == connection_name || active.eq_ignore_ascii_case("suzaku")
    })
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

        if let Ok(contents) = std::fs::read_to_string(&path)
            && (contents.contains("suzaku") || contents.contains("Suzaku"))
        {
            return true;
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
        std::path::Path::new(&home)
            .join(".local/share/fcitx5/inputmethod")
            .join(filename.as_str()),
        std::path::Path::new(&home)
            .join(".config/fcitx")
            .join("inputmethod")
            .join(filename.as_str()),
        std::path::Path::new(&home)
            .join(".config/fcitx5/inputmethod")
            .join(filename.as_str()),
    ];

    paths.iter().any(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxImeFramework, bootstrap_status, recommended_component_name,
        recommended_connection_name,
    };
    use crate::platform::TargetPlatform;
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;

    #[test]
    fn linux_ime_connection_name_stays_stable() {
        assert_eq!(recommended_connection_name(), "dev.suzaku.linux.ime");
        assert_eq!(recommended_component_name(), "org.freedesktop.IBus.Suzaku");
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
            env.set_var("SUZAKU_LINUX_IME_RUNTIME_VISIBLE", "1");
            env.set_var("SUZAKU_LINUX_IME_HOST_READY", "1");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert!(bootstrap.runtime_engine_visible);
            assert!(bootstrap.host_service_ready);
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

    #[test]
    fn registration_marker_and_daemon_do_not_imply_roundtrip_readiness() {
        test_env::with_test_env(|env: &mut ScopedEnv| {
            env.set_var("SUZAKU_LINUX_IME_REGISTERED", "1");
            env.set_var("SUZAKU_LINUX_IME_DAEMON_READY", "1");
            env.set_var("SUZAKU_LINUX_IME_RUNTIME_VISIBLE", "0");
            env.set_var("SUZAKU_LINUX_IME_HOST_READY", "0");
            env.remove_var("SUZAKU_LINUX_IME_MARKED_TEXT");
            env.remove_var("SUZAKU_LINUX_IME_COMMIT");
            env.remove_var("SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW");

            let bootstrap = bootstrap_status(TargetPlatform::Ubuntu);
            assert!(bootstrap.host_registration_ready);
            assert!(!bootstrap.runtime_engine_visible);
            assert!(!bootstrap.host_service_ready);
            assert!(!bootstrap.marked_text_roundtrip_ready);
            assert!(!bootstrap.commit_roundtrip_ready);
            assert!(!bootstrap.native_candidate_window_ready);
        });
    }
}
