use super::{
    SupportTier, TargetPlatform, host_platform, linux, linux_ime, macos, macos_ime, support_for,
    windows, windows_ime,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeHostBackendKind {
    MacOsInputMethodKit,
    WindowsTextServicesFramework,
    LinuxIbusFcitx,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeHostDispatch {
    pub platform: TargetPlatform,
    pub tier: SupportTier,
    pub backend: ImeHostBackendKind,
    pub system_ime_host: bool,
    pub marked_text_roundtrip: bool,
    pub commit_roundtrip: bool,
    pub native_candidate_window: bool,
    pub notes: String,
}

impl ImeHostDispatch {
    pub fn describe(&self) -> String {
        format!(
            "platform={:?} | tier={:?} | backend={:?} | system_host={} | marked_text={} | commit={} | native_candidates={} | notes={}",
            self.platform,
            self.tier,
            self.backend,
            self.system_ime_host,
            self.marked_text_roundtrip,
            self.commit_roundtrip,
            self.native_candidate_window,
            self.notes
        )
    }
}

pub fn current_ime_host_dispatch() -> ImeHostDispatch {
    dispatch_for(host_platform())
}

pub fn dispatch_for(platform: TargetPlatform) -> ImeHostDispatch {
    let support = support_for(platform);

    match platform {
        TargetPlatform::MacOs => {
            let bootstrap = macos_ime::bootstrap_status();
            ImeHostDispatch {
                platform,
                tier: support.tier,
                backend: ImeHostBackendKind::MacOsInputMethodKit,
                system_ime_host: macos::support_profile().capabilities.system_ime_host,
                marked_text_roundtrip: bootstrap.host_session.active
                    || bootstrap.controller_lifecycle_ready,
                commit_roundtrip: bootstrap.host_session.active || bootstrap.server_bootstrap_ready,
                native_candidate_window: bootstrap.candidate_companion.ready,
                notes: if bootstrap.server_bootstrap_ready {
                    format!(
                        "InputMethodKit bootstrap is live; Rust host session is wired for marked text and commit, candidate companion visibility is {}, and the GPU panel should stay in debug-companion role.",
                        bootstrap.candidate_companion.visible
                    )
                } else {
                    "InputMethodKit bridge is present, but the bundle is not yet running as a registered system input method; the GPU panel should remain a debug companion."
                        .to_string()
                },
            }
        }
        TargetPlatform::Windows => {
            let bootstrap = windows_ime::bootstrap_status();
            ImeHostDispatch {
                platform,
                tier: support.tier,
                backend: ImeHostBackendKind::WindowsTextServicesFramework,
                system_ime_host: windows::support_profile().capabilities.system_ime_host,
                marked_text_roundtrip: bootstrap.marked_text_roundtrip_ready,
                commit_roundtrip: bootstrap.commit_roundtrip_ready,
                native_candidate_window: bootstrap.native_candidate_window_ready,
                notes: if bootstrap.host_registration_ready {
                    "Windows TSF host bootstrap is registered and ready for marked-text and commit wiring; keep the GPU panel in debug-companion role."
                        .to_string()
                } else {
                    format!(
                        "Windows will route through Text Services Framework; current profile id is {} and the host shell is not registered yet, so the GPU panel remains a debug companion.",
                        bootstrap.recommended_profile_id
                    )
                },
            }
        }
        TargetPlatform::Ubuntu | TargetPlatform::ArchLinux | TargetPlatform::SteamOs => {
            let bootstrap = linux_ime::bootstrap_status(platform);
            ImeHostDispatch {
                platform,
                tier: support.tier,
                backend: ImeHostBackendKind::LinuxIbusFcitx,
                system_ime_host: support.capabilities.system_ime_host,
                marked_text_roundtrip: bootstrap.marked_text_roundtrip_ready,
                commit_roundtrip: bootstrap.commit_roundtrip_ready,
                native_candidate_window: bootstrap.native_candidate_window_ready,
                notes: if bootstrap.host_registration_ready {
                    format!(
                        "Linux {:?} host bootstrap is registered and running alongside the existing {} voice backend.",
                        bootstrap.framework,
                        linux::linux_voice_backend_label()
                    )
                } else {
                    format!(
                        "Linux {:?} host shell is planned; current connection name is {} and the existing {} voice backend stays separate.",
                        bootstrap.framework,
                        bootstrap.recommended_connection_name,
                        linux::linux_voice_backend_label()
                    )
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImeHostBackendKind, current_ime_host_dispatch, dispatch_for};
    use crate::platform::{TargetPlatform, host_platform};

    #[test]
    fn dispatch_matches_host_platform() {
        let host = host_platform();
        assert_eq!(current_ime_host_dispatch().platform, host);
    }

    #[test]
    fn macos_dispatch_uses_imk_backend() {
        let dispatch = dispatch_for(TargetPlatform::MacOs);
        assert_eq!(dispatch.backend, ImeHostBackendKind::MacOsInputMethodKit);
        assert!(dispatch.system_ime_host);
    }

    #[test]
    fn windows_dispatch_reserves_tsf_backend() {
        let dispatch = dispatch_for(TargetPlatform::Windows);
        assert_eq!(
            dispatch.backend,
            ImeHostBackendKind::WindowsTextServicesFramework
        );
        assert!(dispatch.system_ime_host);
    }

    #[test]
    fn linux_dispatch_uses_ibus_or_fcitx_backend_family() {
        let dispatch = dispatch_for(TargetPlatform::Ubuntu);
        assert_eq!(dispatch.backend, ImeHostBackendKind::LinuxIbusFcitx);
        assert!(dispatch.notes.contains("Linux"));
    }
}
