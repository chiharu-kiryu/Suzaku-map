# Contributing

Linux / IBus on Ubuntu 24.04 amd64 is the current target. Read the
[input rules](docs/ibus-candidates.md) and [limitations](docs/known-limitations.md) first.
Focused fixes with regression tests are especially useful during Alpha.
Current input-quality work keeps **English and Simplified Chinese** as the core while gradually
extending basic Japanese Romaji/Kana and everyday fallback. Japanese still has no full morphological
analyzer or arbitrary long-sentence conversion. See the [development priorities](DEVELOPMENT.md).

Vocabulary is data, not decoder code. Maintain the versioned JSON resources under
[data/lexicons](data/lexicons/README.md), consumed through the platform/model-independent
[lexicon interface](src/lexicon.rs). Do not add vocabulary batches or special-case word text
to language algorithms. Keep phonetic conversion, indexing and candidate limits in the language
implementations; preserve existing entry/layer priority and update quality baselines explicitly.
Embedded resources still require rebuilding/reinstalling; there is no implicit user-file import.

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

On a memory-constrained desktop, especially after an OOM, run build, test and packaging
jobs **one at a time** with `CARGO_BUILD_JOBS=1`; `--test-threads=1` alone does not limit
compiler processes or other simultaneously launched commands. Check available memory,
swap pressure and kernel OOM timestamps first. Do not stop unrelated applications or
assume an earlier timeout/empty response was caused by a later OOM event.

With a working user systemd manager and cgroup memory controller, an isolated local
check can also have a resource ceiling (adjust it to the available headroom):

```bash
systemd-run --user --scope -p MemoryHigh=2G -p MemoryMax=4G -p MemorySwapMax=0 \
  env CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true PYTHONUNBUFFERED=1 \
  bash scripts/test-linux-ci.sh ibus
```

Offline mode requires already cached dependencies. The transient scope limits only this
command and its children, not the desktop or installed input service. A scope OOM is a
failed check, never a pass. Preserve original failures and rerun gates explicitly under
stable conditions; do not add retry-until-green loops or relax IPC/suite deadlines.

Run the paired offline input-quality gates without a model:

```bash
cargo test --locked --all-features --test english_completion_quality --test chinese_completion_quality --test offline_vocabulary_quality --test long_draft_completion -- --test-threads=1
cargo test --locked --all-features --test lexicon_resources -- --test-threads=1
cargo test --locked --all-features --test offline_packs -- --test-threads=1
cargo test --locked --all-features --test offline_pack_quality -- --test-threads=1
cargo test --locked --all-features --test fallback_vocabulary -- --test-threads=1
cargo test --locked --all-features --test daily_social_english --test daily_social_chinese -- --test-threads=1
cargo test --locked --all-features --test daily_objects_english --test daily_objects_chinese -- --test-threads=1
cargo test --locked --all-features --test daily_choices_english --test daily_choices_chinese -- --test-threads=1
cargo test --locked --all-features --test daily_japanese_vocabulary -- --test-threads=1
cargo test --locked --all-features --test daily_choices_japanese -- --test-threads=1
cargo test --locked --all-features --test japanese_sentence_progress -- --test-threads=1
cargo test --locked --all-features --test japanese_reading_boundaries -- --test-threads=1
cargo test --locked --all-features --test chinese_sentence_continuation -- --test-threads=1
cargo test --locked --all-features --test chinese_reading_boundaries -- --test-threads=1
cargo test --locked --all-features --test prediction_protocol --test ime_prediction -- --test-threads=1
cargo test --locked --all-features --test prediction_preservation --test english_prediction_spacing --test english_protected_context -- --test-threads=1
```

To run only the private English/Pinyin word-and-sentence keyboard workflows, use
`SUZAKU_NATIVE_BILINGUAL_ONLY=1 bash scripts/test-linux-ci.sh ibus` under the same
serial/resource-safe environment above. Repeat with `SUZAKU_IBUS_INLINE_PREEDIT=1`
for the alternate preedit mode. A focused pass does not replace the complete IBus gate.

