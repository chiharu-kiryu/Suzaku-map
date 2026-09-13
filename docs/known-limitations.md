# Alpha limitations and support

0.5.7 targets Ubuntu 24.04 amd64, IBus and X11 / GNOME XWayland. Cross-platform architecture
does not imply cross-platform stability. Keep another input method available.

- **App compatibility:** focus/preedit behavior needs broader desktop testing. Do not rely on an
  Alpha input method as the only way to enter essential credentials.
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
- **Build scope:** use `--all-features` for desktop checks. 0.5.7 retains
  strict Clippy for all feature-enabled targets and enforces it in Linux CI. Default no-GPU
  builds/tests are still not supported as a clean release gate.
- **Draft durability:** input and recovery drafts are in memory. Crash, exit or a change of native
  field/context can discard unfinished text; Suzaku is not an autosaving editor.
- **Unconfirmed sends:** no automatic replay. Check the target before retrying. Click the panel's
  input field to recover screen-keyboard text for local editing. Candidate/clear/submit actions
  pause while that draft is unconfirmed.

Report synthetic text, exact keys, version, OS/session, app and input language. Never attach real
input history, keys or unredacted settings. See [contributing](../CONTRIBUTING.md).
