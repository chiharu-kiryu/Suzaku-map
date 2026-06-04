use super::*;

#[test]
fn panel_chrome_defaults_to_llm_disabled() {
    let chrome = PanelChromeState::default();

    assert!(!chrome.llm_enabled);
}

#[test]
fn voice_fallback_only_runs_when_bridge_is_unavailable() {
    assert!(!PanelState::voice_fallback_allowed(
        VoicePermissionState::Pending,
        true
    ));
    assert!(!PanelState::voice_fallback_allowed(
        VoicePermissionState::Denied,
        true
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Unavailable,
        true
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Unknown,
        false
    ));
}

#[test]
fn suzaku_bird_svg_asset_exists() {
    assert!(
        std::path::Path::new(
            "/Users/Shared/chroot/dev/Suzaku-map/src/assets/icons/suzaku-bird.svg"
        )
        .exists()
    );
}

#[test]
fn voice_transcript_normalization_collapses_whitespace() {
    assert_eq!(
        PanelState::normalize_voice_transcript("  hello   xr \n panel  "),
        "hello xr panel"
    );
}

#[test]
fn voice_auto_insert_requires_stable_ready_transcript() {
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "hello",
        2,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
    assert!(PanelState::should_auto_insert_voice_transcript(
        "hello xr",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "hello xr",
        3,
        VoiceCaptureState::Idle,
        VoicePermissionState::Ready,
    ));
}
