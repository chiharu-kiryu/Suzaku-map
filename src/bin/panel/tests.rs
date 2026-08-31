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
fn voice_fallback_allowed_with_unavailable_bridge_always_true() {
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Pending,
        false
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Denied,
        false
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Error,
        false
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Ready,
        false
    ));
    assert!(PanelState::voice_fallback_allowed(
        VoicePermissionState::Unavailable,
        false
    ));
}

#[test]
fn suzaku_bird_svg_asset_exists() {
    let icon_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/assets/icons/suzaku-bird.svg");
    assert!(icon_path.exists(), "missing {}", icon_path.display());
}

#[test]
fn voice_transcript_normalization_collapses_whitespace() {
    assert_eq!(
        PanelState::normalize_voice_transcript("  hello   xr \n panel  "),
        "hello xr panel"
    );
}

#[test]
fn normalize_voice_transcript_preserves_emoji_and_kaomoji_tokens() {
    assert_eq!(
        PanelState::normalize_voice_transcript("  ٩(◕‿◕｡)۶  😀  (^_^)  "),
        "٩(◕‿◕｡)۶ 😀 (^_^)"
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

#[test]
fn voice_auto_insert_accepts_long_single_token_when_stable() {
    assert!(PanelState::should_auto_insert_voice_transcript(
        "abcdef",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_accepts_short_multiple_words_when_stable() {
    assert!(PanelState::should_auto_insert_voice_transcript(
        "a b",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_rejects_when_not_listening_or_not_ready() {
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "hello xr",
        10,
        VoiceCaptureState::Idle,
        VoicePermissionState::Ready,
    ));
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "hello xr",
        10,
        VoiceCaptureState::Listening,
        VoicePermissionState::Unavailable,
    ));
}

#[test]
fn normalize_candidate_text_collapses_whitespace_and_removes_terminal_punctuation() {
    assert_eq!(
        PanelState::normalize_candidate_text("  HELLO   WORLD! "),
        "hello world"
    );
    assert_eq!(
        PanelState::normalize_candidate_text("GBoard??   "),
        "gboard"
    );
    assert_eq!(
        PanelState::normalize_candidate_text("  \n\tfoo   \tbar .   "),
        "foo bar"
    );
}

#[test]
fn normalize_candidate_text_handles_empty_and_whitespace_only_input() {
    assert_eq!(PanelState::normalize_candidate_text(""), "");
    assert_eq!(PanelState::normalize_candidate_text("   \n\t.!? "), "");
}

#[test]
fn normalize_voice_transcript_collapses_newlines_and_tabs_to_single_spaces() {
    assert_eq!(
        PanelState::normalize_voice_transcript("  hello \n\t xr   panel  "),
        "hello xr panel"
    );
}

#[test]
fn voice_auto_insert_rejects_empty_or_whitespace_only_transcript() {
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "   \n\t",
        10,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_accepts_same_transcript_when_stability_reaches_threshold() {
    assert!(PanelState::should_auto_insert_voice_transcript(
        "hello world",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_rejects_stability_below_threshold_even_for_long_phrase() {
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "hello world",
        2,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_rejects_emoji_token_without_stability_or_word_count_signal() {
    assert!(!PanelState::should_auto_insert_voice_transcript(
        "😀",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn voice_auto_insert_accepts_two_emoji_words_when_stable() {
    assert!(PanelState::should_auto_insert_voice_transcript(
        "😀 🙂",
        3,
        VoiceCaptureState::Listening,
        VoicePermissionState::Ready,
    ));
}

#[test]
fn commit_feedback_tick_decrements_and_clears_only_when_reaching_zero() {
    let (ticks, clear_feedback) = advance_commit_feedback_state(3);
    assert_eq!(ticks, 2);
    assert!(!clear_feedback);

    let (ticks, clear_feedback) = advance_commit_feedback_state(1);
    assert_eq!(ticks, 0);
    assert!(clear_feedback);

    let (ticks, clear_feedback) = advance_commit_feedback_state(0);
    assert_eq!(ticks, 0);
    assert!(!clear_feedback);
}

#[test]
fn commit_feedback_tick_works_for_large_values_without_overflow() {
    let (ticks, clear_feedback) = advance_commit_feedback_state(u8::MAX);

    assert_eq!(ticks, u8::MAX - 1);
    assert!(!clear_feedback);
}

#[test]
fn commit_feedback_tick_follows_expected_countdown_until_clear() {
    let mut ticks = 3u8;
    let mut clear_count = 0u8;

    for _ in 0..4 {
        let (next_ticks, clear_feedback) = advance_commit_feedback_state(ticks);
        if clear_feedback {
            clear_count += 1;
        }
        ticks = next_ticks;
    }

    assert_eq!(ticks, 0);
    assert_eq!(clear_count, 1);
}

#[test]
fn commit_feedback_tick_stays_cleared_after_zero() {
    let (ticks, clear_feedback) = advance_commit_feedback_state(1);
    assert_eq!(ticks, 0);
    assert!(clear_feedback);

    let (ticks, clear_feedback) = advance_commit_feedback_state(ticks);
    assert_eq!(ticks, 0);
    assert!(!clear_feedback);
}
