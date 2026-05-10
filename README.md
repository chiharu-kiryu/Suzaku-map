# Suzaku Map

A small set of orthogonal system concepts.

Start here:

- [Map](./src/00-Map.md)

## Set

1. [Signal](./src/01-Signal.md)
2. [Control](./src/02-Control.md)
3. [Resilience](./src/03-Resilience.md)
4. [Coordination](./src/04-Coordination.md)
5. [Expression](./src/05-Expression.md)
6. [IME for XR and Tablet](./src/06-IME-XR-Tablet.md)

## Rule

Each document should answer one question only:

- `Signal`: what is true or no longer trustworthy
- `Control`: what action must be gated or separated
- `Resilience`: how continuity is preserved under constraint
- `Coordination`: how actors cooperate without strong institutions
- `Expression`: how intent becomes valid structured output

## 中文说明

这是一个压缩后的概念集合。

每篇文档只负责一个问题，尽量保持正交。

## Prototype

This repository now contains a TDD-driven Rust IME prototype for XR and tablet environments:

- Engine: [src/ime.rs](./src/ime.rs)
- Demo: `cargo run`
- Tests: `cargo test`

## TDD Loop

The active workflow is:

1. Write or extend a behavior test in [tests/ime_engine.rs](./tests/ime_engine.rs)
2. Implement the smallest engine change in [src/ime.rs](./src/ime.rs)
3. Run `cargo test`

## GPU Rendering

Yes, the project can use GPU rendering.

- The engine remains UI-agnostic.
- A feature-gated `wgpu` scene builder lives in [src/ime.rs](./src/ime.rs).
- Enable it with `cargo test --features gpu` or integrate it into a windowed host renderer.
- Launch the candidate panel with `cargo run --features gpu --bin panel`.

## macOS First

The current host is optimized for testing on macOS first, while keeping the rendering path cross-platform:

- The panel uses macOS activation policy `Regular`, so it behaves like a normal desktop app.
- On macOS the window uses a more native tool-panel style titlebar setup for quick local testing.
- You can quit with `Cmd+Q`.
- You can use the shorthand aliases `cargo panel-macos`, `cargo panel-app-macos`, and `cargo test-gpu`.

For microphone and speech-recognition permission testing on macOS, prefer the app bundle build:

- Build bundle: `cargo panel-app-macos`
- Build and open bundle: `cargo open-panel-app-macos`
- Install to `~/Applications`: `cargo install-panel-app-macos`
- Install and open from `~/Applications`: `cargo install-open-panel-app-macos`
- Output: `target/debug/Suzaku Panel.app`
- Launch with Finder or `open "target/debug/Suzaku Panel.app"`

## System IME Host Direction

The project now has an explicit first-pass system IME host skeleton, separate from the GPU panel:

- Host session model: [src/ime_host.rs](./src/ime_host.rs)
- Cross-platform host dispatch: [src/platform/ime_host_dispatch.rs](./src/platform/ime_host_dispatch.rs)
- macOS IME bootstrap: [src/platform/macos_ime.rs](./src/platform/macos_ime.rs)
- macOS native bridge probe: [src/macos/ime_host_bridge.m](./src/macos/ime_host_bridge.m)
- bootstrap binary: `cargo run --bin macos_ime_host`

The intent is to move toward a real platform IME architecture in two layers:

1. `HostImeSession`
   - owns marked text, candidates, selection, and commit flow
   - can be shared by platform hosts without depending on the GPU panel
2. platform bridge
   - on macOS, this is the future `InputMethodKit` host direction
   - on Windows and Linux, parallel host adapters can be added later

The shared dispatch layer now makes that split explicit:

- macOS -> `InputMethodKit`
- Windows -> `Text Services Framework`
- Linux -> `IBus / Fcitx`

That means the core `HostImeSession` can stay platform-neutral while each desktop family exposes its own lifecycle, marked-text, candidate-window, and commit bridge at the platform edge.

The platform bootstrap modules now line up with that dispatch:

- macOS: [src/platform/macos_ime.rs](./src/platform/macos_ime.rs)
- Windows: [src/platform/windows_ime.rs](./src/platform/windows_ime.rs)
- Linux: [src/platform/linux_ime.rs](./src/platform/linux_ime.rs)

Right now only macOS has a live `marked text -> commit` roundtrip. Windows and Linux now expose stable host-shell metadata and recommended registration identifiers so their native adapters can grow without changing the shared host-session contract.

The current macOS IME host path is still a skeleton, not a registered system input method bundle yet. It now tells us:

- whether `InputMethodKit` is present
- whether we are running from a proper app bundle
- which bundle identifier the process currently exposes
- which `InputMethodConnectionName` the bundle currently exposes
- which controller class name the native bridge publishes
- whether a first-pass `IMKServer` bootstrap can be created

That gives us a stable base for the next step: replacing the standalone bootstrap with a true `IMKInputController`-backed input method bundle.

## Platform Roadmap

The current rollout priority is explicit and now reflected in code:

1. macOS
2. Windows
3. Ubuntu
4. Arch Linux
5. SteamOS

The shared support roadmap lives in [src/platform/mod.rs](./src/platform/mod.rs), with host-specific stubs in:

- [src/platform/macos.rs](./src/platform/macos.rs)
- [src/platform/windows.rs](./src/platform/windows.rs)
- [src/platform/linux.rs](./src/platform/linux.rs)

The intent is to keep the IME engine and renderer portable, while moving platform-specific work into adapters for:

- window lifecycle and chrome
- permission prompts
- voice input bridges
- future IME host integration points

