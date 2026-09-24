#!/usr/bin/env python3
"""Owned IBus daemon loss/restart with a live GTK application and companion.

This explicitly restarts owned processes and reselects the engine. It does not
simulate systemd or claim the user's desktop automatically restores an engine.
"""
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "suzaku_popup_qa", Path(__file__).with_name("test-linux-candidate-window.py"))
ui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
qa = ui.qa
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "bus-restart"

popup = None
subscription = None
passed = 0


def passed_case(*parts):
    global passed
    passed += 1
    print("PASS:", *parts)


def host_ready():
    try:
        return qa.command("Q") == b"1"
    except OSError:
        return False


def companion_shows(frame):
    # The title exposes the selected commit preview, not the raw pinyin/romaji.
    text = frame["candidates"][frame["selected"]]["text"]
    return any("Suzaku XR Candidate Panel" in title and "draft: " + text + " |" in title
               for _, title in x.windows())


try:
    qa.subprocess.run(["setxkbmap", "-layout", "us", "-option", "", "-option", "compose:menu"],
                      check=True, timeout=3)
    # An unavailable bus must not leave a live host or a misleading ready socket.
    early = qa.spawn([str(qa.bins / "linux_ime_host")])
    qa.wait(lambda: early.poll() is not None, "host startup without IBus fails promptly")
    assert early.returncode != 0 and not qa.host_socket.exists()
    passed_case("host started before private IBus fails without advertising readiness")

    bus, x = qa.start()
    daemon = next(process for process in qa.processes if process.args[0] == "ibus-daemon")
    host = next(process for process in qa.processes
                if process.args == [str(qa.bins / "linux_ime_host")] and process.poll() is None)
    ui.Gdk.init([])
    screen = ui.Gdk.get_default_root_window()
    panel = qa.spawn(["/usr/libexec/ibus-ui-gtk3"])
    popup = ui.Popup(panel, x, bus, screen)
    document = qa.root / "suzaku-bus-restart-qa.txt"
    document.write_text("")
    editor = qa.spawn(["gnome-text-editor", "--standalone", str(document)])
    qa.wait(lambda: x.window(document.name), "bus restart QA editor")
    window = x.window(document.name)
    x.focus(window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    qa.settle_input(bus, x, window)
    qa.prepare_editor(x, document)
    companion = qa.spawn([str(qa.bins / "panel")])

    cases = [(language, state, "terminate") for language in ["en", "zh-Hans", "ja"]
             for state in ["draft", "adopted", "compose"]]
    cases += [("en", state, "kill") for state in ["draft", "adopted", "compose"]]
    for language, state, stop in cases:
        assert json.loads(qa.command("L" + language))["ok"]
        qa.clear_document(x, document)
        seed = {"en": "hel", "zh-Hans": "nihao", "ja": "nihongo"}[language]
        x.type(seed)
        qa.wait(lambda: qa.seed_is(seed), "draft before bus loss")
        popup.expect_page()
        if state == "adopted":
            expected = qa.watch.latest["candidates"][1]["text"]
            x.key(qa.IBus.KEY_2)
            qa.wait(lambda: qa.seed_is(expected), "adopted draft before bus loss")
        elif state == "compose":
            x.key(qa.IBus.KEY_Multi_key)
            x.key(qa.IBus.KEY_apostrophe)
            qa.wait(lambda: any("Compose…" in node.get_name() for node in popup.visible()),
                    "unfinished Compose before bus loss")
        qa.save_document(x, document, "")
        old = qa.watch.latest.copy()
        qa.wait(lambda: companion_shows(old),
                "companion has the draft before bus loss", timeout=30)

        subscription = qa.watch.sock
        qa.watch = None
        # Only the directly owned daemon is stopped, never a desktop PID/name.
        getattr(daemon, stop)()
        daemon.wait(timeout=3)
        qa.wait(lambda: host.poll() is not None, "native host exits after bus loss")
        assert host.returncode == 0 and not qa.host_socket.exists()

        def subscription_closed():
            try:
                return not subscription.recv(65536)
            except BlockingIOError:
                return False

        qa.wait(subscription_closed, "old companion subscription reaches EOF")
        subscription.close()
        subscription = None
        qa.wait(lambda: not bus.is_connected(), "observer sees private bus disconnection")
        qa.wait(lambda: panel.poll() is not None or
                not any(node.get_role() == ui.Atspi.Role.WINDOW for node in popup.visible()),
                "bus loss hides the real candidate popup")
        popup.close()
        popup = None
        if panel.poll() is None:
            panel.terminate()
        panel.wait(timeout=3)
        assert editor.poll() is None and companion.poll() is None and x.focused() == window
        qa.save_document(x, document, "")
        x.type("literal")
        qa.save_document(x, document, "literal\n")

        # Recovery is explicit. The same application and companion stay alive;
        # only the owned daemon, native host and stock panel are relaunched.
        # This fixture's fixed filesystem socket survives daemon exit. Remove
        # only its confirmed dead endpoint before rebinding the same address.
        endpoint = qa.runtime / "ibus.sock"
        if endpoint.exists():
            assert endpoint.is_socket() and not endpoint.is_symlink()
            with qa.socket.socket(qa.socket.AF_UNIX) as client:
                client.settimeout(1)
                try:
                    client.connect(str(endpoint))
                except ConnectionRefusedError:
                    pass
                else:
                    raise AssertionError("refusing to unlink a live private IBus endpoint")
            endpoint.unlink()
        daemon = qa.spawn(daemon.args)

        def bus_ready():
            assert daemon.poll() is None, "replacement private IBus exited"
            return bus.is_connected()

        qa.wait(bus_ready, "observer reconnects to replacement private bus", timeout=15)
        host = qa.spawn([str(qa.bins / "linux_ime_host")])
        qa.wait(host_ready, "native host reconnects after private bus restart")
        qa.watch = qa.Watch()
        qa.wait(lambda: qa.watch.latest is not None, "new host snapshot")
        assert qa.watch.latest["host"] != old["host"]
        assert qa.watch.latest["language"] == language
        assert not qa.watch.latest["seed"] and not qa.watch.latest["candidates"]
        # Accessibility appears before stock-panel startup selects US. Observe
        # that selection before explicit activation, or its late reset wins.
        engine_changes = []
        bus.set_watch_ibus_signal(True)
        changed = bus.connect("global-engine-changed", lambda _bus, name: engine_changes.append(name))
        try:
            panel = qa.spawn(["/usr/libexec/ibus-ui-gtk3"])
            popup = ui.Popup(panel, x, bus, screen)
            qa.wait(lambda: "xkb:us::eng" in engine_changes,
                    "replacement stock panel selects fixture's default engine")
        finally:
            bus.disconnect(changed)
        assert bus.set_global_engine("dev.suzaku.linux.ime")
        qa.settle_input(bus, x, window)
        qa.wait(lambda: qa.seed_is(""), "reactivation does not replay the lost draft")
        popup.expect_hidden()
        qa.save_document(x, document, "literal\n")
        qa.clear_document(x, document)
        x.type("fresh")
        qa.wait(lambda: qa.seed_is("fresh"), "running GTK application reconnected without Compose residue")
        popup.expect_page()
        fresh = qa.watch.latest.copy()
        qa.wait(lambda: companion_shows(fresh), "running companion reconnects without restart")
        assert qa.command(f'A{old["host"]} {fresh["revision"]} K0') == b"0"
        qa.save_document(x, document, "")
        assert qa.watch.latest == fresh and x.focused() == window
        if state == "compose":
            expected = fresh["candidates"][0]["text"]
            clicks = len(popup.clicks)
            popup.click(ui.row_text(fresh, 0), character=0)
            qa.wait(lambda: len(popup.clicks) == clicks + 1, "restarted popup sends a real glyph click")
            assert popup.clicks[-1][:2] == (0, 1)
        else:
            expected = fresh["candidates"][fresh["selected"]]["text"]
            x.key(qa.IBus.KEY_Return)
        qa.wait(lambda: qa.seed_is(""), "new host commits exactly once")
        popup.expect_hidden()
        qa.save_document(x, document, expected + "\n")
        assert editor.poll() is None and companion.poll() is None and x.focused() == window
        passed_case("private bus loss, literal fallback and explicit recovery", language, state, stop)

    assert passed == 13
    print("RESULT: 13 strict private-bus startup/restart checks passed")
except Exception:
    if "bus" in globals() and bus.is_connected():
        engine = bus.get_global_engine()
        print("QA bus engine/context:", None if engine is None else engine.get_name(),
              bus.current_input_context())
    qa.diagnose()
    raise
finally:
    if subscription is not None:
        subscription.close()
    if popup is not None:
        popup.close()
    qa.close()
