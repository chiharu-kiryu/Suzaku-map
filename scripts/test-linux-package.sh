#!/usr/bin/env bash
set -euo pipefail
suzaku_package_test_output=$(realpath -- "${1:?Usage: test-linux-package.sh OUTPUT_DIRECTORY}")
suzaku_package_test_tmp=$(mktemp -d /tmp/suzaku-package-test.XXXXXX)
trap 'rm -r -- "$suzaku_package_test_tmp"' EXIT
shopt -s nullglob
suzaku_package_test_checksums=("$suzaku_package_test_output/"*.sha256)
suzaku_package_test_artifacts=("$suzaku_package_test_output/"*.tar.gz "$suzaku_package_test_output/"*.deb)
((${#suzaku_package_test_artifacts[@]} > 0)) || { printf 'No packages found.\n' >&2; exit 1; }
for suzaku_package_test_artifact in "${suzaku_package_test_artifacts[@]}"; do
  test -s "$suzaku_package_test_artifact.sha256" || {
    printf 'Missing package checksum: %s.sha256\n' "$suzaku_package_test_artifact" >&2; exit 1;
  }
  suzaku_package_test_expected=$(cd -- "$suzaku_package_test_output" && sha256sum "$(basename -- "$suzaku_package_test_artifact")")
  [[ $(<"$suzaku_package_test_artifact.sha256") == "$suzaku_package_test_expected" ]] || {
    printf 'Checksum does not identify the expected artifact: %s\n' "$suzaku_package_test_artifact" >&2; exit 1;
  }
done
((${#suzaku_package_test_checksums[@]} == ${#suzaku_package_test_artifacts[@]})) || {
  printf 'Unexpected checksum without a package.\n' >&2; exit 1;
}
for suzaku_package_test_tar in "$suzaku_package_test_output/"*.tar.gz; do
  suzaku_package_test_extract=$(mktemp -d "$suzaku_package_test_tmp/tar.XXXXXX")
  # This test consumes packages built by this repository, not arbitrary downloaded archives.
  tar -tzf "$suzaku_package_test_tar" | awk '/^\// || /(^|\/)\.\.(\/|$)/ {bad=1} END {exit bad}'
  tar --no-same-owner -xzf "$suzaku_package_test_tar" -C "$suzaku_package_test_extract"
  suzaku_package_test_roots=("$suzaku_package_test_extract/"*)
  ((${#suzaku_package_test_roots[@]} == 1))
  suzaku_package_test_tree=${suzaku_package_test_roots[0]}
  (cd -- "$suzaku_package_test_tree" && sha256sum --quiet -c SHA256SUMS)
  jq -e '.format == 1 and .models_bundled == false and .user_data_bundled == false' "$suzaku_package_test_tree/manifest.json" >/dev/null
  for suzaku_package_test_crate in wgpu winit ksni reqwest rustls; do
    jq -e --arg crate "$suzaku_package_test_crate" 'any(.[]; .name == $crate)' "$suzaku_package_test_tree/share/doc/suzaku/dependencies.json" >/dev/null
  done
  for suzaku_package_test_bin in panel linux_ime_host linux_ime_probe suzaku_tool; do
    test -x "$suzaku_package_test_tree/bin/$suzaku_package_test_bin"
    readelf -h "$suzaku_package_test_tree/bin/$suzaku_package_test_bin" >/dev/null
  done
  "$suzaku_package_test_tree/bin/suzaku_tool" --version
  test -s "$suzaku_package_test_tree/share/doc/suzaku/model-providers.md"
  test -s "$suzaku_package_test_tree/model-providers.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/ibus-candidates.md"
  test -s "$suzaku_package_test_tree/ibus-candidates.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/translation.md"
  test -s "$suzaku_package_test_tree/translation.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/interface-languages.md"
  test -s "$suzaku_package_test_tree/interface-languages.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/SECURITY.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/docs/known-limitations.md"
  test -s "$suzaku_package_test_tree/share/doc/suzaku/docs/privacy.md"
  "$suzaku_package_test_tree/bin/suzaku_tool" data --help
  desktop-file-validate "$suzaku_package_test_tree/share/applications/dev.suzaku.Suzaku.desktop"
done
for suzaku_package_test_deb in "$suzaku_package_test_output/"*.deb; do
  dpkg-deb --info "$suzaku_package_test_deb"
  suzaku_package_test_extract=$(mktemp -d "$suzaku_package_test_tmp/deb.XXXXXX")
  dpkg-deb --extract "$suzaku_package_test_deb" "$suzaku_package_test_extract/root"
  dpkg-deb --control "$suzaku_package_test_deb" "$suzaku_package_test_extract/control"
  for suzaku_package_test_hook in preinst postinst prerm postrm; do
    test ! -e "$suzaku_package_test_extract/control/$suzaku_package_test_hook"
  done
  test ! -e "$suzaku_package_test_extract/root/home"
  test ! -e "$suzaku_package_test_extract/root/etc"
  "$suzaku_package_test_extract/root/usr/bin/suzaku-tool" --version
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/model-providers.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/ibus-candidates.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/translation.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/interface-languages.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/SECURITY.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/docs/known-limitations.md"
  test -s "$suzaku_package_test_extract/root/usr/share/doc/suzaku/docs/privacy.md"
  "$suzaku_package_test_extract/root/usr/bin/suzaku-tool" data --help
  desktop-file-validate "$suzaku_package_test_extract/root/usr/share/applications/dev.suzaku.Suzaku.desktop"
  test -x "$suzaku_package_test_extract/root/usr/bin/suzaku-panel"
  grep -Fx 'Exec=/usr/lib/suzaku/panel' "$suzaku_package_test_extract/root/usr/share/applications/dev.suzaku.Suzaku.desktop"
  grep -Fx 'TryExec=/usr/lib/suzaku/panel' "$suzaku_package_test_extract/root/usr/share/applications/dev.suzaku.Suzaku.desktop"
done
printf 'Package contents, checksums, launchers and non-installing smoke checks passed.\n'
