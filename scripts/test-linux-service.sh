#!/usr/bin/env bash
# Real service-manager retries, without access to the user's service manager.
set -euo pipefail
suzaku_service_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$suzaku_service_root"
command -v docker >/dev/null
if [[ -z ${SUZAKU_SERVICE_QA_BIN_DIR:-} ]]; then
  cargo build --locked --all-features --bin suzaku_tool --bin linux_ime_host
  suzaku_service_bins="$suzaku_service_root/target/debug"
else
  suzaku_service_bins=$(realpath -- "$SUZAKU_SERVICE_QA_BIN_DIR")
fi
for suzaku_service_binary in suzaku_tool linux_ime_host; do
  test -x "$suzaku_service_bins/$suzaku_service_binary"
done
suzaku_service_tmp=$(mktemp -d /tmp/suzaku-service-qa.XXXXXX)
cleanup() {
  if [[ -f "$suzaku_service_tmp/container-id" ]]; then
    suzaku_service_id=$(<"$suzaku_service_tmp/container-id")
    if [[ $suzaku_service_id =~ ^[0-9a-f]{64}$ ]]; then
      if ! timeout --kill-after=2s 15s docker rm -f "$suzaku_service_id" >/dev/null; then
        printf 'Unable to remove owned QA container %s; retained its ID in %s.\n' \
          "$suzaku_service_id" "$suzaku_service_tmp" >&2
        return 1
      fi
    fi
  fi
  case "$suzaku_service_tmp" in
    /tmp/suzaku-service-qa.??????)
      [[ ! -L "$suzaku_service_tmp" ]] && rm -r -- "$suzaku_service_tmp"
      ;;
  esac
}
trap cleanup EXIT
# SYS_ADMIN is used only to remount this container's PRIVATE cgroup subtree.
# No privileged mode, host cgroup/PID namespace, devices, desktop bus, home,
# Docker socket or writable host mount. The manager/tests run as uid 1000.
timeout --kill-after=2s 60s docker create --init --cgroupns=private \
  --cap-add=SYS_ADMIN --security-opt=apparmor=unconfined --security-opt=no-new-privileges \
  --network=bridge --label dev.suzaku.qa=service-recovery \
  --cidfile "$suzaku_service_tmp/container-id" \
  --mount "type=bind,source=$suzaku_service_bins,target=/build,readonly" \
  --mount "type=bind,source=$suzaku_service_root/scripts,target=/checks,readonly" \
  --env SUZAKU_SERVICE_CONTAINER_QA=1 --env http_proxy --env https_proxy --env no_proxy \
  "${SUZAKU_SERVICE_QA_IMAGE:-ubuntu:24.04}" sleep 900 >/dev/null
suzaku_service_id=$(<"$suzaku_service_tmp/container-id")
[[ $suzaku_service_id =~ ^[0-9a-f]{64}$ ]]
timeout --kill-after=2s 15s docker start "$suzaku_service_id" >/dev/null
timeout --kill-after=3s 600s docker exec --env DEBIAN_FRONTEND=noninteractive \
  "$suzaku_service_id" sh -c '
  if ! command -v systemd >/dev/null || ! command -v ibus-daemon >/dev/null ||
     ! command -v dbus-daemon >/dev/null ||
     ! /usr/bin/python3 -c '\''import gi; gi.require_version("IBus", "1.0")'\'' 2>/dev/null; then
    apt-get update -qq && apt-get install --no-install-recommends -y \
      systemd dbus ibus python3-gi gir1.2-ibus-1.0
  fi'
# Downloads are finished; the actual fault/recovery checks have no network.
timeout --kill-after=2s 15s docker network disconnect bridge "$suzaku_service_id"
timeout --kill-after=3s 180s docker exec "$suzaku_service_id" \
  /usr/bin/python3 /checks/test-linux-service.py --bootstrap
