#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/panel-gpu.sh [mode]

Modes:
  debug         (default) build/run panel in debug profile with gpu features
  release       build/run panel in release profile with gpu features
  build         build debug binary only (gpu features)
  build-release build release binary only (gpu features)
  help         show this help
EOF
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

mode="${1:-debug}"
case "$mode" in
  debug)
    cargo run --features gpu --bin panel
    ;;
  release)
    cargo run --features gpu --release --bin panel
    ;;
  build)
    cargo build --features gpu --bin panel
    ;;
  build-release)
    cargo build --features gpu --release --bin panel
    ;;
  help|-h|--help)
    usage
    ;;
  *)
    echo "Unknown mode: $mode"
    usage
    exit 1
    ;;
esac

