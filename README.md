# Suzaku Map

Suzaku Map is a multimodal IME project.

Current release: **0.4.9**.

### 0.4.9 Highlights

- Added a native Linux IBus engine host, user-service installer, runtime diagnostics, and a
  private panel-to-host commit channel.
- Added a Suzaku system tray with show/hide, settings, position reset, and quit actions, plus
  per-session single-instance activation.
- Refined the Ubuntu / GNOME Wayland panel with non-focusing input, monitor-bound dragging,
  close-to-tray behavior, compact outer spacing, and correctly positioned settings windows.
- Reduced desktop input latency through idle-aware redraw scheduling, reusable GPU buffers,
  cached system-font atlases, displayed-scene hit testing, and width-aware candidate wrapping.
- Improved the Android IME with clipboard privacy controls, batched native render snapshots,
  interruption-safe animations, and frame-aligned handwriting updates.

### 0.4.8 Highlights

- Scaled-aware text rendering pipeline for clearer zoomed desktop UI.
- Performance-safe font atlas rebuild behavior on window scale changes.
- Updated font atlas and sampling defaults to improve readability at larger scales.
- Tunable handwritten-stroke sampling and smoothing via environment variables, enabling faster on-device touch calibration for 0.4.8.
- Added theme presets for desktop panel visuals.
  - Supported values in `panel-settings.toml` (`theme_preset`):
    - `daylight`
    - `device_dark`
    - `solarized`
    - `high_contrast`
  - Default config paths:
    - Linux: `~/.config/suzaku-panel/panel-settings.toml` (or `$XDG_CONFIG_HOME/suzaku-panel/panel-settings.toml`)
    - macOS: `~/Library/Application Support/SuzakuPanel/panel-settings.toml`
    - Windows: `%APPDATA%/SuzakuPanel/panel-settings.toml`

The repository currently contains three layers that evolve together:

- a shared Rust IME core
- platform-specific system IME host adapters
- debug and companion UIs for desktop and Android

The codebase is no longer a single-file prototype. The current shape is:

- Shared engine: [src/ime.rs](src/ime.rs)
- Host session contract: [src/ime_host.rs](src/ime_host.rs)
- Platform adapters: [src/platform](src/platform)
- Desktop GPU companion: [src/bin/panel.rs](src/bin/panel.rs)
- Android IME app: [android](android)

## Architecture

### 1. Shared IME Core

The shared IME core owns:

- seed text normalization
- candidate generation
- candidate selection
- commit flow
- language-plugin dispatch

Key files:

- [src/ime/core_engine.rs](src/ime/core_engine.rs)
- [src/languages/mod.rs](src/languages/mod.rs)
- [src/languages/english.rs](src/languages/english.rs)
- [src/languages/llm.rs](src/languages/llm.rs)
- [src/languages/llama.rs](src/languages/llama.rs)

### 2. System IME Host Layer

The host layer is where platform-native IME lifecycles connect to the shared session.

Core contract:

- [src/ime_host.rs](src/ime_host.rs)

Adapter and lifecycle contract:

- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)

Dispatch and capability map:

- [src/platform/mod.rs](src/platform/mod.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_dispatch.rs](src/platform/ime_host_dispatch.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/platform/panel_companion_dispatch.rs](src/platform/panel_companion_dispatch.rs)

Current platform direction:

- macOS: InputMethodKit host path is the most complete
- Windows: TSF skeleton is present
- Android: InputMethodService app path is active
- Linux: native IBus engine host is available; Fcitx remains a registration scaffold

### 3. Companion UIs

The project uses two kinds of companion UI:

- desktop GPU debug companion
- native or app-hosted candidate/input surfaces per platform

Desktop GPU scene builders live in:

- [src/ime/gpu](src/ime/gpu)

Companion style lives in:

- [src/platform/companion_style.rs](src/platform/companion_style.rs)

## Repository Layout

### Core Rust

- [src/lib.rs](src/lib.rs)
- [src/ime.rs](src/ime.rs)
- [src/ime_host.rs](src/ime_host.rs)
- [src/platform/ime_host_adapter.rs](src/platform/ime_host_adapter.rs)
- [src/platform/ime_host_runtime.rs](src/platform/ime_host_runtime.rs)
- [src/panel_support.rs](src/panel_support.rs)

### Desktop Panel

- [src/bin/panel.rs](src/bin/panel.rs)
- [src/bin/panel](src/bin/panel)

### macOS

