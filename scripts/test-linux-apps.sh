#!/usr/bin/env bash
# Real application input, on a fresh display and session bus only.
set -euo pipefail
suzaku_apps_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$suzaku_apps_root"
suzaku_apps_suite=${1:-gtk}
case "$suzaku_apps_suite" in
  gtk|gtk3|vocabulary|browser|firefox|vscode|model|qt5|qt6|cross|popup|candidates|bottom-layout|auto-layout|lifecycle|bus-restart|keyboard|editor) ;;
  *) printf 'Usage: bash scripts/test-linux-apps.sh [gtk|gtk3|vocabulary|browser|firefox|vscode|model|qt5|qt6|cross|popup|candidates|bottom-layout|auto-layout|lifecycle|bus-restart|keyboard|editor]\n' >&2; exit 2 ;;
esac
# Existing suites retain their explicit synchronous IBus transport. The GTK3
# diagnostic also exercises the installed client's default, without inheriting
# a desktop override or changing any user environment/configuration.
suzaku_apps_input_env=(IBUS_ENABLE_SYNC_MODE=1)
if [[ $suzaku_apps_suite == gtk3 || $suzaku_apps_suite == candidates || $suzaku_apps_suite == bottom-layout || $suzaku_apps_suite == auto-layout ]]; then
  case "${SUZAKU_GTK3_QA_SYNC_MODE:-default}" in
    default) suzaku_apps_input_env=() ;;
    1) ;;
    *) printf 'SUZAKU_GTK3_QA_SYNC_MODE must be default or 1.\n' >&2; exit 2 ;;
  esac
fi
if [[ $suzaku_apps_suite == model && ( ${SUZAKU_MODEL_LOCAL_QA:-0} != 1 || -z ${SUZAKU_MODEL_QA_MODEL:-} ) ]]; then
  printf 'Live QA needs explicit SUZAKU_MODEL_LOCAL_QA=1 and SUZAKU_MODEL_QA_MODEL (installed local Ollama model).\n' >&2
  exit 2
fi
suzaku_apps_dependencies=(xvfb-run xauth dbus-run-session ibus-daemon xwininfo timeout)
case "$suzaku_apps_suite" in
  gtk|vocabulary|popup|lifecycle|bus-restart|keyboard|editor) suzaku_apps_dependencies+=(gsettings) ;;
esac
[[ $suzaku_apps_suite != gtk ]] || suzaku_apps_dependencies+=(gnome-text-editor zenity)
[[ $suzaku_apps_suite != vocabulary ]] || suzaku_apps_dependencies+=(gnome-text-editor)
[[ $suzaku_apps_suite != popup ]] || suzaku_apps_dependencies+=(gnome-text-editor /usr/libexec/ibus-ui-gtk3)
[[ $suzaku_apps_suite != lifecycle ]] || suzaku_apps_dependencies+=(gnome-text-editor zenity setxkbmap /usr/libexec/ibus-ui-gtk3)
[[ $suzaku_apps_suite != bus-restart ]] || suzaku_apps_dependencies+=(gnome-text-editor setxkbmap /usr/libexec/ibus-ui-gtk3)
[[ $suzaku_apps_suite != keyboard ]] || suzaku_apps_dependencies+=(gnome-text-editor setxkbmap xkbcomp)
[[ $suzaku_apps_suite != editor ]] || suzaku_apps_dependencies+=(gnome-text-editor)
if [[ $suzaku_apps_suite == browser || $suzaku_apps_suite == cross || $suzaku_apps_suite == model ]]; then
  suzaku_apps_dependencies+=("${SUZAKU_APP_QA_BROWSER:-google-chrome}")
fi
[[ $suzaku_apps_suite != firefox ]] || suzaku_apps_dependencies+=("${SUZAKU_APP_QA_FIREFOX:-firefox}")
[[ $suzaku_apps_suite != vscode ]] || suzaku_apps_dependencies+=("${SUZAKU_APP_QA_CODE:-code}")
for suzaku_apps_command in "${suzaku_apps_dependencies[@]}"; do
  command -v "$suzaku_apps_command" >/dev/null || {
    printf 'Missing application QA dependency: %s\n' "$suzaku_apps_command" >&2; exit 1;
  }
done
if [[ $suzaku_apps_suite == candidates || $suzaku_apps_suite == bottom-layout || $suzaku_apps_suite == auto-layout ]]; then
  env PYTHONDONTWRITEBYTECODE=1 /usr/bin/python3 -c \
    'import gi; gi.require_version("Gtk", "3.0"); gi.require_version("GdkX11", "3.0"); from gi.repository import Gtk, GdkX11' || {
      printf 'Native candidate QA needs the GTK3/GdkX11 Python GI bindings.\n' >&2
      exit 1
    }
