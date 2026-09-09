use super::{SupportTier, TargetPlatform, host_platform, linux_ime, support_for};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelCompanionRole {
    PrimaryPanelHost,
    DebugCompanion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelCompanionDispatch {
    pub platform: TargetPlatform,
    pub tier: SupportTier,
    pub gpu_panel: bool,
    pub system_ime_host: bool,
    pub role: PanelCompanionRole,
    pub notes: String,
}

impl PanelCompanionDispatch {
    pub fn describe(&self) -> String {
        format!(
            "platform={:?} | tier={:?} | gpu_panel={} | system_host={} | role={:?} | notes={}",
            self.platform, self.tier, self.gpu_panel, self.system_ime_host, self.role, self.notes
        )
    }

    pub fn default_panel_title(&self) -> &'static str {
        match self.role {
            PanelCompanionRole::PrimaryPanelHost => "Suzaku XR Candidate Panel",
            PanelCompanionRole::DebugCompanion => "Suzaku XR Candidate Panel · Debug Companion",
        }
    }
}

/// UI-safe on Linux; shares the background status cache with the IME dispatcher.
pub fn current_panel_companion_dispatch() -> PanelCompanionDispatch {
    dispatch(host_platform(), true)
}

/// Diagnostic view; may wait for bounded native status probes on Linux.
pub fn dispatch_for(platform: TargetPlatform) -> PanelCompanionDispatch {
    dispatch(platform, false)
}

fn dispatch(platform: TargetPlatform, nonblocking: bool) -> PanelCompanionDispatch {
    let support = support_for(platform);
    let system_ime_host = match platform {
        TargetPlatform::Ubuntu | TargetPlatform::ArchLinux | TargetPlatform::SteamOs => {
            let bootstrap = if nonblocking {
                linux_ime::bootstrap_status_nonblocking(platform)
            } else {
                linux_ime::bootstrap_status(platform)
            };
            support.capabilities.system_ime_host
                || (bootstrap.host_registration_ready
                    && bootstrap.runtime_engine_visible
                    && bootstrap.host_service_ready)
        }
        _ => support.capabilities.system_ime_host,
    };
    let role = if system_ime_host {
        PanelCompanionRole::DebugCompanion
    } else {
        PanelCompanionRole::PrimaryPanelHost
    };

    let notes = match role {
        PanelCompanionRole::DebugCompanion => {
            "The GPU panel is a debug and extended companion surface; system IME host lifecycle should remain primary on this platform."
                .to_string()
        }
        PanelCompanionRole::PrimaryPanelHost => {
            "The GPU panel remains the primary interactive surface until a native system IME host is available on this platform."
                .to_string()
        }
    };

    PanelCompanionDispatch {
        platform,
        tier: support.tier,
        gpu_panel: support.capabilities.gpu_panel,
        system_ime_host,
        role,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::{PanelCompanionRole, current_panel_companion_dispatch, dispatch_for};
    use crate::platform::test_env;
    use crate::platform::test_env::ScopedEnv;
    use crate::platform::{TargetPlatform, host_platform};

    #[test]
    fn panel_dispatch_matches_host_platform() {
        assert_eq!(current_panel_companion_dispatch().platform, host_platform());
    }

    #[test]
    fn macos_and_windows_use_debug_companion_role() {
        for platform in [TargetPlatform::MacOs, TargetPlatform::Windows] {
            assert_eq!(
                dispatch_for(platform).role,
                PanelCompanionRole::DebugCompanion
            );
        }
    }

    #[test]
    fn android_uses_debug_companion_role() {
        assert_eq!(
            dispatch_for(TargetPlatform::Android).role,
            PanelCompanionRole::DebugCompanion
        );
    }

    #[test]
    fn linux_targets_keep_primary_panel_role_for_now() {
        for platform in [
            TargetPlatform::Ubuntu,
            TargetPlatform::ArchLinux,
            TargetPlatform::SteamOs,
        ] {
            test_env::with_test_env(|env: &mut ScopedEnv| {
                env.set_var("SUZAKU_LINUX_IME_REGISTERED", "0");
                env.set_var("SUZAKU_LINUX_IME_RUNTIME_VISIBLE", "0");
                env.set_var("SUZAKU_LINUX_IME_HOST_READY", "0");

                assert_eq!(
                    dispatch_for(platform).role,
                    PanelCompanionRole::PrimaryPanelHost
                );
            });
        }
    }

    #[test]
    fn linux_targets_switch_to_debug_companion_when_host_is_live() {
        for platform in [
            TargetPlatform::Ubuntu,
            TargetPlatform::ArchLinux,
            TargetPlatform::SteamOs,
        ] {
            test_env::with_test_env(|env: &mut ScopedEnv| {
                env.set_var("SUZAKU_LINUX_IME_REGISTERED", "1");
                env.set_var("SUZAKU_LINUX_IME_DAEMON_READY", "1");
                env.set_var("SUZAKU_LINUX_IME_RUNTIME_VISIBLE", "1");
                env.set_var("SUZAKU_LINUX_IME_HOST_READY", "1");

                assert_eq!(
                    dispatch_for(platform).role,
                    PanelCompanionRole::DebugCompanion
                );
            });
        }
    }
}
