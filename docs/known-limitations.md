# Alpha limitations and support

0.7.3 targets Ubuntu 24.04 amd64, IBus and X11 / GNOME XWayland. Cross-platform architecture
does not imply cross-platform stability. Keep another input method available.

- **In-memory candidate preferences (0.7.3):** Linux/IBus now ranks existing
  candidates using bounded, decaying feedback from confirmed choices. Preferences are scoped to
  the exact language/query/candidate, not a persistent user dictionary; restarting the host
  forgets them. Private fields bypass learning/ranking, and the tray can clear the cache.
  Other hosts retain their existing candidate policy. No installation is changed by this source
  addition. See [frequency cache](ibus-candidates.md#偏好频率缓存).

- **App compatibility:** focus/preedit behavior needs broader desktop testing. Do not rely on an
  Alpha input method as the only way to enter essential credentials.
  Isolated real GTK application checks now cover Text Editor and Zenity, with a 0.6.2 fix
  for stale synchronous IBus preedit. Zenity 4.0.1 `--entry --hide-text` does not declare a password
  purpose and can expose its synthetic test input to candidates; use a trusted alternative input
  method for credentials. This is distinct from the tested `--password` dialog. See the
  [GTK application audit](bug-audit-gtk-apps-2026-09-23.md).
  Tests included in 0.6.6 also pass 23 Firefox 156.0 and 14 VS Code 1.138.0 / Electron 42.10.0
  workflows on private X11 sessions, including focus cancellation and declared password fields.
  The Firefox executable was launched directly from its installed Snap files; Snap launcher,
  native Wayland, arbitrary Electron apps, terminal and webview editors remain unqualified.
  See the [browser/model audit](bug-audit-browser-model-2026-09-24.md).
  The 0.6.7 source-tag CI stopped at a GTK popup saved-document assertion; later package steps did
  not run. The 0.6.8 [editor-save follow-up](bug-audit-editor-save-2026-09-24.md) strengthens
  owned-buffer observation and isolates the editor's background draft saves. It does not patch
  personal applications or establish remote CI success; default editor autosave timing is not qualified.
- **Draft presentation and numeric fields:** 0.6.2 moves the default Linux draft
  preview to the candidate area/companion, preventing tested Chrome/Qt clients from confirming
  cached inline preedit on focus changes. Empty-draft digits now pass literally; digits still
  select candidates inside a draft. The advanced `SUZAKU_IBUS_INLINE_PREEDIT=1` host option
  restores inline previews, including the client's implicit-confirmation risk. Some clients
  still omit numeric/privacy purposes: literal digit routing does not make the whole field
  private or reject panel injection. Use a trusted alternative for sensitive input. See the
  [compatibility fixes](bug-audit-cross-app-fixes-2026-09-23.md). A source tag does not update an installation.
- **Stock candidate popup:** 0.6.3 bounds long auxiliary previews by display cells,
  not just code-point count. Its compatibility fix places ordinal shortcuts
  inside candidate text hit targets, avoiding IBus 1.5.29's broken separate label-column clicks.
  All 46 stock GTK3 workflows, including 17 actual ordinal-glyph checks, pass on an 800x600 private
  screen. No system files are patched; other engines using the broken column are unaffected.
  This is not arbitrary-font/DPI or GNOME Shell/Wayland acceptance, and does not update an older
  installation. See the [ordinal compatibility fix](bug-audit-ordinal-click-2026-09-23.md).
- **Dead keys and Compose:** 0.5.5 can lose accents or produce literal sequence parts.
  0.5.6 adds locale/XCompose-based composition into editable drafts, with
  isolated tests for accents, symbols, cancellation and field isolation. It needs an available
  Compose table; arbitrary layouts and custom tables are not fully validated. Only printable
  composition results of at most 255 UTF-8 bytes are supported (the system library may impose a
  smaller limit); results are never truncated or
  interpreted as application shortcuts. A sequence absent from the compiled table keeps its final
  printable key literally.
  Eight interface/translation languages do not imply eight complete native input systems.
- **Keyboard layouts and locks:** additional 0.6.6 source-tree tests cover 26 real GTK
  workflows using physical XTest keycodes: US/UK/German/French/US-international layouts, Caps,
  both Shift keys, NumLock, AltGr, keypad literals and mid-draft layout/lock changes. They verify
  XKB lock/indicator state and keymap preservation on a private Xvfb server, not actual keyboard
  LEDs, firmware, arbitrary remappings, native Wayland or all application shortcuts. No new
  product defect was reproduced. See the [keyboard audit](bug-audit-keyboard-layouts-2026-09-24.md).
- **Settings accessibility:** 0.6.9 adds visible Tab/arrow-key focus,
  Enter/Space activation, Ctrl+F search and scroll-to-focus; enlarge and separate zoom targets;
  fix zoom reset using stale state and GNOME/X11 orb clicks consumed by immediate WM dragging;
  and remove input text/debug hints from window titles.
  These fixes were installed and checked locally before this source release; the 0.6.8 tag lacks them.
  This custom-drawn UI still lacks full native accessibility/screen-reader semantics and
  human comfort/touchscreen validation. See the [installed ergonomics check](ergonomics-audit-2026-09-25.md).
- **Shortcut presets (0.6.7):** opt-in Alt home-row aliases apply only to public, nonempty
  Linux IBus drafts; direct panel editing keeps editor semantics. Defaults preserve application
  Alt keys. Desktop-reserved chords, arbitrary remapping, non-QWERTY ergonomics and native Wayland
  need separate validation. These are not global activation hotkeys. See [shortcut scope](shortcuts.md).
- **Held adoption keys (0.6.7):** N49 prevents physical number/Shift+Enter/Alt+semicolon
  repeats from chaining new candidates or losing immediate undo. Physical key identity and release
  events are required; zero-keycode synthetic events retain discrete semantics. Real GTK/Xvfb and
  private IBus checks do not qualify arbitrary remappers or native Wayland. See the
  [repeat audit](bug-audit-shortcut-repeat-2026-09-24.md).
- **Small dictionaries:** English collocations and Chinese/Japanese conversion are bounded. There
  is no complete Japanese morphological analyzer, personal learning dictionary or arbitrary
  long-sentence offline conversion. Unknown text remains available literally.
  The 0.6.9 source expands the deduplicated English index to 3,906 forms and
  Chinese readings to 602 entries, with more authored collocations and a stable Pinyin
  first-letter index. This is still a curated vocabulary, not a downloaded corpus or learned
  user history. After restoring the agent session's access to the existing Rust toolchain,
  897 Rust tests and strict Clippy pass; 27 opt-in tests are skipped by the ordinary suite.
  The complete private IBus regressions pass separately in default and inline-preedit modes.
  Expanded Pinyin branches no longer crowd authored/model sentences off the first page.
  Independent performance measurements, GUI and installation remain unverified for this expansion.
  See [vocabulary expansion and validation status](ibus-candidates.md#中英词库扩充0692026-09-27).
  0.7.3 adds everyday writing vocabulary: 5,251 indexed English
  forms, 1,143 Chinese readings, 238 English and 256 Chinese authored sentences. Earlier English
  index ranks, Chinese entry order and all input/model budgets remain unchanged. It was subsequently
  installed locally: 141 isolated installed-app checks and five current GNOME desktop workflows pass,
  including new vocabulary and draft-preserving language switching. That development installation
  predates the data refactor and preference cache; it is not full 0.7.3 installation acceptance, a
  general language-quality benchmark or arbitrary-application/Wayland acceptance; see the
  [current vocabulary scope](ibus-candidates.md#中英日常写作词库).
  The subsequent data refactor, also in 0.7.3, separates EN/ZH/JA vocabulary into versioned JSON
  resources behind a model/platform-independent data interface. Resources are still embedded:
  editing them requires rebuilding/reinstalling, not hot reload or personal-dictionary import.
  See the [data contract](../data/lexicons/README.md).
  Current input work prioritizes English and Simplified Chinese. N47 in 0.6.6 makes existing
  Chinese words match valid separated syllables (`shu ru fa` as well as `shurufa`), with fixed
  English/Chinese quality regressions. This does not provide a complete Pinyin dictionary,
  typo correction, arbitrary Chinese/English mixed conversion or a general quality score.
  See the [bilingual core audit](bug-audit-bilingual-core-2026-09-24.md).
  N48 in 0.6.7 preserves literal spacing/quotes and non-Pinyin Unicode case around converted
  text; line boundaries are no longer syllable joins in the core converter. This does not add
  native multiline input or relax companion control-character checks. See the
  [literal-boundary audit](bug-audit-chinese-literal-boundaries-2026-09-24.md).
  N50 in 0.6.8 fixes English one-word classification and model-derived next-word choices before
  a typed Space. This uses the existing known-word guard, not arbitrary grammar or vocabulary
  inference; see the [next-word audit](bug-audit-english-next-word-2026-09-24.md).
  N51 in 0.6.8 fixes optional Pinyin tone digits before sentence punctuation/closing delimiters,
  with guards for decimals, time and grouped numeric forms. It does not add tone-sensitive ranking,
  accented-vowel Pinyin, broader vocabulary or a general mixed-text parser; see the
  [tone-boundary audit](bug-audit-pinyin-tone-punctuation-2026-09-24.md).
  N52 in 0.6.8 also preserves the exact known local-conversion prefix through model parsing and
  candidate merging, including literal padding. This does not make unknown mixed text lossless,
  change Japanese offline segmentation or improve real-model semantics; see the
  [model-prefix audit](bug-audit-model-prefix-spacing-2026-09-25.md).
- **Long native drafts:** automatic predictions stop beyond 256 Unicode code points; IBus keeps
  the complete literal draft. Companion text is limited to 8192 UTF-8 bytes, so longer native
  drafts are hidden from the floating panel, not truncated or automatically committed. Native
  editing/Enter still work, and shortening the draft restores its mirror. Oversized panel
  replacements are rejected without changing the existing draft. See the
  [length-boundary audit](bug-audit-draft-limits-2026-09-23.md).
  N53 in 0.6.8 fixes an earlier internal 128-round Pinyin cutoff below that public limit;
  exact conversions after long literal prefixes now finish within the same bounded search.
  This does not increase the maximum draft length for predictions or qualify arbitrary long-sentence
  conversion; see the [long-Pinyin audit](bug-audit-long-pinyin-continuation-2026-09-25.md).
  0.6.9 adds bounded local tails for English and adopted Chinese drafts up
  to the existing 8192-byte mirror limit. The earlier prefix stays byte-exact; the model limit
  remains 256 code points. Candidate counts shrink to fit the unchanged frame budget, and
  unbroken oversized readings / protected identifiers remain literal. Chinese IBus labels expose
  shared-prefix tail differences. This is not whole-document model context or arbitrary long-Pinyin
  conversion; see the [local-window audit](bug-audit-long-draft-window-2026-09-25.md).
  The 0.7.0 N54 follow-up retains the longest authored Han phrase after long-draft adoption,
  so local sentences remain available before/after Space. It preserves literal prefixes, protected
  tokens and model limits; it is not arbitrary paragraph continuation. See the
  [adopted-continuation and CI audit](bug-audit-adopted-continuation-2026-09-27.md).
  Its N55 follow-up uses the same suffix match in the ordinary IBus mix, preventing sentence loss
  when Pinyin adoption shrinks a draft back below the local-window threshold. It does not add
  vocabulary or enlarge model context; see the [threshold audit](bug-audit-continuation-threshold-2026-09-27.md).
- **External model quality:** offline fallback needs no model, but translation needs a configured
  provider. Language quality depends on that provider/model. Not every proprietary API is compatible.
  Local `llama3.2:3b` Q4_K_M runs on CPU on this machine (`size_vram: 0`). The round-33 baseline
  exceeded even 5 seconds in all seven candidate probes. The compact requests in 0.6.6 improve the
  final 5-second-budget run to seven valid responses in 0.763–1.700 seconds, and a strict model-only
  candidate adoption/undo/continuation/commit workflow passes alongside six safety checks.
  This is not reliable default-budget acceptance: a final 1200 ms English run still times out in
  two of five cases; output and timing vary. Offline input remains independent.
  Translation now restates the task and rejects missing source question marks without changing the
  draft, but semantic omissions/changes and mixed-language output still occur in Japanese/Korean.
  Transport/script/punctuation checks are not language-quality acceptance; no automatic model
  switch or longer foreground wait was added. See the [baseline](bug-audit-browser-model-2026-09-24.md)
  and [latency/output follow-up](bug-audit-model-latency-2026-09-24.md).
- **In-flight model cancellation:** 0.6.6 local HTTP candidate requests now stop obsolete
  connected I/O cooperatively (50 ms polling), including discovery; latest drafts no longer wait
  for the old generation's full deadline. This does not speed up a single inference or guarantee
  server-side compute cancellation. Connection setup/shared discovery-lock waits remain bounded
  by the existing operations; blocking cloud HTTPS and legacy custom providers still finish or
  time out before discarding results. Translation cancellation is unchanged. See the
  [cancellation audit](bug-audit-model-cancellation-2026-09-24.md).
- **Wayland/Fcitx:** the Linux no-focus companion currently uses X11/XWayland. Native Wayland focus,
  positioning and output permissions are not fully validated. Fcitx is not a complete native backend.
- **Desktop integration:** GNOME tray visibility requires desktop support. Container install checks
  do not prove login/logout behavior or compatibility with every GNOME extension.
  The 0.6.5 N43 fix keeps the registered host retrying every two seconds across a long
  IBus outage, without exhausting systemd's start quota. A real, isolated systemd user manager
  verifies recovery and explicit-stop behavior; it does not validate personal desktop startup or
  automatically restore the chosen engine/unfinished draft. Existing service files need explicit
  re-registration with the new tool; changing source alone does not update installations. See the
  [service recovery audit](bug-audit-service-recovery-2026-09-24.md).
  The first GNOME user-install check exposed missing input sources after host recovery and a
  password-probe cleanup race. N44/N45 now add same-bus, observed-host replacement recovery and
  reset the probe's own purpose before bounded, verified restoration. A subsequent explicitly
  authorized GNOME 46/X11 installation passes TERM/KILL recovery in roughly 2.1 seconds,
  recovery during an owned composition without replay, two password probes with stable Rime
  restoration, and tray activation/release/quit/reopen. These fixes are included in 0.6.5; the
  installed build was tested before the version bump. Prefer isolated sessions for routine
  password QA; this is not blanket desktop automation permission or login/logout/Wayland qualification.
  Recovery only runs while the panel owns the registered desktop service, preserves a valid
  current engine on read-back, and drops its history when IBus itself disconnects. It does not
  inspect application text/purpose or replace the existing native privacy checks. See the
  [input-source recovery audit](bug-audit-engine-recovery-2026-09-24.md).
- **Speech/handwriting:** Linux speech currently uses a simulated bridge, not production microphone
  transcription. Handwriting recognition is limited; the input tabs do not have equal maturity.
- **Other platforms:** macOS/Windows have compile checks, not equivalent native IME acceptance.
  Android, ARM64, other distributions and older glibc are not release-qualified here.
- **Build scope:** use `--all-features` for desktop checks. 0.6.8 retains
  strict Clippy for all feature-enabled targets and enforces it in Linux CI. Default no-GPU
  builds/tests are still not supported as a clean release gate. Development/test profiles now
  omit debug symbols and incremental caches; edits may rebuild more slowly. Release settings
  are unchanged. Debugger overrides and scoped cleanup are documented in
  [contributing](../CONTRIBUTING.md#keeping-local-disk-use-bounded).
- **Draft durability:** input and recovery drafts are in memory. Crash, exit or a change of native
  field/context can discard unfinished text; Suzaku is not an autosaving editor.
  The opt-in inline mode allows applications to confirm cached preedit on focus changes;
  engine cleanup cannot undo text the application already accepted.
  The 0.6.3 lifecycle tests cover 21 real GTK switching/reconnection workflows: a companion
  restart preserves the running host's draft, whereas host exit or a stock panel selecting a
  different engine discards it. Host recovery is explicitly launched/reactivated in this test,
  not proof of systemd automatic recovery. See the [input lifecycle audit](bug-audit-app-lifecycle-2026-09-23.md).
  The 0.6.5 source-tree tests cover 13 private IBus startup/daemon-loss cases, including forced
  exits and explicit recovery with the same GTK app/companion. Discarded drafts are not replayed;
  these tests still do not validate desktop systemd restart policy, login/logout or native Wayland.
  See the [private IBus restart audit](bug-audit-bus-restart-2026-09-24.md).
- **Unconfirmed sends:** no automatic replay. Check the target before retrying. Click the panel's
  input field to recover screen-keyboard text for local editing. Candidate/clear/submit actions
  pause while that draft is unconfirmed.

Report synthetic text, exact keys, version, OS/session, app and input language. Never attach real
input history, keys or unredacted settings. See [contributing](../CONTRIBUTING.md).
