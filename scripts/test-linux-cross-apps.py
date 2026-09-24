#!/usr/bin/env python3
"""Physical X11 events -> browser/Qt fields -> application-observed text, on Xvfb."""
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)
IBus = qa.IBus
from gi.repository import Gio

fixtures = Path(__file__).resolve().parent / "fixtures"
passed = 0


def passed_case(*parts):
    global passed
    passed += 1
    print("PASS:", *parts)


class EngineCommits:
    """Observe only this fixture's private engine; never alter input contexts."""
    def __init__(self, bus):
        self.connection = bus.get_connection()
        self.texts = []
        self.preedit_updates = []
        self.auxiliary = ""
        self.auxiliary_visible = False

        def committed(_connection, _sender, _path, _interface, _signal, params, *_data):
            text = IBus.Serializable.deserialize_object(params.get_child_value(0).get_variant())
            self.texts.append(text.get_text())

        def preedit(_connection, _sender, _path, _interface, _signal, params, *_data):
            self.preedit_updates.append(params.unpack())

        def auxiliary(_connection, _sender, _path, _interface, _signal, params, *_data):
            text = IBus.Serializable.deserialize_object(params.get_child_value(0).get_variant())
            self.auxiliary = text.get_text()
            self.auxiliary_visible = params.get_child_value(1).get_boolean()

        def hide_auxiliary(*_args):
            self.auxiliary_visible = False

        self.subscriptions = [self.connection.signal_subscribe(
            "org.freedesktop.IBus.Suzaku", "org.freedesktop.IBus.Engine", signal, None, None,
            Gio.DBusSignalFlags.NONE, callback) for signal, callback in [
                ("CommitText", committed), ("UpdatePreeditText", preedit),
                ("UpdateAuxiliaryText", auxiliary), ("HideAuxiliaryText", hide_auxiliary)]]

    def close(self):
        for subscription in self.subscriptions:
            self.connection.signal_unsubscribe(subscription)


class Bridge:
    def __init__(self):
        self.token = secrets.token_hex(16)
        self.state = None
        self.commands = []
        self.serial = 0
        self.lock = threading.Lock()
        bridge = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def reply(self, body, content_type="application/json"):
                self.send_response(200)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                if self.path == "/case/" + bridge.token:
                    self.reply((fixtures / "input-fields.html").read_bytes(), "text/html; charset=utf-8")
                elif self.path == "/next/" + bridge.token:
                    with bridge.lock:
                        pending, bridge.commands = bridge.commands, []
                    self.reply(json.dumps(pending).encode())
                else:
                    self.send_error(404)

            def do_POST(self):
                length = int(self.headers.get("Content-Length", "0"))
                if self.path != "/state/" + bridge.token or not 0 < length <= 65536:
                    self.send_error(400)
                    return
                self.connection.settimeout(2)
                state = json.loads(self.rfile.read(length))
                with bridge.lock:
                    if bridge.state is None or state["seq"] > bridge.state["seq"]:
                        bridge.state = state
                self.reply(b"{}")

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.base = "http://127.0.0.1:" + str(self.server.server_port)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def focus(self, field, start=None, end=None):
        with self.lock:
            self.serial += 1
            command = {"id": self.serial, "field": field}
            if start is not None:
                command.update(start=start, end=start if end is None else end)
            self.commands.append(command)
        qa.wait(lambda: self.state and self.state["ack"] >= command["id"] and
                self.state["active"] == field, "application field focus")

    def value(self, field):
        return None if self.state is None else self.state["values"][field]

    def expect(self, field, text):
        qa.wait(lambda: self.value(field) == text and not self.state["composition"][field],
                f"application text/preedit for {field}: expected {text!r}")

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)


