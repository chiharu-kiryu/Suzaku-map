# Suzaku Android IME Skeleton

This folder is the first Android-facing shell for Suzaku's future mobile IME path.

It currently provides:

- a Gradle Android app shell
- an `InputMethodService` entry point
- a launcher activity for bootstrap/debug status
- a candidate-strip-first layout that points toward a Gboard-style daily UI
- a Rust `cdylib` bridge target through `libsuzaku_map.so`
- an Android pre-build hook that compiles Rust `.so` outputs into `android/app/src/main/jniLibs`

Useful commands from the repo root:

- `cargo android-doctor`
- `cargo android-ime-host`
- `cargo android-build-native`
- `cd android && ./gradlew assembleDebug`
- `cargo android-install-debug`
- `cargo android-enable-ime`

Current IME id:

- `dev.suzaku.android.ime/.SuzakuInputMethodService`

What still needs a local Android toolchain:

- NDK integration for producing and packaging native `.so` outputs

Current machine status:

- Rust Android targets are installed
- Android SDK command-line tools are installed under `/opt/homebrew/share/android-commandlinetools`
- `platform-tools`, `platforms;android-35`, and `build-tools;35.0.0` are installed
- `android/local.properties` already points Gradle at that SDK root
- Homebrew `openjdk` is installed
- Homebrew `gradle` is installed, but may still need host-specific follow-up on macOS 26 native services
- Android NDK is installed and the Rust build script resolves it automatically