Today, macOS is the most complete target. Windows is the next primary host target. Ubuntu, Arch Linux, and SteamOS remain planned secondary hosts after the desktop path is stable on macOS and Windows.

## Ubuntu Host Status

Ubuntu now has a dedicated host path instead of falling straight into the generic fallback:

- Linux support profile: [src/platform/linux.rs](./src/platform/linux.rs)
- Linux voice backend: [src/platform/linux_voice.rs](./src/platform/linux_voice.rs)
- Shared host selector: [src/platform/voice_host.rs](./src/platform/voice_host.rs)

Today this Ubuntu path supports:

- Ubuntu-specific font preference order
- Ubuntu-specific settings directory naming
- a dedicated Linux voice backend shared across Ubuntu, Arch, and SteamOS host selection
- debug transcript injection through `SUZAKU_LINUX_VOICE_SAMPLE` and the Ubuntu compatibility key `SUZAKU_UBUNTU_VOICE_SAMPLE`
- panel integration that reports `PipeWire / Portal Host` with the current Linux host flavor
- runtime probe for Portal/PipeWire availability, with explicit override flags through `SUZAKU_LINUX_PORTAL_AVAILABLE=1` and `SUZAKU_LINUX_PIPEWIRE_AVAILABLE=1`

It does not yet expose real live speech capture on Ubuntu, but it is now a first-class host path rather than a generic fallback bucket.

## Linux Host Flavors

Ubuntu, Arch Linux, and SteamOS now share the same Linux host selector and voice-backend shape:

- Linux support profile: [src/platform/linux.rs](./src/platform/linux.rs)
- Shared Linux voice backend: [src/platform/linux_voice.rs](./src/platform/linux_voice.rs)
- Host selector: [src/platform/voice_host.rs](./src/platform/voice_host.rs)

The current Linux flavor can be overridden with:

- `SUZAKU_LINUX_HOST=ubuntu`
- `SUZAKU_LINUX_HOST=arch`
- `SUZAKU_LINUX_HOST=steamos`

All three flavors currently share:

- the same `PipeWire / Portal Host` voice backend family
- a Linux native bridge skeleton at [src/linux/speech_bridge.c](./src/linux/speech_bridge.c)
- runtime probe for `xdg-desktop-portal`, `DBUS_SESSION_BUS_ADDRESS`, `pipewire`, `pipewire-pulse`, and `PIPEWIRE_RUNTIME_DIR`
- explicit environment overrides through `SUZAKU_LINUX_PORTAL_AVAILABLE=1` and `SUZAKU_LINUX_PIPEWIRE_AVAILABLE=1`
- debug transcript injection through `SUZAKU_LINUX_VOICE_SAMPLE`

The current Linux native bridge already owns:

- portal availability detection
- PipeWire availability detection
- live-capture readiness gating
- transcript queue plumbing for debug-seeded end-to-end panel testing

What it does not do yet is open a real PipeWire stream or xdg-desktop-portal speech session. That remaining work is now concentrated in [src/linux/speech_bridge.c](./src/linux/speech_bridge.c), without needing more Rust-side host refactors.

### Arch Linux Host Status

Arch Linux currently uses the shared Linux selector with an Arch-specific host flavor label:

- backend label: `PipeWire / Portal Host · Arch`
- support tier: secondary desktop host
- current limitation: live capture is still gated on the shared Linux backend probe and has not been wired to a real system recognizer yet

### SteamOS Host Status

SteamOS currently uses the shared Linux selector with a SteamOS-specific host flavor label:

- backend label: `PipeWire / Portal Host · SteamOS`
- support tier: secondary desktop/handheld host
- current limitation: live capture is still gated on the shared Linux backend probe and has not been wired to a real system recognizer yet

## Windows Voice Status

Windows voice input now has a native bridge boundary in place:

- Native bridge: [src/windows/speech_bridge.cpp](./src/windows/speech_bridge.cpp)
- Rust host wrapper: [src/platform/windows_voice.rs](./src/platform/windows_voice.rs)
- Host selector: [src/platform/voice_host.rs](./src/platform/voice_host.rs)

Today this Windows path supports:

- native FFI state, permission, start, stop, and transcript polling entry points
- debug transcript injection through `SUZAKU_WINDOWS_VOICE_SAMPLE`
- debug live-capture and error-state toggles through environment variables
- full panel-to-transcript-to-seed integration on the Rust side

It does not yet ship real live speech capture from the Windows system recognizer. The current bridge reports that live capture is not available yet, so the remaining work is concentrated in the native bridge implementation rather than the panel host flow.

Current Windows bridge debug toggles:

- `SUZAKU_WINDOWS_VOICE_SAMPLE="hello world"` seeds a transcript into the native bridge
- `SUZAKU_WINDOWS_VOICE_NATIVE_LIVE=1` reports that live capture is available
- `SUZAKU_WINDOWS_VOICE_FORCE_DENIED=1` forces a denied permission state
- `SUZAKU_WINDOWS_VOICE_FORCE_ERROR=1` forces an error state

## Panel Controls

The GPU candidate panel is a lightweight host for XR/tablet-style selection:

- `1`, `2`, `3`: load different seed phrases
- `Up` / `Down`: move candidate selection
- Left click: hit-test and select a candidate
- `D`: switch to degraded signal mode
- `R`: restore normal signal mode
- `Enter` or `Space`: force-commit the current selection

The panel now uses a cross-platform hierarchical text layout layer for in-panel copy, with title/status/candidate sections, width-constrained blocks, wrapping, max-line clipping, and ellipsis handling rendered as GPU quads. The window title still mirrors key state for quick debugging.
