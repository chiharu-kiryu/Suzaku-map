use crate::ime_host::host_bridge_snapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacOsImeBridgeState {
    pub input_methodkit_available: bool,
    pub bundled_runtime: bool,
    pub main_bundle_identifier: Option<String>,
    pub bundle_connection_name: Option<String>,
    pub controller_class_name: Option<String>,
    pub controller_lifecycle_ready: bool,
    pub server_bootstrap_ready: bool,
    pub controller_debug_state: MacOsImeControllerDebugState,
    pub candidate_companion: MacOsImeCandidateCompanionState,
    pub host_session: MacOsImeHostSessionDebugState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsImeBootstrap {
    pub input_methodkit_available: bool,
    pub bundled_runtime: bool,
    pub adapter_summary: Option<String>,
    pub registration_target: Option<String>,
    pub registration_hint: Option<String>,
    pub registration_ready: bool,
    pub on_demand_companion: bool,
    pub main_bundle_identifier: Option<String>,
    pub bundle_connection_name: Option<String>,
    pub controller_class_name: Option<String>,
    pub controller_lifecycle_ready: bool,
    pub server_bootstrap_ready: bool,
    pub controller_debug_state: MacOsImeControllerDebugState,
    pub candidate_companion: MacOsImeCandidateCompanionState,
    pub host_session: MacOsImeHostSessionDebugState,
    pub recommended_connection_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsImeControllerDebugState {
    pub active: bool,
    pub init_count: usize,
    pub activate_count: usize,
    pub deactivate_count: usize,
    pub input_count: usize,
    pub commit_count: usize,
    pub last_marked_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsImeHostSessionDebugState {
    pub active: bool,
    pub marked_text: String,
    pub draft_text: String,
    pub committed_text: String,
    pub candidate_count: usize,
    pub selected_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacOsImeCandidateCompanionState {
    pub ready: bool,
    pub visible: bool,
    pub refresh_count: usize,
    pub selected_index: usize,
    pub hovered_index: Option<usize>,
    pub primary_candidate: Option<String>,
}

impl MacOsImeBootstrap {
    pub fn describe(&self) -> String {
        format!(
            "IMK available: {} | bundled: {} | adapter: {} | registration target: {} | registration hint: {} | registration ready: {} | on-demand companion: {} | bundle id: {} | bundle connection: {} | controller: {} | lifecycle ready: {} | server ready: {} | controller active: {} | controller init/activate/input/commit: {}/{}/{}/{} | last marked: {} | candidate companion ready: {} | visible: {} | companion refreshes: {} | companion selected: {} | companion hovered: {} | companion primary: {} | host session active: {} | host marked: {} | host candidates: {} | host committed: {} | recommended connection: {}",
            self.input_methodkit_available,
            self.bundled_runtime,
            self.adapter_summary.as_deref().unwrap_or("(none)"),
            self.registration_target.as_deref().unwrap_or("(none)"),
            self.registration_hint.as_deref().unwrap_or("(none)"),
            self.registration_ready,
            self.on_demand_companion,
            self.main_bundle_identifier.as_deref().unwrap_or("(none)"),
            self.bundle_connection_name.as_deref().unwrap_or("(none)"),
            self.controller_class_name.as_deref().unwrap_or("(none)"),
            self.controller_lifecycle_ready,
            self.server_bootstrap_ready,
            self.controller_debug_state.active,
            self.controller_debug_state.init_count,
            self.controller_debug_state.activate_count,
            self.controller_debug_state.input_count,
            self.controller_debug_state.commit_count,
            self.controller_debug_state
                .last_marked_text
                .as_deref()
                .unwrap_or("(none)"),
            self.candidate_companion.ready,
            self.candidate_companion.visible,
            self.candidate_companion.refresh_count,
            self.candidate_companion.selected_index,
            self.candidate_companion
                .hovered_index
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(none)".to_string()),
            self.candidate_companion
                .primary_candidate
                .as_deref()
                .unwrap_or("(none)"),
            self.host_session.active,
            if self.host_session.marked_text.is_empty() {
                "(none)"
            } else {
                self.host_session.marked_text.as_str()
            },
            self.host_session.candidate_count,
            if self.host_session.committed_text.is_empty() {
                "(none)"
            } else {
                self.host_session.committed_text.as_str()
            },
            self.recommended_connection_name
        )
    }
}

pub fn recommended_connection_name() -> String {
    "dev.suzaku.inputmethod".to_string()
}

pub fn bootstrap_status() -> MacOsImeBootstrap {
    let bridge = bridge_state();

    #[cfg(target_os = "macos")]
    {
        return MacOsImeBootstrap {
            input_methodkit_available: bridge.input_methodkit_available,
            bundled_runtime: bridge.bundled_runtime,
            adapter_summary: adapter_summary(),
            registration_target: registration_target(),
            registration_hint: registration_hint(),
            registration_ready: registration_ready(),
            on_demand_companion: on_demand_companion(),
            main_bundle_identifier: bridge.main_bundle_identifier,
            bundle_connection_name: bridge.bundle_connection_name,
            controller_class_name: bridge.controller_class_name,
            controller_lifecycle_ready: bridge.controller_lifecycle_ready,
            server_bootstrap_ready: bridge.server_bootstrap_ready,
            controller_debug_state: bridge.controller_debug_state,
            candidate_companion: bridge.candidate_companion,
            host_session: bridge.host_session,
            recommended_connection_name: recommended_connection_name(),
        };
    }

    #[allow(unreachable_code)]
    MacOsImeBootstrap {
        input_methodkit_available: bridge.input_methodkit_available,
        bundled_runtime: bridge.bundled_runtime,
        adapter_summary: None,
        registration_target: None,
        registration_hint: None,
        registration_ready: false,
        on_demand_companion: true,
        main_bundle_identifier: bridge.main_bundle_identifier,
        bundle_connection_name: bridge.bundle_connection_name,
        controller_class_name: bridge.controller_class_name,
        controller_lifecycle_ready: bridge.controller_lifecycle_ready,
        server_bootstrap_ready: bridge.server_bootstrap_ready,
        controller_debug_state: bridge.controller_debug_state,
        candidate_companion: bridge.candidate_companion,
        host_session: bridge.host_session,
        recommended_connection_name: recommended_connection_name(),
    }
}

pub(crate) fn bridge_state() -> MacOsImeBridgeState {
    #[cfg(target_os = "macos")]
    {
        return MacOsImeBridgeState {
            input_methodkit_available: input_methodkit_available(),
            bundled_runtime: bundled_runtime(),
            main_bundle_identifier: main_bundle_identifier(),
            bundle_connection_name: bundle_connection_name(),
            controller_class_name: controller_class_name(),
            controller_lifecycle_ready: controller_lifecycle_ready(),
            server_bootstrap_ready: bootstrap_server(),
            controller_debug_state: controller_debug_state(),
            candidate_companion: candidate_companion_state(),
            host_session: host_session_debug_state(),
        };
    }

    #[allow(unreachable_code)]
    MacOsImeBridgeState {
        input_methodkit_available: false,
        bundled_runtime: false,
        main_bundle_identifier: None,
        bundle_connection_name: None,
        controller_class_name: None,
        controller_lifecycle_ready: false,
        server_bootstrap_ready: false,
        controller_debug_state: MacOsImeControllerDebugState {
            active: false,
            init_count: 0,
            activate_count: 0,
            deactivate_count: 0,
            input_count: 0,
            commit_count: 0,
            last_marked_text: None,
        },
        candidate_companion: MacOsImeCandidateCompanionState {
            ready: false,
            visible: false,
            refresh_count: 0,
            selected_index: 0,
            hovered_index: None,
            primary_candidate: None,
        },
        host_session: MacOsImeHostSessionDebugState {
            active: false,
            marked_text: String::new(),
            draft_text: String::new(),
            committed_text: String::new(),
            candidate_count: 0,
            selected_index: 0,
        },
    }
}

#[cfg(target_os = "macos")]
fn adapter_summary() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_adapter_summary() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_adapter_summary)
}

#[cfg(target_os = "macos")]
fn registration_target() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_registration_target() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_registration_target)
}

