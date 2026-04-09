#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/target/debug"
APP_NAME="Suzaku Panel"
APP_DIR="${TARGET_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
BIN_PATH="${TARGET_DIR}/panel"
PLIST_PATH="${ROOT_DIR}/src/macos/SuzakuPanel-Info.plist"

cargo build --features gpu --bin panel --manifest-path "${ROOT_DIR}/Cargo.toml"

mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"
cp "${PLIST_PATH}" "${CONTENTS_DIR}/Info.plist"
cp "${BIN_PATH}" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

printf '%s\n' "${APP_DIR}"
