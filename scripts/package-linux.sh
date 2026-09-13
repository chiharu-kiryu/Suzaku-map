#!/usr/bin/env bash
# Build a binary tarball and/or a native Debian package without installing anything.
set -euo pipefail
umask 022

suzaku_package_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
suzaku_package_format=all
suzaku_package_output="$suzaku_package_root/dist"
suzaku_package_build=1
while (($#)); do
  case "$1" in
    --format) suzaku_package_format=${2:?Missing format}; shift 2 ;;
    --output) suzaku_package_output=${2:?Missing output directory}; shift 2 ;;
    --skip-build) suzaku_package_build=0; shift ;;
    --help|-h)
      printf '%s\n' 'Usage: bash scripts/package-linux.sh [--format tar|deb|all] [--output DIR] [--skip-build]' \
        'Native Linux builds only. Never installs, starts a service, downloads a model or reads user settings.' \
        'Existing output names are not overwritten; choose a new output directory to rebuild a version.'
      exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; exit 1 ;;
  esac
done
[[ $(uname -s) == Linux ]] || { printf 'Linux is required.\n' >&2; exit 1; }
[[ $suzaku_package_format =~ ^(tar|deb|all)$ ]] || { printf 'Invalid format.\n' >&2; exit 1; }
for suzaku_package_command in cargo rustc jq tar gzip sha256sum readelf; do
  command -v "$suzaku_package_command" >/dev/null || { printf 'Missing tool: %s\n' "$suzaku_package_command" >&2; exit 1; }
done
if [[ $suzaku_package_format != tar ]]; then
  for suzaku_package_command in dpkg-deb dpkg-shlibdeps dpkg-architecture; do
    command -v "$suzaku_package_command" >/dev/null || { printf 'Install dpkg-dev: %s is missing.\n' "$suzaku_package_command" >&2; exit 1; }
  done
fi
cd -- "$suzaku_package_root"
suzaku_package_tmp=$(mktemp -d /tmp/suzaku-package.XXXXXX)
trap 'rm -r -- "$suzaku_package_tmp"' EXIT
cargo metadata --locked --all-features --format-version 1 > "$suzaku_package_tmp/metadata.json"
suzaku_package_version=$(jq -r '.packages[] | select(.name == "suzaku-map") | .version' "$suzaku_package_tmp/metadata.json")
suzaku_package_target=$(rustc -vV | sed -n 's/^host: //p')
if [[ -n ${CARGO_BUILD_TARGET:-} && $CARGO_BUILD_TARGET != "$suzaku_package_target" ]]; then
  printf 'Cross-target packaging is not supported by this script.\n' >&2; exit 1
fi
suzaku_package_target_dir=$(jq -r '.target_directory' "$suzaku_package_tmp/metadata.json")
[[ $suzaku_package_version =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][A-Za-z0-9.-]+)?$ ]] || { printf 'Invalid version.\n' >&2; exit 1; }
if [[ $suzaku_package_build == 1 ]]; then
  cargo build --locked --release --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
fi
suzaku_package_bins="$suzaku_package_target_dir/release"
if [[ -n ${CARGO_BUILD_TARGET:-} ]]; then
  suzaku_package_bins="$suzaku_package_target_dir/$suzaku_package_target/release"
fi
[[ $("$suzaku_package_bins/suzaku_tool" --version) == "suzaku-map $suzaku_package_version" ]] || {
  printf 'Built binary version differs from Cargo.toml; rebuild first.\n' >&2; exit 1;
}
suzaku_package_name="suzaku-$suzaku_package_version-$suzaku_package_target"
suzaku_package_tree="$suzaku_package_tmp/$suzaku_package_name"
mkdir -p -- "$suzaku_package_tree/bin" "$suzaku_package_tree/share/doc/suzaku/licenses" \
  "$suzaku_package_tree/share/applications" "$suzaku_package_tree/share/icons/hicolor/scalable/apps"
for suzaku_package_bin in panel linux_ime_host linux_ime_probe suzaku_tool; do
  [[ -x "$suzaku_package_bins/$suzaku_package_bin" ]] || { printf 'Missing release binary: %s\n' "$suzaku_package_bin" >&2; exit 1; }
  readelf -h "$suzaku_package_bins/$suzaku_package_bin" >/dev/null
  install -m 755 -- "$suzaku_package_bins/$suzaku_package_bin" "$suzaku_package_tree/bin/$suzaku_package_bin"
