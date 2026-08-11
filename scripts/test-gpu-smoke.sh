#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./scripts/test-gpu-smoke.sh [mode]

Modes:
  local         (default) run GPU smoke tests with local LLM bridge test skipped unless enabled
  llm-bridge    run full GPU smoke tests with local llama bridge test enabled
  help          show this help
EOF
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

mode="${1:-local}"
case "$mode" in
  local)
    echo "Running GPU smoke tests (local mode)."
    cargo test --features gpu -- --nocapture
    ;;
  llm-bridge)
    echo "Running GPU smoke tests with local LLM bridge test enabled."
    RUN_LOCAL_LLM_BRIDGE_TESTS=1 cargo test --features gpu -- --nocapture
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
