# Suzaku Map

Suzaku Map is a multimodal IME project.

Current release: **0.5.0**.

### 0.5.0 Highlights

- Added shared English, Chinese and Japanese input profiles, with English-first offline word and
  phrase completion, language-aware commit spacing, and persisted settings.
- Added opt-in asynchronous local Llama candidates through Ollama or compatible local servers,
  with configuration, status, warmup and probe tools; offline candidates remain available.
- Completed Linux/IBus tray activation and restoration, plus real-time native composition,
  candidate and selection synchronization with the non-focusing panel. Revision-checked panel
  actions prevent stale commits, and private fields do not expose input to the panel.
- Restored direct panel keyboard editing, including IME preedit/commit events and focus return,
  while keeping candidate clicks and background dragging non-focusing.
- Fixed multilingual glyph fallback, candidate visibility, handwriting overlap, control alignment,
  background dragging, and content-fit resizing through tab folding, zoom and compact mode.

Native input synchronization is currently implemented and exercised on Linux/IBus. The shared
language framework is portable; other native host adapters and larger CJK dictionaries remain
follow-up work. See verification and known limitations below.

### 0.5.0 IME foundation

- Shared Chinese, English and Japanese language profiles with language-specific conversion and
  commit spacing; the former English demonstration sentences have been removed.
- Immediate offline candidates plus debounced, asynchronous local-LLM enrichment in the native IBus
  host and desktop companion. Late responses cannot replace a newer composition or a selected list.
- Tray language selection, local-LLM opt-in, model configuration reload, and persisted IME settings.
- A literal fallback stays available, AI candidates are marked, and only explicit selection commits
  generated text. In native IBus input, password/PIN fields bypass the engine and private fields use
  offline conversion only; the companion commit channel also refuses password/PIN targets.

This is the first functional foundation, not full dictionary coverage. Chinese currently has a small
bootstrap Pinyin vocabulary and bounded phrase segmentation; Japanese has Romaji/Kana conversion
and a small Kanji vocabulary. Large dictionaries, learned user vocabulary, reliable long-sentence
conversion and automatic next-word suggestions after commit remain follow-up work. Language
profiles can be extended independently of the LLM provider and platform host.

### Using multilingual Llama candidates locally

After building/installing the native host, right-click the Suzaku tray icon and select **输入语言**:

- **中文（简体拼音）**: e.g. `nihao` → `你好`, `shurufa` → `输入法`.
- **English**: literal input first, then word/phrase completions; Space commits with a separating space.
- **日本語（ローマ字）**: e.g. `nihongo` → `日本語` / `にほんご` / `ニホンゴ`.

English is the default for new settings; saved Chinese/Japanese choices are retained. The panel
starts with an empty composition instead of a Chinese demo phrase. The project-authored offline
English baseline has over 900 common words, contractions and computing terms in a cached prefix
index, with explicit collocations for context-sensitive ranking. It is not a comprehensive spelling
dictionary or automatic correction system: literal input remains first and unknown input is kept.

Examples: `hel` → `hello` / `help`, `please sen` → `please send`, `good m` → `good morning`,
and `thank you ` → `thank you for` / `thank you very`. Typed case and spacing are preserved;
URLs, paths and identifiers are not split into word completions. Same-session committed words can
rank the next prefix (`good` followed by `m` prefers `morning`); this context is cleared on focus
loss and private fields do not retain it. An empty preedit does not yet open next-word candidates.

Panel word chips complete the current word or append only the immediate next word. For example,
clicking `hello` after `hel` produces `hello`, never `hel hello`; clicking `world` then gives
`hello world`. The back action reverses each exact completion. Single-word full candidates remain
visible for immediate commit; no arbitrary `is/can/will` words fill an empty live candidate list.
On native IBus, use **Tab** to select the best completion, then **Space** to commit it with one
space; **Shift+Tab** moves back. Plain Space keeps the literal unless another candidate is selected.

