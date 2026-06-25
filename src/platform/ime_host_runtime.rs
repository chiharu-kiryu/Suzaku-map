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
    pub registration_hint: String,
    pub preferred_command: &'static str,
    pub dispatch: ime_host_dispatch::ImeHostDispatch,
    pub panel_dispatch: panel_companion_dispatch::PanelCompanionDispatch,
}

impl ImeHostRuntimeProfile {
    pub fn describe(&self) -> String {
        format!(
            "{}\nbootstrap: {}\nime-dispatch: {}\npanel-dispatch: {}\nregistration-hint: {}\npreferred-command: {}",
            self.title,
            self.bootstrap_summary,
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
    ImeHostRuntimeProfile {
        platform,
        title: runtime_title(platform),
        bootstrap_summary: bootstrap_summary(platform),
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
    use super::{current_runtime_profile, runtime_profile_for};
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
}
