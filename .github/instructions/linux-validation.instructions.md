---
description: Resource-safe local Linux build, native-input and release validation
applyTo: "scripts/test-linux-*.sh,scripts/test-native-sync.py,scripts/package-linux.sh,Cargo.toml,.github/workflows/ci.yml"
---

# Local validation resource safety

On memory-constrained desktops or after an OOM, run compilation, native suites and packaging
serially with `CARGO_BUILD_JOBS=1`; serial Rust test threads do not limit concurrent builds.
Use a task-owned, memory-capped user systemd scope where available, following the concrete
command in [CONTRIBUTING](../../CONTRIBUTING.md), without stopping unrelated processes or
changing desktop-wide limits. Check OOM timestamps and preserve each failing log: a later
OOM or a successful rerun alone does not prove the cause of an earlier timeout or empty
reply. Keep resource-limit exits as failures and do not weaken assertions, extend deadlines
or retry until green to obtain a release result.