Enable **本地 LLM 联想** to enrich the current composition. Offline candidates remain usable
immediately; the local first candidate never changes under Space. The model gets a structured
language ID, raw composition, local conversion, and up to 160 characters committed in the current
focused session. Context is cleared on focus loss and never written to settings. No surrounding
desktop text is collected. This version accepts only loopback HTTP endpoints, not cloud services.
The former synchronous `IME_NEXT_TOKEN_MODEL_*` environment-variable path has been removed;
use the shared settings and LLM switch below. Panel refreshes never make their own model requests,
and Chinese/Japanese previews do not insert English next-word fillers.

On Linux the model configuration is `$XDG_CONFIG_HOME/suzaku-ime/settings.json`, defaulting to
`~/.config/suzaku-ime/settings.json`. Tray changes create it automatically. Example:

```json
{
  "language": "en",
  "llm_enabled": false,
  "llm_endpoint": "http://127.0.0.1:11434/api/chat",
  "llm_model": "llama3.2:3b",
  "llm_timeout_ms": 1200,
  "llm_temperature_tenths": 4
}
```

The primary local baseline is **Llama 3.2 3B through Ollama**. The native `/api/chat` integration
uses a bounded JSON candidate schema, a 2,048-token context, and a five-minute idle residency.
Existing `/v1/chat/completions` configurations remain compatible, including local llama.cpp servers.
English completions and already-converted CJK phrases keep their local prefix; unrelated output,
unchanged input and pronunciation-only replacements are filtered out. Unknown Pinyin/Romaji can
still be converted by the model without being forced to keep the raw Latin prefix. These guards
do not judge semantic correctness; small-model language quality still needs broader evaluation.
English model requests distinguish incomplete-word completion from next-word continuation, and
reject outputs that merely append words to an unfinished fragment. The literal and best offline
English completion keep their positions when AI results arrive; selecting a candidate freezes
the list until the next edit. `suzaku_tool llama probe en` exercises five fixed English examples.
Set `llm_model` to an **already installed** local model and run its local service separately.
Suzaku never downloads a model during typing or starts a model server implicitly.
Choose **重新加载模型配置** after editing. `SUZAKU_IME_CONFIG` can select an isolated settings file
for development. An absent, slow or invalid model response leaves offline candidates available.

The tray provides **检查本机 Llama 模型** and **预热 Llama**. Checking only reads model metadata;
preheating sends an empty request, not typed text, and runs independently of input-method switching.
The configured service, missing model, timeout, malformed response, and empty candidates are
distinguishable instead of silently appearing as one generic failure. Status reports describe the
last explicit check, not continuous monitoring.

```sh
# Install/start Ollama separately first; keep it local-only (OLLAMA_NO_CLOUD=1).
ollama pull llama3.2:3b
cargo build --release --all-features --bin suzaku_tool
target/release/suzaku_tool llama configure    # selects the baseline; preserves language and opt-in
target/release/suzaku_tool llama status
target/release/suzaku_tool llama warmup       # empty request, bounded to 30 seconds
target/release/suzaku_tool llama probe all    # fixed zh-Hans/en/ja examples, not desktop input
```

`llama configure --model NAME --endpoint URL --timeout-ms N` updates only the supplied fields;
no-argument `configure` selects the default Llama/Ollama model and endpoint. Reload from the tray
afterward. `probe` preheats Ollama and reports local-conversion time, model-request time and actual
candidate text. It does not change the active input method or enable LLM input by itself.

