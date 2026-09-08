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
  cargo build --locked --all-features --bin linux_ime_host
  suzaku_ci_tmp="$(mktemp -d /tmp/suzaku-sync-qa.XXXXXX)"
  env -u DISPLAY -u WAYLAND_DISPLAY dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_ci_tmp" \
    XDG_CONFIG_HOME="$suzaku_ci_tmp/config" \
    XDG_DATA_HOME="$suzaku_ci_tmp/data" \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
    SUZAKU_IME_CONFIG="$suzaku_ci_tmp/ime.json" \
    SUZAKU_LINUX_IME_SOCKET="$suzaku_ci_tmp/suzaku-ime/host.sock" \
    IBUS_ADDRESS="unix:path=$suzaku_ci_tmp/ibus.sock" \
    SUZAKU_NATIVE_SYNC_QA=1 \
    /usr/bin/python3 scripts/test-native-sync.py
else
  suzaku_ci_tmp="$(mktemp -d /tmp/suzaku-ui-qa.XXXXXX)"
  mkdir -m 700 "$suzaku_ci_tmp/runtime"
  suzaku_ci_tests="$(cargo test --locked --all-features --bin panel -- --list)"
  for suzaku_ci_test in \
    windowing_native_test::native_window_fits_content_through_fold_zoom_and_restore \
    keyboard_native_test::native_keyboard_editing_and_focus_return \
    font_atlas::visual_tests::multilingual_candidates_render_through_the_gpu_without_question_mark_fallback
  do
    # A renamed/missing opt-in test must not become a successful zero-test run.
    if [[ $'\n'"$suzaku_ci_tests"$'\n' != *$'\n'"$suzaku_ci_test: test"$'\n'* ]]; then
      printf 'Required CI test is missing: %s\n' "$suzaku_ci_test" >&2
      exit 1
    fi
    env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS \
      dbus-run-session -- env \
      XDG_RUNTIME_DIR="$suzaku_ci_tmp/runtime" \
      XDG_CONFIG_HOME="$suzaku_ci_tmp/$suzaku_ci_test/config" \
      XDG_DATA_HOME="$suzaku_ci_tmp/$suzaku_ci_test/data" \
      GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
      SUZAKU_IME_CONFIG="$suzaku_ci_tmp/$suzaku_ci_test/ime.json" \
      SUZAKU_LINUX_IME_SOCKET="$suzaku_ci_tmp/runtime/host.sock" \
      SUZAKU_PANEL_NATIVE_QA=1 SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
      xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
      cargo test --locked --all-features --bin panel "$suzaku_ci_test" \
      -- --exact --ignored --nocapture --test-threads=1
  done
fi
