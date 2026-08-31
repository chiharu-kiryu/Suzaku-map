#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

profile="${SUZAKU_ANDROID_PROFILE:-debug}"
case "$profile" in
  debug)
    exec cargo run --features gpu --bin suzaku_tool -- android-build-native
    ;;
  release)
    exec cargo run --features gpu --bin suzaku_tool -- android-build-native --release
    ;;
  *)
    echo "Unsupported SUZAKU_ANDROID_PROFILE: $profile (expected debug or release)" >&2
    exit 2
    ;;
esac