done
install -m 644 -- README.md LICENSE Cargo.lock docs/linux-packaging-data.md docs/model-providers.md docs/ibus-candidates.md docs/translation.md docs/interface-languages.md "$suzaku_package_tree/share/doc/suzaku/"
# Keep the public support/privacy docs in packages, with their repository-relative links.
install -m 644 -- SECURITY.md CONTRIBUTING.md DEVELOPMENT.md "$suzaku_package_tree/share/doc/suzaku/"
mkdir -p -- "$suzaku_package_tree/share/doc/suzaku/docs/releases"
install -m 644 -- docs/linux-packaging-data.md docs/model-providers.md docs/ibus-candidates.md \
  docs/translation.md docs/interface-languages.md docs/known-limitations.md docs/privacy.md \
  docs/functional-network.md docs/functional-network.mmd docs/bug-audit-2026-09-13.md \
  "$suzaku_package_tree/share/doc/suzaku/docs/"
install -m 644 -- "docs/releases/$suzaku_package_version.md" "$suzaku_package_tree/share/doc/suzaku/docs/releases/"
install -m 644 -- packaging/linux/dev.suzaku.Suzaku.desktop "$suzaku_package_tree/share/applications/"
install -m 644 -- src/assets/icons/suzaku-bird.svg "$suzaku_package_tree/share/icons/hicolor/scalable/apps/dev.suzaku.Suzaku.svg"
install -m 644 -- docs/linux-packaging-data.md "$suzaku_package_tree/README.md"
install -m 644 -- docs/model-providers.md "$suzaku_package_tree/model-providers.md"
install -m 644 -- docs/ibus-candidates.md "$suzaku_package_tree/ibus-candidates.md"
install -m 644 -- docs/translation.md "$suzaku_package_tree/translation.md"
install -m 644 -- docs/interface-languages.md "$suzaku_package_tree/interface-languages.md"

