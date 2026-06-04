#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APK_PATH="$ROOT_DIR/android/app/build/outputs/apk/debug/app-debug.apk"

if [ ! -f "$APK_PATH" ]; then
  echo "Debug APK not found: $APK_PATH" >&2
  echo "Run ./gradlew assembleDebug in android/ first." >&2
  exit 1
fi

if ! adb get-state >/dev/null 2>&1; then
  echo "No Android device or emulator is online." >&2
  echo "Connect a device or start an emulator, then retry." >&2
  exit 1
fi

adb install -r "$APK_PATH"
echo "Installed: $APK_PATH"
