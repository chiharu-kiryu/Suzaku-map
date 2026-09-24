# Contributing

Linux / IBus on Ubuntu 24.04 amd64 is the current target. Read the
[input rules](docs/ibus-candidates.md) and [limitations](docs/known-limitations.md) first.
Focused fixes with regression tests are especially useful during Alpha.
Current input-quality work prioritizes **English and Simplified Chinese**; Japanese stays compatible
without expanding its scope. See the [development priorities](DEVELOPMENT.md).

## Build and test

CI uses Rust 1.95.0 and the committed Cargo.lock. Ubuntu dependencies:

```bash
sudo apt install build-essential pkg-config libibus-1.0-dev libglib2.0-dev \
  libx11-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev \
  libegl1-mesa-dev libgl1-mesa-dri libvulkan1 mesa-vulkan-drivers \
  dbus-daemon ibus gir1.2-ibus-1.0 gir1.2-pango-1.0 python3-gi \
  xvfb xauth fontconfig fonts-dejavu-core fonts-noto-cjk shellcheck \
  dpkg-dev jq desktop-file-utils
rustup component add rustfmt clippy
cargo fmt --all --check
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features -- --test-threads=1
bash scripts/test-linux-ci.sh ibus
bash scripts/test-linux-ci.sh ui
```

Run the paired offline input-quality gates without a model:

```bash
cargo test --locked --all-features --test english_completion_quality --test chinese_completion_quality
```

The English gate covers 40 authored word/sentence scenarios; Chinese covers 40 known word/phrase
cases in four spelling forms (160 primary-conversion checks), plus boundary, completion, literal and
commit checks. These are project-authored regressions, not independent corpus accuracy. The native
`ibus` gate adds 12 bilingual numeric-adoption/undo/continuation/explicit-commit workflows.

Real application checks additionally need GTK input modules and Qt test bindings:

```bash
sudo apt install gnome-text-editor zenity x11-utils x11-xkb-utils libxtst6 ibus-gtk3 ibus-gtk4 \
  gir1.2-gtk-3.0 gir1.2-atspi-2.0 at-spi2-core python3-pyqt5 python3-pyqt6 qt6-qpa-plugins
bash scripts/test-linux-apps.sh gtk
bash scripts/test-linux-apps.sh keyboard
bash scripts/test-linux-apps.sh popup
bash scripts/test-linux-apps.sh lifecycle
bash scripts/test-linux-apps.sh bus-restart
bash scripts/test-linux-apps.sh qt5
bash scripts/test-linux-apps.sh qt6
SUZAKU_IBUS_INLINE_PREEDIT=1 bash scripts/test-linux-ci.sh ibus
```

`bash scripts/test-linux-apps.sh cross` also tests an already installed `google-chrome`, or the
executable selected by `SUZAKU_APP_QA_BROWSER`. It uses an owned temporary profile and local page,
not personal browser tabs. CI runs GTK/Qt; browser/VS Code checks below are local, not CI gates.
Application tests explicitly clear the inline-preedit opt-in to verify the default draft mode.

Additional installed-application gates:

```bash
bash scripts/test-linux-apps.sh firefox
bash scripts/test-linux-apps.sh vscode
```

Use `SUZAKU_APP_QA_FIREFOX` / `SUZAKU_APP_QA_CODE` to select an installed executable.
Firefox uses an owned profile with external traffic directed to a closed loopback proxy.
On Ubuntu Snap, the `/usr/bin/firefox` migration wrapper and Snap mount namespace are separate
from application compatibility: this machine was tested with
`SUZAKU_APP_QA_FIREFOX=/snap/firefox/current/usr/lib/firefox/firefox`, with the browser sandbox
left enabled. That does not qualify the Snap launcher. VS Code uses an owned profile, empty
extension directory, and the repository's development-only observer; no extension is installed
into the personal editor. The observer focuses fields/sets selections and reads documents;
all text comes from physical XTest events, with exact Ctrl+S disk read-back too. Fixtures disable
updates and online suggestions; VS Code uses a private basic password store to avoid a
first-run keyring prompt. Existing VS Code IPC/portable variables are cleared by the runner.

Live model input-safety checks are **opt-in** and use only synthetic text and the already running
Ollama at `127.0.0.1:11434`, through an owned observation relay. No downloads, service restarts,
personal configuration changes or cloud fallback:

