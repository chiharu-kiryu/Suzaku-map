# Development notes and history

Historical implementation notes, including earlier behavior and experimental platforms.
Start with the [README](README.md) and [known limitations](docs/known-limitations.md).

Current source version: **0.5.5 — Linux Alpha preview**.

### 0.5.5 — Draft translation, clearer text and compact settings

- Declared numeric/decimal/phone fields bypass candidate shortcuts and panel injection.
  Screen-keyboard bursts retain unsent edits behind one acknowledged write; stable letter
  presses survive candidate refreshes. Uncertain writes never automatically replay across fields.
- English word/sentence continuation shares authored collocations through spaces and partial
  words. Long previews show the differing suffix without changing payloads; Ctrl+Backspace
  deletes a whitespace-delimited English draft word.
- Linux packaging declares keyboard runtime and Latin/CJK font dependencies. Clean Ubuntu
  container checks cover install/upgrade/remove; registration checks cover special paths and safety.

- **Settings → Appearance → Interface** selects the application's display language independently
  of input and translation languages. English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese are bundled offline; **System** follows the locale environment
  with English fallback. Changes update panel/settings/tray immediately and survive restarts and
  settings backups. Search accepts translated and English labels; narrow layouts reflow longer
  labels. Drafts, candidates, model results and provider consent are unchanged. Raw system/service
  diagnostics retain their original wording. See [interface languages](docs/interface-languages.md).

- The panel's **文 / Translation** tool translates the current draft between English, Simplified
  Chinese, Japanese, Korean, Spanish, French, German and Portuguese. Choose **From / To**, then
  **Translate**; **Auto** detects the source. Read longer output with the page arrows and use
  **Use draft** to replace the editable composition, never to send it automatically.
  Translation uses the configured local/cloud provider with a separate 30-second deadline;
  cloud consent remains required. Source text, translations and navigation are session-only.
  Editing the source, changing language/context or reloading model settings discards stale results.
  See [translation behavior and limits](docs/translation.md).

- Settings are grouped into **Appearance**, **Input** and **Model** tabs. Search spans every tab;
  selecting a tab clears search and resets scrolling. Labels/options share aligned columns, the
  search field uses the available width, and the window fits the current page's content height
  while retaining scrolling on smaller screens. Category changes never modify saved IME options.

- **Settings → Appearance → Title Bar → Hide** removes the desktop window title bar from the panel and
  settings immediately; **Show** restores it. Suzaku's own drag regions and close buttons stay
  available. The choice survives restarts, settings backups and orb expansion. Existing profiles
  keep their title bars until changed; no GNOME-wide settings are modified.
- Borderless panels and settings now have transparent exteriors instead of a square pale
  backplate. Rounded cards retain their subtle shadow and unchanged controls. Both windows request
  alpha-capable native surfaces, including Linux/XWayland's inherited-alpha path; opaque-only
  drivers fall back to the theme background. Showing the system title bar restores its backdrop.
  On hybrid-GPU Linux laptops, an alpha-capable hardware adapter is preferred over an opaque-only
  adapter, with fallback if device initialization fails; software rendering is not forced for this.

- Panel and settings text now rasterizes at the glyph's physical display height, rather than
  shrinking a shared large bitmap. Pixel-aligned ink retains its native width and antialiasing;
  fractional layout advances and clipping stay intact, including at 90% zoom and higher DPI.
- A bounded, padded atlas caches multiple sizes of each character without stretching narrow
  letters or mixing neighboring glyphs. Zoom and raster eviction retain resolved fonts and stable
  layout measurements. Saved font, smoothing and zoom choices are preserved; system IBus popup
  rendering and global font settings are not changed.

### 0.5.4 Guardian themes, continuous input and service lifecycle