- [src/platform/macos_ime.rs](src/platform/macos_ime.rs)
- [src/macos/ime_host_bridge.m](src/macos/ime_host_bridge.m)
- [src/macos/speech_bridge.m](src/macos/speech_bridge.m)
- [src/macos/text_output_bridge.m](src/macos/text_output_bridge.m)

### Android

- [src/platform/android_ime.rs](src/platform/android_ime.rs)
- [src/platform/android_jni_bridge.rs](src/platform/android_jni_bridge.rs)
- [android/app/src/main/java/dev/suzaku/android/ime](android/app/src/main/java/dev/suzaku/android/ime)

### Concept Notes

The original system notes are still part of the repository and now serve as background design documents:

- [src/00-Map.md](src/00-Map.md)
- [src/01-Signal.md](src/01-Signal.md)
- [src/02-Control.md](src/02-Control.md)
- [src/03-Resilience.md](src/03-Resilience.md)
- [src/04-Coordination.md](src/04-Coordination.md)
- [src/05-Expression.md](src/05-Expression.md)
- [src/06-IME-XR-Tablet.md](src/06-IME-XR-Tablet.md)

## Main Commands

### Rust checks

```bash
cargo check --features gpu
./scripts/test-gpu-smoke.sh
cargo test --features gpu
cargo build --features gpu
cargo build --release --features gpu
cargo run --release --features gpu --bin suzaku-map -- --help
cargo ime-host
```

### Desktop GPU companion

```bash
cargo run --features gpu --bin panel
```

Or use one-command launcher:

```bash
./scripts/panel-gpu.sh
./scripts/panel-gpu.sh release
./scripts/panel-gpu.sh build-release
```

#### Current local priority: Ubuntu / Wayland

The desktop GPU companion is the primary development path on the current workstation. Its event
loop sleeps while the UI is idle, wakes at frame cadence only for active voice/feedback/text-scroll
animation, reuses streaming GPU vertex buffers, and uses the last displayed scene for pointer hit
testing. Installed platform fonts are rasterized into a cached GPU atlas, with the built-in bitmap
font retained only as a startup fallback. On GNOME Wayland with XWayland available, the main panel
automatically uses a non-focusing X11 override-redirect window. It opens at the bottom center,
continues to accept pointer/touch input, and leaves IBus focus in the target application while
candidates are clicked. The top-center grip provides manual drag handling for this unmanaged window,
keeps the panel inside the active monitor, and preserves its expanded position. The default 100%
layout uses a compact outer gutter, a dedicated close-to-tray button, and readable two-column
candidates with an unpaired candidate spanning the final row. A native StatusNotifierItem with a
Suzaku red-and-gold icon remains available after the panel is hidden: click it to show or hide the
panel, or use its menu to open settings, reset the window position, and quit the process. The
settings window is placed above the main panel and clamped to the active monitor. Tray-state updates
run off the UI thread. Launching the panel again signals the existing process to show its window
rather than creating duplicate windows or tray icons. Set
`SUZAKU_LINUX_PANEL_BACKEND=wayland` to force the native Wayland window path.
Cross-platform adapters remain in the tree, but local desktop behavior is validated first.

### GPU smoke test profile

The default GPU smoke test command intentionally skips local-loopback llama bridge tests to stay stable in restricted environments.

```bash
# Local validation (default)
cargo test-gpu-smoke
./scripts/test-gpu-smoke.sh
```

### Integration validation (local-loopback llama bridge required)

```bash
RUN_LOCAL_LLM_BRIDGE_TESTS=1 cargo test-gpu-smoke-llm
./scripts/test-gpu-smoke.sh llm-bridge
```

### Common build note (important)

The root crate is feature-gated for GPU components.  
Builds without `--features gpu` may fail with errors about missing `winit` or `ime::gpu` items.  
For desktop companion/scene/debug work, prefer:

```bash
cargo build --features gpu
cargo run --features gpu --bin panel
cargo run --release --features gpu --bin suzaku-map -- --help
```

Shortcuts from Cargo aliases:

```bash
cargo panel-macos
# GPU-focused tests:
cargo test-gpu                 # raw GPU test run (no --nocapture)
cargo test-gpu-smoke           # local smoke run (bridge test skipped unless enabled)
cargo test-gpu-smoke-llm       # same command; set RUN_LOCAL_LLM_BRIDGE_TESTS=1 to include bridge test
```

### macOS panel bundle

