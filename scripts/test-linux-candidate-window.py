#!/usr/bin/env python3
"""Stock IBus GTK popup -> XTest clicks -> saved synthetic GTK document.

Accessibility is observation-only and scoped to the owned panel PID. Neither
Atspi actions nor direct Engine.CandidateClicked calls substitute for clicks.
"""
import importlib.util
import json
import os
from pathlib import Path
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)  # Validate isolation before importing accessibility.
assert os.environ["SUZAKU_APP_QA_SUITE"] in {"popup", "lifecycle", "bus-restart"}
os.environ["NO_AT_BRIDGE"] = "0"
os.environ.pop("GTK_A11Y", None)
os.environ["GTK_MODULES"] = "atk-bridge"
qa.gi.require_version("Atspi", "2.0")
qa.gi.require_version("Gdk", "3.0")
from gi.repository import Atspi, Gdk, Gio

passed = 0
ORDINALS = "¹²³⁴⁵⁶"


def row_text(frame, index):
    return ORDINALS[index % 6] + " " + frame["candidates"][index]["ibus_label"]


def passed_case(*parts):
    global passed
    passed += 1
    print("PASS:", *parts)


def nodes(node):
    pending = [node]
    seen = 0
    while pending:
        current = pending.pop()
        # A GTK child may disappear between GetChildCount and GetChildAtIndex
        # while the popup hides/rebuilds. Do not turn that null child into a
        # test crash; expected visible controls still have to appear in time.
        if current is None:
            continue
        seen += 1
        assert seen < 1000, "unexpectedly large fixture accessibility tree"
        yield current
        pending.extend(current.get_child_at_index(i) for i in range(current.get_child_count()))


def panel_app(pid):
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is not None and app.get_process_id() == pid:
            return app
    return None


