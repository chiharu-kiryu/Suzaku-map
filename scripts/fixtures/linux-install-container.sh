#!/usr/bin/env bash
# Never invoke this fixture against the host root filesystem.
set -euo pipefail
[[ ${SUZAKU_PACKAGE_CONTAINER_QA:-} == 1 && -f /.dockerenv && $(id -u) == 0 ]] || {
  printf 'This fixture requires the dedicated disposable install-test container.\n' >&2; exit 1;
}
if [[ ${1:-} == --panel-smoke ]]; then
  # xwininfo cannot decode a Unicode window title in the minimal image's C locale.
  export LC_ALL=C.UTF-8
  suzaku_install_gui=$(mktemp -d /tmp/suzaku-installed-panel.XXXXXX)
  export XDG_RUNTIME_DIR="$suzaku_install_gui" XDG_CONFIG_HOME="$suzaku_install_gui/config" XDG_DATA_HOME="$suzaku_install_gui/data"
  export GSETTINGS_BACKEND=memory GIO_USE_VFS=local LIBGL_ALWAYS_SOFTWARE=1
  export SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus SUZAKU_PANEL_SURFACE_DIAGNOSTICS=1
  mkdir -p "$XDG_CONFIG_HOME/suzaku-panel"
  printf '%s\n' 'ui_language=zh-Hans' > "$XDG_CONFIG_HOME/suzaku-panel/panel-settings.toml"
  /usr/bin/suzaku-panel > "$suzaku_install_gui/panel.log" 2>&1 &
  suzaku_install_panel_pid=$!
  # shellcheck disable=SC2317 # Invoked by the EXIT trap in this conditional branch.
  cleanup_panel() {
    kill "$suzaku_install_panel_pid" 2>/dev/null || true
    wait "$suzaku_install_panel_pid" 2>/dev/null || true
  }
  trap cleanup_panel EXIT
  suzaku_install_rendered=0
  for ((suzaku_install_attempt=0; suzaku_install_attempt<100; suzaku_install_attempt++)); do
    kill -0 "$suzaku_install_panel_pid" 2>/dev/null || break
    suzaku_install_windows=$(xwininfo -root -tree)
    if [[ $suzaku_install_windows == *'Suzaku XR Candidate Panel'*'font: system-atlas'*'Noto Sans CJK'* &&
          $suzaku_install_windows != *'glyphs unavailable'* ]]; then
      suzaku_install_rendered=1
      break
    fi
    sleep 0.1
  done
  if [[ $suzaku_install_rendered != 1 ]]; then
    sed -n '1,120p' "$suzaku_install_gui/panel.log" >&2
    xwininfo -root -tree >&2 || true
    fc-match -f '%{file}\n%{index}\n' 'sans-serif:lang=zh-cn' >&2
    printf 'Installed panel did not reach a frame with real fonts.\n' >&2; exit 1;
  fi
  sleep 1
  kill -0 "$suzaku_install_panel_pid"
  printf 'PASS: installed panel starts and renders Chinese UI on an isolated X11 display.\n'
  exit 0
fi
export DEBIAN_FRONTEND=noninteractive
suzaku_install_version=$(dpkg-deb --field /packages/suzaku.deb Version)
suzaku_install_tmp=$(mktemp -d /tmp/suzaku-deb-lifecycle.XXXXXX)
chmod 755 "$suzaku_install_tmp"
dpkg-deb --raw-extract /packages/suzaku.deb "$suzaku_install_tmp/previous"
# A synthetic predecessor verifies dpkg upgrade behavior without publishing a version.
sed -i "s/^Version:.*/Version: $suzaku_install_version~install-test/" "$suzaku_install_tmp/previous/DEBIAN/control"
dpkg-deb --root-owner-group --build "$suzaku_install_tmp/previous" "$suzaku_install_tmp/previous.deb"
suzaku_install_apt_options=(-o Acquire::Retries=1 -o Acquire::http::Timeout=20 -o Acquire::https::Timeout=20)
apt-get "${suzaku_install_apt_options[@]}" update
if [[ -d /package-cache ]]; then
  # The base image's post-update cleanup must run before pre-filling archives.
  find /package-cache -maxdepth 1 -name '*.deb' -type f -exec cp -t /var/cache/apt/archives/ -- {} +