```bash
cargo panel-app-macos
cargo open-panel-app-macos
cargo install-panel-app-macos
cargo install-open-panel-app-macos
```

### macOS IME bundle

```bash
cargo ime-app-macos
cargo open-ime-app-macos
cargo install-ime-app-macos
cargo install-open-ime-app-macos
cargo enable-ime-app-macos
cargo install-enable-ime-app-macos
```

### Android

Environment check:

```bash
cargo android-doctor
```

Linux registration helper:

```bash
cargo linux-register install
cargo linux-register status
cargo linux-register uninstall
cargo linux-register verify
cargo linux-register diag
```

Rust-side host bootstrap:

```bash
cargo ime-host
cargo android-ime-host
```

Native helper actions (Rust-native runner):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cd android && ./gradlew assembleDebug
cargo android-install-debug
cargo android-enable-ime
```

### Handwriting tuning (desktop panel)

The desktop panel supports runtime tuning for touch/mouse handwriting capture through environment variables (set before launch):

- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS` (default `8`) – minimum touch sample interval, milliseconds.
- `SUZAKU_HANDWRITING_MOUSE_SAMPLE_MS` (default `4`) – minimum mouse sample interval, milliseconds.
- `SUZAKU_HANDWRITING_EDGE_PADDING` (default `4.0`) – in-canvas inset for clamped stroke coordinates.
- `SUZAKU_HANDWRITING_MIN_POINT_DISTANCE` (default `1.2`) – base minimum point distance.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP` (default `3.2`) – base interpolation segment size.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA` (default `0.22`) – base smoothing amount.
- `SUZAKU_HANDWRITING_SPEED_REFERENCE` (default `16.0`) – speed reference for adaptive profile.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MIN` (default `0.55`) – minimum speed ratio.
- `SUZAKU_HANDWRITING_SPEED_RATIO_MAX` (default `2.0`) – maximum speed ratio.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MIN` (default `2.0`) – lower clamp for interpolation step.
- `SUZAKU_HANDWRITING_INTERPOLATION_STEP_MAX` (default `6.8`) – upper clamp for interpolation step.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MIN` (default `0.78`) – lower clamp for min distance.
- `SUZAKU_HANDWRITING_MIN_DISTANCE_MAX` (default `2.0`) – upper clamp for min distance.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MIN` (default `0.14`) – lower smoothing clamp.
- `SUZAKU_HANDWRITING_SMOOTH_ALPHA_MAX` (default `0.34`) – upper smoothing clamp.
- `SUZAKU_HANDWRITING_TOUCH_SMOOTH_FACTOR` (default `1.05`) – extra touch smoothing multiplier.
- `SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE` (default `1.18`) – touch distance scale.
- `SUZAKU_HANDWRITING_TOUCH_INTERPOLATION_SCALE` (default `0.9`) – touch interpolation scale.
- `SUZAKU_HANDWRITING_TOUCH_SAMPLE_DISTANCE_SCALE` (default `0.85`) – touch-specific distance scale.

Example:

```bash
SUZAKU_HANDWRITING_TOUCH_SAMPLE_MS=6 SUZAKU_HANDWRITING_TOUCH_MIN_DISTANCE_SCALE=1.2 cargo run --features gpu --bin panel
```

Recommended default entry (no script dependency):

```bash
eval "$(cargo android-env)"
cargo android-build-native
cargo android-install-debug
cargo android-enable-ime
cargo linux-register install
```

APK output:

- [android/app/build/outputs/apk/debug/app-debug.apk](android/app/build/outputs/apk/debug/app-debug.apk)

## Current Platform Status

### macOS

Most complete desktop path today.

Available:

- GPU panel companion
- app bundle packaging
- IMK host bootstrap
- marked text / commit roundtrip skeleton
- native candidate companion window path
- voice bridge

### Windows

Scaffolded:

- platform capability map
- IME host skeleton
- voice backend skeleton

### Android

Active app-host path.

Available:

- InputMethodService app
- shared HostImeSession integration
- adaptive letter, number, and symbol keyboard layouts
- one-shot Shift, Caps Lock, editor actions, and Unicode-safe repeating backspace
- context-aware email/URI shortcuts, dedicated numeric/phone/date-time pads, and action labels
- system keyboard switching plus candidate-boundary spaces, punctuation, and Enter actions
- app-provided Android completions and stale-composition cancellation on cursor movement
- transient clipboard paste drawer with sensitive-preview protection and no saved history
- persistent Android controls for auto-capitalization, a dedicated number row, and haptics
- a setup/settings hub for enabling Suzaku, choosing it, sharing preferences, and diagnostics
- keyboard, voice, and handwrite drawers
- secure password-field input with candidates, voice, and handwriting disabled
- compact-bubble-first activation flow
- in-panel candidate strip
- batched native render snapshots, reused keyboard/candidate views, interruption-safe panel
  animation, and frame-aligned handwriting redraws for a smoother input loop