#[cfg(target_os = "macos")]
fn registration_hint() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_registration_hint() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_registration_hint)
}

#[cfg(target_os = "macos")]
fn registration_ready() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_registration_ready() -> bool;
    }

    unsafe { suzaku_input_methodkit_registration_ready() }
}

#[cfg(target_os = "macos")]
fn on_demand_companion() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_on_demand_companion() -> bool;
    }

    unsafe { suzaku_input_methodkit_on_demand_companion() }
}

#[cfg(target_os = "macos")]
fn input_methodkit_available() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_available() -> bool;
    }

    unsafe { suzaku_input_methodkit_available() }
}

#[cfg(target_os = "macos")]
fn bundled_runtime() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_bundled_runtime() -> bool;
    }

    unsafe { suzaku_input_methodkit_bundled_runtime() }
}

#[cfg(target_os = "macos")]
fn main_bundle_identifier() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_main_bundle_identifier() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_main_bundle_identifier)
}

#[cfg(target_os = "macos")]
fn bundle_connection_name() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_connection_name() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_connection_name)
}

#[cfg(target_os = "macos")]
fn controller_class_name() -> Option<String> {
    unsafe extern "C" {
        fn suzaku_input_methodkit_controller_class_name() -> *mut std::os::raw::c_char;
    }

    read_optional_native_string(suzaku_input_methodkit_controller_class_name)
}

#[cfg(target_os = "macos")]
fn controller_lifecycle_ready() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_controller_lifecycle_ready() -> bool;
    }

    unsafe { suzaku_input_methodkit_controller_lifecycle_ready() }
}