- Linux panel startup now starts the registered `suzaku-ibus.service` and waits for its IPC
  readiness without activating Suzaku. Activation retries a stopped host automatically. A full
  quit restores the observed previous input method before stopping the host; hide-to-tray keeps
  it running. Startup/shutdown run on the background controller, including desktops without a tray
  extension. Restore/stop failures remain visible and retryable; a disconnected host is no longer
  mislabeled as an outdated version. Custom/private endpoints remain owned by their own launcher.
- Native IBus now uses one continuous writing stream: plain **1–6** adopts the current page's
  candidate without committing, **Alt+digits** enters numbers, and Space/punctuation stay in the
  draft. Enter or a primary candidate click submits it. Shift+number symbols retain the active
  keyboard layout. English, Pinyin and Romaji share the same rules; both local and model
  predictions see the complete uncommitted draft. Panel candidate shortcuts also keep text editable.
- New default **Suzaku** theme: warm ivory surfaces, vermilion accents, soft gold details and
  quieter candidate highlights. Choose **Settings → Theme → Suzaku** for an existing profile;
  explicitly saved themes are retained. The native companion and backup format recognize `suzaku`.
- Three additional styles under **Settings → Theme**: **白虎 · 米白** (warm ivory / graphite),
  **青龙 · 青紫** (misty jade / violet), and **玄武 · 靛蓝** (deep indigo / silver blue).
  They cover the panel, candidate highlights, keyboard, handwriting, dictation, settings and
  tooltips. Saved IDs are `baihu`, `qinglong`, and `xuanwu`; settings reloads and backups retain them.
  The orb and Linux tray use matching curved tiger, dragon, and tortoise/serpent emblems. Tray
  icons update only when the theme changes, on the background worker, not on input/render frames.
- Rounded cards and the redesigned toolbar icons use resolution-independent, antialiased curves
  and strokes. The default floating orb and Linux tray share a swept-feather phoenix emblem; tray sizes
  are supersampled individually. The orb has transparent surroundings when the compositor and
  graphics surface support premultiplied alpha, with a themed background fallback otherwise.
- New profiles use the system Auto font and Smooth text. Loading settings no longer silently
  replaces Auto/Smooth with Monaco/Sharp. Text layout, centering and the caret use cached system
  glyph widths, so narrow letters are no longer stretched across a fixed character cell.
- Existing candidate actions, non-functional drag regions, close-to-tray behavior and content-fit
  resizing are retained. Desktop panel styling does not override GNOME's system IBus popup theme.
- Bug fixes: the input row preserves repeated spaces and scrolls long drafts with the caret,
  instead of wrapping over controls or eliding editable text. Its height follows text size.
  Fractional zoom keeps glyph geometry intact, Smooth/Sharp changes the active sampling filter,
  and new tooltip/feedback glyphs are measured before the first frame is displayed. Single-symbol
  controls fit their real glyph width so the zoom `+` cannot turn into an ellipsis at Large text size.
- Model reloads preserve the current IBus draft while a provider change forgets previous committed
  context. English model candidates retain typed indentation, spacing and literal list prefixes
  through parsing, ranking and commit. A missing/disconnected local model invalidates discovery
  for the next request instead of keeping a stale success entry for 60 seconds; no failed generation
  is automatically retried.
- Output fixes: consecutive native commits preserve each candidate's exact leading/trailing
  spaces. Standalone Linux panel sends use the displayed candidate, not another bridge's candidate or
  cumulative history. Only acknowledged delivery clears the editable draft and records a local
  commit; rejected/uncertain sends retain it with an explicit warning and are never retried automatically.
- IBus focus boundaries now reject late key, navigation and candidate-click events from unfocused
  engine objects. Focus-in clears the previous field's preedit, undo and prediction context even
  when focus-out is delayed. Native candidates commit only on primary clicks without application
  modifiers; physical Mod4 and virtual Super/Meta/Hyper shortcuts pass through without changing input.
- Linux configuration loading opens nonblocking and validates the opened file type before reading.
  FIFOs (including symlink targets), sockets and directories fail without freezing native input;
  failed reloads preserve the current settings and draft. Symlinks to regular JSON files still load.
  The real activation/input/release regression now checks six-row English/Chinese/Japanese
  candidates and runs in CI only under a guarded private IBus session.

