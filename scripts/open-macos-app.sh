#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_DIR="${ROOT_DIR}/target/debug/Suzaku Panel.app"

if [ ! -d "${APP_DIR}" ]; then
  sh "${ROOT_DIR}/scripts/build-macos-app.sh" >/dev/null
fi

open "${APP_DIR}"
