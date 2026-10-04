#!/usr/bin/env bash
# Reproduce the opt-in Linux CI checks without touching the desktop session.
set -euo pipefail

suzaku_ci_mode="${1:-}"
case "$suzaku_ci_mode" in
  ibus|ui) ;;
  *)
    printf 'Usage: bash scripts/test-linux-ci.sh {ibus|ui}\n' >&2
    exit 2
    ;;
esac
if [[ "$(uname -s)" != Linux ]]; then
  printf 'These native checks require Linux.\n' >&2
  exit 2
fi

suzaku_ci_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$suzaku_ci_script_dir/.."
suzaku_ci_tmp=""
cleanup() {
  # Remove only a fresh, task-owned mktemp directory with the exact expected shape.
  case "$suzaku_ci_tmp" in
    /tmp/suzaku-sync-qa.??????|/tmp/suzaku-ui-qa.??????)
      if [[ -d "$suzaku_ci_tmp" && ! -L "$suzaku_ci_tmp" ]]; then
        rm -rf -- "$suzaku_ci_tmp"
      fi
      ;;
  esac
}
trap cleanup EXIT

if [[ "$suzaku_ci_mode" == ibus ]]; then
  if [[ -z ${SUZAKU_NATIVE_QA_BIN_DIR:-} ]]; then
    cargo build --locked --all-features --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
    suzaku_ci_native_bins=$(realpath -e -- "$suzaku_ci_script_dir/../target/debug")
  else
    suzaku_ci_native_bins=$(realpath -e -- "$SUZAKU_NATIVE_QA_BIN_DIR") || {
      printf 'Native QA binary directory does not exist: %s\n' "$SUZAKU_NATIVE_QA_BIN_DIR" >&2
      exit 1
    }
  fi
  [[ -d "$suzaku_ci_native_bins" ]] || {
    printf 'Native QA binary path is not a directory: %s\n' "$suzaku_ci_native_bins" >&2
    exit 1
  }
  for suzaku_ci_native_binary in linux_ime_host linux_ime_probe suzaku_tool; do
    [[ -f "$suzaku_ci_native_bins/$suzaku_ci_native_binary" && -x "$suzaku_ci_native_bins/$suzaku_ci_native_binary" ]] || {
      printf 'Missing executable native QA binary: %s/%s\n' "$suzaku_ci_native_bins" "$suzaku_ci_native_binary" >&2
      exit 1
    }
  done
  # The override selects the host/probe/tool under test; activation checks still
  # need the source-built panel test executable. Installed panel coverage belongs
  # to the application gates with SUZAKU_APP_QA_BIN_DIR.
  printf 'Native QA host/probe/tool: %s (panel activation tests are source-built)\n' "$suzaku_ci_native_bins"
  suzaku_ci_panel_test=$(cargo test --locked --all-features --bin panel --no-run --message-format=json |
    jq -r 'select(.reason == "compiler-artifact" and .target.name == "panel" and .profile.test == true) | .executable // empty')
  [[ -x "$suzaku_ci_panel_test" ]] || { printf 'Missing panel test executable.\n' >&2; exit 1; }
  suzaku_ci_tmp="$(mktemp -d /tmp/suzaku-sync-qa.XXXXXX)"
  # IBus may migrate Compose files in place. Never hand it the source fixture.
  install -m 600 "$suzaku_ci_script_dir/fixtures/compose.XCompose" "$suzaku_ci_tmp/compose.XCompose"
  timeout --kill-after=3s 120s env -u DISPLAY -u WAYLAND_DISPLAY -u SUZAKU_LEXICON_DIR dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_ci_tmp" \
    XDG_CONFIG_HOME="$suzaku_ci_tmp/config" \
    XDG_DATA_HOME="$suzaku_ci_tmp/data" \
    XCOMPOSEFILE="$suzaku_ci_tmp/compose.XCompose" \
    XLOCALEDIR=/usr/share/X11/locale \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
    SUZAKU_IME_CONFIG="$suzaku_ci_tmp/ime.json" \
    SUZAKU_LINUX_IME_SOCKET="$suzaku_ci_tmp/suzaku-ime/host.sock" \
    IBUS_ADDRESS="unix:path=$suzaku_ci_tmp/ibus.sock" \
    SUZAKU_NATIVE_SYNC_QA=1 \
    SUZAKU_NATIVE_QA_BIN_DIR="$suzaku_ci_native_bins" \
    SUZAKU_NATIVE_ACTIVATION_TEST="$suzaku_ci_panel_test" \
    /usr/bin/python3 -u scripts/test-native-sync.py