class Popup:
    def __init__(self, panel, x, bus, screen):
        qa.wait(lambda: panel_app(panel.pid), "stock candidate panel accessibility", timeout=15)
        self.app = panel_app(panel.pid)
        self.panel = panel
        self.x, self.screen = x, screen
        self.clicks = []
        self.connection = bus.get_connection()

        def clicked(_connection, _sender, _path, _interface, _signal, params, *_args):
            self.clicks.append(params.unpack())

        self.subscription = self.connection.signal_subscribe(
            "org.freedesktop.IBus.Panel", "org.freedesktop.IBus.Panel", "CandidateClicked",
            None, None, Gio.DBusSignalFlags.NONE, clicked)

    def visible(self):
        assert self.panel.poll() is None, "stock candidate panel exited unexpectedly"
        return [node for node in nodes(self.app)
                if node.get_state_set().contains(Atspi.StateType.SHOWING)]

    def named(self, name):
        return [node for node in self.visible() if node.get_name() == name]

    def click(self, name, button=1, character=None):
        qa.wait(lambda: self.named(name), "visible popup control: " + repr(name))
        matches = self.named(name)
        assert len(matches) == 1, (name, len(matches))
        bounds = matches[0].get_component_iface().get_extents(Atspi.CoordType.SCREEN)
        assert bounds.width > 0 and bounds.height > 0
        assert (0 <= bounds.x < bounds.x + bounds.width <= self.screen.get_width() and
                0 <= bounds.y < bounds.y + bounds.height <= self.screen.get_height()), \
            "popup control is clipped"
        if character is not None:
            # Observe the actual glyph's bounds, not the centre of the whole
            # candidate row. Input still goes through real XTest mouse events.
            glyph = matches[0].get_text_iface().get_character_extents(character, Atspi.CoordType.SCREEN)
            assert glyph.width > 0 and glyph.height > 0
            assert (bounds.x <= glyph.x < glyph.x + glyph.width <= bounds.x + bounds.width and
                    bounds.y <= glyph.y < glyph.y + glyph.height <= bounds.y + bounds.height)
            bounds = glyph
        self.x.click(bounds.x + bounds.width // 2, bounds.y + bounds.height // 2, button)

    def expect_page(self):
        frame = qa.watch.latest
        page = frame["selected"] // 6 * 6
        expected = [row_text(frame, i) for i in range(page, min(page + 6, len(frame["candidates"])))]
        qa.wait(lambda: all(self.named(name) for name in expected), "actual popup candidate page")
        assert expected
        assert not any(self.named(key) for key in ORDINALS), "duplicate standalone ordinal column"
        self.expect_bounds()

    def expect_bounds(self):
        windows = [node for node in self.visible() if node.get_role() == Atspi.Role.WINDOW]
        assert len(windows) == 1, "one stock candidate window must be showing"
        bounds = windows[0].get_component_iface().get_extents(Atspi.CoordType.SCREEN)
        assert (0 <= bounds.x < bounds.x + bounds.width <= self.screen.get_width() and
                0 <= bounds.y < bounds.y + bounds.height <= self.screen.get_height()), \
            ("candidate popup extends beyond screen", bounds.x, bounds.y, bounds.width, bounds.height)
        return bounds

    def expect_hidden(self):
        qa.wait(lambda: not any(node.get_role() == Atspi.Role.WINDOW for node in self.visible()),
                "finished draft dismisses the actual popup")

    def close(self):
        self.connection.signal_unsubscribe(self.subscription)


def check_popup(bus, x, popup, window, document):
    def draft(seed, page=0):
        assert qa.seed_is("") and not document.read_text()
        x.type(seed)
        qa.wait(lambda: qa.seed_is(seed), "physical keys start popup draft")
        popup.expect_page()
        if page:
            assert len(qa.watch.latest["candidates"]) > 6
            popup.click("Down")
            qa.wait(lambda: qa.watch.latest["selected"] == 6, "actual next-page button")
            popup.expect_page()
        assert x.focused() == window
        return qa.watch.latest

    def finish(expected):
        qa.wait(lambda: qa.seed_is(""), "candidate commit clears draft")
        popup.expect_hidden()
        assert x.focused() == window, "native candidate control stole application focus"
        qa.save_document(x, document, expected + "\n")
        qa.clear_document(x, document)

    def click_candidate(frame, slot, button=1, ordinal=False):
        index = frame["selected"] // 6 * 6 + slot
        candidate = frame["candidates"][index]
        before = len(popup.clicks)
        popup.click(row_text(frame, index), button, character=0 if ordinal else None)
        qa.wait(lambda: len(popup.clicks) == before + 1, "stock panel click signal observed")
        assert popup.clicks[-1][:2] == (slot, button), popup.clicks[-1]
        return candidate["text"]

    for language, seed in [("en", "hel"), ("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(qa.command("L" + language))["ok"]
        for slot in [0, 1]:
            frame = draft(seed)
            finish(click_candidate(frame, slot))
            passed_case("stock candidate body click and exact commit", language, slot)

    assert json.loads(qa.command("Len"))["ok"]
    for page in [0, 1]:
        for slot in [0, 2]:
            frame = draft("hel", page)
            finish(click_candidate(frame, slot))
            passed_case("page-local candidate click", page, slot)

    frame = draft("hel")
    for button, expected in [(5, 1), (4, 0)]:
        popup.click(row_text(frame, 0), button)
        qa.wait(lambda: qa.watch.latest["selected"] == expected, "physical scroll moves selection")
        assert qa.seed_is("hel")
    for control, expected in [("Down", 6), ("Down", len(frame["candidates"]) - 1),
                              ("Down", len(frame["candidates"]) - 1),
                              ("Up", len(frame["candidates"]) - 7), ("Up", 0), ("Up", 0)]:
        popup.click(control)
        qa.wait(lambda: qa.watch.latest["selected"] == expected, "bounded popup page navigation")
        popup.expect_page()
        assert qa.seed_is("hel") and x.focused() == window
    qa.save_document(x, document, "")
    x.key(qa.IBus.KEY_Escape)
    qa.wait(lambda: qa.seed_is(""), "cancel navigation draft")
    popup.expect_hidden()
    passed_case("scroll, next/previous and first/last-page boundaries without committing")

    for page in [0, 1]:
        for button in [2, 3]:
            frame = draft("hel", page)
            click_candidate(frame, 1, button)
            qa.save_document(x, document, "")
            assert qa.watch.latest == frame, "secondary click changed native composition"
            x.key(qa.IBus.KEY_Escape)
            qa.wait(lambda: qa.seed_is(""), "cancel secondary-click draft")
            popup.expect_hidden()
            passed_case("secondary button does not commit", page, button)

    for key in [qa.IBus.KEY_Escape, qa.IBus.KEY_BackSpace]:
        draft("x")
        x.key(key)
        qa.wait(lambda: qa.seed_is(""), "keyboard cancellation clears draft")
        popup.expect_hidden()
        qa.save_document(x, document, "")
        passed_case("actual popup dismissal", key)

    long_cases = [("W" * 160, "…" + "W" * 48),
                  ("界" * 160, "…" + "界" * 24),
                  ("😀" * 160, "…" + "😀" * 24)]
    for position in [(0, 0), (250, 400)]:
        # Only the owned editor moves. Its caret approaches the bottom/right
        # edge; the stock popup must reposition entirely inside the screen.
        x.x.XMoveWindow(x.display, window, *position)
        x.x.XSync(x.display, 0)
        for seed, preview in long_cases:
            for mode in ["enter", "click"]:
                frame = qa.watch.latest
                assert qa.command(f'A{frame["host"]} {frame["revision"]} T{seed}') == b"1"
                qa.wait(lambda: qa.seed_is(seed), "version-bound long draft replacement")
                qa.wait(lambda: any(node.get_name().startswith("Suzaku · " + preview + "\n")
                                    for node in popup.visible()), "width-bounded preview label")
                popup.expect_page()
                bounds = popup.expect_bounds()
                assert qa.watch.latest["candidates"][0]["text"] == seed
                if mode == "enter":
                    x.key(qa.IBus.KEY_Return)
                else:
                    assert click_candidate(qa.watch.latest, 0) == seed
                finish(seed)
                passed_case("800x600 popup stays on-screen and commits full draft", seed[0], mode,
                            position, (bounds.width, bounds.height))
    assert passed == 29

    # Ordinal clicks are now mandatory, not a separately enabled known failure.
    # Exercise every slot of a full first page and a partial second page.
    for page, slots in [(0, range(6)), (1, range(3))]:
        for slot in slots:
            frame = draft("hel", page)
            assert len(frame["candidates"]) == 9, "fixture must exercise a partial final page"
            finish(click_candidate(frame, slot, ordinal=True))
            passed_case("actual superscript glyph click commits the correct page-local candidate", page, slot)

    for language, seed in [("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(qa.command("L" + language))["ok"]
        for slot in [0, 1]:
            frame = draft(seed)
            finish(click_candidate(frame, slot, ordinal=True))
            passed_case("superscript glyph click preserves exact multilingual text", language, slot)

    assert json.loads(qa.command("Len"))["ok"]
    for page in [0, 1]:
        for button in [2, 3]:
            frame = draft("hel", page)
            click_candidate(frame, 1, button, ordinal=True)
            qa.save_document(x, document, "")
            assert qa.watch.latest == frame, "secondary ordinal click changed native composition"
            x.key(qa.IBus.KEY_Escape)
            qa.wait(lambda: qa.seed_is(""), "cancel secondary ordinal click draft")
            popup.expect_hidden()
            passed_case("secondary ordinal button does not select or commit", page, button)

    assert passed == 46
    print("RESULT: 46 strict stock-popup workflow checks passed, including 17 ordinal-glyph checks")


if __name__ == "__main__":
    popup = None
    try:
        bus, x = qa.start()
        Gdk.init([])
        screen = Gdk.get_default_root_window()
        assert (screen.get_width(), screen.get_height()) == (800, 600)
        panel = qa.spawn(["/usr/libexec/ibus-ui-gtk3"])
        popup = Popup(panel, x, bus, screen)
        document = qa.root / "suzaku-popup-qa.txt"
        document.write_text("")
        editor = qa.spawn(["gnome-text-editor", "--standalone", str(document)])
        qa.wait(lambda: x.window(document.name), "candidate QA editor")
        window = x.window(document.name)
        x.focus(window)
        x.click(200, 100)
        assert bus.set_global_engine("dev.suzaku.linux.ime")
        qa.settle_input(bus, x, window)
        qa.prepare_editor(x, document)
        check_popup(bus, x, popup, window, document)
    except Exception:
        qa.diagnose()
        raise
    finally:
        if popup is not None:
            popup.close()
        qa.close()
