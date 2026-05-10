use super::windows;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsImeBootstrap {
    pub bundled_runtime: bool,
    pub text_services_framework_available: bool,
    pub host_registration_ready: bool,
    pub marked_text_roundtrip_ready: bool,
    pub commit_roundtrip_ready: bool,
    pub native_candidate_window_ready: bool,
    pub recommended_profile_id: String,
}

impl WindowsImeBootstrap {
    pub fn describe(&self) -> String {
        format!(
            "TSF available: {} | bundled runtime: {} | host registration ready: {} | marked text: {} | commit: {} | native candidates: {} | recommended profile: {}",
            self.text_services_framework_available,
            self.bundled_runtime,
            self.host_registration_ready,
            self.marked_text_roundtrip_ready,
            self.commit_roundtrip_ready,
            self.native_candidate_window_ready,
            self.recommended_profile_id
        )
    }
}

pub fn recommended_profile_id() -> String {
    "dev.suzaku.windows.tsf".to_string()
}

pub fn bootstrap_status() -> WindowsImeBootstrap {
    WindowsImeBootstrap {
        bundled_runtime: cfg!(target_os = "windows"),
        text_services_framework_available: windows::support_profile().capabilities.system_ime_host,
        host_registration_ready: false,
        marked_text_roundtrip_ready: false,
        commit_roundtrip_ready: false,
        native_candidate_window_ready: false,
        recommended_profile_id: recommended_profile_id(),
    }
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_status, recommended_profile_id};

    #[test]
    fn windows_ime_profile_id_stays_stable() {
        assert_eq!(recommended_profile_id(), "dev.suzaku.windows.tsf");
    }

    #[test]
    fn windows_ime_bootstrap_reports_tsf_support() {
        let bootstrap = bootstrap_status();
        assert!(bootstrap.text_services_framework_available);
        assert!(!bootstrap.host_registration_ready);
    }
}