def start_app(kind, bridge, sync_mode):
    if kind == "firefox":
        profile = qa.root / "firefox-profile"
        profile.mkdir()
        shutil.copyfile(fixtures / "firefox-user.js", profile / "user.js")
        return qa.spawn([
            os.environ.get("SUZAKU_APP_QA_FIREFOX", "firefox"),
            "--no-remote", "--new-instance", "--profile", str(profile),
            "--width", "1000", "--height", "900", bridge.base + "/case/" + bridge.token,
        ], env=dict(os.environ, MOZ_ENABLE_WAYLAND="0", MOZ_CRASHREPORTER_DISABLE="1"))
    if kind == "browser":
        profile = qa.root / "browser-profile"
        return qa.spawn([
            os.environ.get("SUZAKU_APP_QA_BROWSER", "google-chrome"),
            "--user-data-dir=" + str(profile), "--no-first-run", "--no-default-browser-check",
            "--disable-background-networking", "--disable-component-update", "--disable-sync",
            "--disable-extensions", "--disable-default-apps", "--password-store=basic",
            "--disable-features=Translate,OptimizationHints,MediaRouter",
            "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1",
            "--ozone-platform=x11", "--window-size=1000,900", "--window-position=0,0",
            "--app=" + bridge.base + "/case/" + bridge.token,
        ])
    env = dict(os.environ, QT_IM_MODULE="ibus", QT_QPA_PLATFORM="xcb", QT_SCALE_FACTOR="1",
               QT_AUTO_SCREEN_SCALE_FACTOR="0", QT_STYLE_OVERRIDE="Fusion", QT_QPA_PLATFORMTHEME="",
               IBUS_ENABLE_SYNC_MODE=str(sync_mode))
    if os.environ.get("SUZAKU_QT_QA_SITE"):
        env["PYTHONPATH"] = str(Path(os.environ["SUZAKU_QT_QA_SITE"]).resolve())
    return qa.spawn(["/usr/bin/python3", str(fixtures / "qt-input-fields.py"), kind[-1],
                     bridge.base, bridge.token], env=env)