### 0.5.3 Model services and IBus candidate experience

Model identity, local/cloud scope and API protocol are now independent. Local discovery prefers
an installed LLaMA through fixed loopback metadata endpoints; explicitly configured HTTPS cloud
services require opt-in consent. Credentials are read from a named environment variable, never
saved in settings/backups; restoring a backup revokes cloud consent. Legacy Llama configuration
and command aliases remain compatible. See [model provider configuration](docs/model-providers.md).

IBus now mixes alphabet/Pinyin/Romaji word completions with short phrase/sentence candidates.
There are six rows per page and at most twelve candidates. Superscripts distinguish words (`ᵂ`),
sentences/phrases (`ˢ`), literal input (`ᴿ`) and model suggestions (`ᴬᴵ`); subscripts show bounded
ranking weights, not confidence percentages. The first page reserves room for both types when
available, while preserving the literal/best-offline typing anchors. Offline collocations work
without a model; enabled local or cloud providers can add typed word and sentence suggestions.
See [IBus candidate UX and keyboard controls](docs/ibus-candidates.md).

Continuous CJK input now completes an unfinished final word without losing the converted prefix
(`woxihuanbei` → `我喜欢北京`, `watashihanihong` → `私は日本語`). Pinyin spaces preserve
syllable boundaries (`xi an` / `xi'an` → `西安`, distinct from `xian`).

Shift+Enter adopts a full candidate into editable preedit; Space continues from an explicitly
selected candidate. Immediate Backspace restores the previous spelling. This one-step undo stays
in memory and is cleared at editing, reset, language, privacy and focus boundaries. No candidate
annotation or truncated preview enters the committed text.

### 0.5.2 Linux packaging and data management

- Build binary `.tar.gz` and Debian/Ubuntu `.deb` packages with
  `bash scripts/package-linux.sh`. Packages include the four Linux programs, desktop launcher
  (for `.deb`), manifest, checksums and dependency license inventory. Installation never starts
  a service, registers an input source or downloads a model automatically.
- Tray **数据管理** provides configuration backup and folder shortcuts. `suzaku-tool data`
  supports status, backup, validation and preview-first restore. Linux restore requires stopped
  hosts/panel, saves a before-restore backup and preserves unrelated user files.
- Existing configuration locations are retained; new configuration writes are private and atomic.
  No input history, drafts, caches or external model weights are collected.

See [Linux packaging and data management](docs/linux-packaging-data.md) for installation,
upgrade/uninstall, path conventions, restore commands and compatibility limits.

### 0.5.1 Patch fixes

- Preserve literal English digits and preedit across modifier/function keys. Keep CJK numeric
  candidate selection, and use absolute candidate navigation for later-page AI probe results.
- Prevent stale candidate presses and duplicate mouse/touch completions. Synchronize Tone through
  acknowledged native settings, preserve custom temperatures, and retain decimal/version prefixes
  in Llama responses. Use a process-lifetime lock for concurrent panel startup and crash recovery.
- Keep Linux/IBus IPC reads and replies off the blocking path with main-loop socket sources.
  Incomplete requests have a one-second absolute deadline, a 65,535-byte limit, and a cap of
  16 pending clients. Oversized, invalid UTF-8 and incomplete requests cannot commit a prefix;
  legacy commits are rejected if focus changes before the complete request arrives.
- Refresh Linux panel capability/status checks in one on-demand background worker. Startup uses
  a conservative snapshot; an expired three-second cache returns its last result immediately.
  External status commands have a 350 ms deadline and 64 KiB output limit; timed-out probe
  process groups are stopped and the direct child is reaped. Diagnostic APIs remain synchronous
  with these bounds; UI input handlers never wait for a status subprocess.
