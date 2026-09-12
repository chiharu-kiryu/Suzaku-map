#!/usr/bin/env bash
# Exercise a native Debian package without giving it access to the user's desktop.
set -euo pipefail
suzaku_install_test_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
suzaku_install_test_output=$(realpath -- "${1:?Usage: test-linux-install.sh PACKAGE_DIRECTORY}")
shopt -s nullglob
suzaku_install_test_packages=("$suzaku_install_test_output/"suzaku_*.deb)
((${#suzaku_install_test_packages[@]} == 1)) || {
  printf 'Expected exactly one Suzaku Debian package.\n' >&2; exit 1;
}
suzaku_install_test_package=${suzaku_install_test_packages[0]}
[[ $(dpkg-deb --field "$suzaku_install_test_package" Package) == suzaku ]]
test -s "$suzaku_install_test_package.sha256" || { printf 'Package checksum is missing.\n' >&2; exit 1; }
suzaku_install_test_expected=$(cd -- "$suzaku_install_test_output" && sha256sum "$(basename -- "$suzaku_install_test_package")")
[[ $(<"$suzaku_install_test_package.sha256") == "$suzaku_install_test_expected" ]] || {
  printf 'Checksum does not identify the package being installed.\n' >&2; exit 1;
}
command -v docker >/dev/null || { printf 'Docker is required for the isolated install test.\n' >&2; exit 1; }
suzaku_install_test_network=${SUZAKU_PACKAGE_QA_NETWORK:-bridge}
[[ $suzaku_install_test_network == bridge || $suzaku_install_test_network == host ]] || {
  printf 'Package test network must be bridge or host.\n' >&2; exit 1;
}
suzaku_install_test_tmp=$(mktemp -d /tmp/suzaku-install-test.XXXXXX)
cleanup() {
  if [[ -f "$suzaku_install_test_tmp/container-id" ]]; then
    suzaku_install_test_id=$(<"$suzaku_install_test_tmp/container-id")
    if [[ $suzaku_install_test_id =~ ^[0-9a-f]{64}$ ]]; then
      docker rm -f "$suzaku_install_test_id" >/dev/null 2>&1 || true
    fi
  fi
  case "$suzaku_install_test_tmp" in
    /tmp/suzaku-install-test.??????) rm -r -- "$suzaku_install_test_tmp" ;;
  esac
}
trap cleanup EXIT
install -m 644 "$suzaku_install_test_root/scripts/fixtures/linux-install-container.sh" "$suzaku_install_test_tmp/install.sh"
suzaku_install_test_cache=()
if [[ -n ${SUZAKU_PACKAGE_QA_CACHE:-} ]]; then
  suzaku_install_test_cache_dir=$(realpath -- "$SUZAKU_PACKAGE_QA_CACHE")
  [[ -d $suzaku_install_test_cache_dir ]]
  suzaku_install_test_cache=(--mount "type=bind,source=$suzaku_install_test_cache_dir,target=/package-cache,readonly")
fi
# No home, session bus, device, Docker socket, source tree or writable host mount.
timeout --kill-after=10s 900s docker run --rm --init \
  --network "$suzaku_install_test_network" \
  --cidfile "$suzaku_install_test_tmp/container-id" \
  --mount "type=bind,source=$suzaku_install_test_package,target=/packages/suzaku.deb,readonly" \
  --mount "type=bind,source=$suzaku_install_test_tmp/install.sh,target=/checks/install.sh,readonly" \
  --env SUZAKU_PACKAGE_CONTAINER_QA=1 \
  --env http_proxy --env https_proxy --env no_proxy \
  "${suzaku_install_test_cache[@]}" \
  "${SUZAKU_PACKAGE_QA_IMAGE:-ubuntu:24.04}" bash /checks/install.sh
