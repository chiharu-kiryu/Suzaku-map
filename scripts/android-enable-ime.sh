#!/usr/bin/env bash
set -euo pipefail

IME_ID="dev.suzaku.android.ime/.SuzakuInputMethodService"

if ! adb get-state >/dev/null 2>&1; then
  echo "No Android device or emulator is online." >&2
  echo "Connect a device or start an emulator, then retry." >&2
  exit 1
fi

adb shell ime enable "$IME_ID"
adb shell ime set "$IME_ID"
echo "Enabled and selected IME: $IME_ID"