Offline-pack tests use owned data directories and fresh subprocesses, covering the CLI/SDK,
schema/decoder profiles, append-only rank preservation, conflicts, limits, locks, malformed records,
and frozen startup snapshots. Run `SUZAKU_NATIVE_PACKS_ONLY=1 bash scripts/test-linux-ci.sh ibus`
for the real IBus EN/ZH/JA pack path (no desktop input or personal installation changes).
The pack-quality gate enables all eleven recommended collections in an owned registry, checking
240 authored English forms, 144 Chinese readings, 32 paired scenarios per language (four Chinese
spellings), long adopted drafts, literal spacing and synthetic provider failures. These are pack
entry counts, not net additions after merging built-ins. The native pack gate includes study/cooking/travel/work,
numeric adoption/undo and Space continuation; run it in both default and opt-in inline modes.
The optional environment override `SUZAKU_LEXICON_DIR` must be removed or set to an owned
absolute directory in fixtures; library tests do not auto-load personal packs.

The English gate covers 40 authored word/sentence scenarios plus short/long draft line boundaries,
protected tokens, committed-context truncation and literal horizontal spacing. Eight further native
workflows and two physical GTK workflows check protected English context, fresh phrases,
adoption/undo and exact commits; see the [N59 audit](docs/bug-audit-english-context-2026-09-28.md).
Chinese covers 40 known word/phrase
cases in four spelling forms (160 primary-conversion checks), plus boundary, completion, literal and
commit checks. These are project-authored regressions, not independent corpus accuracy. The native
`ibus` gate adds 96 bilingual numeric-adoption/undo/continuation/explicit-commit workflows and
the Japanese daily/prefix workflows described below, plus
64 literal-boundary workflows using owned-prefix key continuation and companion replacement.
Chinese literal padding, non-Pinyin case and line boundaries also have direct-engine regressions.
The expanded vocabulary gate covers explicit word forms, homophones and authored sentences that
must stay on page one even when a larger lexicon adds more Pinyin branches. Everyday-writing
cases exercise word/space/partial-prefix transitions, explicit English inflections and regional
spellings, straight/curly-apostrophe contractions, five Pinyin spellings, same-sound choices, and
short/long adopted drafts with literal padding. Synthetic tone digits test boundaries, not
pronunciation; numeric key entry still follows the IME's literal-digit path. Lexicon unit checks
retain every pre-expansion English rank and
validate authored table uniqueness and reachability. The fallback gate checks 68 English and
68 Chinese everyday scenarios (Chinese in four spellings), 64 standalone English word forms,
twenty unfinished Pinyin syllables, case/spacing/apostrophe variants, long drafts and synthetic
typed provider errors. Explanation/learning/follow-up, digital-life, home-life and errands cases also check
adopted-word sentence continuations with literal padding and subsequent partial English words
in uncommitted drafts.
The native gate separately exercises sixteen English/Chinese examples through actual loopback
HTTP 503 and deadline failures (32 workflows per complete IBus mode): local candidates stay
available while pending and after failure, with numeric adoption,
undo, Space continuity and exact commits. No live model quality is inferred. Use `--all-features`
for these project checks, matching CI; bare `cargo test` does not enable the required GPU/platform
modules. If a restricted agent session reports an unconfigured Rust installation, first compare
`rustup show` and `cargo --version` with the host terminal before reinstalling or changing defaults.
The tool runner itself reporting a missing executable is also distinct from a project build error;
check the session's filesystem access before changing the host Rust installation.
For Codex sessions, see the official [sandboxing documentation](https://learn.chatgpt.com/docs/sandboxing).

Unix socket fixtures need short paths. If an IDE supplies a long `TMPDIR`, use a fresh private
directory for that test process (do not overwrite the shell's persistent environment):

```bash
suzaku_test_tmp=$(mktemp -d /tmp/szqa.XXXXXX)
env TMPDIR="$suzaku_test_tmp" TMP="$suzaku_test_tmp" TEMP="$suzaku_test_tmp" \
  cargo test --locked --all-features -- --test-threads=1
rmdir -- "$suzaku_test_tmp" # Only removes the empty directory after fixture cleanup.
```

Tone-number checks cover sentence punctuation/closing quotes without stripping decimal, time,
grouped-number or fraction digits; see the [tone-boundary audit](docs/bug-audit-pinyin-tone-punctuation-2026-09-24.md).
Core multiline tests do not relax the native protocol's control-character restrictions.
English checks pair complete-word continuations before/after Space and project one actual next
word from sentence-only model replies. Complete sentences remain selectable, and URLs/identifiers
are not split. See the [next-word audit](docs/bug-audit-english-next-word-2026-09-24.md).
Twelve additional native workflows use a synthetic HTTP provider to verify exact model prefixes,
long Chinese candidates, adoption, undo and Space/Enter continuation. The standalone engine and
both provider protocols have matching [prefix-spacing checks](docs/bug-audit-model-prefix-spacing-2026-09-25.md).
Twenty additional offline native workflows check long Chinese/English tails, exact adoption/undo
and continued input, plus a 257-to-256-character recovery check. The corresponding four real-editor
scenarios and bounded search regressions are described in the
[long-Pinyin audit](docs/bug-audit-long-pinyin-continuation-2026-09-25.md).
Twenty-eight further native workflows cover adopted words retaining their authored sentences
across short/long routing, Space and punctuation deletion, with exact undo and explicit commits.
Four real-editor workflows also verify the saved text; see the
[threshold-crossing audit](docs/bug-audit-continuation-threshold-2026-09-27.md).
Stock-popup ordinal checks derive the
number of slots from the real fixture, still requiring a partial final page and clicking every
available slot; vocabulary size is not a fixed nine-candidate assertion. See the
[adopted-continuation/CI follow-up](docs/bug-audit-adopted-continuation-2026-09-27.md).

For repeated native tray activation/release checks on a private bus, run
`SUZAKU_NATIVE_ACTIVATION_ONLY=1 bash scripts/test-linux-ci.sh ibus`.
The default is 10 rounds; `SUZAKU_NATIVE_ACTIVATION_REPEATS` accepts 1–20. Failures still fail
immediately. The fixture retains real IBus command errors and exit codes for failed assertions;
its isolation/failure-propagation checks run with `python3 scripts/test-native-ibus-trace.py`.
See the [0.7.0 activation CI follow-up](docs/ci-activation-followup-2026-09-27.md).

For the custom-candidate/default and IBus-fallback display handshake, run
`SUZAKU_NATIVE_PRESENTATION_ONLY=1 bash scripts/test-linux-ci.sh ibus` (also part of the full gate).
This checks exact frame/owner validation, renewal/expiry and unchanged input/undo/model state.
The private GTK3 gate additionally observes actual lookup/auxiliary signals from the real host:
an actually rendered companion must suppress both, Hide must restore them, Show must reacquire,
and killing only the owned private companion must restore system candidates without another key.
Default and synchronous GTK3 modes also type `henqiang` / `henlihai` with the model disabled,
requiring local word/sentence choices, numeric adoption and an exact single commit.

For slow settings persistence, run
`SUZAKU_NATIVE_CONTROL_IO_ONLY=1 bash scripts/test-linux-ci.sh ibus`.
Its bounded `fsync` barrier is injected into one private, display-free host only; nine cases check
input responsiveness, no-op adoption undo, single-worker backpressure, timeout/disconnect discard,
I/O failure, external edits and password redaction. Do not preload this fixture into the desktop.
See the [settings I/O audit](docs/bug-audit-settings-io-2026-09-27.md); this controlled reproduction
does not establish the cause of earlier release-time empty replies.

The same slow-I/O gate also checks ten focus-bound keyboard language changes: ongoing typing,
timeout, reset, Escape, commit, focus transfer, privacy, pending/completed Compose and engine destruction.
`SUZAKU_NATIVE_LANGUAGE_ONLY=1 bash scripts/test-linux-ci.sh ibus` runs the focused Ctrl+Shift+Space
regression (24 profile/language/draft/lock combinations, two long Unicode drafts, plus failure and repeat boundaries); both
full IBus modes include it. The real keyboard gate adds four owned GTK workflows with long holds,
early modifier release, unchanged keymaps/LEDs, adoption and exact application commits.
A fifth workflow retains a pending dead-key sequence through physical Ctrl/Shift presses.

Real application checks additionally need GTK input modules and Qt test bindings:

```bash
sudo apt install gnome-text-editor zenity x11-utils x11-xkb-utils libxtst6 ibus-gtk3 ibus-gtk4 \
  gir1.2-gtk-3.0 gir1.2-atspi-2.0 at-spi2-core python3-pyqt5 python3-pyqt6 qt6-qpa-plugins
python3 scripts/test-editor-observer.py
python3 scripts/test-x11-focus.py
bash scripts/test-linux-apps.sh editor
bash scripts/test-linux-apps.sh gtk
bash scripts/test-linux-apps.sh gtk3
SUZAKU_GTK3_QA_SYNC_MODE=1 bash scripts/test-linux-apps.sh gtk3
bash scripts/test-linux-apps.sh vocabulary
bash scripts/test-linux-apps.sh keyboard
bash scripts/test-linux-apps.sh popup
bash scripts/test-linux-apps.sh candidates
SUZAKU_GTK3_QA_SYNC_MODE=1 bash scripts/test-linux-apps.sh candidates
## Optional tablet-style bottom layout, including real settings persistence,
## screen-key continuation after commit, orb restore, folding and page selection:
bash scripts/test-linux-apps.sh bottom-layout
SUZAKU_GTK3_QA_SYNC_MODE=1 bash scripts/test-linux-apps.sh bottom-layout
## Automatic platform choice using a private Phosh session hint, including
## manual override, restart persistence and restoring Auto through real settings:
bash scripts/test-linux-apps.sh auto-layout
SUZAKU_GTK3_QA_SYNC_MODE=1 bash scripts/test-linux-apps.sh auto-layout
bash scripts/test-linux-apps.sh lifecycle
bash scripts/test-linux-apps.sh bus-restart
bash scripts/test-linux-apps.sh qt5
bash scripts/test-linux-apps.sh qt6
SUZAKU_IBUS_INLINE_PREEDIT=1 bash scripts/test-linux-ci.sh ibus
```

For `qt5`, `qt6` or `cross`, an existing set of privately extracted Qt bindings can be used
without installing them into the system. Set `SUZAKU_QT_QA_SITE` to its absolute module
directory and, when its Python ABI differs from the system default, pair it explicitly with
an existing compatible interpreter:

```bash
env SUZAKU_QT_QA_PYTHON=/usr/bin/python3.12 \
  SUZAKU_QT_QA_SITE=/absolute/path/to/extracted/usr/lib/python3/dist-packages \
  bash scripts/test-linux-apps.sh qt5
```

Use the interpreter actually matching the chosen PyQt/SIP bindings; the path above is an
example, not a required Python version. The runner uses that same executable for the import
preflight and Qt fixture only. The GI/IBus controller remains on `/usr/bin/python3`, and
neither the system default nor installed packages are changed. Do not globally export the
extracted module directory as `PYTHONPATH`. Qt platform plugins and their shared-library
dependencies must also be available; an import pass alone is not an input-workflow pass.
Missing bindings, an incompatible interpreter or a failed fixture remain failed checks,
not skipped cases. Distinguish dependency/ABI failures from defects in Suzaku's input path.

`bash scripts/test-linux-apps.sh cross` also tests an already installed `google-chrome`, or the
executable selected by `SUZAKU_APP_QA_BROWSER`. It uses an owned temporary profile and local page,
not personal browser tabs. CI runs GTK/Qt; browser/VS Code checks below are local, not CI gates.
Application tests explicitly clear the inline-preedit opt-in to verify the default draft mode.
The `candidates` gate tests the **native panel**, not the stock `popup`: six actual drawn
cards, a partial final page, physical PageUp/PageDown, actual page buttons/wheel, numeric
adoption/undo, every English candidate clicked once into GTK, Chinese commits, and caret-only
movement near screen edges. Rendered coordinates are exposed only by the opt-in synthetic
diagnostic after GPU presentation. The fixture also rejects a stale mouse release after typing
and moving the input. It uses a private 1024×768 Xvfb display; pure geometry tests cover negative
monitor origins/scaled absolute coordinates, not real mixed-DPI/Wayland acceptance.
The `auto-layout` gate starts with no panel settings and sets `XDG_SESSION_DESKTOP=phosh`
only in the isolated child session. It exercises the production detector, real Auto/manual
settings clicks and fresh-process persistence, followed by the full bottom-layout candidate
and screen-keyboard workflows. No personal session variables or chassis files are changed;
pure detector tests cover chassis precedence, malformed metadata and desktop fallbacks.
The `gtk3` gate uses owned GTK3 TextViews and checks every physical key against the exact
host seed and application buffer. It waits for a stable, non-placeholder IBus context and
widget focus, without retyping a lost startup key. It covers repeated letters, Space,
numeric adoption/undo, explicit commits, bilingual shortcuts, field changes and the companion.
Run both the installed client's default IBus transport (unset sync override) and explicit
`SUZAKU_GTK3_QA_SYNC_MODE=1`. Failures report the requested/observed key, expected/actual draft,
buffer, widget/X11 focus and input context; deadlines remain four seconds per key. Focus
ownership accepts the exact target window or a proved descendant (not a sibling sharing
the root), while still requiring the active TextView. Bounded focus-transition and
press/release logs retain timing and ancestry; `test-x11-focus.py` checks the ancestry
boundaries without a display. These
isolated checks do not establish the cause of an earlier under-instrumented desktop failure
or replace GNOME/Wayland acceptance. The runner never changes the personal input module.
The ten GTK3 workflows include a running panel, activation from the private US keyboard
engine through the real tray menu, and a Chinese-to-English change before the original
English phrase. An owned minimal StatusNotifierWatcher permits the actual panel to register
its tray; it does not substitute a menu or engine. The driver opens that menu once and then
only observes readiness, instead of repeatedly requesting refreshes while waiting. This is
not acceptance of GNOME's tray extension, rendering or personal input-source switching.
For a separately authorized personal-desktop check, XWayland's `-enable-ei-portal` may route
XTest through desktop input authorization. A successful injection call is not proof that
the widget received a key. Stop on focus loss or missing delivery; do not disable the
portal, retry keys or count the private-display pass as desktop acceptance. Establish
delivery independently of Suzaku before attributing a synthetic-input failure to the IME.
The `vocabulary` gate exercises eighty-six offline English/Chinese/Japanese vocabulary workflows and three
multi-word Chinese sentence-progress workflows in the real
GTK editor: physical spelling, word/sentence labels, numeric adoption, exact undo, Space
continuation and saved word/sentence commits. Authored sentence continuations must remain on
page one after Space where a continuation exists. It uses no companion-seeded text or live model.
For a focused home-life diagnostic, set `SUZAKU_VOCABULARY_QA_SCOPE=home` with the `vocabulary`
runner. This explicitly reports four vocabulary cases plus three sentence-progress cases, not a
complete-gate pass. CI keeps the default `all` scope. As the corpus grows, the shell runner
executes all cases in two sequential private sessions (`part-1` / `part-2`), with the original
240-second deadline **per session**. Alternating case indices keep all three languages in each part;
the first part includes the three sentence-progress controls (46 + 43 = 89 workflows total).
Only both successful parts count as the complete gate. No case or assertion is skipped;
explicit `home`, `daily` and `packs` scopes still report only their own coverage.
For daily reactions/meeting/rest, food/weather/arrangement, communication/coordination and
social check-ins/invitations/polite declines and object location/borrowing/quantities, use
`SUZAKU_VOCABULARY_QA_SCOPE=daily`; this reports thirty vocabulary cases plus the three
sentence-progress controls (33 total).
The controls include the physical Pinyin sequence
“帮我看看 → 有没有 → 漏掉什么”, with both word and sentence adoption/undo, literal Space
continuation and an exact saved commit. The default gate includes these cases too. Independent
fixed-expectation coverage lives in `tests/daily_english_vocabulary.rs`,
`tests/daily_chinese_vocabulary.rs`, `tests/daily_needs_english.rs`,
`tests/daily_needs_chinese.rs`, `tests/daily_coordination_english.rs`,
`tests/daily_coordination_chinese.rs`, `tests/daily_social_english.rs` and
`tests/daily_social_chinese.rs`, `tests/daily_objects_english.rs` and
`tests/daily_objects_chinese.rs`; model-failure coverage
uses typed synthetic errors in `tests/fallback_vocabulary.rs`, not live provider claims.
For basic Japanese only, `SUZAKU_VOCABULARY_QA_SCOPE=japanese` exercises four GTK workflows
(including uppercase `JYUNBI`) without claiming the whole gate passed. The full gate includes them.
`SUZAKU_NATIVE_JAPANESE_ONLY=1 bash scripts/test-linux-ci.sh ibus` exercises daily word/sentence
adoption, exact undo, Space continuity, a later-page literal choice, tail homophones and adopted
Katakana continuation through private IBus; run both default and inline-preedit modes.
Independent Japanese data coverage is in `tests/daily_japanese_vocabulary.rs` and
`tests/daily_choices_japanese.rs`; the latter adds comparisons, preferences and conditions,
partial Romaji, adopted-word particles and exact word/sentence commits. Reading spaces are
still conversion separators, with the exact original text available as a literal choice.
`tests/japanese_reading_boundaries.rs` separately checks unfinished readings before trailing
horizontal separators, adopted mixed-script word boundaries, exact literal commits and
non-crossing line boundaries. The private Japanese IBus fixture also exercises actual Space
events after partial Romaji and `同じ` adoption followed by `kanji`; no physical keyboard
or personal desktop acceptance is implied.
For the optional bilingual study/cooking/travel/work packs, use
`SUZAKU_VOCABULARY_QA_SCOPE=packs bash scripts/test-linux-apps.sh vocabulary`.
This separate gate uses the selected installation's adjacent `suzaku_tool` to export/install
eight packs into the private QA data directory **before** starting its host and panel. It
checks eight pack workflows plus three builtin sentence-progress workflows (11 total), including the
live companion's system-font draft rendering and editor focus. It neither reads nor changes
the personal pack registry, and does not replace the default builtin gate.
The private IBus gate also checks sentence progress in short, threshold-crossing and long drafts,
including consecutive numeric adoptions, valid homophones, exact undo and one explicit commit.
To validate a user-local installation instead of rebuilding debug binaries, set
`SUZAKU_APP_QA_BIN_DIR="$HOME/.local/libexec/suzaku"`; the directory must contain `panel` and
`linux_ime_host` (and `suzaku_tool` for the `packs` scope). This still uses a private display/bus
and is not personal-desktop acceptance.
The native `ibus` runner has a separate installed-binary entry point:

```bash
env CARGO_BUILD_JOBS=1 SUZAKU_NATIVE_QA_BIN_DIR="$HOME/.local/libexec/suzaku" \
  bash scripts/test-linux-ci.sh ibus
```

The directory must contain executable `linux_ime_host`, `linux_ime_probe` and `suzaku_tool`;
all three are selected together, without falling back to debug binaries. The runner still builds
the source `panel` test executable for activation/recovery checks, so use the matching source
snapshot. It does **not** run the installed `panel`; that is covered separately by application
gates with `SUZAKU_APP_QA_BIN_DIR`. Repeat the native gate with
`SUZAKU_IBUS_INLINE_PREEDIT=1` for opt-in inline preedit. Neither entry point changes personal
input sources, configuration or services; private-gate results are not GNOME/Wayland acceptance.
Owned GNOME Text Editor windows enable read-only accessibility observation: saving requires the
exact application buffer and a finished load/save indicator before and after one physical Ctrl+S.
File contents alone are not a completion barrier. Missing observation, stale files or wrong text
fail the test; no input or save is retried. The fourteen display-free observer checks and a real
two-editor observation gate also run in CI. Only owned editors use disposable keyfile settings with
`auto-save-delay=300`, beyond the runner's unchanged 240-second limit: the editor's periodic draft
backup must not disable the Save action during a verification chord. Manual saving is unchanged.
The editor-only configuration also disables decorative GTK animations so each check observes save
completion without waiting for the progress indicator's fade. Other test apps and personal settings
are unchanged; this does not qualify default editor autosave timing or animation behavior.
The private IBus discovery directory is shared unchanged so running editors can reconnect after
daemon restarts. Accessibility is enabled only in the owned editor and stock-panel processes.
See the [save synchronization audit](docs/bug-audit-editor-save-2026-09-24.md).

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

The `keyboard` gate has 44 physical-keycode workflows in real GTK: Caps/Shift/NumLock, numeric
adoption versus keypad literals, US/UK/German/French/US-international layouts, AltGr, dead keys,
Compose, mid-draft layout/lock changes, bilingual adoption holds, modifier release order, quick
separate taps, repeating navigation and releasing adoption keys after switching between two
owned editor windows. It reads back XKB lock indicators and the keymap;
all mapping/lock/repeat-rate changes affect only its owned Xvfb. This is not physical-device LED, native
Wayland or arbitrary-keyboard-remapping acceptance. See the
[keyboard layout audit](docs/bug-audit-keyboard-layouts-2026-09-24.md),
[shortcut repeat audit](docs/bug-audit-shortcut-repeat-2026-09-24.md) and
[focus follow-up](docs/bug-audit-shortcut-focus-2026-09-24.md).
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
The IBus and application runners copy the Compose fixture into their owned temporary directory:
newer IBus can rewrite that file and create a backup. Never point `XCOMPOSEFILE` directly at the
source fixture or personal configuration, even for read-only input checks. The Python preflight
rejects shared paths, symlinks and hard links; `python3 scripts/test-native-ibus-trace.py` checks
that isolation along with the activation diagnostic shim.

The Python native observer also keeps borrowed IBus signal objects alive until each C dispatch
returns. IBus checks the floating reference after emitting; a short-lived PyGObject callback
can otherwise release the last wrapper too early. Drain `IBusSignalObjects` in the observer's
non-reentrant main-loop pump, including after synchronous post-processing, rather than hiding
GLib warnings or leaking a reference per event. `/usr/bin/python3 scripts/test-ibus-signal-lifetime.py`
compiles a tiny GObject-only emitter and checks delivery, post-emission liveness and prompt release
without connecting to any input bus. This regression is part of Linux CI.
GTK editor observation reuses topology, roles and states only within one read; it starts fresh
for every pre/post-save check. Preserve exact buffer, idle-progress and saved-file assertions,
the single physical save, and the original native/application deadlines when optimizing QA.

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

Version and feature changes can still leave several generations of project binaries and tests.
After stopping builds and processes using those artifacts, first preview a package-scoped cleanup:

```bash
cargo clean --locked --offline -p suzaku-map --profile dev --dry-run
cargo clean --locked --offline -p suzaku-map --profile release --dry-run
```

Inspect the previews, then repeat without `--dry-run` to remove the project's rebuildable
artifacts while retaining third-party dependency caches. The next build/test run regenerates
the removed artifacts; installed copies outside `target/` are unaffected.

If more space is needed, `cargo clean --profile dev` and `cargo clean --profile release`
also remove dependency caches for those profiles, at the cost of a fuller rebuild. Prefer
these scoped commands to deleting all of `target/`: local IBus headers and extracted Qt/lint
dependencies may be stored there. Android cross-target caches, `android/app/build/` and generated
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
