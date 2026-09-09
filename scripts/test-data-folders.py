#!/usr/bin/env python3
"""Private D-Bus FileManager1 fixture: never opens a real desktop folder."""
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time

from gi.repository import Gio, GLib

if os.environ.get("SUZAKU_DATA_QA") != "1":
    raise SystemExit("Run under dbus-run-session with SUZAKU_DATA_QA=1")

binary = Path(__file__).resolve().parent.parent / "target/debug/suzaku_tool"
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
result = connection.call_sync(
    "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
    GLib.Variant("(su)", ("org.freedesktop.FileManager1", 4)), GLib.VariantType("(u)"),
    Gio.DBusCallFlags.NONE, 1000, None,
)
if result.unpack() != (1,):
    raise SystemExit("Fixture could not own the private FileManager1 name")

seen = []
failures = []
interface = Gio.DBusNodeInfo.new_for_xml("""
<node><interface name="org.freedesktop.FileManager1"><method name="ShowFolders">
<arg type="as" direction="in"/><arg type="s" direction="in"/>
</method></interface></node>
""").interfaces[0]


def show_folders(_connection, _sender, _path, _interface, method, parameters, invocation):
    assert method == "ShowFolders"
    uris, startup_id = parameters.unpack()
    seen.append((uris, startup_id))
    invocation.return_value(GLib.Variant("()", ()))


registration = connection.register_object("/org/freedesktop/FileManager1", interface, show_folders, None, None)
with tempfile.TemporaryDirectory(prefix="suzaku-data-folders-") as temporary:
    # Exercise spaces, non-ASCII, quotes, percent and shell metacharacters as path data.
    root = Path(temporary) / "中文 日本語 ' % $ ;"
    environment = dict(os.environ, HOME=str(root), XDG_CONFIG_HOME=str(root / "config"),
                       XDG_DATA_HOME=str(root / "data"), XDG_RUNTIME_DIR=str(root / "runtime"))
    environment.pop("SUZAKU_IME_CONFIG", None)
    environment.pop("SUZAKU_LINUX_IME_SOCKET", None)
    destinations = [root / "data/suzaku/backups", root / "config/suzaku-ime", root / "config/suzaku-panel"]

    def client():
        try:
            for target in ("backups", "ime", "panel"):
                completed = subprocess.run([str(binary), "data", "open", target], env=environment,
                                           capture_output=True, text=True, timeout=5, check=False)
                assert completed.returncode == 0, completed.stderr
        except BaseException as error:
            failures.append(error)

    worker = threading.Thread(target=client)
    worker.start()
    deadline = time.monotonic() + 18
    context = GLib.MainContext.default()
    while worker.is_alive() and time.monotonic() < deadline:
        context.iteration(False)
        time.sleep(0.005)
    worker.join(timeout=1)
    assert not worker.is_alive(), "fixture deadline exceeded"
    assert not failures, failures
    assert seen == [([path.as_uri()], "") for path in destinations], seen
    assert all(path.is_dir() and path.stat().st_mode & 0o777 == 0o700 for path in destinations)
connection.unregister_object(registration)
print("PASS: three data-folder actions use private FileManager1, preserve literal paths and create private directories")
