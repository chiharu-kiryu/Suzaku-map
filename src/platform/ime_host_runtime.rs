use crate::ime::EngineConfig;
use crate::ime_host::HostImeSession;

use super::{
    TargetPlatform, host_platform, ime_host_adapter, ime_host_dispatch, panel_companion_dispatch,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeHostRuntimeProfile {
    pub platform: TargetPlatform,
    pub title: String,
    pub bootstrap_summary: String,
    pub registration_target: String,
    pub registration_ready: bool,
    pub registration_hint: String,
    pub preferred_command: &'static str,
    pub dispatch: ime_host_dispatch::ImeHostDispatch,
    pub panel_dispatch: panel_companion_dispatch::PanelCompanionDispatch,
}

impl ImeHostRuntimeProfile {
    pub fn describe(&self) -> String {
        format!(
            "{}\nbootstrap: {}\nregistration: {}\nime-dispatch: {}\npanel-dispatch: {}\nregistration-hint: {}\npreferred-command: {}",
            self.title,
            self.bootstrap_summary,
            format!(
                "target=\"{}\" ready={}",
                self.registration_target, self.registration_ready
            ),
            self.dispatch.describe(),
            self.panel_dispatch.describe(),
            self.registration_hint,
            self.preferred_command
        )
    }

    pub fn render_report(&self, include_session_probe: bool) -> String {
        let mut lines = vec![self.describe()];

        if include_session_probe {
            let mut session = HostImeSession::new(EngineConfig::default());
            let update = session.activate();
            lines.push(format!(
                "session-probe: active={} candidates={} committed=\"{}\"",
                update.active,
                update.candidates.len(),
                update.committed_text
            ));
        }

        lines.join("\n")
    }
}

pub fn current_runtime_profile() -> ImeHostRuntimeProfile {
    runtime_profile_for(host_platform())
}

pub fn runtime_profile_for(platform: TargetPlatform) -> ImeHostRuntimeProfile {
    let adapter = ime_host_adapter::adapter_profile_for(platform);
    ImeHostRuntimeProfile {
        platform,
        title: runtime_title(platform),
        bootstrap_summary: bootstrap_summary(platform),
        registration_target: adapter.registration_target,
        registration_ready: adapter.registration_ready,
        registration_hint: registration_hint(platform),
        preferred_command: preferred_command(platform),
        dispatch: ime_host_dispatch::dispatch_for(platform),
        panel_dispatch: panel_companion_dispatch::dispatch_for(platform),
    }
}

pub fn current_runtime_report(include_session_probe: bool) -> String {
    current_runtime_profile().render_report(include_session_probe)
}

pub fn runtime_report_for(platform: TargetPlatform, include_session_probe: bool) -> String {
    runtime_profile_for(platform).render_report(include_session_probe)
}

fn runtime_title(platform: TargetPlatform) -> String {
    match platform {
        TargetPlatform::MacOs => "Suzaku macOS IME Host".to_string(),
        TargetPlatform::Windows => "Suzaku Windows IME Host".to_string(),
        TargetPlatform::Android => "Suzaku Android IME Host".to_string(),
        TargetPlatform::Ubuntu => "Suzaku Ubuntu IME Host".to_string(),
        TargetPlatform::ArchLinux => "Suzaku Arch Linux IME Host".to_string(),
        TargetPlatform::SteamOs => "Suzaku SteamOS IME Host".to_string(),
    }
}

fn bootstrap_summary(platform: TargetPlatform) -> String {
    ime_host_adapter::adapter_profile_for(platform).bootstrap_summary
}

fn registration_hint(platform: TargetPlatform) -> String {
    ime_host_adapter::adapter_profile_for(platform).registration_hint
}

fn preferred_command(platform: TargetPlatform) -> &'static str {
    match platform {
        TargetPlatform::MacOs => "cargo ime-host",
        TargetPlatform::Windows => "cargo ime-host",
        TargetPlatform::Android => "cargo android-ime-host",
        TargetPlatform::Ubuntu | TargetPlatform::ArchLinux | TargetPlatform::SteamOs => {
            "cargo ime-host"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        current_runtime_profile, current_runtime_report, runtime_profile_for, runtime_report_for,
    };
    use crate::platform::{TargetPlatform, host_platform};

    #[test]
    fn runtime_profile_tracks_host_platform() {
        assert_eq!(current_runtime_profile().platform, host_platform());
    }

    #[test]
    fn macos_profile_mentions_input_methodkit() {
        let profile = runtime_profile_for(TargetPlatform::MacOs);
        assert!(profile.bootstrap_summary.contains("IMK available"));
        assert!(profile.registration_hint.contains("InputMethodKit"));
    }

    #[test]
    fn android_profile_mentions_input_method_service() {
        let profile = runtime_profile_for(TargetPlatform::Android);
        assert!(profile.registration_hint.contains("InputMethodService"));
    }

    #[test]
    fn runtime_profile_contains_expected_preferred_command() {
        assert_eq!(
            runtime_profile_for(TargetPlatform::MacOs).preferred_command,
            "cargo ime-host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::Windows).preferred_command,
            "cargo ime-host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::Android).preferred_command,
            "cargo android-ime-host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::Ubuntu).preferred_command,
            "cargo ime-host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::ArchLinux).preferred_command,
            "cargo ime-host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::SteamOs).preferred_command,
            "cargo ime-host"
        );
    }

    #[test]
    fn runtime_profile_includes_platform_title_for_steam_and_arch_linux() {
        assert_eq!(
            runtime_profile_for(TargetPlatform::ArchLinux).title,
            "Suzaku Arch Linux IME Host"
        );
        assert_eq!(
            runtime_profile_for(TargetPlatform::SteamOs).title,
            "Suzaku SteamOS IME Host"
        );
    }

    #[test]
    fn runtime_report_can_include_session_probe_line() {
        let compact = runtime_report_for(TargetPlatform::Ubuntu, false);
        let with_probe = runtime_report_for(TargetPlatform::Ubuntu, true);

        assert!(!compact.contains("session-probe:"));
        assert!(with_probe.contains("session-probe:"));
        assert!(with_probe.contains("committed=\"\""));
    }

    #[test]
    fn runtime_report_for_platform_includes_profile_title_and_preferred_command() {
        let report = runtime_report_for(TargetPlatform::SteamOs, false);

        assert!(report.contains("Suzaku SteamOS IME Host"));
        assert!(report.contains("preferred-command: cargo ime-host"));
        assert!(report.contains("registration-hint"));
    }

    #[test]
    fn runtime_profile_describe_mentions_all_sections() {
        let profile = runtime_profile_for(TargetPlatform::Windows);
        let description = profile.describe();

        assert!(description.contains(&profile.title));
        assert!(description.contains("bootstrap:"));
        assert!(description.contains("registration:"));
        assert!(description.contains("ime-dispatch:"));
        assert!(description.contains("panel-dispatch:"));
        assert!(description.contains("registration-hint:"));
        assert!(description.contains("preferred-command:"));
    }

    #[test]
    fn current_runtime_report_can_toggle_session_probe_line() {
        let without_probe = current_runtime_report(false);
        let with_probe = current_runtime_report(true);

        assert!(!without_probe.contains("session-probe:"));
        assert!(with_probe.contains("session-probe:"));
        assert!(with_probe.contains("committed=\"\""));
    }

    #[test]
    fn current_runtime_report_includes_profile_title_and_preferred_command() {
        let profile = current_runtime_profile();
        let report = current_runtime_report(false);

        assert!(report.contains(&profile.title));
        assert!(report.contains(&format!(
            "preferred-command: {}",
            profile.preferred_command
        )));
    }

    #[test]
    fn current_runtime_report_matches_runtime_report_for_host_platform() {
        let platform = host_platform();

        assert_eq!(
            current_runtime_report(false),
            runtime_report_for(platform, false)
        );
        assert_eq!(
            current_runtime_report(true),
            runtime_report_for(platform, true)
        );
    }

    #[test]
    fn runtime_report_for_all_platforms_exposes_profile_fields() {
        let platforms = [
            TargetPlatform::MacOs,
            TargetPlatform::Windows,
            TargetPlatform::Android,
            TargetPlatform::Ubuntu,
            TargetPlatform::ArchLinux,
            TargetPlatform::SteamOs,
        ];

        for platform in platforms {
            let profile = runtime_profile_for(platform);
            let compact = runtime_report_for(platform, false);
            let with_probe = runtime_report_for(platform, true);

            assert!(compact.contains(&profile.title));
            assert!(compact.contains("bootstrap:"));
            assert!(compact.contains("registration:"));
            assert!(compact.contains("ime-dispatch:"));
            assert!(compact.contains("panel-dispatch:"));
            assert!(compact.contains("registration-hint:"));
            assert!(compact.contains(&format!(
                "preferred-command: {}",
                profile.preferred_command
            )));
            assert!(!compact.contains("session-probe:"));

            assert!(with_probe.contains("session-probe:"));
            assert!(with_probe.contains("active="));
            assert!(with_probe.contains("candidates="));
            assert!(with_probe.contains("committed=\"\""));
        }
    }
}