fi
# Bindings may be extracted under target/ for local QA, never installed implicitly.
suzaku_apps_qt_python=/usr/bin/python3
if [[ $suzaku_apps_suite == qt5 || $suzaku_apps_suite == qt6 || $suzaku_apps_suite == cross ]]; then
  suzaku_apps_qt_python=$(realpath -e -- "${SUZAKU_QT_QA_PYTHON:-/usr/bin/python3}") || {
    printf 'Qt QA Python path does not exist: %s\n' "${SUZAKU_QT_QA_PYTHON:-/usr/bin/python3}" >&2
    exit 1
  }
  [[ -f "$suzaku_apps_qt_python" && -x "$suzaku_apps_qt_python" ]] || {
    printf 'Qt QA Python path is not an executable file: %s\n' "$suzaku_apps_qt_python" >&2
    exit 1
  }
fi
for suzaku_apps_qt in 5 6; do
  if [[ $suzaku_apps_suite == "qt$suzaku_apps_qt" || $suzaku_apps_suite == cross ]]; then
    env PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="${SUZAKU_QT_QA_SITE:-}" "$suzaku_apps_qt_python" -c \
      'import importlib, sys; importlib.import_module("PyQt" + sys.argv[1] + ".QtWidgets")' \
      "$suzaku_apps_qt" || {
        printf 'Missing PyQt%s test bindings; install the QA dependency or set SUZAKU_QT_QA_SITE.\n' \
          "$suzaku_apps_qt" >&2; exit 1;
      }
  fi
done
suzaku_apps_script=scripts/test-linux-cross-apps.py
[[ $suzaku_apps_suite != gtk ]] || suzaku_apps_script=scripts/test-linux-apps.py
[[ $suzaku_apps_suite != gtk3 ]] || suzaku_apps_script=scripts/test-linux-gtk3.py
[[ $suzaku_apps_suite != vocabulary ]] || suzaku_apps_script=scripts/test-linux-vocabulary.py
[[ $suzaku_apps_suite != popup ]] || suzaku_apps_script=scripts/test-linux-candidate-window.py
[[ $suzaku_apps_suite != candidates ]] || suzaku_apps_script=scripts/test-linux-native-candidates.py
[[ $suzaku_apps_suite != bottom-layout ]] || suzaku_apps_script=scripts/test-linux-native-candidates.py
[[ $suzaku_apps_suite != auto-layout ]] || suzaku_apps_script=scripts/test-linux-native-candidates.py
[[ $suzaku_apps_suite != lifecycle ]] || suzaku_apps_script=scripts/test-linux-input-lifecycle.py
[[ $suzaku_apps_suite != bus-restart ]] || suzaku_apps_script=scripts/test-linux-bus-restart.py
[[ $suzaku_apps_suite != keyboard ]] || suzaku_apps_script=scripts/test-linux-keyboard.py
[[ $suzaku_apps_suite != editor ]] || suzaku_apps_script=scripts/test-linux-editor-observer.py
[[ $suzaku_apps_suite != vscode ]] || suzaku_apps_script=scripts/test-linux-vscode.py
[[ $suzaku_apps_suite != model ]] || suzaku_apps_script=scripts/test-linux-model-live.py
suzaku_apps_binaries=(panel linux_ime_host)
if [[ $suzaku_apps_suite == candidates || $suzaku_apps_suite == bottom-layout || $suzaku_apps_suite == auto-layout ]]; then
  suzaku_apps_binaries+=(suzaku_tool)
fi
if [[ -z ${SUZAKU_APP_QA_BIN_DIR:-} ]]; then
  suzaku_apps_build_args=()
  for suzaku_apps_binary in "${suzaku_apps_binaries[@]}"; do
    suzaku_apps_build_args+=(--bin "$suzaku_apps_binary")
  done
  cargo build --locked --all-features "${suzaku_apps_build_args[@]}"
  suzaku_apps_bins="$suzaku_apps_root/target/debug"
else
  suzaku_apps_bins=$(realpath -- "$SUZAKU_APP_QA_BIN_DIR")
fi
for suzaku_apps_binary in "${suzaku_apps_binaries[@]}"; do
  test -x "$suzaku_apps_bins/$suzaku_apps_binary"
done
# Keep the growing full vocabulary gate within the unchanged per-session
# deadline. Build once above, then run both mixed-language partitions with
# independent displays/buses/configuration; each includes EN/ZH/JA, and neither
# part is the complete gate.
if [[ $suzaku_apps_suite == vocabulary && ${SUZAKU_VOCABULARY_QA_SCOPE-all} == all ]]; then
  for suzaku_apps_part in part-1 part-2; do
    env SUZAKU_APP_QA_BIN_DIR="$suzaku_apps_bins" \
      SUZAKU_VOCABULARY_QA_SCOPE="$suzaku_apps_part" \
      bash "$suzaku_apps_root/scripts/test-linux-apps.sh" vocabulary
  done
  printf 'PASS: complete vocabulary scope=all; both mixed-language parts passed.\n'
  exit 0