- Apply one total deadline to Linux client connections, partial writes and responses: 200 ms
  for subscription setup/each read turn, 350 ms for native actions, availability checks and legacy
  text output, and 700 ms for settings. A saturated accept queue cannot block indefinitely;
  trickled responses cannot reset the budget. Timed-out operations are never replayed.
- Reuse the bounded command runner for tray IBus operations, including output collection, with
  a two-second deadline and 64 KiB output limit. Inherited stdout cannot extend the deadline;
  the unreaped direct child reserves its process-group identity until cleanup. Failed/uncertain
  switches retain their restore target. Native sync workers support cancellation on panel exit,
  and partial subscription frames yield without recursion or discarding their buffered bytes.
- Keep voice transcripts and handwriting drafts until a revision-bound native replacement is
  acknowledged. Busy, rejected, disconnected or timed-out requests retain the source for explicit
  retry, without falling back to another target. Late acknowledgements cannot clear newer drafts.
- Wait for 900 ms without a transcript change before automatic voice insertion, including backends
  that emit each update only once. Changed partials restart that interval; failed handoffs stop
  capture and never auto-retry. This is a quiet-time heuristic, not a final-utterance signal.
- Preserve leading/trailing spaces, tabs and newlines in fallback text output. Blank-only input is
  still rejected and embedded NUL bytes are still replaced by spaces.
- Remove macOS speech file logging, including transcripts and framework error descriptions.
  This prevents new logs; pre-existing logs are not removed. macOS runtime verification is pending.

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

### Using multilingual model candidates (local or cloud)

After building/installing the native host, right-click the Suzaku tray icon and select **输入语言**:

- **中文（简体拼音）**: e.g. `nihao` → `你好`, `shurufa` → `输入法`.
- **English**: literal input first, then word/phrase completions; Space keeps the writing stream editable.
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
Mouse and touch completion/rewind use one repeat guard. If a model response replaces the
candidate list during a press, releasing that old press cannot select or commit its replacement;
refreshing an unchanged list still permits the click.
On native IBus, **1–6** adopts the current page's full candidate into editable preedit in all three
languages. **Tab / Shift+Tab** navigates, and **PageUp/PageDown** changes pages. **Space** keeps
composing: it adopts an explicitly selected candidate and appends a space, or simply preserves the
literal spelling and adds a separator. **Shift+Space** remains an alias. For example,
`hel` → `2` → Space → `world!` stays one editable `hello world!` draft; only **Enter** or a candidate
left-click commits it. Punctuation does not implicitly end the writing stream.
Use **Alt+0–9** or keypad digits for literal numbers such as `v123` and `2026`; this preserves
Shift+number punctuation (`! @ #`, etc.) and the active keyboard layout. Plain number keys select
while candidates exist; empty slots (including unassigned `0/7/8/9`) do nothing. Without a draft,
digits can start literal input. Neither this mapping nor continuation modifies system lock state.
**Shift+Enter** adopts the current full candidate into preedit without adding a space or committing.
An immediate **Backspace** undoes the replacement and restores the exact previous spelling;
ordinary editing, another selection, reset, language/privacy changes and focus loss clear this
one-step, memory-only undo. AI sentence previews also expand to their full editable text.
Shift, Caps Lock and other non-text keys do not submit pending input. AltGr and system shortcuts
remain separate from candidate selection; password/PIN input still bypasses Suzaku entirely.

Enable **LLM 联想** to enrich the current composition. Offline candidates remain usable
immediately; the local first candidate never changes under Space. The model gets a structured
language ID, raw composition, local conversion, and up to 160 characters committed in the current
focused session. Context is cleared on focus loss and never written to settings. No surrounding
desktop text is collected. Local discovery is the default; cloud input requires an explicit HTTPS
endpoint, model and separate consent. Private input fields never request model candidates.
The former synchronous `IME_NEXT_TOKEN_MODEL_*` environment-variable path has been removed;
use the shared settings and LLM switch below. Panel refreshes never make their own model requests,
and Chinese/Japanese previews do not insert English next-word fillers.
Outside the native mirrored view, sending a standalone Linux panel candidate clears its draft only after
the target acknowledges delivery. Rejected or uncertain output leaves the draft and prior committed
context intact; check the target before an explicit retry, since a lost reply cannot prove non-delivery.