### Linux

Current workstation-first path: the Rust GPU companion and native IBus host on Ubuntu / GNOME
Wayland.

- event-driven desktop redraw with idle suspension and bounded animation wakeups
- reusable GPU vertex buffers and displayed-scene pointer hit testing
- platform-font GPU atlas rendering with a deterministic bitmap fallback
- settings-window synchronization without redraw loops or repeated idle config writes
- bottom-centered, pointer-active, non-focusing XWayland panel mode on GNOME Wayland, with an
  explicit native-Wayland override, visible drag grip, and monitor-bound position clamping
- StatusNotifierItem system tray with a built-in Suzaku icon, show/hide activation, position reset,
  direct settings access, close-to-tray behavior, and an explicit quit action
- per-session single-instance control that reopens the existing panel on a repeated launch
- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- native IBus `Factory`/`Engine` host backed by the shared Rust candidate engine
- IBus preedit, lookup-table navigation, numeric selection, candidate clicks, and commit
- local user-only panel-to-IBus commit channel at `$XDG_RUNTIME_DIR/suzaku-ime/host.sock`
- truthful runtime registration status:
  - bootstrap reads `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to switch to Fcitx checks
  - a component marker alone does not imply marked-text or commit readiness
  - IBus static and dynamically registered engines are probed separately from daemon and host state
  - quick local override for staging: `SUZAKU_LINUX_IME_REGISTERED=1`
  - lifecycle simulation overrides for host checks:
    - `SUZAKU_LINUX_IME_DAEMON_READY=1|0`
    - `SUZAKU_LINUX_IME_RUNTIME_VISIBLE=1|0`
    - `SUZAKU_LINUX_IME_ACTIVE=1|0`
    - `SUZAKU_LINUX_IME_HOST_READY=1|0`
    - `SUZAKU_LINUX_IME_MARKED_TEXT=1|0`
    - `SUZAKU_LINUX_IME_COMMIT=1|0`
    - `SUZAKU_LINUX_IME_NATIVE_CANDIDATE_WINDOW=1|0`

Build and install the native host without root:

```bash
# Normally install the build headers once through your distribution:
# sudo apt install libibus-1.0-dev
cargo build --release --features linux-ibus --bin linux_ime_host --bin suzaku_tool
SUZAKU_LINUX_IME_HOST_BIN="$PWD/target/release/linux_ime_host" \
  target/release/suzaku_tool linux-register install
target/release/suzaku_tool linux-register verify
```

The installer copies the host to `~/.local/libexec/suzaku/linux_ime_host`, enables
`~/.config/systemd/user/suzaku-ibus.service`, and dynamically registers
`dev.suzaku.linux.ime` with the running IBus daemon. During a host upgrade it records and restores
the active IBus engine, so restarting the service does not leave the desktop on an empty engine.
`linux-register uninstall` disables the user service and removes the installed host and component
metadata.

Useful commands:

- `cargo linux-register install`
- `cargo linux-register status`
- `cargo linux-register uninstall`
- `cargo linux-register verify`
- `cargo linux-register diag`
- `cargo run --features linux-ibus --bin linux_ime_probe -- ni`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --ipc "panel commit probe"`

Framework selection is shared with bootstrap:

- `SUZAKU_LINUX_IME_FRAMEWORK=fcitx` to register and check Fcitx layout
- default remains IBus when not set

Fcitx currently retains marker/status support; a native Fcitx engine service is still pending.

## Refactor Policy

This repository is intentionally being kept in a refactor-friendly early state.

Current codebase rules that the project is trying to hold:

- keep modules small enough to reason about
- separate shared IME logic from platform lifecycles
- separate system-host code from debug companion UI
- let README describe the current codebase, not the historical prototype

## Current Refactor Snapshot

This repository was recently cleaned up in two important ways:

- the GPU scene builder no longer carries the old unused `*.inc` and `*_macro` fragments
- the top-level README now describes the current architecture instead of the original single-file prototype story

That makes the current structure a better base for continued platform work, especially Android and system-host integration.