```bash
SUZAKU_MODEL_LOCAL_QA=1 SUZAKU_MODEL_QA_MODEL=YOUR_INSTALLED_LOCAL_MODEL \
  bash scripts/test-linux-apps.sh model
```

This needs the Chrome dependency above and a model with an observable in-flight request. It checks
foreground input, actual request dispatch, cancellation/privacy and offline fallback, not whether
the model meets the latency budget or produces good language. `Unavailable` remains explicitly
reported; it is not successful model-candidate acceptance. See the
[Firefox/Electron and live-model audit](docs/bug-audit-browser-model-2026-09-24.md).

Add `SUZAKU_MODEL_QA_REQUIRE_CANDIDATE=1` for a seventh, strict positive workflow: the owned host
uses a 5000 ms background budget and must show a **model-only** candidate, adopt it by number,
undo, re-adopt, continue and commit exactly once. Offline fallback/`Unavailable` fails that check;
the original six safety workflows still use 1200 ms. Neither gate measures general language quality.
The edit/Escape/password/disable checks now require observable upstream socket closure within
500 ms of the boundary, triggered within 250 ms of dispatch; a completed reply or eventual host
timeout is not cancellation acceptance. This opt-in timing gate needs an observable pending call.
Deterministic loopback regressions cover quiet polling, partial/chunked responses, total deadlines,
discovery-cache recovery and input boundaries; the normal native `ibus` gate also covers eight
in-flight cancellation boundaries without using a real model. See the
[N46 cancellation audit](docs/bug-audit-model-cancellation-2026-09-24.md).

For model-side timing counters, the ignored
`languages::model::live::profile_synthetic_candidate_request` library test requires
`SUZAKU_MODEL_LOCAL_QA=1` and an explicit `SUZAKU_IME_CONFIG` under the temporary directory.
It checks local/non-remote Ollama routing, warms the installed model and prints only fixed synthetic
samples. Its 30-second diagnostic completion deadline does **not** replace the production budget
or make a slow/filtered response pass; use `suzaku_tool model probe` for that gate. The companion
`diagnose_synthetic_translation_holdouts` prints results/rejections without claiming semantic success.
Run these probes sequentially, without simultaneous inference or builds, to avoid skewing latency.

The `keyboard` gate adds 26 physical-keycode workflows in real GTK: Caps/Shift/NumLock, numeric
adoption versus keypad literals, US/UK/German/French/US-international layouts, AltGr, dead keys,
Compose and mid-draft layout/lock changes. It reads back XKB lock indicators and the keymap;
all mapping/lock changes affect only its owned Xvfb. This is not physical-device LED, native
Wayland or arbitrary-keyboard-remapping acceptance. See the
[keyboard layout audit](docs/bug-audit-keyboard-layouts-2026-09-24.md).
The `popup` gate uses stock IBus GTK3 on an 800x600 private display: 46 checks for text/ordinal
glyph clicks, paging, scrolling, dismissal and bounded long previews. It is not GNOME Shell's
candidate UI. Ordinals are rendered inside each candidate hit target to avoid stock IBus
1.5.29's separate label-column dispatch bug. All 17 ordinal cases are mandatory; the old
`SUZAKU_POPUP_QA_LABELS` opt-in is no longer needed. See the
[ordinal compatibility fix](docs/bug-audit-ordinal-click-2026-09-23.md).
The `lifecycle` gate adds 21 real GTK workflows across engine switching, declared password fields,
stock-panel resets, companion restarts and explicit host restarts/reactivation. Its Compose key
mapping only changes the owned Xvfb keyboard. It does not restart the desktop's IBus daemon or
exercise systemd service recovery. See the [input lifecycle audit](docs/bug-audit-app-lifecycle-2026-09-23.md).
The `bus-restart` gate adds 13 startup/daemon-loss checks: three languages across drafts, adopted
completions and unfinished Compose, plus English forced exits. It preserves the running GTK app
and companion, verifies literal fallback, then explicitly relaunches only its owned daemon/host/
stock panel and reactivates Suzaku. Old requests cannot replay into the replacement host; keyboard
and actual ordinal clicks commit fresh text. It does not test desktop systemd recovery or restore
unfinished text. See the [private IBus restart audit](docs/bug-audit-bus-restart-2026-09-24.md).

