#!/usr/bin/env bash
# Keep real IBus errors visible only inside the guarded native CI fixture.
set -euo pipefail
[[ ${SUZAKU_NATIVE_SYNC_QA:-} == 1 ]]
[[ ${XDG_RUNTIME_DIR:-} == /tmp/suzaku-sync-qa.* && ! -L $XDG_RUNTIME_DIR ]]
[[ ${XDG_RUNTIME_DIR%/*} == /tmp ]]
[[ ${IBUS_ADDRESS:-} == "unix:path=$XDG_RUNTIME_DIR/ibus.sock" ]]
[[ -z ${DISPLAY:-} && -z ${WAYLAND_DISPLAY:-} ]]
[[ ${SUZAKU_NATIVE_REAL_IBUS:-} == /* && -x $SUZAKU_NATIVE_REAL_IBUS ]]
[[ ! $SUZAKU_NATIVE_REAL_IBUS -ef ${BASH_SOURCE[0]} ]]
[[ $# -ge 1 && $# -le 2 && $1 == engine ]]
suzaku_trace="$XDG_RUNTIME_DIR/activation-ibus.log"
{
  printf 'ibus'
  printf ' %q' "$@"
  printf '\n'
} >> "$suzaku_trace"
suzaku_status=0
"$SUZAKU_NATIVE_REAL_IBUS" "$@" 2>> "$suzaku_trace" || suzaku_status=$?
printf 'exit=%s\n' "$suzaku_status" >> "$suzaku_trace"
exit "$suzaku_status"