fi
suzaku_apps_tmp=$(mktemp -d /tmp/suzaku-app-qa.XXXXXX)
cleanup() {
  case "$suzaku_apps_tmp" in
    /tmp/suzaku-app-qa.??????)
      if [[ -d "$suzaku_apps_tmp" && ! -L "$suzaku_apps_tmp" ]]; then
        if [[ ${SUZAKU_APP_QA_KEEP:-0} == 1 ]]; then
          printf 'Synthetic application QA files: %s\n' "$suzaku_apps_tmp"
        else
          rm -r -- "$suzaku_apps_tmp"
        fi
      fi
      ;;
  esac
}
trap cleanup EXIT
mkdir -m 700 "$suzaku_apps_tmp/runtime"
# Newer IBus may rewrite Compose files and create adjacent backups.
install -m 600 "$suzaku_apps_root/scripts/fixtures/compose.XCompose" "$suzaku_apps_tmp/compose.XCompose"
suzaku_apps_screen=1920x1080x24
[[ $suzaku_apps_suite != popup ]] || suzaku_apps_screen=800x600x24
[[ $suzaku_apps_suite != candidates ]] || suzaku_apps_screen=1024x768x24
[[ $suzaku_apps_suite != bottom-layout ]] || suzaku_apps_screen=1024x768x24
[[ $suzaku_apps_suite != auto-layout ]] || suzaku_apps_screen=1024x768x24
# Use the production session detector, not a test-only layout override. These
# variables belong only to the fresh fixture child; the desktop is untouched.
suzaku_apps_desktop_env=(XDG_CURRENT_DESKTOP=SuzakuQA XDG_SESSION_DESKTOP=SuzakuQA)
if [[ $suzaku_apps_suite == auto-layout ]]; then
  suzaku_apps_desktop_env=(XDG_CURRENT_DESKTOP=Phosh XDG_SESSION_DESKTOP=phosh)
fi
# shellcheck disable=SC2016 # Expand DISPLAY only after xvfb-run assigns the private display.
timeout --kill-after=3s 240s env -u DISPLAY -u WAYLAND_DISPLAY -u IBUS_ADDRESS -u IBUS_ENABLE_SYNC_MODE \
  -u SUZAKU_IBUS_INLINE_PREEDIT -u SUZAKU_LEXICON_DIR \
  -u VSCODE_IPC_HOOK_CLI -u VSCODE_IPC_HOOK -u VSCODE_PORTABLE -u ELECTRON_RUN_AS_NODE \
  -u AT_SPI_BUS_ADDRESS -u SESSION_MANAGER -u DBUS_STARTER_ADDRESS -u DBUS_STARTER_BUS_TYPE \
  XDG_RUNTIME_DIR="$suzaku_apps_tmp/runtime" XDG_CONFIG_HOME="$suzaku_apps_tmp/config" \
  XDG_DATA_HOME="$suzaku_apps_tmp/data" XDG_CACHE_HOME="$suzaku_apps_tmp/cache" \
  SUZAKU_APP_QA=1 SUZAKU_APP_QA_ROOT="$suzaku_apps_tmp" SUZAKU_APP_QA_BIN_DIR="$suzaku_apps_bins" \
  SUZAKU_APP_QA_SUITE="$suzaku_apps_suite" \
  SUZAKU_QT_QA_PYTHON="$suzaku_apps_qt_python" \
  SUZAKU_PANEL_TEST_FRAME_LOG=1 \
  SUZAKU_IME_CONFIG="$suzaku_apps_tmp/ime.json" \
  SUZAKU_LINUX_IME_SOCKET="$suzaku_apps_tmp/runtime/suzaku-ime/host.sock" \
  IBUS_ADDRESS="unix:path=$suzaku_apps_tmp/runtime/ibus.sock" \
  XCOMPOSEFILE="$suzaku_apps_tmp/compose.XCompose" \
  XLOCALEDIR=/usr/share/X11/locale GSETTINGS_BACKEND=memory GIO_USE_VFS=local \
  GTK_IM_MODULE=ibus "${suzaku_apps_input_env[@]}" IBUS_DISCARD_PASSWORD=0 \
  GDK_BACKEND=x11 GDK_SCALE=1 GDK_DPI_SCALE=1 GTK_A11Y=none NO_AT_BRIDGE=1 \
  "${suzaku_apps_desktop_env[@]}" XDG_SESSION_TYPE=x11 WINIT_X11_SCALE_FACTOR=1 \
  SUZAKU_LINUX_PANEL_BACKEND=x11-nofocus LIBGL_ALWAYS_SOFTWARE=1 \
  LC_ALL=C.UTF-8 PYTHONUNBUFFERED=1 \
  xvfb-run -a -s "-screen 0 $suzaku_apps_screen -nolisten tcp" \
  dbus-run-session -- \
  sh -c 'export SUZAKU_APP_QA_DISPLAY="$DISPLAY"; exec "$@"' sh \
  /usr/bin/python3 "$suzaku_apps_script"
