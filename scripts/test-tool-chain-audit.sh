#!/usr/bin/env bash
# Focused F32/F33 regressions, also run by Linux UI CI. Never use the desktop or a microphone.
set -euo pipefail
[[ $(uname -s) == Linux ]] || { printf 'These diagnostics require Linux.\n' >&2; exit 2; }
suzaku_tool_audit_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$suzaku_tool_audit_dir"
suzaku_tool_audit_tests=$(cargo test --locked --all-features --bin panel -- --list)
suzaku_tool_audit_tmp=$(mktemp -d /tmp/suzaku-tools-audit.XXXXXX)
trap 'rm -r -- "$suzaku_tool_audit_tmp"' EXIT
mkdir -m 700 "$suzaku_tool_audit_tmp/runtime"
suzaku_tool_audit_failed=0
for suzaku_tool_audit_case in \
  audit_voice_auto_insert_must_not_follow_an_unrelated_native_context \
  audit_local_tool_insertion_must_use_the_caret_word_boundary
do
  suzaku_tool_audit_test="native_sync::tool_chain_audit_test::$suzaku_tool_audit_case"
  if [[ $'\n'"$suzaku_tool_audit_tests"$'\n' != *$'\n'"$suzaku_tool_audit_test: test"$'\n'* ]]; then
    printf 'Required diagnostic is missing: %s\n' "$suzaku_tool_audit_test" >&2
    exit 2
  fi
  if ! timeout --kill-after=3s 60s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS \
    -u SUZAKU_LINUX_IME_ACTIVE dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_tool_audit_tmp/runtime" \
    XDG_CONFIG_HOME="$suzaku_tool_audit_tmp/$suzaku_tool_audit_case/config" \
    XDG_DATA_HOME="$suzaku_tool_audit_tmp/$suzaku_tool_audit_case/data" \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
    SUZAKU_IME_CONFIG="$suzaku_tool_audit_tmp/$suzaku_tool_audit_case/ime.json" \
    SUZAKU_LINUX_IME_SOCKET="$suzaku_tool_audit_tmp/runtime/host.sock" \
    SUZAKU_LINUX_PORTAL_AVAILABLE=0 SUZAKU_LINUX_PIPEWIRE_AVAILABLE=0 \
    SUZAKU_LINUX_VOICE_FORCE_READY=1 SUZAKU_LINUX_VOICE_SAMPLE='hello world' \
    SUZAKU_PANEL_NATIVE_QA=1 SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
    xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
    cargo test --locked --all-features --bin panel "$suzaku_tool_audit_test" \
    -- --exact --ignored --nocapture --test-threads=1
  then
    suzaku_tool_audit_failed=1
  fi
done
exit "$suzaku_tool_audit_failed"
