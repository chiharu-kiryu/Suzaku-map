#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

source "$SCRIPT_DIR/android-env.sh"

unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY

PROFILE="${SUZAKU_ANDROID_PROFILE:-debug}"
MODE_FLAG=()
if [ "$PROFILE" = "release" ]; then
  MODE_FLAG+=(--release)
fi

build_one() {
  local abi="$1"
  local rust_target="$2"
  local clang="$3"
  local cargo_var="$4"

  local out_dir="$REPO_ROOT/android/app/src/main/jniLibs/$abi"
  mkdir -p "$out_dir"

  echo "Building $rust_target for $abi"
  export "$cargo_var=$SUZAKU_ANDROID_NDK_BIN/$clang"
  local build_cmd=(cargo build --features gpu --target "$rust_target")
  if [ "$PROFILE" = "release" ]; then
    build_cmd+=(--release)
  fi
  "${build_cmd[@]}"

  local profile_dir="$PROFILE"
  local source_lib="$REPO_ROOT/target/$rust_target/$profile_dir/libsuzaku_map.so"
  if [ ! -f "$source_lib" ]; then
    echo "Expected Rust library missing: $source_lib" >&2
    exit 1
  fi

  cp "$source_lib" "$out_dir/libsuzaku_map.so"
}

build_one "arm64-v8a" "aarch64-linux-android" "aarch64-linux-android29-clang" "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
build_one "armeabi-v7a" "armv7-linux-androideabi" "armv7a-linux-androideabi29-clang" "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER"
build_one "x86" "i686-linux-android" "i686-linux-android29-clang" "CARGO_TARGET_I686_LINUX_ANDROID_LINKER"
build_one "x86_64" "x86_64-linux-android" "x86_64-linux-android29-clang" "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"

echo "Rust Android libraries copied into android/app/src/main/jniLibs"
