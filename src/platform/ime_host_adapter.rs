use crate::ime::InputSource;
use crate::ime_host::{
    HostImeBridgeSnapshot, host_bridge_activate, host_bridge_clear_marked_text,
    host_bridge_commit_selected, host_bridge_deactivate, host_bridge_move_selection,
    host_bridge_replace_marked_text, host_bridge_select_candidate, host_bridge_snapshot,
    host_bridge_take_last_committed_text,
};
use std::ffi::CString;

use super::{
    TargetPlatform, android_ime, host_platform, linux_ime, macos_ime, panel_companion_dispatch,
    windows_ime,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeHostLifecycleCapabilities {
    pub marked_text_roundtrip: bool,
    pub candidate_selection: bool,
    pub commit_roundtrip: bool,
    pub native_candidate_window: bool,
    pub on_demand_companion: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeHostAdapterProfile {
    pub platform: TargetPlatform,
    pub backend_id: &'static str,
    pub registration_ready: bool,
    pub registration_target: String,
    pub bootstrap_summary: String,
    pub registration_hint: String,
    pub lifecycle: ImeHostLifecycleCapabilities,
}

impl ImeHostAdapterProfile {
    pub fn describe(&self) -> String {
        format!(
            "platform={:?} | backend={} | registration_ready={} | registration_target={} | marked_text={} | candidates={} | commit={} | native_candidates={} | on_demand_companion={} | hint={}",
            self.platform,
            self.backend_id,
            self.registration_ready,
            self.registration_target,
            self.lifecycle.marked_text_roundtrip,
            self.lifecycle.candidate_selection,
            self.lifecycle.commit_roundtrip,
            self.lifecycle.native_candidate_window,
            self.lifecycle.on_demand_companion,
            self.registration_hint
        )
    }
}

pub trait ImePlatformAdapter {
    fn profile(&self) -> ImeHostAdapterProfile;
}

pub trait ImeHostSessionBridge {
    fn activate_session(&self) -> bool;
    fn deactivate_session(&self);
    fn replace_marked_text(&self, text: &str, source: InputSource) -> bool;
    fn clear_marked_text(&self);
    fn move_selection(&self, delta: isize);
    fn select_candidate(&self, index: usize);
    fn commit_selected(&self, force: bool) -> bool;
    fn snapshot(&self) -> HostImeBridgeSnapshot;
    fn take_last_committed_text(&self) -> Option<String>;
}

pub struct SharedHostSessionBridge;

pub struct MacOsPlatformAdapter;
pub struct WindowsPlatformAdapter;
pub struct AndroidPlatformAdapter;
pub struct LinuxPlatformAdapter {
    platform: TargetPlatform,
}

impl ImeHostSessionBridge for SharedHostSessionBridge {
    fn activate_session(&self) -> bool {
        host_bridge_activate()
    }

    fn deactivate_session(&self) {
        host_bridge_deactivate();
    }

    fn replace_marked_text(&self, text: &str, source: InputSource) -> bool {
        host_bridge_replace_marked_text(text, source)
    }

    fn clear_marked_text(&self) {
        host_bridge_clear_marked_text();
    }

    fn move_selection(&self, delta: isize) {
        host_bridge_move_selection(delta);
    }

    fn select_candidate(&self, index: usize) {
        host_bridge_select_candidate(index);
    }

    fn commit_selected(&self, force: bool) -> bool {
        host_bridge_commit_selected(force)
    }

    fn snapshot(&self) -> HostImeBridgeSnapshot {
        host_bridge_snapshot()
    }

    fn take_last_committed_text(&self) -> Option<String> {
        host_bridge_take_last_committed_text()
    }
}

impl ImePlatformAdapter for MacOsPlatformAdapter {
    fn profile(&self) -> ImeHostAdapterProfile {
        let bridge = macos_ime::bridge_state();
        let panel_role = panel_companion_dispatch::dispatch_for(TargetPlatform::MacOs).role;
        let registration_target = bridge
            .bundle_connection_name
            .clone()
            .unwrap_or_else(|| macos_ime::recommended_connection_name());
        ImeHostAdapterProfile {
            platform: TargetPlatform::MacOs,
            backend_id: "inputmethodkit",
            registration_ready: bridge.server_bootstrap_ready,
            registration_target: registration_target.clone(),
            bootstrap_summary: format!(
                "IMK available: {} | bundled: {} | bundle id: {} | bundle connection: {} | controller: {} | lifecycle ready: {} | server ready: {} | candidate companion ready: {} | host session active: {}",
                bridge.input_methodkit_available,
                bridge.bundled_runtime,
                bridge.main_bundle_identifier.as_deref().unwrap_or("(none)"),
                registration_target,
                bridge.controller_class_name.as_deref().unwrap_or("(none)"),
                bridge.controller_lifecycle_ready,
                bridge.server_bootstrap_ready,
                bridge.candidate_companion.ready,
                bridge.host_session.active
            ),
            registration_hint: format!(
                "Register {} as the InputMethodKit connection and keep the GPU panel in {:?} mode.",
                registration_target, panel_role
            ),
            lifecycle: ImeHostLifecycleCapabilities {
                marked_text_roundtrip: bridge.host_session.active
                    || bridge.controller_lifecycle_ready,
                candidate_selection: bridge.candidate_companion.ready,
                commit_roundtrip: bridge.host_session.active || bridge.server_bootstrap_ready,
                native_candidate_window: bridge.candidate_companion.ready,
                on_demand_companion: true,
            },
        }
    }
}

impl ImePlatformAdapter for WindowsPlatformAdapter {
    fn profile(&self) -> ImeHostAdapterProfile {
        let bootstrap = windows_ime::bootstrap_status();
        ImeHostAdapterProfile {
            platform: TargetPlatform::Windows,
            backend_id: "tsf",
            registration_ready: bootstrap.host_registration_ready,
            registration_target: bootstrap.recommended_profile_id.clone(),
            bootstrap_summary: bootstrap.describe(),
            registration_hint: format!(
                "Register {} as the TSF profile before promoting Suzaku beyond companion mode.",
                bootstrap.recommended_profile_id
            ),
            lifecycle: ImeHostLifecycleCapabilities {
                marked_text_roundtrip: bootstrap.marked_text_roundtrip_ready,
                candidate_selection: true,
                commit_roundtrip: bootstrap.commit_roundtrip_ready,
                native_candidate_window: bootstrap.native_candidate_window_ready,
                on_demand_companion: true,
            },
        }
    }
}

impl ImePlatformAdapter for AndroidPlatformAdapter {
    fn profile(&self) -> ImeHostAdapterProfile {
        let bootstrap = android_ime::bootstrap_status();
        ImeHostAdapterProfile {
            platform: TargetPlatform::Android,
            backend_id: "input-method-service",
            registration_ready: bootstrap.host_registration_ready,
            registration_target: bootstrap.recommended_service_name.clone(),
            bootstrap_summary: bootstrap.describe(),
            registration_hint: format!(
                "Ship {} as the InputMethodService and only open Suzaku's expanded panel on explicit user action.",
                bootstrap.recommended_service_name
            ),
            lifecycle: ImeHostLifecycleCapabilities {
                marked_text_roundtrip: bootstrap.marked_text_roundtrip_ready,
                candidate_selection: true,
                commit_roundtrip: bootstrap.commit_roundtrip_ready,
                native_candidate_window: bootstrap.native_candidate_window_ready,
                on_demand_companion: true,
            },
        }
    }
}

impl LinuxPlatformAdapter {
    pub fn new(platform: TargetPlatform) -> Self {
        Self { platform }
    }
}

impl ImePlatformAdapter for LinuxPlatformAdapter {
    fn profile(&self) -> ImeHostAdapterProfile {
        let bootstrap = linux_ime::bootstrap_status(self.platform);
        ImeHostAdapterProfile {
            platform: self.platform,
            backend_id: "ibus-fcitx",
            registration_ready: bootstrap.host_registration_ready,
            registration_target: bootstrap.recommended_connection_name.clone(),
            bootstrap_summary: bootstrap.describe(),
            registration_hint: format!(
                "Keep the GPU panel primary until {} is registered through {:?}.",
                bootstrap.recommended_connection_name, bootstrap.framework
            ),
            lifecycle: ImeHostLifecycleCapabilities {
                marked_text_roundtrip: bootstrap.marked_text_roundtrip_ready,
                candidate_selection: true,
                commit_roundtrip: bootstrap.commit_roundtrip_ready,
                native_candidate_window: bootstrap.native_candidate_window_ready,
                on_demand_companion: false,
            },
        }
    }
}

pub fn adapter_profile_for(platform: TargetPlatform) -> ImeHostAdapterProfile {
    match platform {
        TargetPlatform::MacOs => MacOsPlatformAdapter.profile(),
        TargetPlatform::Windows => WindowsPlatformAdapter.profile(),
        TargetPlatform::Android => AndroidPlatformAdapter.profile(),
        TargetPlatform::Ubuntu | TargetPlatform::ArchLinux | TargetPlatform::SteamOs => {
            LinuxPlatformAdapter::new(platform).profile()
        }
    }
}

pub fn current_adapter_profile() -> ImeHostAdapterProfile {
    adapter_profile_for(host_platform())
}

pub fn shared_session_bridge() -> SharedHostSessionBridge {
    SharedHostSessionBridge
}

fn into_raw_c_string(value: String) -> *mut std::os::raw::c_char {
    CString::new(value)
        .expect("host adapter bridge string must not contain interior nulls")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_adapter_summary_utf8() -> *mut std::os::raw::c_char {
    into_raw_c_string(current_adapter_profile().describe())
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_registration_target_utf8() -> *mut std::os::raw::c_char {
    into_raw_c_string(current_adapter_profile().registration_target)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_registration_hint_utf8() -> *mut std::os::raw::c_char {
    into_raw_c_string(current_adapter_profile().registration_hint)
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_registration_ready() -> bool {
    current_adapter_profile().registration_ready
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_on_demand_companion() -> bool {
    current_adapter_profile().lifecycle.on_demand_companion
}

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_host_platform_free_utf8(raw: *mut std::os::raw::c_char) {
    if raw.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(raw);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ImeHostSessionBridge, adapter_profile_for, current_adapter_profile, shared_session_bridge,
    };
    use crate::ime::InputSource;
    use crate::platform::TargetPlatform;

    #[test]
    fn android_profile_requires_on_demand_companion() {
        let profile = adapter_profile_for(TargetPlatform::Android);
        assert_eq!(profile.backend_id, "input-method-service");
        assert!(profile.lifecycle.on_demand_companion);
    }

    #[test]
    fn shared_bridge_round_trips_marked_text() {
        let bridge = shared_session_bridge();
        assert!(bridge.activate_session());
        assert!(bridge.replace_marked_text("ni hao", InputSource::OnScreenPanel));
        assert_eq!(bridge.snapshot().marked_text, "ni hao");
    }

    #[test]
    fn current_profile_description_mentions_backend() {
        let profile = current_adapter_profile();
        assert!(profile.describe().contains("backend="));
    }
}
