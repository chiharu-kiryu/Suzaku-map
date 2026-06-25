#[cfg(target_os = "android")]
use super::{
    TargetPlatform, android_ime, ime_host_adapter, ime_host_dispatch, panel_companion_dispatch,
};
#[cfg(target_os = "android")]
use crate::ime::InputSource;
#[cfg(target_os = "android")]
use crate::platform::ime_host_adapter::{ImeHostSessionBridge, shared_session_bridge};
use std::ffi::{CString, c_char};

#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JString};
#[cfg(target_os = "android")]
use jni::sys::{jboolean, jint, jstring};

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

#[unsafe(no_mangle)]
pub extern "C" fn suzaku_android_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        let _ = CString::from_raw(ptr);
    }
}
