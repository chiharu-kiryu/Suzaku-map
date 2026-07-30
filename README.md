# Suzaku Map

Suzaku Map is a multimodal IME project.

Current release: **0.4.0**.

### 0.4.0 Highlights

- Scaled-aware text rendering pipeline for clearer zoomed desktop UI.
- Performance-safe font atlas rebuild behavior on window scale changes.
- Updated font atlas and sampling defaults to improve readability at larger scales.

The repository currently contains three layers that evolve together:

- a shared Rust IME core
- platform-specific system IME host adapters
- debug and companion UIs for desktop and Android

The codebase is no longer a single-file prototype. The current shape is:

- Shared engine: [/Users/Shared/chroot/dev/Suzaku-map/src/ime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime.rs)
- Host session contract: [/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs)
- Platform adapters: [/Users/Shared/chroot/dev/Suzaku-map/src/platform](/Users/Shared/chroot/dev/Suzaku-map/src/platform)
- Desktop GPU companion: [/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel.rs](/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel.rs)
- Android IME app: [/Users/Shared/chroot/dev/Suzaku-map/android](/Users/Shared/chroot/dev/Suzaku-map/android)

## Architecture

### 1. Shared IME Core

The shared IME core owns:

- seed text normalization
- candidate generation
- candidate selection
- commit flow
- language-plugin dispatch

Key files:

- [/Users/Shared/chroot/dev/Suzaku-map/src/ime/core_engine.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime/core_engine.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/languages/mod.rs](/Users/Shared/chroot/dev/Suzaku-map/src/languages/mod.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/languages/english.rs](/Users/Shared/chroot/dev/Suzaku-map/src/languages/english.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/languages/llm.rs](/Users/Shared/chroot/dev/Suzaku-map/src/languages/llm.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/languages/llama.rs](/Users/Shared/chroot/dev/Suzaku-map/src/languages/llama.rs)

### 2. System IME Host Layer

The host layer is where platform-native IME lifecycles connect to the shared session.

Core contract:

- [/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs)

Adapter and lifecycle contract:

- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs)

Dispatch and capability map:

- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/mod.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/mod.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_dispatch.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_dispatch.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_runtime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_runtime.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/panel_companion_dispatch.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/panel_companion_dispatch.rs)

Current platform direction:

- macOS: InputMethodKit host path is the most complete
- Windows: TSF skeleton is present
- Android: InputMethodService app path is active
- Linux: IBus/Fcitx direction is scaffolded

### 3. Companion UIs

The project uses two kinds of companion UI:

- desktop GPU debug companion
- native or app-hosted candidate/input surfaces per platform

Desktop GPU scene builders live in:

- [/Users/Shared/chroot/dev/Suzaku-map/src/ime/gpu](/Users/Shared/chroot/dev/Suzaku-map/src/ime/gpu)

Companion style lives in:

- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/companion_style.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/companion_style.rs)

## Repository Layout

### Core Rust

- [/Users/Shared/chroot/dev/Suzaku-map/src/lib.rs](/Users/Shared/chroot/dev/Suzaku-map/src/lib.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/ime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs](/Users/Shared/chroot/dev/Suzaku-map/src/ime_host.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_adapter.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_runtime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/ime_host_runtime.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/panel_support.rs](/Users/Shared/chroot/dev/Suzaku-map/src/panel_support.rs)

### Desktop Panel

- [/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel.rs](/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel](/Users/Shared/chroot/dev/Suzaku-map/src/bin/panel)

### macOS

- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/macos_ime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/macos_ime.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/macos/ime_host_bridge.m](/Users/Shared/chroot/dev/Suzaku-map/src/macos/ime_host_bridge.m)
- [/Users/Shared/chroot/dev/Suzaku-map/src/macos/speech_bridge.m](/Users/Shared/chroot/dev/Suzaku-map/src/macos/speech_bridge.m)
- [/Users/Shared/chroot/dev/Suzaku-map/src/macos/text_output_bridge.m](/Users/Shared/chroot/dev/Suzaku-map/src/macos/text_output_bridge.m)

### Android

- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/android_ime.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/android_ime.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/src/platform/android_jni_bridge.rs](/Users/Shared/chroot/dev/Suzaku-map/src/platform/android_jni_bridge.rs)
- [/Users/Shared/chroot/dev/Suzaku-map/android/app/src/main/java/dev/suzaku/android/ime](/Users/Shared/chroot/dev/Suzaku-map/android/app/src/main/java/dev/suzaku/android/ime)

### Concept Notes

The original system notes are still part of the repository and now serve as background design documents:

- [/Users/Shared/chroot/dev/Suzaku-map/src/00-Map.md](/Users/Shared/chroot/dev/Suzaku-map/src/00-Map.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/01-Signal.md](/Users/Shared/chroot/dev/Suzaku-map/src/01-Signal.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/02-Control.md](/Users/Shared/chroot/dev/Suzaku-map/src/02-Control.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/03-Resilience.md](/Users/Shared/chroot/dev/Suzaku-map/src/03-Resilience.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/04-Coordination.md](/Users/Shared/chroot/dev/Suzaku-map/src/04-Coordination.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/05-Expression.md](/Users/Shared/chroot/dev/Suzaku-map/src/05-Expression.md)
- [/Users/Shared/chroot/dev/Suzaku-map/src/06-IME-XR-Tablet.md](/Users/Shared/chroot/dev/Suzaku-map/src/06-IME-XR-Tablet.md)

## Main Commands

### Rust checks

```bash
cargo check --features gpu
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
cargo test-gpu
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

Rust-side host bootstrap:

```bash
cargo ime-host
cargo android-ime-host
```

Build native Rust libraries:

```bash
bash scripts/android-build-native.sh
```

Build Android debug APK:

```bash
cd android
./gradlew assembleDebug
```

Install and enable on device:

```bash
bash scripts/android-install-debug.sh
bash scripts/android-enable-ime.sh
```

APK output:

- [/Users/Shared/chroot/dev/Suzaku-map/android/app/build/outputs/apk/debug/app-debug.apk](/Users/Shared/chroot/dev/Suzaku-map/android/app/build/outputs/apk/debug/app-debug.apk)

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
- keyboard, voice, and handwrite drawers
- compact-bubble-first activation flow
- in-panel candidate strip

### Linux

Scaffolded:

- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- Linux IME host direction for IBus / Fcitx

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
