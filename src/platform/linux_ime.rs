use super::{TargetPlatform, linux};

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
    LinuxImeBootstrap {
        framework: detected_framework(),
        host_platform: platform,
        daemon_detected: linux::linux_voice_portal_available()
            || linux::linux_voice_pipewire_available(),
        host_registration_ready: false,
        marked_text_roundtrip_ready: false,
        commit_roundtrip_ready: false,
        native_candidate_window_ready: false,
        recommended_connection_name: recommended_connection_name(),
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
            assert!(!bootstrap.host_registration_ready);
        });
    }
}
