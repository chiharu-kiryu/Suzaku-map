# Contributing

Linux / IBus on Ubuntu 24.04 amd64 is the current target. Read the
[input rules](docs/ibus-candidates.md) and [limitations](docs/known-limitations.md) first.
Focused fixes with regression tests are especially useful during Alpha.

## Build and test

CI uses Rust 1.95.0 and the committed Cargo.lock. Ubuntu dependencies:

```bash
sudo apt install build-essential pkg-config libibus-1.0-dev libglib2.0-dev \
  libx11-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev \
  libegl1-mesa-dev libgl1-mesa-dri libvulkan1 mesa-vulkan-drivers \
  dbus-daemon ibus gir1.2-ibus-1.0 gir1.2-pango-1.0 python3-gi \
  xvfb xauth fontconfig fonts-dejavu-core fonts-noto-cjk shellcheck \
  dpkg-dev jq desktop-file-utils
cargo fmt --all --check
cargo test --locked --all-features -- --test-threads=1
bash scripts/test-linux-ci.sh ibus
bash scripts/test-linux-ci.sh ui
```

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
