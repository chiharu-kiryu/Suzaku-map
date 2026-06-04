#!/usr/bin/env bash
set -euo pipefail

DEFAULT_ANDROID_SDK_ROOT="/opt/homebrew/share/android-commandlinetools"
DEFAULT_JAVA_HOME="/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home"

export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$DEFAULT_ANDROID_SDK_ROOT}}"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
export JAVA_HOME="${JAVA_HOME:-$DEFAULT_JAVA_HOME}"

if [ ! -d "$ANDROID_SDK_ROOT" ]; then
  echo "Android SDK root not found: $ANDROID_SDK_ROOT" >&2
  exit 1
fi

if [ ! -d "$JAVA_HOME" ]; then
  echo "Java home not found: $JAVA_HOME" >&2
  exit 1
fi

export PATH="$JAVA_HOME/bin:$PATH"

LATEST_NDK_DIR="$(find "$ANDROID_SDK_ROOT/ndk" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | tail -n 1 || true)"
if [ -z "$LATEST_NDK_DIR" ]; then
  echo "Android NDK not found under $ANDROID_SDK_ROOT/ndk" >&2
  exit 1
fi

export ANDROID_NDK_HOME="$LATEST_NDK_DIR"

HOST_TAG="darwin-x86_64"
LLVM_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin"
if [ ! -d "$LLVM_BIN" ]; then
  echo "NDK LLVM toolchain not found: $LLVM_BIN" >&2
  exit 1
fi

export SUZAKU_ANDROID_NDK_BIN="$LLVM_BIN"