#[cfg(target_os = "macos")]
fn bootstrap_server() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_bootstrap_server() -> bool;
    }

    unsafe { suzaku_input_methodkit_bootstrap_server() }
}

#[cfg(target_os = "macos")]
fn controller_debug_state() -> MacOsImeControllerDebugState {
    unsafe extern "C" {
        fn suzaku_input_methodkit_controller_init_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_controller_activate_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_controller_deactivate_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_controller_input_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_controller_commit_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_controller_is_active() -> bool;
        fn suzaku_input_methodkit_controller_last_marked_text() -> *mut std::os::raw::c_char;
    }

    MacOsImeControllerDebugState {
        active: unsafe { suzaku_input_methodkit_controller_is_active() },
        init_count: unsafe { suzaku_input_methodkit_controller_init_count() as usize },
        activate_count: unsafe { suzaku_input_methodkit_controller_activate_count() as usize },
        deactivate_count: unsafe { suzaku_input_methodkit_controller_deactivate_count() as usize },
        input_count: unsafe { suzaku_input_methodkit_controller_input_count() as usize },
        commit_count: unsafe { suzaku_input_methodkit_controller_commit_count() as usize },
        last_marked_text: read_optional_native_string(
            suzaku_input_methodkit_controller_last_marked_text,
        ),
    }
}

#[cfg(target_os = "macos")]
fn candidate_companion_state() -> MacOsImeCandidateCompanionState {
    unsafe extern "C" {
        fn suzaku_input_methodkit_candidate_companion_refresh_count() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_candidate_companion_visible() -> bool;
        fn suzaku_input_methodkit_candidate_companion_selected_index() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_candidate_companion_hovered_index() -> std::os::raw::c_ulong;
        fn suzaku_input_methodkit_candidate_companion_primary_candidate()
        -> *mut std::os::raw::c_char;
    }

    let hovered_index =
        unsafe { suzaku_input_methodkit_candidate_companion_hovered_index() as usize };

    MacOsImeCandidateCompanionState {
        ready: candidate_companion_window_ready(),
        visible: unsafe { suzaku_input_methodkit_candidate_companion_visible() },
        refresh_count: unsafe {
            suzaku_input_methodkit_candidate_companion_refresh_count() as usize
        },
        selected_index: unsafe {
            suzaku_input_methodkit_candidate_companion_selected_index() as usize
        },
        hovered_index: if hovered_index == usize::MAX {
            None
        } else {
            Some(hovered_index)
        },
        primary_candidate: read_optional_native_string(
            suzaku_input_methodkit_candidate_companion_primary_candidate,
        ),
    }
}

#[cfg(target_os = "macos")]
fn candidate_companion_window_ready() -> bool {
    unsafe extern "C" {
        fn suzaku_input_methodkit_candidate_companion_window_ready() -> bool;
    }

    unsafe { suzaku_input_methodkit_candidate_companion_window_ready() }
}

fn host_session_debug_state() -> MacOsImeHostSessionDebugState {
    let snapshot = host_bridge_snapshot();
    MacOsImeHostSessionDebugState {
        active: snapshot.active,
        marked_text: snapshot.marked_text,
        draft_text: snapshot.draft_text,
        committed_text: snapshot.committed_text,
        candidate_count: snapshot.candidate_count,
        selected_index: snapshot.selected_index,
    }
}

#[cfg(target_os = "macos")]
fn read_optional_native_string(
    provider: unsafe extern "C" fn() -> *mut std::os::raw::c_char,
) -> Option<String> {
    use std::ffi::CStr;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn suzaku_input_methodkit_free_c_string(value: *mut c_char);
    }

    let raw = unsafe { provider() };
    if raw.is_null() {
        return None;
    }
    let value = unsafe { CStr::from_ptr(raw) }.to_string_lossy().to_string();
    unsafe { suzaku_input_methodkit_free_c_string(raw) };
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_status, recommended_connection_name};

    #[test]
    fn macos_ime_connection_name_stays_stable() {
        assert_eq!(recommended_connection_name(), "dev.suzaku.inputmethod");
    }

    #[test]
    fn macos_ime_bootstrap_always_reports_connection_name() {
        assert_eq!(
            bootstrap_status().recommended_connection_name,
            "dev.suzaku.inputmethod"
        );
    }

    #[test]
    fn macos_ime_bootstrap_reports_controller_lifecycle_state() {
        let bootstrap = bootstrap_status();
        if bootstrap.input_methodkit_available {
            // On real macOS hosts, counters can be non-zero depending on existing system state.
        } else {
            assert_eq!(bootstrap.controller_debug_state.init_count, 0);
            assert_eq!(bootstrap.controller_debug_state.activate_count, 0);
            assert_eq!(bootstrap.controller_debug_state.input_count, 0);
            assert_eq!(bootstrap.controller_debug_state.commit_count, 0);
            assert_eq!(bootstrap.host_session.candidate_count, 0);
        }
        assert!(bootstrap.candidate_companion.ready || !bootstrap.input_methodkit_available);
    }
}
