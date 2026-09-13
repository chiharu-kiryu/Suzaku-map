#!/usr/bin/env bash
# Focused F42/F44 regressions, run by Linux CI. No desktop IME, real models or user settings.
set -euo pipefail
[[ $(uname -s) == Linux && $EUID -ne 0 ]] || { printf 'Run these diagnostics on Linux as a non-root user.\n' >&2; exit 2; }
suzaku_settings_audit_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$suzaku_settings_audit_dir"
suzaku_settings_audit_tmp=$(mktemp -d /tmp/suzaku-settings-audit.XXXXXX)
trap 'rm -r -- "$suzaku_settings_audit_tmp"' EXIT
mkdir -m 700 "$suzaku_settings_audit_tmp/runtime"
suzaku_settings_audit_failed=0
for suzaku_settings_audit_case in \
  settings_chain_audit_test::audit_first_status_must_preserve_a_startup_ui_edit \
  audit_native_patch_must_not_clobber_a_newer_saved_configuration \
  settings_chain_audit_test::audit_display_save_failure_must_be_visible_or_rolled_back
do
  if [[ "$suzaku_settings_audit_case" == settings_chain_audit_test::* ]]; then
    suzaku_settings_audit_target=(--bin panel)
  else
    suzaku_settings_audit_target=(--test settings_chain_audit)
  fi
  suzaku_settings_audit_tests=$(cargo test --locked --all-features "${suzaku_settings_audit_target[@]}" -- --list)
  if [[ $'\n'"$suzaku_settings_audit_tests"$'\n' != *$'\n'"$suzaku_settings_audit_case: test"$'\n'* ]]; then
    printf 'Required diagnostic is missing: %s\n' "$suzaku_settings_audit_case" >&2
    exit 2
  fi
  if ! timeout --kill-after=3s 60s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS \
    -u SUZAKU_LINUX_IME_ACTIVE dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_settings_audit_tmp/runtime" \
    XDG_CONFIG_HOME="$suzaku_settings_audit_tmp/$suzaku_settings_audit_case/config" \
    XDG_DATA_HOME="$suzaku_settings_audit_tmp/$suzaku_settings_audit_case/data" \
    SUZAKU_IME_CONFIG="$suzaku_settings_audit_tmp/$suzaku_settings_audit_case/ime.json" \
    SUZAKU_LINUX_IME_SOCKET="$suzaku_settings_audit_tmp/runtime/host.sock" \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
    SUZAKU_LINUX_PORTAL_AVAILABLE=0 SUZAKU_LINUX_PIPEWIRE_AVAILABLE=0 \
    SUZAKU_LINUX_VOICE_FORCE_READY=0 SUZAKU_PANEL_NATIVE_QA=1 \
    SUZAKU_SETTINGS_CHAIN_AUDIT=1 SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
    xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
    cargo test --locked --all-features "${suzaku_settings_audit_target[@]}" "$suzaku_settings_audit_case" \
    -- --exact --ignored --nocapture --test-threads=1
  then
    suzaku_settings_audit_failed=1
  fi
done
exit "$suzaku_settings_audit_failed"