`bash scripts/test-linux-service.sh` additionally needs Docker. It tests the generated service
with a real systemd user manager in a disposable Ubuntu 24.04 container: legacy retry exhaustion,
long-outage recovery, fresh input, explicit stop and cancelling scheduled retries. Package
dependencies are downloaded in the container; its network is disconnected before the checks.
The runner grants SYS_ADMIN and disables AppArmor only for that container to remount its private
cgroup subtree, then drops to uid 1000 with no effective/permitted capabilities. It does not use
privileged mode, host PID/cgroup namespaces, writable host mounts, desktop sockets or devices.
Container-local packages and data are removed afterwards; no test image is built. A pre-prepared
compatible image can be selected with `SUZAKU_SERVICE_QA_IMAGE`. See the
[service recovery audit](docs/bug-audit-service-recovery-2026-09-24.md); this is not login/logout or
personal-desktop recovery acceptance.

The native `ibus` gate also tests the production engine-recovery observer on its private bus
and models asynchronous password-purpose switching with an owned panel. For a focused rerun,
use `SUZAKU_NATIVE_RECOVERY_ONLY=1 bash scripts/test-linux-ci.sh ibus`. Production observers are
disabled for custom/private host endpoints; the guarded test explicitly owns its observer.
See the [N44/N45 recovery audit](docs/bug-audit-engine-recovery-2026-09-24.md).

FFI tests share process-wide state, so keep them serial. Native runners create private D-Bus/IBus/
Xvfb sessions; never remove isolation guards or aim them at the real desktop. Regression tests
must not connect a real microphone or cloud provider.

On hybrid NVIDIA/Mesa hosts, if private Xvfb cannot create EGL surfaces, prefix the UI runner with
`__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json` when that file exists.
This is only a fixture override, not a permanent desktop setting.

Packaging validation (Docker is needed for the last step):

```bash
bash scripts/package-linux.sh --output dist
bash scripts/test-linux-package.sh dist
bash scripts/test-linux-install.sh dist
```

Use a new output directory for rebuilds; existing packages are not overwritten. Container checks
do not install on the host. All CI gates are in [.github/workflows/ci.yml](.github/workflows/ci.yml).

## Keeping local disk use bounded

Development and test profiles omit debug symbols and disable incremental compilation, matching
CI. Ordinary dependency caching and debug assertions remain enabled; release builds are unchanged.
Edits may take longer to compile, but repeated feature/profile combinations no longer retain
large incremental trees. For an interactive debugger, temporarily use
`CARGO_PROFILE_DEV_DEBUG=2` (or `CARGO_PROFILE_TEST_DEBUG=2` for tests). Return to the defaults
afterwards; `CARGO_INCREMENTAL=1` explicitly opts back into the larger incremental cache.

After stopping builds and test processes, `cargo clean --profile dev` and
`cargo clean --profile release` remove only those Rust build profiles. Prefer these scoped
commands to deleting all of `target/`: local IBus headers and extracted Qt/lint dependencies
may be stored there. Android cross-target caches, `android/app/build/` and generated
`android/app/src/main/jniLibs/` can be rebuilt when Android work resumes; do not remove shared
SDKs, signing files or `android/local.properties` as build waste.

Keep only package versions needed for distribution/rollback in `dist/`. Test runners normally
remove their isolated temporary directories; use `SUZAKU_APP_QA_KEEP=1` only while investigating
a failure, then remove that specific, inactive synthetic fixture. Never blanket-delete `/tmp`,
user settings, model files, the Git history or global Cargo/Gradle caches.

## Changes and reports

- Preserve continuous drafts: Space/punctuation and number-based candidate adoption must not submit.
- Retain source text until acknowledged; never replay uncertain writes across fields, inject into
  private/numeric targets or clear newer drafts on late replies.
- Keep model work off input/UI paths and independent of model names.
- Add synthetic regressions; include localization and small-window checks for UI changes.
- Do not commit credentials, personal settings, real typing logs, model weights or generated packages.
- Report version, OS/session, app, language and exact keys with synthetic text. Use
  [SECURITY.md](SECURITY.md) for vulnerabilities, not a public exploit report.

Contributions use the project's MIT license. Preserve dependency notices; external fonts,
dictionaries, icons and models need their source and redistribution terms.
