#[cfg(target_os = "android")]
use super::{
    TargetPlatform, android_ime, ime_host_adapter, ime_host_dispatch, panel_companion_dispatch,
};
#[cfg(target_os = "android")]
use crate::ime::InputSource;
#[cfg(target_os = "android")]
use crate::platform::ime_host_adapter::{ImeHostSessionBridge, shared_session_bridge};
use std::ffi::{CString, c_char};

#[cfg(any(target_os = "android", test))]
use crate::ime_host::HostImeBridgeSnapshot;
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JObjectArray, JString};
#[cfg(target_os = "android")]
use jni::sys::{jboolean, jint, jobjectArray, jstring};

#[cfg(any(target_os = "android", test))]
const ANDROID_RENDER_CANDIDATE_LIMIT: usize = 6;

#[cfg(target_os = "android")]
const JNI_FALSE: jboolean = 0;
#[cfg(target_os = "android")]
const JNI_TRUE: jboolean = 1;

#[cfg(target_os = "android")]
fn as_jboolean(value: bool) -> jboolean {
    if value { JNI_TRUE } else { JNI_FALSE }
}

#[cfg(target_os = "android")]
fn into_java_string(env: &mut JNIEnv<'_>, value: impl AsRef<str>) -> jstring {
    env.new_string(value.as_ref())
        .expect("android jni string allocation failed")
        .into_raw()
}

#[cfg(target_os = "android")]
fn into_java_string_array(env: &mut JNIEnv<'_>, values: Vec<String>) -> jobjectArray {
    let array: JObjectArray<'_> = env
        .new_object_array(values.len() as jint, "java/lang/String", JObject::null())
        .expect("android jni string array allocation failed");
    for (index, value) in values.into_iter().enumerate() {
        let string = env
            .new_string(value)
            .expect("android jni snapshot string allocation failed");
        env.set_object_array_element(&array, index as jint, string)
            .expect("android jni snapshot array write failed");
    }
    array.into_raw()
}

#[cfg(any(target_os = "android", test))]
fn render_snapshot_payload(snapshot: HostImeBridgeSnapshot) -> Vec<String> {
    let display_text = if snapshot.draft_text.is_empty() {
        snapshot.marked_text
    } else {
        snapshot.draft_text
    };
    let mut payload = Vec::with_capacity(ANDROID_RENDER_CANDIDATE_LIMIT + 2);
    payload.push(display_text);
    payload.push(snapshot.selected_index.to_string());
    payload.extend(
        snapshot
            .candidate_labels
            .into_iter()
            .take(ANDROID_RENDER_CANDIDATE_LIMIT),
    );
    payload
}

#[cfg(target_os = "android")]
fn from_java_string(env: &mut JNIEnv<'_>, value: JString<'_>) -> Option<String> {
    env.get_string(&value)
        .ok()
        .map(|text| text.to_string_lossy().into_owned())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDescribeImeHost(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        ime_host_adapter::adapter_profile_for(TargetPlatform::Android).describe(),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDescribeImeDispatch(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        ime_host_dispatch::dispatch_for(TargetPlatform::Android).describe(),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDescribePanelCompanion(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        panel_companion_dispatch::dispatch_for(TargetPlatform::Android).describe(),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDescribeBootstrap(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(&mut env, android_ime::bootstrap_status().describe())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeRegistrationHint(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        ime_host_adapter::adapter_profile_for(TargetPlatform::Android).registration_hint,
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeRegistrationTarget(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        ime_host_adapter::adapter_profile_for(TargetPlatform::Android).registration_target,
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeRegistrationReady(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jboolean {
    as_jboolean(ime_host_adapter::adapter_profile_for(TargetPlatform::Android).registration_ready)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeActivateSession(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jboolean {
    as_jboolean(shared_session_bridge().activate_session())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDeactivateSession(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    shared_session_bridge().deactivate_session();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeReplaceMarkedText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    value: JString<'_>,
) -> jboolean {
    let text = from_java_string(&mut env, value).unwrap_or_default();
    as_jboolean(shared_session_bridge().replace_marked_text(&text, InputSource::OnScreenPanel))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeClearMarkedText(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    shared_session_bridge().clear_marked_text();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeMoveSelection(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    delta: jint,
) {
    shared_session_bridge().move_selection(delta as isize);
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeSelectCandidate(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    index: jint,
) {
    if index >= 0 {
        shared_session_bridge().select_candidate(index as usize);
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCommitSelected(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    force: jboolean,
) -> jboolean {
    as_jboolean(shared_session_bridge().commit_selected(force != JNI_FALSE))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCandidateCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    shared_session_bridge().snapshot().candidate_count as jint
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeSelectedIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    shared_session_bridge().snapshot().selected_index as jint
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCandidateLabel(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    index: jint,
) -> jstring {
    let label = if index >= 0 {
        shared_session_bridge()
            .snapshot()
            .candidate_labels
            .get(index as usize)
            .cloned()
            .unwrap_or_default()
    } else {
        String::new()
    };
    into_java_string(&mut env, label)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDisplayText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let snapshot = shared_session_bridge().snapshot();
    let text = if !snapshot.draft_text.is_empty() {
        snapshot.draft_text
    } else {
        snapshot.marked_text
    };
    into_java_string(&mut env, text)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeRenderSnapshot(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jobjectArray {
    let payload = render_snapshot_payload(shared_session_bridge().snapshot());
    into_java_string_array(&mut env, payload)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeTakeLastCommittedText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    into_java_string(
        &mut env,
        shared_session_bridge()
            .take_last_committed_text()
            .unwrap_or_default(),
    )
}

/// Releases a string allocated by the Android bridge.
///
/// # Safety
/// `ptr` must be null or an unfreed pointer returned by `CString::into_raw`
/// using this library's allocator. The string's length must be unchanged, and
/// the caller must transfer exclusive ownership of the allocation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn suzaku_android_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: The caller transfers a live, unchanged CString allocation to us.
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::render_snapshot_payload;
    use crate::ime_host::HostImeBridgeSnapshot;

    #[test]
    fn string_free_accepts_null_and_owned_cstring() {
        let raw = std::ffi::CString::new("android-bridge").unwrap().into_raw();
        // SAFETY: Null is accepted, and this test owns the unchanged allocation.
        unsafe {
            super::suzaku_android_string_free(std::ptr::null_mut());
            super::suzaku_android_string_free(raw);
        }
    }

    fn snapshot(
        draft_text: &str,
        marked_text: &str,
        candidates: Vec<String>,
    ) -> HostImeBridgeSnapshot {
        HostImeBridgeSnapshot {
            active: true,
            marked_text: marked_text.to_string(),
            draft_text: draft_text.to_string(),
            committed_text: String::new(),
            candidate_count: candidates.len(),
            primary_candidate: candidates.first().cloned(),
            candidate_labels: candidates,
            selected_index: 2,
        }
    }

    #[test]
    fn render_payload_prefers_draft_and_carries_selection() {
        let payload = render_snapshot_payload(snapshot(
            "draft",
            "marked",
            vec!["one".to_string(), "two".to_string()],
        ));

        assert_eq!(payload, vec!["draft", "2", "one", "two"]);
    }

    #[test]
    fn render_payload_falls_back_to_marked_text_and_limits_candidates() {
        let candidates = (0..8).map(|index| format!("candidate-{index}")).collect();
        let payload = render_snapshot_payload(snapshot("", "marked", candidates));

        assert_eq!(payload[0], "marked");
        assert_eq!(payload.len(), 8);
        assert_eq!(payload.last().map(String::as_str), Some("candidate-5"));
    }
}
