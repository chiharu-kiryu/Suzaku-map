#!/usr/bin/env bash
# F45-F49 regressions run by Linux CI. Never use the desktop bus, runtime or configuration.
set -euo pipefail
[[ $(uname -s) == Linux && $EUID -ne 0 ]] || { printf 'Run on Linux as a non-root user.\n' >&2; exit 2; }
suzaku_data_audit_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$suzaku_data_audit_dir"
suzaku_data_audit_tests=$(cargo test --locked --all-features --test data_chain_audit -- --list)
suzaku_data_audit_tmp=$(mktemp -d /tmp/suzaku-data-audit.XXXXXX)
cleanup() {
  case "$suzaku_data_audit_tmp" in
    /tmp/suzaku-data-audit.??????)
      if [[ -d "$suzaku_data_audit_tmp" && ! -L "$suzaku_data_audit_tmp" ]]; then
        rm -r -- "$suzaku_data_audit_tmp"
      fi
      ;;
  esac
}
trap cleanup EXIT
suzaku_data_audit_failed=0
for suzaku_data_audit_case in \
  audit_restore_must_respect_a_live_host_using_a_settings_file_alias \
  audit_data_failures_preserve_sources_and_recovery_copies
do
  [[ $'\n'"$suzaku_data_audit_tests"$'\n' == *$'\n'"$suzaku_data_audit_case: test"$'\n'* ]] || {
    printf 'Required diagnostic is missing: %s\n' "$suzaku_data_audit_case" >&2
    exit 2
  }
  if ! timeout --kill-after=3s 45s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS \
    -u SUZAKU_LINUX_IME_SOCKET -u SUZAKU_IME_CONFIG dbus-run-session -- env \
    XDG_RUNTIME_DIR="$suzaku_data_audit_tmp" \
    XDG_CONFIG_HOME="$suzaku_data_audit_tmp/config" XDG_DATA_HOME="$suzaku_data_audit_tmp/data" \
    GSETTINGS_BACKEND=memory GIO_USE_VFS=local SUZAKU_DATA_CHAIN_AUDIT=1 \
    cargo test --locked --all-features --test data_chain_audit "$suzaku_data_audit_case" \
    -- --exact --ignored --nocapture --test-threads=1
  then
    suzaku_data_audit_failed=1
  fi
done
cleanup
trap - EXIT
exit "$suzaku_data_audit_failed"
