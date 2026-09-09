#!/bin/sh
# Dedicated, non-recursive fixture. It never executes a test binary or real ibus.
set -eu
[ "${SUZAKU_STATUS_NATIVE_QA:-}" = 1 ] || exit 64
[ "${1:-}" = engine ] || exit 1
/bin/sleep 1.2
printf '%s\n' rime
