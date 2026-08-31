# Suzaku Android IME Skeleton

This folder is the first Android-facing shell for Suzaku's future mobile IME path.

It currently provides:

- a Gradle Android app shell
- an `InputMethodService` entry point
- a launcher/settings activity for enablement status, system setup, shared preferences,
  and bootstrap diagnostics
- a candidate-strip-first layout that points toward a Gboard-style daily UI
- adaptive letter, number, and symbol layouts with one-shot Shift and Caps Lock
- editor-aware Enter actions, Unicode-safe backspace, and cancellable long-press delete repeat
- direct-input email and URI shortcuts, dedicated numeric/phone/date-time pads,
  and action-aware Go/Search/Send/Next/Done keys
- a system keyboard switch key with long-press picker access and candidate-boundary
  punctuation handling
- app-provided autocomplete candidates and cursor-safe composition cancellation
- persistent settings in both the IME drawer and system-facing settings activity for
  automatic capitalization, number-row visibility, and key-press haptics
- secure password-field handling that bypasses candidates and disables voice and
  handwriting input while the field is active
- a Rust `cdylib` bridge target through `libsuzaku_map.so`
- an Android pre-build hook that compiles Rust `.so` outputs into `android/app/src/main/jniLibs`

Useful commands from the repo root:

- `cargo android-doctor`
- `cargo android-ime-host`
- `eval "$(cargo android-env)"` to export the detected SDK, NDK, and full JDK
- `cargo android-build-native`
- `cd android && ./gradlew assembleDebug`
- `cargo android-install-debug`
- `cargo android-enable-ime`

Current IME id:

- `dev.suzaku.android.ime/.SuzakuInputMethodService`

Local Android prerequisites:

- Android SDK with `platform-tools`, `platforms;android-35`, and `build-tools;35.0.0`
- Android NDK with an LLVM toolchain
- JDK 17 or newer, including the `javac` compiler
- Rust Android targets for `aarch64-linux-android`, `armv7-linux-androideabi`,
  `i686-linux-android`, and `x86_64-linux-android`
- `ANDROID_SDK_ROOT` or `ANDROID_HOME`; `JAVA_HOME` is optional when `javac` is on `PATH`

If Gradle cannot discover the SDK from the environment, create an untracked
`android/local.properties` from `android/local.properties.example` and replace
the placeholder path.

Run `cargo android-doctor` to inspect the current machine without relying on
machine-specific paths committed to the repository.
