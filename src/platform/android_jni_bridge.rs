#[cfg(target_os = "android")]
use super::{TargetPlatform, android_ime, ime_host_dispatch, panel_companion_dispatch};
#[cfg(target_os = "android")]
use crate::ime::InputSource;
#[cfg(target_os = "android")]
use crate::ime_host::{
    host_bridge_activate, host_bridge_clear_marked_text, host_bridge_commit_selected,
    host_bridge_deactivate, host_bridge_move_selection, host_bridge_replace_marked_text,
    host_bridge_select_candidate, host_bridge_snapshot, host_bridge_take_last_committed_text,
};
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
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeActivateSession(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jboolean {
    as_jboolean(host_bridge_activate())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeDeactivateSession(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    host_bridge_deactivate();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeReplaceMarkedText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    value: JString<'_>,
) -> jboolean {
    let text = from_java_string(&mut env, value).unwrap_or_default();
    as_jboolean(host_bridge_replace_marked_text(
        &text,
        InputSource::OnScreenPanel,
    ))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeClearMarkedText(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    host_bridge_clear_marked_text();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeMoveSelection(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    delta: jint,
) {
    host_bridge_move_selection(delta as isize);
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeSelectCandidate(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    index: jint,
) {
    if index >= 0 {
        host_bridge_select_candidate(index as usize);
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCommitSelected(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    force: jboolean,
) -> jboolean {
    as_jboolean(host_bridge_commit_selected(force != JNI_FALSE))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCandidateCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    host_bridge_snapshot().candidate_count as jint
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeSelectedIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    host_bridge_snapshot().selected_index as jint
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_suzaku_android_ime_SuzakuNativeBridge_nativeCandidateLabel(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    index: jint,
) -> jstring {
    let label = if index >= 0 {
        host_bridge_snapshot()
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
    let snapshot = host_bridge_snapshot();
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
        host_bridge_take_last_committed_text().unwrap_or_default(),
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