fi
apt-get "${suzaku_install_apt_options[@]}" -y --no-install-recommends install "$suzaku_install_tmp/previous.deb"
[[ $(dpkg-query -W -f='${Version}' suzaku) == "$suzaku_install_version~install-test" ]]
for suzaku_install_binary in panel linux_ime_host linux_ime_probe suzaku_tool; do
  test -x "/usr/lib/suzaku/$suzaku_install_binary"
  suzaku_install_libraries=$(ldd "/usr/lib/suzaku/$suzaku_install_binary")
  [[ $suzaku_install_libraries != *'not found'* ]]
done
# Check runtime tools/fonts before adding the testing tools, so those cannot mask
# missing dependencies in the package itself.
for suzaku_install_command in ibus gdbus systemctl pgrep fc-match; do
  command -v "$suzaku_install_command" >/dev/null || {
    printf 'Missing packaged runtime command: %s\n' "$suzaku_install_command" >&2; exit 1;
  }
done
for suzaku_install_font in /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf; do
  test -s "$suzaku_install_font" || { printf 'Missing packaged font: %s\n' "$suzaku_install_font" >&2; exit 1; }
done
python3 -c 'import ctypes; [ctypes.CDLL(name) for name in ["libxkbcommon.so.0", "libxkbcommon-x11.so.0", "libwayland-client.so.0", "libX11.so.6", "libX11-xcb.so.1", "libXcursor.so.1", "libXi.so.6", "libEGL.so.1", "libGL.so.1", "libvulkan.so.1"]]'
printf 'PASS: clean --no-install-recommends install includes runtime tools, fonts and linked libraries.\n'

mkdir -p /home/suzaku-test/.config/suzaku-ime /home/suzaku-test/.config/suzaku-panel \
  /home/suzaku-test/.local/share/suzaku/backups
printf '%s\n' '{"language":"ja","llm_enabled":false}' > /home/suzaku-test/.config/suzaku-ime/settings.json
printf '%s\n' 'ui_language=pt' > /home/suzaku-test/.config/suzaku-panel/panel-settings.toml
printf '%s\n' 'private backup sentinel' > /home/suzaku-test/.local/share/suzaku/backups/keep.txt
find /home/suzaku-test -type f -exec sha256sum {} + > "$suzaku_install_tmp/user-data.sha256"
# Simulate a damaged old executable and verify upgrade replaces it from the package.
printf '%s\n' 'old installation sentinel' > /usr/lib/suzaku/suzaku_tool
apt-get "${suzaku_install_apt_options[@]}" -y -qq --no-install-recommends install /packages/suzaku.deb
[[ $(dpkg-query -W -f='${Status} ${Version}' suzaku) == "install ok installed $suzaku_install_version" ]]
[[ $(suzaku-tool --version) == "suzaku-map $suzaku_install_version" ]]
if suzaku-tool linux-register install; then
  printf 'Registration must refuse root/sudo.\n' >&2; exit 1;
fi
grep -Fx 'Exec=/usr/lib/suzaku/panel' /usr/share/applications/dev.suzaku.Suzaku.desktop
grep -Fx 'TryExec=/usr/lib/suzaku/panel' /usr/share/applications/dev.suzaku.Suzaku.desktop
cmp /usr/lib/suzaku/suzaku_tool "$suzaku_install_tmp/previous/usr/lib/suzaku/suzaku_tool"
sha256sum --quiet -c "$suzaku_install_tmp/user-data.sha256"
test ! -e /home/suzaku-test/.config/systemd/user/suzaku-ibus.service
test ! -e /home/suzaku-test/.local/share/ibus/component/dev.suzaku.linux.ime.xml
test ! -e /home/suzaku-test/.ollama
printf 'PASS: upgrade replaces program files and does not register, switch input methods or overwrite user data.\n'

# Added only after checking package runtime dependencies, never to hide missing ones.
apt-get "${suzaku_install_apt_options[@]}" -y -qq --no-install-recommends install xvfb xauth x11-utils
timeout --kill-after=3s 25s dbus-run-session -- xvfb-run -a -s '-screen 0 1280x800x24 -nolisten tcp' bash /checks/install.sh --panel-smoke

apt-get -y -qq remove suzaku
for suzaku_install_path in /usr/bin/suzaku-panel /usr/bin/suzaku-tool /usr/lib/suzaku/panel \
  /usr/share/applications/dev.suzaku.Suzaku.desktop; do
  [[ ! -e $suzaku_install_path && ! -L $suzaku_install_path ]]
done
sha256sum --quiet -c "$suzaku_install_tmp/user-data.sha256"
dpkg --purge suzaku
sha256sum --quiet -c "$suzaku_install_tmp/user-data.sha256"
printf 'PASS: remove/purge clean up installed launchers and preserve user settings and backups.\n'