def check_fields(kind, bridge, bus, x, sync_mode, commits):
    def focus(field, start=None, end=None):
        bridge.focus(field, start, end)
        qa.settle_input(bus, x, window)

    def clear(field):
        focus(field)
        if qa.watch.latest["seed"]:
            x.key(IBus.KEY_Escape)
            qa.wait(lambda: qa.seed_is(""), "cancel before clearing application text")
        x.key(IBus.KEY_a, IBus.KEY_Control_L)
        x.key(IBus.KEY_BackSpace)
        bridge.expect(field, "")

    title = "Suzaku Browser QA" if kind in ("browser", "firefox") else "Suzaku Qt" + kind[-1] + " QA"
    qa.wait(lambda: bridge.state and x.window(title), kind + " application ready", timeout=20)
    window = x.window(title)
    x.focus(window)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    focus("editor")
    print("READY:", kind, bridge.state.get("qt_version", bridge.state.get("userAgent")),
          "IBus key sync mode:", sync_mode)
    for field in ["editor", "rich", "entry"]:
        clear(field)
        x.type("hello world")
        qa.wait(lambda: qa.seed_is("hello world"), kind + " continuous draft")
        bridge.expect(field, "")
        qa.wait(lambda: commits.auxiliary_visible and
                commits.auxiliary.startswith("Suzaku · hello world\n"), "draft shown in candidate area")
        assert not commits.preedit_updates, "default mode must not populate the application preedit cache"
        qa.commit(x)
        bridge.expect(field, "hello world")
        qa.wait(lambda: not commits.auxiliary_visible, "committed candidate-area draft clears")
        assert commits.texts[-1:] == ["hello world"], "private engine commit observer must be live"
        x.type(" again")
        qa.wait(lambda: qa.seed_is("again"), "leading empty-draft space belongs to application")
        qa.commit(x)
        bridge.expect(field, "hello world again")
        passed_case(kind, field, "Space continuity and exact consecutive commits")

        clear(field)
        x.type("hel")
        qa.wait(lambda: qa.seed_is("hel"), "candidate prefix")
        qa.choose_number(x, "hello")
        x.key(IBus.KEY_BackSpace)
        qa.wait(lambda: qa.seed_is("hel"), "completion undo")
        qa.choose_number(x, "hello")
        x.type(" world")
        qa.wait(lambda: qa.seed_is("hello world"), "completion continuation")
        qa.commit(x)
        bridge.expect(field, "hello world")
        passed_case(kind, field, "number adoption, undo, continuation and commit")

        focus(field, 6, 11)
        x.type("friend")
        qa.wait(lambda: qa.seed_is("friend"), "replacement preedit")
        qa.commit(x)
        bridge.expect(field, "hello friend")
        focus(field, 5)
        x.type(" dear")
        qa.wait(lambda: qa.seed_is("dear"), "insertion preedit")
        qa.commit(x)
        bridge.expect(field, "hello dear friend")
        passed_case(kind, field, "selection replacement and caret insertion")

        focus(field, 6, 10)
        x.type("cancelled")
        qa.wait(lambda: qa.seed_is("cancelled"), "draft over a selection before cancel")
        bridge.expect(field, "hello dear friend")
        x.key(IBus.KEY_Escape)
        qa.wait(lambda: qa.seed_is(""), "cancel selection replacement")
        bridge.expect(field, "hello dear friend")
        passed_case(kind, field, "cancel leaves existing selected application text intact")

        clear(field)
        x.type("discard")
        qa.wait(lambda: qa.seed_is("discard"), "cancel preedit")
        x.key(IBus.KEY_Escape)
        qa.wait(lambda: qa.seed_is(""), "Escape cancel")
        bridge.expect(field, "")
        x.type("x")
        qa.wait(lambda: qa.seed_is("x"), "single letter before delete")
        x.key(IBus.KEY_BackSpace)
        qa.wait(lambda: qa.seed_is(""), "delete to empty")
        bridge.expect(field, "")
        passed_case(kind, field, "cancel and deletion clear application preedit")

    clear("editor")
    x.type("discardme")
    qa.wait(lambda: qa.seed_is("discardme"), "draft before field switch")
    before = len(commits.texts)
    focus("entry")
    qa.wait(lambda: qa.seed_is(""), "field switch ends old draft")
    qa.wait(lambda: not bridge.state["composition"]["editor"], "old application preedit ends")
    assert len(commits.texts) == before, "engine unexpectedly committed during a focus change"
    bridge.expect("editor", "")
    bridge.expect("entry", "")
    x.type("new field")
    qa.wait(lambda: qa.seed_is("new field"), "new field independent draft")
    qa.commit(x)
    bridge.expect("entry", "new field")
    passed_case(kind, "new field does not inherit old preedit")

    clear("editor")
    x.type("cancel first")
    qa.wait(lambda: qa.seed_is("cancel first"), "draft before explicit cancel and switch")
    x.key(IBus.KEY_Escape)
    qa.wait(lambda: qa.seed_is(""), "explicit cancel before field switch")
    bridge.expect("editor", "")
    focus("entry")
    bridge.expect("editor", "")
    bridge.expect("entry", "new field")
    passed_case(kind, "Escape before changing fields prevents implicit application commits")

    start = len(qa.watch.frames)
    focus("password")
    x.type("synthetic42")
    bridge.expect("password", "synthetic42")
    assert all(not frame["seed"] and not frame["candidates"] for frame in qa.watch.frames[start:])
    passed_case(kind, "password keys do not enter candidates or companion text")

    start = len(qa.watch.frames)
    focus("number")
    x.type("12.5")
    bridge.expect("number", "12.5")
    assert all(not frame["seed"] and not frame["candidates"] for frame in qa.watch.frames[start:])
    passed_case(kind, "empty-draft numbers pass literally without needing purpose hints")
    for field in ["editor", "rich", "entry"]:
        clear(field)
        x.type("2026, 12.5")
        bridge.expect(field, "2026, 12.5")
        assert qa.seed_is("") and not qa.watch.latest["candidates"]
        x.type(" hel")
        qa.wait(lambda: qa.seed_is("hel"), "normal candidate selection resumes after literal numbers")
        qa.choose_number(x, "hello")
        qa.commit(x)
        bridge.expect(field, "2026, 12.5 hello")
        passed_case(kind, field, "literal numbers followed by ordinary candidate-number adoption")
    clear("editor")
    x.type("public again")
    qa.wait(lambda: qa.seed_is("public again"), "public draft resumes after protected fields")
    qa.commit(x)
    bridge.expect("editor", "public again")
    passed_case(kind, "public input resumes after password and number fields")
    assert not commits.preedit_updates


def main():
    bridge = None
    commits = None
    try:
        bus, x = qa.start()
        suite = os.environ["SUZAKU_APP_QA_SUITE"]
        kinds = ["browser", "qt5", "qt6"] if suite == "cross" else [suite]
        for kind in kinds:
            for sync_mode in ([0, 1] if kind.startswith("qt") else [1]):
                bridge = Bridge()
                commits = EngineCommits(bus)
                app = start_app(kind, bridge, sync_mode)
                check_fields(kind, bridge, bus, x, sync_mode, commits)
                commits.close()
                commits = None
                app.terminate()
                app.wait(timeout=5)
                bridge.close()
                bridge = None
        print(f"RESULT: {passed} workflow checks passed; strict focus and literal-number regressions passed")
    except Exception:
        state = None if bridge is None or bridge.state is None else dict(bridge.state)
        if state is not None:
            state["events"] = state["events"][-15:]
        print("Application state:", state)
        qa.diagnose()
        raise
    finally:
        if commits is not None:
            commits.close()
        qa.close()
        if bridge is not None:
            bridge.close()


if __name__ == "__main__":
    main()