On Linux the model configuration is `$XDG_CONFIG_HOME/suzaku-ime/settings.json`, defaulting to
`~/.config/suzaku-ime/settings.json`. Tray changes create it automatically. Example:

```json
{
  "language": "en",
  "llm_enabled": false,
  "llm_scope": "local",
  "llm_protocol": "auto",
  "llm_endpoint": "http://127.0.0.1:11434/api/chat",
  "llm_model": "auto",
  "llm_api_key_env": null,
  "llm_cloud_consent": false,
  "llm_timeout_ms": 1200,
  "llm_temperature_tenths": 4
}
```

The provider is model-agnostic: model identity, local/cloud scope and API protocol are independent.
Default `auto` discovery prefers an installed **LLaMA** without pinning a particular release or size;
other local model families can also be selected. The native Ollama `/api/chat` integration
uses a bounded typed JSON candidate schema (up to three words and three short sentences),
a 256-token output budget, a 2,048-token context, and a five-minute idle residency.
Existing `/v1/chat/completions` configurations remain compatible, including local llama.cpp servers.
English completions and already-converted CJK phrases keep their local prefix; unrelated output,
unchanged input and pronunciation-only replacements are filtered out. Unknown Pinyin/Romaji can
still be converted by the model without being forced to keep the raw Latin prefix. These guards
do not judge semantic correctness; small-model language quality still needs broader evaluation.
English candidates preserve the exact typed prefix, including indentation, repeated spaces and
literal list markers; response cleanup only removes padding outside that prefix.
English model requests distinguish incomplete-word completion from next-word continuation, and
reject outputs that merely append words to an unfinished fragment. The literal and best offline
English completion keep their positions when AI results arrive; selecting a candidate freezes
the list until the next edit. `suzaku_tool llama probe en` exercises five fixed English examples.
Leave `llm_model` as `auto` to discover local models, or set an **already installed** model explicitly.
Start its service separately. Existing saved model names are preserved, not silently migrated to auto.
Suzaku never downloads a model during typing or starts a model server implicitly.
Choose **重新加载模型配置** after editing. `SUZAKU_IME_CONFIG` can select an isolated settings file
for development. An absent, slow or invalid model response leaves offline candidates available.
Reloading preserves the current IBus composition unless the language changes. Changing the model,
endpoint, protocol, scope or credential reference clears earlier committed context and pending
prediction results, so the new provider starts with only the current draft.
The settings window's **Tone** updates the native Linux host and its saved configuration:
Focused = 0.2, Balanced = 0.4, Expressive = 0.7. Both windows follow acknowledged values;
failed writes roll back, and late replies do not overwrite a newer selection. Reloaded custom
temperatures remain exact and appear as **Custom**, rather than being rounded to a preset.
Tone changes preserve the current composition and do not change the model or language.
The compatible API's line parser preserves decimal/version prefixes such as `3.14` and `1.2.3`.

The tray provides **发现 / 检查本机模型** and **预热本机 Ollama 模型**. Checking only reads metadata;
preheating sends an empty request, not typed text, and runs independently of input-method switching.
The configured service, missing model, timeout, malformed response, and empty candidates are
distinguishable instead of silently appearing as one generic failure. Status reports describe the
last explicit check, not continuous monitoring.

```sh
# Install/start Ollama separately first; keep it local-only (OLLAMA_NO_CLOUD=1).
cargo build --release --all-features --bin suzaku_tool
target/release/suzaku_tool model configure   # local auto; preserves language and opt-in
target/release/suzaku_tool model discover    # metadata only; no downloads or warmup
target/release/suzaku_tool model status
target/release/suzaku_tool model warmup      # Ollama only; empty request, bounded to 30 seconds
target/release/suzaku_tool model probe all   # fixed zh-Hans/en/ja examples, not desktop input
```