# Include declared licenses and the license/notice files supplied by locked Cargo dependencies.
# This inventory covers the resolved lockfile, including other-platform dependencies.
jq '[.packages[] | {name, version, license, source}]' "$suzaku_package_tmp/metadata.json" > "$suzaku_package_tree/share/doc/suzaku/dependencies.json"
while IFS=$'\t' read -r suzaku_package_crate suzaku_package_crate_version suzaku_package_manifest; do
  suzaku_package_crate_dir=$(dirname -- "$suzaku_package_manifest")
  suzaku_package_licenses="$suzaku_package_tree/share/doc/suzaku/licenses/$suzaku_package_crate-$suzaku_package_crate_version"
  mkdir -p -- "$suzaku_package_licenses"
  while IFS= read -r -d '' suzaku_package_license; do
    cp -R -- "$suzaku_package_license" "$suzaku_package_licenses/"
  done < <(find "$suzaku_package_crate_dir" -maxdepth 1 \( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' \) -print0)
done < <(jq -r '.packages[] | select(.source != null) | [.name, .version, .manifest_path] | @tsv' "$suzaku_package_tmp/metadata.json")

suzaku_package_epoch=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}
[[ $suzaku_package_epoch =~ ^[0-9]+$ ]] || { printf 'Invalid SOURCE_DATE_EPOCH.\n' >&2; exit 1; }
jq -n --arg version "$suzaku_package_version" --arg target "$suzaku_package_target" \
  --arg revision "$(git rev-parse HEAD)" --arg dirty "$(git status --porcelain --untracked-files=normal)" \
  --arg libc "$(getconf GNU_LIBC_VERSION)" --argjson epoch "$suzaku_package_epoch" \
  '{format:1, version:$version, target:$target, revision:$revision, source_dirty:($dirty != ""), build_libc:$libc, source_date_epoch:$epoch, models_bundled:false, user_data_bundled:false}' \
  > "$suzaku_package_tree/manifest.json"
(
  cd -- "$suzaku_package_tree"
  # SHA256SUMS is explicitly excluded from find's inputs.
  # shellcheck disable=SC2094
  find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
)
if [[ $suzaku_package_format != deb ]]; then
  tar --sort=name --mtime="@$suzaku_package_epoch" --owner=0 --group=0 --numeric-owner \
    -C "$suzaku_package_tmp" -cf - "$suzaku_package_name" | gzip -n > "$suzaku_package_tmp/$suzaku_package_name.tar.gz"
fi
if [[ $suzaku_package_format != tar ]]; then
  suzaku_package_arch=$(dpkg-architecture -qDEB_BUILD_ARCH)
  suzaku_package_deb="$suzaku_package_tmp/deb"
  mkdir -p -- "$suzaku_package_deb/DEBIAN" "$suzaku_package_deb/usr/lib/suzaku" "$suzaku_package_deb/usr/bin" "$suzaku_package_tmp/debian"
  cp -R -- "$suzaku_package_tree/share" "$suzaku_package_deb/usr/"
  cp -- "$suzaku_package_tree/bin/"* "$suzaku_package_deb/usr/lib/suzaku/"
  cp -- "$suzaku_package_tree/manifest.json" "$suzaku_package_deb/usr/share/doc/suzaku/manifest.json"
  ln -s ../lib/suzaku/panel "$suzaku_package_deb/usr/bin/suzaku-panel"
  ln -s ../lib/suzaku/suzaku_tool "$suzaku_package_deb/usr/bin/suzaku-tool"
  printf 'Source: suzaku\nSection: utils\nPriority: optional\nMaintainer: Suzaku contributors\n\nPackage: suzaku\nArchitecture: any\nDescription: Local multilingual input method\n' > "$suzaku_package_tmp/debian/control"
  suzaku_package_depends=$(cd -- "$suzaku_package_tmp" && dpkg-shlibdeps -O \
    -e"$suzaku_package_deb/usr/lib/suzaku/panel" -e"$suzaku_package_deb/usr/lib/suzaku/linux_ime_host" \
    -e"$suzaku_package_deb/usr/lib/suzaku/linux_ime_probe" -e"$suzaku_package_deb/usr/lib/suzaku/suzaku_tool")
  suzaku_package_depends=${suzaku_package_depends#shlibs:Depends=}
  # These are runtime-loaded libraries/tools, not all visible to dpkg-shlibdeps.
  # CJK glyph coverage is a core input feature, not an optional recommendation.
  printf 'Package: suzaku\nVersion: %s\nArchitecture: %s\nSection: utils\nPriority: optional\nMaintainer: Suzaku contributors\nInstalled-Size: %s\nDepends: %s, ibus, systemd, procps, libglib2.0-bin, fontconfig, fonts-dejavu-core, fonts-noto-cjk, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6, libx11-xcb1, libxcursor1, libxi6, libegl1, libgl1, libvulkan1\nRecommends: mesa-vulkan-drivers\nHomepage: https://github.com/chiharu-kiryu/Suzaku-map\nDescription: Local-first multilingual IBus input method and candidate panel\n English, Chinese and Japanese candidates with optional local or cloud models.\n User registration is explicit; installation never switches the active input method.\n' \
    "$suzaku_package_version" "$suzaku_package_arch" "$(du -sk "$suzaku_package_deb/usr" | cut -f1)" "$suzaku_package_depends" > "$suzaku_package_deb/DEBIAN/control"
  SOURCE_DATE_EPOCH="$suzaku_package_epoch" dpkg-deb --root-owner-group --build "$suzaku_package_deb" "$suzaku_package_tmp/suzaku_${suzaku_package_version}_${suzaku_package_arch}.deb"
fi
mkdir -p -- "$suzaku_package_output"
suzaku_package_artifacts=()
[[ $suzaku_package_format == deb ]] || suzaku_package_artifacts+=("$suzaku_package_name.tar.gz")
[[ $suzaku_package_format == tar ]] || suzaku_package_artifacts+=("suzaku_${suzaku_package_version}_${suzaku_package_arch}.deb")
for suzaku_package_artifact in "${suzaku_package_artifacts[@]}"; do
  (cd -- "$suzaku_package_tmp" && sha256sum "$suzaku_package_artifact" > "$suzaku_package_artifact.sha256")
  for suzaku_package_suffix in '' .sha256; do
    # noclobber reserves the output atomically, including across concurrent package builds.
    (set -o noclobber; : > "$suzaku_package_output/$suzaku_package_artifact$suzaku_package_suffix") || {
      printf 'Output exists: %s\n' "$suzaku_package_output/$suzaku_package_artifact$suzaku_package_suffix" >&2; exit 1;
    }
    cp -- "$suzaku_package_tmp/$suzaku_package_artifact$suzaku_package_suffix" "$suzaku_package_output/$suzaku_package_artifact$suzaku_package_suffix"
  done
  printf 'Created: %s/%s\n' "$suzaku_package_output" "$suzaku_package_artifact"
done
