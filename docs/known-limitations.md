# Alpha limitations and support

0.6.4 targets Ubuntu 24.04 amd64, IBus and X11 / GNOME XWayland. Cross-platform architecture
does not imply cross-platform stability. Keep another input method available.

- **App compatibility:** focus/preedit behavior needs broader desktop testing. Do not rely on an
  Alpha input method as the only way to enter essential credentials.
  Isolated real GTK application checks now cover Text Editor and Zenity, with a 0.6.2 fix
  for stale synchronous IBus preedit. Zenity 4.0.1 `--entry --hide-text` does not declare a password
  purpose and can expose its synthetic test input to candidates; use a trusted alternative input
  method for credentials. This is distinct from the tested `--password` dialog. See the
  [GTK application audit](bug-audit-gtk-apps-2026-09-23.md).
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
- **Small dictionaries:** English collocations and Chinese/Japanese conversion are bounded. There
  is no complete Japanese morphological analyzer, personal learning dictionary or arbitrary
  long-sentence offline conversion. Unknown text remains available literally.
- **Long native drafts:** automatic predictions stop beyond 256 Unicode code points; IBus keeps
  the complete literal draft. Companion text is limited to 8192 UTF-8 bytes, so longer native
  drafts are hidden from the floating panel, not truncated or automatically committed. Native
  editing/Enter still work, and shortening the draft restores its mirror. Oversized panel
  replacements are rejected without changing the existing draft. See the
  [length-boundary audit](bug-audit-draft-limits-2026-09-23.md).
- **External model quality:** offline fallback needs no model, but translation needs a configured
  provider. Language quality depends on that provider/model. Not every proprietary API is compatible.
- **Wayland/Fcitx:** the Linux no-focus companion currently uses X11/XWayland. Native Wayland focus,
  positioning and output permissions are not fully validated. Fcitx is not a complete native backend.
- **Desktop integration:** GNOME tray visibility requires desktop support. Container install checks
  do not prove login/logout behavior or compatibility with every GNOME extension.
- **Speech/handwriting:** Linux speech currently uses a simulated bridge, not production microphone
  transcription. Handwriting recognition is limited; the input tabs do not have equal maturity.
- **Other platforms:** macOS/Windows have compile checks, not equivalent native IME acceptance.
  Android, ARM64, other distributions and older glibc are not release-qualified here.
- **Build scope:** use `--all-features` for desktop checks. 0.6.4 retains
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
- **Unconfirmed sends:** no automatic replay. Check the target before retrying. Click the panel's
  input field to recover screen-keyboard text for local editing. Candidate/clear/submit actions
  pause while that draft is unconfirmed.

Report synthetic text, exact keys, version, OS/session, app and input language. Never attach real
input history, keys or unredacted settings. See [contributing](../CONTRIBUTING.md).