else
  suzaku_ci_tmp="$(mktemp -d /tmp/suzaku-ui-qa.XXXXXX)"
  mkdir -m 700 "$suzaku_ci_tmp/runtime"
  suzaku_ci_tests="$(cargo test --locked --all-features --bin panel -- --list)"
  for suzaku_ci_test in \
    windowing_native_test::native_window_fits_content_through_fold_zoom_and_restore \
    native_sync::tests::hidden_panel_wakes_for_new_contexts_after_coalesced_updates \
    keyboard_native_test::native_keyboard_editing_and_focus_return \
    candidates_native_test::native_candidate_clicks_and_async_refresh_are_safe \
    settings_native_test::native_tone_controls_follow_acknowledgements_and_reload \
    functional_network_audit_test::audit_provider_change_must_forget_standalone_commit_context \
    functional_network_audit_test::model_configuration_stays_bound_until_confirmed_reload \
    functional_network_audit_test::translation_requests_reject_stale_press_targets \
    translation::tests::native_translation_preserves_drafts_and_rejects_stale_contexts \
    native_sync::tests::native_source_insertions_preserve_drafts_until_acknowledged \
    native_sync::action_audit_test::native_candidate_actions_preserve_followup_input \
    native_sync::tool_chain_audit_test::audit_voice_auto_insert_must_not_follow_an_unrelated_native_context \
    native_sync::tool_chain_audit_test::tool_adoption_gestures_must_not_cross_native_targets \
    native_sync::tool_chain_audit_test::audit_local_tool_insertion_must_use_the_caret_word_boundary \
    status_native_test::native_panel_input_does_not_wait_for_status_probes \
    font_atlas::visual_tests::multilingual_candidates_render_through_the_gpu_without_question_mark_fallback
  do
    # A renamed/missing opt-in test must not become a successful zero-test run.
    if [[ $'\n'"$suzaku_ci_tests"$'\n' != *$'\n'"$suzaku_ci_test: test"$'\n'* ]]; then
      printf 'Required CI test is missing: %s\n' "$suzaku_ci_test" >&2
      exit 1
    fi
    suzaku_ci_probe_env=()
    if [[ "$suzaku_ci_test" == candidates_native_test::* ]]; then
      # Exercise the independent panel even when the in-process bridge reports
      # ready. Its synthetic Unix socket, not the desktop IBus, acknowledges sends.
      suzaku_ci_probe_env=(
        SUZAKU_LINUX_IME_FRAMEWORK=ibus SUZAKU_LINUX_IME_DAEMON_READY=1
        SUZAKU_LINUX_IME_REGISTERED=1 SUZAKU_LINUX_IME_RUNTIME_VISIBLE=1
        SUZAKU_LINUX_IME_HOST_READY=1 SUZAKU_LINUX_IME_ACTIVE=0
        SUZAKU_LINUX_IME_MARKED_TEXT=1 SUZAKU_LINUX_IME_COMMIT=1
      )
    fi
    if [[ "$suzaku_ci_test" == status_native_test::* ]]; then
      mkdir -m 700 "$suzaku_ci_tmp/bin"
      install -m 700 scripts/fixtures/slow-ibus.sh "$suzaku_ci_tmp/bin/ibus"
      suzaku_ci_probe_env=(
        "PATH=$suzaku_ci_tmp/bin:$PATH" SUZAKU_STATUS_NATIVE_QA=1
        "SUZAKU_STATUS_FIXTURE_BIN=$suzaku_ci_tmp/bin"
        SUZAKU_LINUX_IME_FRAMEWORK=ibus SUZAKU_LINUX_IME_DAEMON_READY=1
        SUZAKU_LINUX_IME_REGISTERED=1 SUZAKU_LINUX_IME_RUNTIME_VISIBLE=1
        SUZAKU_LINUX_IME_HOST_READY=1
      )
    fi
    timeout --kill-after=3s 90s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS \
      -u SUZAKU_LINUX_IME_ACTIVE -u SUZAKU_LEXICON_DIR \
      dbus-run-session -- env \
      "${suzaku_ci_probe_env[@]}" \
      XDG_RUNTIME_DIR="$suzaku_ci_tmp/runtime" \
      XDG_CONFIG_HOME="$suzaku_ci_tmp/$suzaku_ci_test/config" \
      XDG_DATA_HOME="$suzaku_ci_tmp/$suzaku_ci_test/data" \
      GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
      SUZAKU_IME_CONFIG="$suzaku_ci_tmp/$suzaku_ci_test/ime.json" \
      SUZAKU_LINUX_IME_SOCKET="$suzaku_ci_tmp/runtime/host.sock" \
      SUZAKU_LINUX_PORTAL_AVAILABLE=0 SUZAKU_LINUX_PIPEWIRE_AVAILABLE=0 \
      SUZAKU_LINUX_VOICE_FORCE_READY=1 SUZAKU_LINUX_VOICE_SAMPLE='hello world' \
      SUZAKU_PANEL_NATIVE_QA=1 SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
      xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
      cargo test --locked --all-features --bin panel "$suzaku_ci_test" \
      -- --exact --ignored --nocapture --test-threads=1
  done

  # Exercise the executable's startup gate, not just the instance helper. A
  # failed handoff must never fall through into a second unguarded window.
  suzaku_ci_launch_test=single_instance_failures_never_launch_an_unguarded_panel
  suzaku_ci_launch_tests=$(cargo test --locked --all-features --test linux_panel_instance_cli -- --list)
  [[ $'\n'"$suzaku_ci_launch_tests"$'\n' == *$'\n'"$suzaku_ci_launch_test: test"$'\n'* ]] || {
    printf 'Required panel launch test is missing.\n' >&2
    exit 1
  }
  timeout --kill-after=3s 90s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS -u SUZAKU_LEXICON_DIR \
    dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_ci_tmp/runtime" \
    XDG_CONFIG_HOME="$suzaku_ci_tmp/launch/config" \
    XDG_DATA_HOME="$suzaku_ci_tmp/launch/data" \
    SUZAKU_PANEL_NATIVE_QA=1 SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
    xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
    cargo test --locked --all-features --test linux_panel_instance_cli "$suzaku_ci_launch_test" \
    -- --exact --ignored --nocapture --test-threads=1
fi
