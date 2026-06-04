use super::android;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidImeBootstrap {
    pub bundled_runtime: bool,
    pub input_method_service_available: bool,
    pub host_registration_ready: bool,
    pub marked_text_roundtrip_ready: bool,
    pub commit_roundtrip_ready: bool,
    pub native_candidate_window_ready: bool,
    pub recommended_service_name: String,
}

impl AndroidImeBootstrap {
    pub fn describe(&self) -> String {
        format!(
            "IMS available: {} | bundled runtime: {} | host registration ready: {} | marked text: {} | commit: {} | native candidates: {} | recommended service: {}",
            self.input_method_service_available,
            self.bundled_runtime,
            self.host_registration_ready,
            self.marked_text_roundtrip_ready,
            self.commit_roundtrip_ready,
            self.native_candidate_window_ready,
            self.recommended_service_name
        )
    }
}

pub fn recommended_service_name() -> String {
    "dev.suzaku.android.ime/.SuzakuInputMethodService".to_string()
}

pub fn bootstrap_status() -> AndroidImeBootstrap {
    AndroidImeBootstrap {
        bundled_runtime: cfg!(target_os = "android"),
        input_method_service_available: android::support_profile().capabilities.system_ime_host,
        host_registration_ready: false,
        marked_text_roundtrip_ready: false,
        commit_roundtrip_ready: false,
        native_candidate_window_ready: false,
        recommended_service_name: recommended_service_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_status, recommended_service_name};

    #[test]
    fn android_service_name_stays_stable() {
        assert_eq!(
            recommended_service_name(),
            "dev.suzaku.android.ime/.SuzakuInputMethodService"
        );
    }

    #[test]
    fn android_bootstrap_reports_ims_support() {
        let bootstrap = bootstrap_status();
        assert!(bootstrap.input_method_service_available);
        assert!(!bootstrap.host_registration_ready);
    }
}