Meta's Llama 3.2 model card does not list Chinese or Japanese among officially supported languages;
these profiles retain deterministic local candidates, and model-quality evaluation is still needed.
See the [Llama 3.2 model card](https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct),
[Ollama chat API](https://docs.ollama.com/api/chat), and
[local-only/preload settings](https://docs.ollama.com/faq).

Romaji spelling conventions were checked against the upstream
[Mozc conversion table](https://github.com/google/mozc/blob/master/src/data/preedit/romanji-hiragana.tsv).
Suzaku's current converter and bootstrap vocabulary are independent, limited implementations.

### IME foundation verification

The Linux build is verified with `cargo test --all-features` and release builds using
`--all-features`. Native IBus checks cover all three languages, AI-candidate selection, private
fields, password/PIN bypass, panel commits, and activation/restoration. Model transport checks
include synthetic loopback HTTP fixtures as well as the local real-model smoke test below.

On 2026-09-08, Ollama 0.33.3 with `llama3.2:3b` (Q4_K_M, approximately 2.02 GB on disk)
was exercised on a Linux machine with an RTX 4050 Laptop GPU (6 GB VRAM). The model used a
2,048-token context and approximately 2.32 GB of VRAM. Initial empty-request loading took about
26 seconds; early generation requests hit the 1.2-second timeout and safely kept local candidates.
After warmup, one fixed `probe all` run measured 523 ms (Chinese), 665 ms (English), and 789 ms
(Japanese), with local conversion taking 14–35 microseconds. These are individual observations,
not latency percentiles, end-to-end keypress timings, or a comprehensive quality benchmark.

An isolated real IBus session also successfully selected and committed AI candidates for `nihao`,
`hello`, and `nihongo`, retained local-only candidates in private fields, bypassed password fields,
and restored its previous engine after each probe. The desktop's active `rime` engine and LLM
opt-in remained unchanged. The feature-enabled regression suite passed 576 tests (four opt-in UI
tests are ignored by default; GPU readback, isolated native-window and keyboard tests have separate opt-in commands). The local service binds loopback only with cloud use disabled;
model residency expires after five idle minutes.

The English-completion follow-up verified native Tab + Space commits for `hel`, `HEL`, `sched`,
`compati` and `don'`, alongside lossless literal commits and Chinese/Japanese privacy checks.
Five fixed English Llama examples returned useful-prefix candidates in 225–461 ms in one warmed
run. This is a transport/interaction smoke test, not a grammatical-accuracy benchmark. GPU readback
also covers English word chips and equal-width collapsed word cards without short-word truncation.

Known repository-wide follow-ups remain: strict `cargo clippy --all-features --all-targets -- -D warnings`
is not clean (including existing GPU/FFI diagnostics), and no-GPU/default-feature builds still have
UI-type/feature coupling. Use the feature-enabled commands above for this native/desktop build.

### Continuous integration

[GitHub Actions CI](https://github.com/chiharu-kiryu/Suzaku-map/actions/workflows/ci.yml)
runs on pushes to `main`, pull requests targeting `main`, and manual dispatches. It uses Rust
1.95.0 and the checked-in lockfile, with read-only repository permissions and pinned action commits.
New commits cancel superseded runs on the same branch or pull request.

- **Rust formatting** checks the entire workspace with `cargo fmt --all --check`.
- **Linux** runs the complete feature-enabled test suite serially, then exercises real IBus
  composition/candidate synchronization in a private D-Bus session. Separate Xvfb processes test
  keyboard focus, content-fit resizing/dragging, and multilingual GPU readback using Mesa software
  rendering and installed CJK fonts. The job also builds the four Linux release binaries.
- **macOS and Windows** compile-check GPU-enabled desktop code and test targets on native runners;
  these checks do not claim native input-method or graphical runtime coverage.

The native checks use temporary settings and a deterministic local model fixture. They do not
download Llama, contact a real model, capture the desktop, or change the desktop's input method.
Reproduce them on Linux after installing the packages listed in `.github/workflows/ci.yml`:

```bash
bash scripts/test-linux-ci.sh ibus
LIBGL_ALWAYS_SOFTWARE=1 bash scripts/test-linux-ci.sh ui
```

CI does not yet gate on the known strict-Clippy/default-feature issues above, package Android,
publish releases, or claim real-model quality/latency coverage. A committed workflow is not proof
of a passing run: GitHub Actions must be enabled, and reading private-repository results requires
a GitHub connection authorized for this repository.

### 0.5.0 UI refinements

- Allocate handwriting title, status, guidance, canvas and footer using full text line heights.
  Narrow guidance can wrap onto two rows without moving the canvas when status changes; Undo,
  Clear and candidate chips share a non-overlapping row. Ink is clipped to the canvas after resize.
- Drag from the header, margins and other non-interactive space in the main panel, settings or
  floating icon. Functional hit targets (including their touch padding) take priority; the seed
  text row remains editable, and dragging across a button cannot click it on release. Grab/grabbing
  cursors distinguish window movement from text editing, handwriting and scale/scroll controls.
- Fit the native panel height to the visible input mode and candidates on startup, tab fold/unfold,
  zoom and content changes. Width-driven control scaling avoids height/scale feedback; the panel
  stays top-anchored during resize and no longer has a fixed 1360px content-width cap.
- Deduplicate native resize requests and keep delayed programmatic size acknowledgements out of
  the zoom base, including a quick shrink-and-restore before the first acknowledgement arrives.
  Content fitting respects the monitor height limit; settings keep their independent window.
- Switch minimum/maximum size constraints when entering the 92×92 floating icon, then restore the
  main panel constraints and its previous decoration style when expanding again.
- Render visible Unicode glyphs on demand, with per-character system-font fallback instead of
  replacing Chinese/Japanese candidates with question marks. Repeated frames reuse glyphs and
  GPU coordinates; the atlas has a fixed memory budget and evicts stale glyphs when full.
- Use full-width CJK geometry consistently for candidate labels, wrapping, cursor measurements
  and scrolling. Collapsed CJK candidates share available width so alternate readings stay visible.
- Center keyboard, candidate-chip, and toolbar labels within their button surfaces at every text scale.
- Share one settings layout between standalone and embedded views, with measured option widths and font-aware row heights.
- Keep matching settings expanded during search, show an empty-result hint, and clip scrolling text and click targets to the content viewport.
- Draw tooltips and commit feedback above the full panel scene; delay hover hints until a 600 ms dwell, hide them during input, and keep them inside the window without idle redraw polling.

#### Font compatibility and visual verification

The panel keeps the chosen UI font and falls back per missing character. On Linux, Fontconfig
selects a language-appropriate CJK face (including the face index inside `.ttc` collections);
known Noto/WenQuanYi paths cover systems without Fontconfig. macOS and Windows have native CJK
fallback paths as well; Windows font paths respect `WINDIR`. Only Linux has been exercised on
hardware for this change. A CJK font must be installed (this machine uses Noto Sans CJK SC/JP).
After installing fonts, restart the panel. Unsupported glyphs use a distinct missing-glyph marker,
not literal `?` candidate text; the window title also reports missing coverage. Color-emoji and
complex-script shaping are not provided by this single-glyph renderer.

The opt-in visual test renders synthetic Chinese, English and Japanese candidates using the actual
panel shaders, font atlas and GPU readback, including 2.5× scaling, collapsed/expanded layouts,
bounded-cache eviction and repeated-frame reuse. Handwriting scenes additionally cover 420–2500px
widths at medium/large text sizes. It does not capture the desktop or read typed text:

```bash
cargo test --all-features --offline --bin panel \
  multilingual_candidates_render_through_the_gpu_without_question_mark_fallback -- --ignored --nocapture
# Optional: set SUZAKU_GLYPH_QA_DIR to an output directory to retain synthetic BMP images.
cargo test --all-features --offline -- --test-threads=1
# In an isolated test IBus session with English selected:
target/release/linux_ime_probe --complete hel   # Tab + Space, validates exact selected text + space
```

Some existing FFI tests share global theme state and can interfere under parallel test execution;
the serial regression command avoids that interference. This does not remove the repository-wide
strict-Clippy and default-feature limitations described above.

The Linux native-window regression uses an isolated Xvfb display and temporary settings. It tests
keyboard/voice/handwriting folding, zoom and restore through real window-size events. It also checks
mouse/touch drag routing, control priority, cancellation, releases over other controls, compact
background clicks, and actual X11 window movement. It does not capture the desktop, switch its input
method or inject global input events. The fixed English example goes from 900×487 expanded to
900×165 folded and back without zoom-base drift:

```bash
suzaku_fit_qa=$(mktemp -d /tmp/suzaku-fit-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_fit_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_fit_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_window_fits_content_through_fold_zoom_and_restore -- --ignored --nocapture
```

Geometry tests also cover empty/non-empty English/CJK candidates, three input modes and scaled
viewports; GPU readback uses the fitted height rather than a fixed large canvas. Only Linux native
window behavior has been exercised on this machine.

The keyboard regression sends X11 key events only to windows it creates on an isolated Xvfb
display. It checks actual focus acquisition after clicking the input field, text/digits/spaces on
all three input tabs, IME preedit/commit routing, focus return, and self-target commit protection:

```bash
suzaku_key_qa=$(mktemp -d /tmp/suzaku-keyboard-qa.XXXXXX)
env -u WAYLAND_DISPLAY XDG_CONFIG_HOME="$suzaku_key_qa/config" \
  SUZAKU_IME_CONFIG="$suzaku_key_qa/ime.json" SUZAKU_PANEL_NATIVE_QA=1 \
  SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus \
  xvfb-run -a -s '-screen 0 1920x1080x24 -nolisten tcp' \
  cargo test --all-features --offline --bin panel \
  native_keyboard_editing_and_focus_return -- --ignored --nocapture
```

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
panel, or use its menu to activate/release the input method, open settings, reset the window
position, and quit the process. Activating Suzaku hides the large companion and remembers the
previous IBus engine; native candidates appear only while composing in the target app. Releasing
or normally quitting restores the previous engine, unless the user has already switched to another
input method. A failed restore keeps the tray open with a retryable error. The
settings window is placed above the main panel and clamped to the active monitor. Tray-state updates
run off the UI thread. Launching the panel again signals the existing process to show its window
rather than creating duplicate windows or tray icons. Set
`SUZAKU_LINUX_PANEL_BACKEND=wayland` to force the native Wayland window path.
Cross-platform adapters remain in the tree, but local desktop behavior is validated first.

### GPU smoke test profile

The GPU smoke suite includes deterministic loopback HTTP fixtures. No downloaded model is required.

```bash
# Local validation (default)
cargo test-gpu-smoke
./scripts/test-gpu-smoke.sh
```

### Integration validation (local-loopback llama bridge required)

```bash
cargo test-gpu-smoke-llm
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
cargo test-gpu-smoke-llm       # includes a deterministic local HTTP fixture, not a live model
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
- explicit click-to-type focus for the panel input field, independent of keyboard/voice/handwriting
  tabs; regular text, digits, repeat keys and single spaces take priority over panel commands
- OS IME preedit/commit input for Chinese, Japanese and accented text; preedit stays a display-only
  preview until committed. The event lifecycle follows the [winit IME contract](https://docs.rs/winit/0.30.12/winit/event/enum.Ime.html).
- Enter / Esc finishes panel editing and restores its borrowed X11 focus if the user has not
  already switched apps; show, drag and candidate clicks never request editing focus. On native
  Wayland and other platforms, focus activation/restoration remains subject to the OS/window manager.
  Before sending a candidate, focus the target app; Linux rejects commits into the panel itself.
- StatusNotifierItem system tray with a built-in Suzaku icon, show/hide activation, position reset,
  direct settings access, close-to-tray behavior, and an explicit quit action
- per-session single-instance control that reopens the existing panel on a repeated launch
- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- native IBus `Factory`/`Engine` host backed by the shared Rust candidate engine
- IBus preedit and a cursor-anchored minimal candidate window with three visible rows and bounded
  label previews; arrow/Tab navigation, paging, `1`–`3` selection, candidate clicks, and commit
- tray-controlled IBus activation/restoration with verified switches, bounded off-thread host I/O,
  on-menu-open status refresh, and no global keyboard hook or input polling
- native roundtrip probe coverage for input-triggered preedit/candidates, final commit, and dismissal
- local user-only panel-to-IBus commit channel at `$XDG_RUNTIME_DIR/suzaku-ime/host.sock`
- live native-to-panel composition subscription: preedit, exact candidate labels/text, selection,
  language and asynchronous AI updates come from the native host rather than a second LLM request
- revision-checked panel candidate commits and word/on-screen completion edits; stale clicks, old
  host instances and private contexts are rejected without retrying or falling back to a new target
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
`~/.config/systemd/user/suzaku-ibus.service`, dynamically registers
`dev.suzaku.linux.ime` with the running IBus daemon, and safely appends Suzaku to GNOME's input
source switcher when that setting is available. During a host upgrade it records and restores
the active IBus engine, so restarting the service does not leave the desktop on an empty engine.
After installation, launch `panel` and right-click its tray icon:

- **激活 Suzaku 输入法** selects the native engine and hides the large panel. Focus a text field
  and type to summon the cursor-anchored candidate window; on the non-focusing Linux backend the
  companion also reappears as a collapsed candidate view. No held shortcut is required.
- **释放并恢复：…** restores the input method used just before activation. The menu shows that
  actual engine, not a guessed English/default layout. Repeated activation preserves it.
- **退出 Suzaku** releases a tray-owned activation before exiting. Hiding the panel alone leaves
  the input method enabled. A manually selected different engine is never overwritten on release.

This control currently targets Linux / IBus. It does not install global shortcuts, change the
configured input-source list, or automatically take over at startup. If Suzaku was already selected
outside this tray session, switch back through the system input-source menu; there is no known
previous engine to restore. Forced process termination is not a normal tray quit and cannot run
the session's restore step.

Native companion synchronization uses a persistent `W` subscription on the existing user-only
host socket. Frames carry a host-instance UUID, focus-context ID and monotonic revision. The host
pushes on input, selection, prediction, commit/cancel and focus/privacy changes; the panel queues
only the newest frame and does not run its own predictor while mirroring. Slow readers are
disconnected using nonblocking writes, and a restarted host supplies a fresh snapshot on reconnect.
The settings/status (`S`) channel remains text-free.

Clicking a mirrored candidate submits that exact native candidate through a revision-checked `A`
action; word chips and the on-screen keyboard update native preedit through the same channel.
There is no automatic retry or fallback commit when focus/content/selection changes. A candidate
press is cancelled if its displayed revision changes before release. Directly clicking the seed
input returns to the independent panel draft and the normal click-to-type behavior.

Password/PIN and application-marked private contexts send empty frames, never text or candidates;
surrounding text and committed history are never included. Privacy detection depends on the client
reporting its IBus content purpose/hints. Automatically summoned views hide after completion or
focus loss; an explicitly shown panel stays open but clears native text. Manually hiding a native
view suppresses automatic reopening for that focus context. This transport is implemented for
Linux/IBus; other platform hosts still need their own native subscription adapters.

The real native sync test runs its own D-Bus and IBus daemon, creates synthetic input contexts,
and uses a deterministic loopback model fixture (not the user's model/settings or desktop):

```bash
cargo build --all-features --offline --bin linux_ime_host
suzaku_sync_qa=$(mktemp -d /tmp/suzaku-sync-qa.XXXXXX)
env -u DISPLAY -u WAYLAND_DISPLAY dbus-run-session -- env \
  XDG_RUNTIME_DIR="$suzaku_sync_qa" XDG_CONFIG_HOME="$suzaku_sync_qa/config" \
  XDG_DATA_HOME="$suzaku_sync_qa/data" GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
  SUZAKU_IME_CONFIG="$suzaku_sync_qa/ime.json" \
  IBUS_ADDRESS="unix:path=$suzaku_sync_qa/ibus.sock" SUZAKU_NATIVE_SYNC_QA=1 \
  /usr/bin/python3 scripts/test-native-sync.py
```

This checks full candidate/selection equality, late subscriptions, panel commits and preedit edits,
stale actions across input contexts and host restarts, asynchronous AI updates, English/Chinese/
Japanese, privacy, Escape and focus-out. The Xvfb keyboard regression additionally exercises the
panel's native-view routing, independent draft preservation, and cancellation of outdated presses.

You can also use `Super+Space` to select **Suzaku**, type a Latin seed, use arrows or Tab to
move through candidates, `Page Up` / `Page Down` to change pages, `1`–`3` or a mouse click to choose,
and Space or Enter to commit. Escape cancels the current preedit. `linux-register uninstall`
removes Suzaku from the GNOME switcher, disables the user service, and removes the installed host
and component metadata.

Useful commands:

- `cargo linux-register install`
- `cargo linux-register status`
- `cargo linux-register uninstall`
- `cargo linux-register verify`
- `cargo linux-register diag`
- `cargo run --features linux-ibus --bin linux_ime_probe -- ni`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --ipc "panel commit probe"`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --llm nihao`
  (requires an enabled, running local model; waits for an AI candidate and selects it)
- `cargo run --features linux-ibus --bin linux_ime_probe -- --private nihao`
- `cargo run --features linux-ibus --bin linux_ime_probe -- --password nihao`
- `cargo test --features linux-ibus --bin panel native_activation_input_and_release_roundtrip -- --ignored --test-threads=1`
  (opt-in live-desktop check: activates, types in an isolated input context, then restores)

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
