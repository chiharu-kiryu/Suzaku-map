#!/usr/bin/env python3
"""Owned GTK document and stock candidate UI across native lifecycle boundaries."""
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
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "lifecycle"

popup = None
passed = 0


def passed_case(*parts):
    global passed
    passed += 1
    print("PASS:", *parts)


try:
    # Only the owned Xvfb keyboard: give physical Compose tests a known key.
    qa.subprocess.run(["setxkbmap", "-layout", "us", "-option", "", "-option", "compose:menu"],
                      check=True, timeout=3)
    bus, x = qa.start()
    host = next(process for process in qa.processes
                if process.args == [str(qa.bins / "linux_ime_host")])
    ui.Gdk.init([])
    screen = ui.Gdk.get_default_root_window()
    panel = qa.spawn(["/usr/libexec/ibus-ui-gtk3"])
    popup = ui.Popup(panel, x, bus, screen)
    document = qa.root / "suzaku-lifecycle-qa.txt"
    document.write_text("")
    editor = qa.spawn(["gnome-text-editor", "--standalone", str(document)])
    qa.wait(lambda: x.window(document.name), "lifecycle QA editor")
    window = x.window(document.name)
    x.focus(window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    qa.settle_input(bus, x, window)
    qa.prepare_editor(x, document)
    assert "xkb:us::eng" in [engine.get_name() for engine in bus.list_engines()]

    def engine_is(name):
        current = bus.get_global_engine()
        return current is not None and current.get_name() == name

    def setup(language, state):
        assert json.loads(qa.command("L" + language))["ok"]
        qa.clear_document(x, document)
        seed = {"en": "hel", "zh-Hans": "nihao", "ja": "nihongo"}[language]
        x.type(seed)
        qa.wait(lambda: qa.seed_is(seed), "draft before lifecycle boundary")
        popup.expect_page()
        if state == "adopted":
            expected = qa.watch.latest["candidates"][1]["text"]
            x.key(qa.IBus.KEY_2)
            qa.wait(lambda: qa.seed_is(expected), "editable completion before lifecycle boundary")
        elif state == "compose":
            x.key(qa.IBus.KEY_Multi_key)
            x.key(qa.IBus.KEY_apostrophe)
            qa.wait(lambda: popup.named("Compose… · Esc / Backspace 取消") or
                    any("Compose…" in node.get_name() for node in popup.visible()),
                    "incomplete Compose before lifecycle boundary")
        qa.save_document(x, document, "")
        return qa.watch.latest.copy()

    def fresh_input():
        assert qa.seed_is("")
        x.type("fresh")
        qa.wait(lambda: qa.seed_is("fresh"), "no draft or Compose leaks into resumed input")
        popup.expect_page()
        frame = qa.watch.latest
        expected = frame["candidates"][frame["selected"]]["text"]
        x.key(qa.IBus.KEY_Return)
        qa.wait(lambda: qa.seed_is(""), "fresh input commits")
        popup.expect_hidden()
        qa.save_document(x, document, expected + "\n")

    for language in ["en", "zh-Hans", "ja"]:
        for state in ["draft", "adopted", "compose"]:
            old = setup(language, state)
            assert bus.set_global_engine("xkb:us::eng")
            qa.wait(lambda: not qa.watch.latest["focused"], "switching engines revokes native focus")
            popup.expect_hidden()
            qa.save_document(x, document, "")
            x.type("literal")
            qa.save_document(x, document, "literal\n")
            assert qa.command(f'A{old["host"]} {old["revision"]} K0') == b"0"
            assert bus.set_global_engine("dev.suzaku.linux.ime")
            qa.wait(lambda: qa.seed_is(""), "reactivation starts an empty draft")
            assert qa.watch.latest["context"] != old["context"]
            popup.expect_hidden()
            qa.clear_document(x, document)
            fresh_input()
            passed_case("real engine switch, literal fallback and fresh composition", language, state)

    for state in ["draft", "adopted", "compose"]:
        old = setup("en", state)
        popup.close()
        popup = None
        panel.terminate()
        panel.wait(timeout=3)
        panel = qa.spawn(["/usr/libexec/ibus-ui-gtk3"])
        popup = ui.Popup(panel, x, bus, screen)
        # Stock GTK3 reloads this fixture's default (US) engine on startup.
        # That is a real engine switch, not permission to replay the old draft.
        qa.wait(lambda: engine_is("xkb:us::eng") and
                not qa.watch.latest["focused"], "stock panel restart selects its configured default")
        popup.expect_hidden()
        qa.save_document(x, document, "")
        assert qa.command(f'A{old["host"]} {old["revision"]} K0') == b"0"
        assert bus.set_global_engine("dev.suzaku.linux.ime")
        qa.wait(lambda: qa.seed_is(""), "explicit reactivation after stock-panel reset")
        popup.expect_hidden()
        fresh_input()
        passed_case("stock panel reset clears old draft; explicit reactivation resumes input", state)

    for state in ["draft", "adopted", "compose"]:
        old = setup("en", state)
        with (qa.root / "synthetic-password.txt").open("w") as output:
            hidden = qa.spawn(["zenity", "--password", "--title=Suzaku Lifecycle Password"], output=output)
            qa.wait(lambda: x.window("Suzaku Lifecycle Password"), "owned password dialog")
            x.focus(x.window("Suzaku Lifecycle Password"))
            qa.wait(lambda: qa.watch.latest["private"], "password input boundary")
            popup.expect_hidden()
            start = len(qa.watch.frames)
            assert qa.command(f'A{old["host"]} {old["revision"]} K0') == b"0"
            x.type("synthetic42")
            x.key(qa.IBus.KEY_Return)
            qa.wait(lambda: hidden.poll() is not None, "password dialog result")
            assert hidden.returncode == 0
        assert (qa.root / "synthetic-password.txt").read_text() == "synthetic42\n"
        assert all(not frame["seed"] and not frame["candidates"] for frame in qa.watch.frames[start:])
        popup.expect_hidden()
        x.focus(window)
        qa.settle_input(bus, x, window)
        qa.save_document(x, document, "")
        fresh_input()
        passed_case("password focus hides actual popup, rejects old sends and restores public input", state)

    companion = None
    for state in ["draft", "adopted", "compose"]:
        old = setup("en", state)
        for _ in range(2):
            companion = qa.spawn([str(qa.bins / "panel")])
            qa.wait(lambda: any("Suzaku XR Candidate Panel" in title and
                    "draft: " + old["seed"] + " |" in title for _, title in x.windows()),
                    "new companion receives current draft without another key", timeout=30)
            assert x.focused() == window and qa.watch.latest == old
            qa.save_document(x, document, "")
            companion.terminate()
            companion.wait(timeout=3)
            qa.wait(lambda: x.window("Suzaku XR Candidate Panel") is None, "owned companion closed")
        x.key(qa.IBus.KEY_Escape)
        if state == "compose":
            x.key(qa.IBus.KEY_Escape)
        qa.wait(lambda: qa.seed_is(""), "companion restarts do not prevent draft cancellation")
        popup.expect_hidden()
        fresh_input()
        passed_case("companion process restart preserves current target and draft", state)

    companion = qa.spawn([str(qa.bins / "panel")])
    for state in ["draft", "adopted", "compose"]:
        old = setup("en", state)
        qa.wait(lambda: any("Suzaku XR Candidate Panel" in title and
                "draft: " + old["seed"] + " |" in title for _, title in x.windows()),
                "companion mirrors draft before host exit", timeout=30)
        qa.watch.sock.close()
        qa.watch = None
        host.terminate()
        host.wait(timeout=3)
        popup.expect_hidden()
        qa.save_document(x, document, "")
        host = qa.spawn([str(qa.bins / "linux_ime_host")])

        def ready():
            try:
                return qa.command("Q") == b"1"
            except OSError:
                return False

        qa.wait(ready, "replacement host ready")
        qa.watch = qa.Watch()
        qa.wait(lambda: qa.watch.latest is not None, "fresh subscription after host restart")
        assert qa.watch.latest["host"] != old["host"]
        assert bus.set_global_engine("dev.suzaku.linux.ime")
        qa.wait(lambda: qa.seed_is(""), "host reactivation does not replay the crashed draft")
        x.type("fresh")
        qa.wait(lambda: qa.seed_is("fresh"), "real application reconnected to new host")
        popup.expect_page()
        qa.wait(lambda: any("Suzaku XR Candidate Panel" in title and "draft: fresh |" in title
                for _, title in x.windows()), "running companion reconnects without restart")
        fresh = qa.watch.latest.copy()
        forged = f'A{old["host"]} {fresh["revision"]} K0'
        assert qa.command(forged) == b"0", "an earlier host token was accepted"
        qa.save_document(x, document, "")
        assert qa.watch.latest == fresh and x.focused() == window
        x.key(qa.IBus.KEY_Return)
        qa.wait(lambda: qa.seed_is(""), "new host commits once")
        popup.expect_hidden()
        qa.save_document(x, document, "fresh\n")
        assert companion.poll() is None
        passed_case("host restart reconnects app and companion without stale draft replay", state)

    assert passed == 21
    print("RESULT: 21 strict real-application lifecycle checks passed")
except Exception:
    qa.diagnose()
    raise
finally:
    if popup is not None:
        popup.close()
    qa.close()
