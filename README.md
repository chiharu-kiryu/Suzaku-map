# Suzaku Map · 朱雀

A local-first, continuous-writing input method with word and sentence candidates,
optional language models, and a compact floating companion panel.

**0.5.5 · Linux / IBus Alpha preview.** Current validation targets **Ubuntu 24.04,
amd64, IBus, X11 or GNOME with XWayland**. Keep your usual input method available
as a fallback. Other platforms remain experimental.

朱雀是以 Linux 为核心的连续写作输入法：用字母、拼音或罗马音起笔，同时获得词与句子候选。
这是供试用和反馈的 Alpha，并非已覆盖所有桌面和应用的稳定版。

## What works today

- English-first completion, Chinese Pinyin and Japanese Romaji/Kana with small offline vocabularies.
  Word and phrase/sentence candidates appear together when available.
- Continuous drafts: spaces and punctuation keep writing open; Enter or a candidate click submits.
  Labels distinguish candidate type, model origin and ranking weight, not confidence.
- Optional local or explicitly authorized HTTPS cloud models through Ollama and compatible chat APIs.
  Local discovery prefers an already installed LLaMA; no model is bundled or automatically downloaded.
- A floating panel/orb, screen keyboard, tray activation/restoration, guardian themes, sharper text,
  compact searchable settings and optional system title bars.
- Eight interface and translation languages: English, Simplified Chinese, Japanese, Korean, Spanish,
  French, German and Portuguese. Translation needs a configured model and an explicit action;
  these are **not** eight native input dictionaries.
- Linux configuration backup and preview-first restore. Packages do not collect personal data.

Read [known limitations](docs/known-limitations.md), especially dead keys/Compose, native Wayland,
speech and application compatibility. This is not a replacement for a full Chinese/Japanese dictionary.

## Install the Linux preview

Get the matching `.deb` and `.sha256` from [Releases](https://github.com/chiharu-kiryu/Suzaku-map/releases).
In the download directory, on Ubuntu 24.04 amd64:

```bash
sha256sum -c suzaku_0.5.5_amd64.deb.sha256
sudo apt install ./suzaku_0.5.5_amd64.deb
```

Installation alone does not activate an input method or start a user service. Register once
**as your desktop user, without sudo**, then open the panel:

```bash
/usr/bin/suzaku-tool linux-register install
/usr/bin/suzaku-tool linux-register verify
/usr/bin/suzaku-panel
```

Registration adds Suzaku and enables its user host service, preserving the active input method.
Choose **Activate Suzaku** in the tray to type; **Release / restore** returns to the previously
observed input method. Full quit restores it before stopping the managed host; hiding to tray keeps
the host running. GNOME tray visibility depends on the desktop's tray support.

Finish/cancel the draft and fully quit before upgrading. To remove, quit first, run
`/usr/bin/suzaku-tool linux-register uninstall` without sudo, then `sudo apt remove suzaku`.
Settings and backups are retained. The `.tar.gz` is an alternative, not a static universal binary.
See [Linux installation and data](docs/linux-packaging-data.md) for dependencies, tarball use,
service behavior and safe restore. Checksums provide integrity, not a signature.

## Type continuously

In a normal text field, type `hel`, press **2** to adopt `hello`, then **Space** and `world!`.
The entire `hello world!` remains editable until **Enter** or a candidate click.

| Key | Action in an active draft |
| --- | --- |
| 1–6 | Adopt that candidate into the editable draft |
| Alt+0–9 / numeric keypad | Enter literal digits |
| Space / punctuation | Continue the draft, without submitting |
| Tab / Shift+Tab, arrows | Navigate candidates |
| Shift+Enter | Adopt the selected candidate without submitting |
| Immediate Backspace after completion | Restore the previous spelling |
| Ctrl+Backspace in English | Delete the last whitespace-delimited draft word |
| Enter / primary candidate click | Submit the candidate |
| Esc | Cancel the draft |

Password/PIN and explicitly declared numeric/decimal/phone fields bypass these candidate rules.
Applications must report input purpose correctly. See [IBus behavior](docs/ibus-candidates.md)
for literal input, selection, privacy and recovery after an unconfirmed send.

## Models and privacy

Suggestions are opt-in. Default discovery checks fixed loopback endpoints for models already
served locally. Suzaku does not install or start Ollama/llama.cpp for you.

```bash
/usr/bin/suzaku-tool model discover
/usr/bin/suzaku-tool model status
```

Follow the [model guide](docs/model-providers.md) to configure providers. Cloud use needs explicit
consent and may incur charges. Key values come from named environment variables, not settings/backups.
A local gateway can forward data elsewhere; trust and configure the server separately.

Enabled suggestions can send the current draft, local conversion and up to 160 characters of
committed context from the current focus session. Explicit translation sends its source draft.
Suzaku does not collect surrounding application text or write an input history to disk.
See [privacy boundaries](docs/privacy.md); third-party provider retention is outside this guarantee.

## Build and contribute

Rust 1.95.0 is the CI toolchain. Install the dependencies in [CONTRIBUTING.md](CONTRIBUTING.md), then:

```bash
cargo build --locked --release --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
cargo test --locked --all-features -- --test-threads=1
```

Native tests use private IBus/Xvfb sessions, not your active desktop. macOS/Windows CI checks
compilation, not native input acceptance. Report bugs with synthetic text and exact reproduction
keys, never real typing logs or credentials. For security concerns, read [SECURITY.md](SECURITY.md).

## Documentation

- [Input and candidates](docs/ibus-candidates.md) · [Model providers](docs/model-providers.md)
- [Translation](docs/translation.md) · [Interface languages](docs/interface-languages.md)
- [Linux installation/data](docs/linux-packaging-data.md) · [Known limitations](docs/known-limitations.md)
- [0.5.5 release notes](docs/releases/0.5.5.md) · [Development history and architecture](DEVELOPMENT.md)

MIT licensed; see [LICENSE](LICENSE). Packages include dependency license metadata and available
license/notice files. External model weights have their own licenses.