`model configure --model NAME --endpoint URL --timeout-ms N` updates only the supplied fields;
no-argument `configure` restores local auto discovery and clears cloud credentials/consent references.
Reload from the tray afterward. `llama` remains a command alias. `probe` preheats explicitly configured
Ollama models and reports local-conversion time, model-request time and actual
candidate text. It does not change the active input method or enable LLM input by itself.

Discovery checks only `127.0.0.1:11434` (Ollama), `127.0.0.1:8080` (llama.cpp) and
`127.0.0.1:1234` (compatible desktop servers), using `/api/tags` or `/v1/models`. A custom
endpoint is checked alone. It uses an 800 ms total metadata budget, caches success for 60 seconds
and failure for 5 seconds, and runs on the prediction worker when first needed, never the UI thread.
Local generation failures reporting a missing model/interface (404) or unavailable service discard
the matching success entry; the next request discovers again, without retrying the failed request.
Timeouts, rate limits and cloud failures do not trigger discovery.
It does not scan files/ports, load/download weights, or fall back to cloud. Ollama entries marked
remote are excluded even when an explicit local model alias is configured. Keep Ollama in local-only
mode as an additional runtime safeguard. The inventory describes the service at check time.

For HTTPS services exposing compatible chat completions, see [model configuration](docs/model-providers.md).
The panel now says **Configured service**, not a fixed Llama preset; `settings.json` is authoritative.

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
opt-in remained unchanged. The current feature-enabled regression suite passes 634 tests, with eight
opt-in tests ignored by default. All seven isolated UI/GPU checks pass separately; the live-desktop IME
activation test is intentionally not run. The local service binds loopback only with cloud use disabled;
model residency expires after five idle minutes.

The Linux blocking-path follow-up uses private IBus/Xvfb sessions and a fixed, non-recursive slow
status-command fixture. In one local run, an idle/partial IPC client increased native key handling
to about 953 ms before the fix and about 1–2 ms afterward. With a 1.2-second status-command fixture,
panel focus/edit/selection handlers returned in about 0.4–0.8 ms, including cache expiry. These are
synthetic regression observations, not latency percentiles or whole-desktop performance claims.
Transport checks also cover fragmented Unicode, exact/oversized frames, trickling clients, focus
changes before a legacy commit, abandoned replies, and bounded pending-client/descriptor cleanup.

The client/tray follow-up also reproduces a full Unix accept queue and inherited stdout without
using the desktop host. The corrected subscription/action/settings/availability calls returned at
about 200/350/700/350 ms, a trickled settings reply stopped at 700 ms, and the tray command that
previously waited three seconds stopped at its two-second limit. Isolated native-sync shutdown
checks cancelled and joined both workers in about 0.6–24 ms, including connected peers withholding
frames/acknowledgements. These timings describe the fixed fixtures, not whole-application shutdown
or general performance guarantees; IME restoration on quit remains a separate bounded operation.

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
  keyboard focus, mouse/touch candidate completion and async-refresh races, content-fit
  resizing/dragging, Tone acknowledgement/reload, voice/handwriting handoff safety, slow status
  probes, native-worker cancellation, and multilingual GPU readback using Mesa software rendering
  and installed CJK fonts.
  The job also verifies data-folder actions against a private FileManager1 fixture, builds the four
  Linux release binaries, packages `.tar.gz`/`.deb`, checks extracted contents and uploads packages
  plus checksums as 14-day CI artifacts. None of these checks installs a package on the host.
- **macOS and Windows** compile-check GPU-enabled desktop code and test targets on native runners;
  these checks do not claim native input-method or graphical runtime coverage.

The native checks use temporary settings and a deterministic local model fixture. They do not
download Llama, contact a real model, capture the desktop, or change the desktop's input method.
They check actual model-request temperatures, rejected/failed configuration writes, draft/context
boundaries on model reload, and persistence across host restart. Single-instance tests cover
concurrent launches, crash recovery and legacy running panels without opening desktop windows.
Candidate tests also cover exact consecutive commits, standalone delivery acknowledgements,
confirmation gates, and retained drafts after rejection or a lost acknowledgement.
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
target/release/linux_ime_probe --complete hel   # Tab + Space (editable), then Enter; verifies exact text
```

Some existing FFI tests share global theme state and can interfere under parallel test execution;
the serial regression command avoids that interference. This does not remove the repository-wide
strict-Clippy and default-feature limitations described above.

The isolated UI suite also checks voice/handwriting handoff acknowledgements, source retention on
failure and late replies, and Linux's consume-once transcript path with an injected clock. It uses
only synthetic text, with live voice capabilities disabled; no microphone is opened. This validates
panel/bridge integration, not production Linux speech recognition. Text-output tests use a private
Unix socket, and a cross-platform source guard prevents macOS speech logging from being reintroduced.

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
- per-session single-instance control that reopens the existing panel on a repeated launch;
  a process-lifetime OS lock serializes startup and releases on crash. The empty `panel.lock`
  file is intentionally retained after clean shutdown so contenders always lock the same file.
- Ubuntu / Arch / SteamOS capability profiles
- Linux voice backend and probe path
- native IBus `Factory`/`Engine` host backed by the shared Rust candidate engine
- IBus preedit and a cursor-anchored candidate window with six visible rows, mixed word/sentence
  ranking, and bounded annotated previews; arrow/Tab navigation, paging, 1–6 editable selection
  in all three languages, Alt+digits literal input, candidate clicks, Space continuation, and commit
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
After installation, launching `panel` ensures the registered user service is ready without
changing the selected input method. Right-click its tray icon:

- **激活 Suzaku 输入法** first starts a stopped host, then selects the native engine and hides the large panel. Focus a text field
  and type to summon the cursor-anchored candidate window; on the non-focusing Linux backend the
  companion also reappears as a collapsed candidate view. No held shortcut is required.
- **释放并恢复：…** restores the input method used just before activation. The menu shows that
  actual engine, not a guessed English/default layout. Repeated activation preserves it.
- **退出 Suzaku** (or the full quit shortcut) restores the observed previous input method, stops
  the managed host service, then exits. Restore/stop failures leave the UI available for retry.
  Hiding the panel or closing it to the tray leaves input working. A manually selected different
  engine is never overwritten on release. The release action alone keeps the host ready for reuse.

This control currently targets Linux / IBus. It does not install global shortcuts, change the
configured input-source list, or automatically take over at startup. If Suzaku was already selected
outside this tray session, switch back through the system input-source menu; there is no known
previous engine to restore. Private/custom host endpoints remain owned by their original launcher,
not by the desktop service controller. Forced process termination is not a normal tray quit and cannot run
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
bash scripts/test-linux-ci.sh ibus
```

This checks full candidate/selection equality, late subscriptions, panel commits and preedit edits,
stale actions across input contexts and host restarts, asynchronous AI updates, English/Chinese/
Japanese, privacy, Escape and focus-out. Direct engine peers also check reordered focus notifications,
late key/navigation/click events, field-context isolation and primary-click selection on both pages.
The runner builds and locates the activation test executable, verifies the test exists, and exercises
the tray controller's real activation/input/release path in all three languages, including cleanup
restoration. Special/invalid configuration reloads must leave both native keys and the host responsive.
The Xvfb keyboard regression additionally exercises the panel's native-view routing, independent draft
preservation, and cancellation of outdated presses.

You can also use `Super+Space` to select **Suzaku**, type a Latin seed, use arrows or Tab to
move through candidates, `Page Up` / `Page Down` to change pages, and `1`–`6` to adopt a candidate
without committing. Space and punctuation continue the draft; Enter or a left click submits it.
Alt+digits enters literal numbers. Escape cancels the current preedit. `linux-register uninstall`
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
- `bash scripts/test-linux-ci.sh ibus` (private activation/input/release and native regression suite)
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
