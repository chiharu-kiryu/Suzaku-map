#!/usr/bin/env python3
"""Real native candidate pages and caret placement, only on owned Xvfb/IBus.

Rendered-frame logs and the host subscription are observation-only. Every draft,
adoption, navigation and candidate commit goes through XTest -> GTK/IBus or the
actual drawn companion controls. This is not the stock IBus popup suite.
"""
from collections import deque
import ctypes as C
import importlib.util
import json
from pathlib import Path
import sys
import time

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "suzaku_app_qa", Path(__file__).with_name("test-linux-apps.py"))
qa = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qa)  # Validate the private display, bus and config first.
suite = qa.os.environ["SUZAKU_APP_QA_SUITE"]
assert suite in ("candidates", "bottom-layout", "auto-layout")
auto_layout = suite == "auto-layout"
bottom_layout = suite in ("bottom-layout", "auto-layout")
if auto_layout:
    assert qa.os.environ.get("XDG_CURRENT_DESKTOP") == "Phosh"
    assert qa.os.environ.get("XDG_SESSION_DESKTOP") == "phosh"
assert qa.os.environ.get("GTK_IM_MODULE") == "ibus"
IBus = qa.IBus
bus = x = window = view = xid = companion = None
insertions = []
events = deque(maxlen=24)
committed = ""
passed = 0
last_action = None
screen_size = (1024, 768)
original_follow_width = None


def prepare_candidate_budget_pack():
    """Load authored data only into the fresh fixture, before either process starts."""
    data_home = qa.root / "data"
    assert qa.os.environ["XDG_DATA_HOME"] == str(data_home)
    assert not qa.os.environ.get("SUZAKU_LEXICON_DIR")
    assert not data_home.is_symlink() and data_home.resolve() == qa.root.resolve() / "data"
    assert not (data_home / "suzaku" / "lexicons").exists()
    assert not qa.processes, "the startup vocabulary snapshot must not already be frozen"
    identifier = "org.suzaku.test.native-candidate-budget"
    readings = [
        {"reading": reading, "text": body + suffix, "kind": "word"}
        for reading, body in [("ce'shi", "字" * 3000), ("shi'yan", "文" * 2000),
                              ("jian'yan", '"\\' * 1800)]
        for suffix in "甲乙丙丁戊己庚辛"
    ]
    package = {
        "format_version": 1,
        "manifest": {"id": identifier, "version": "1.0.0", "name": "Native candidate budget QA",
                     "description": "Valid synthetic long candidates, private display only.",
                     "language": "zh-Hans", "topics": ["testing"], "license": "MIT",
                     "authors": ["Suzaku tests"]},
        "lexicon": {"format_version": 1, "language": "zh-Hans", "readings": readings},
    }
    raw = json.dumps(package, ensure_ascii=False, separators=(",", ":"))
    assert len(raw.encode("utf-8")) <= 256 * 1024
    destination = qa.root / "native-candidate-budget.json"
    with destination.open("x", encoding="utf-8") as stream:
        stream.write(raw)
    tool = qa.bins / "suzaku_tool"
    for operation in ("validate", "install"):
        qa.subprocess.run([str(tool), "pack", operation, str(destination)], check=True,
                          capture_output=True, text=True, timeout=10)
    result = qa.subprocess.run([str(tool), "pack", "list"], check=True,
                               capture_output=True, text=True, timeout=10)
    report = json.loads(result.stdout)
    assert not report["next_startup"]["errors"]
    assert report["next_startup"]["loaded"] == [identifier]
    assert [p["manifest"]["id"] for p in report["packages"] if p["enabled"]] == [identifier]
    print("READY: valid long-candidate pack installed only in the private QA data directory", flush=True)


def window_frame(kind):
    """Observe this fixture's live panel process, including its actual settings window."""
    assert companion is not None and companion.poll() is None
    index = qa.processes.index(companion)
    lines = (qa.root / f"process-{index}.log").read_text().splitlines()
    for offset, line in enumerate(reversed(lines)):
        if line.startswith("Suzaku frame: "):
            try:
                frame = json.loads(line.removeprefix("Suzaku frame: "))
            except json.JSONDecodeError:
                continue
            if frame.get("window") == kind:
                frame["_qa_log_line"] = len(lines) - offset
                return frame
    return {}


def control_rect(frame, kind):
    return next((item["rect"] for item in frame.get("controls", [])
                 if item["kind"] == kind), None)


def panel_window_id(kind):
    titles = ("Suzaku · Panel Settings",) if kind == "Settings" else ("Suzaku · Input", "Suzaku")
    matches = [window for window, title in x.windows() if title in titles]
    assert len(matches) <= 1, (kind, x.windows())
    return matches[0] if matches else None


def panel_window_mapped(kind):
    window_id = panel_window_id(kind)
    return window_id is not None and "Map State: IsViewable" in qa.subprocess.run(
        ["xwininfo", "-id", str(window_id)], capture_output=True, text=True,
        timeout=2).stdout


def show_owned_main():
    if panel_window_mapped("Main"):
        return
    assert companion.poll() is None and seed_is("")
    # Exercise the real launcher/single-instance show path. A previous rendered
    # frame alone never authorizes clicking coordinates of a hidden window.
    existing_pid = companion.pid
    previous_frame_line = window_frame("Main").get("_qa_log_line", 0)
    launcher = qa.spawn([str(qa.bins / "panel")])
    wait(lambda: launcher.poll() is not None,
         "second real launch forwards show to the owned companion")
    assert launcher.returncode == 0, "single-instance show launcher failed"
    wait(lambda: panel_window_mapped("Main"), "single-instance show maps the owned panel")
    wait(lambda: window_frame("Main").get("_qa_log_line", 0) > previous_frame_line,
         "shown main panel presents a fresh frame before its controls are used")
    assert companion.pid == existing_pid and companion.poll() is None
    assert owned_focus() and seed_is("")


def click_control(kind, window_kind="Main"):
    global last_action
    wait(lambda: control_rect(window_frame(window_kind), kind),
         f"actual {window_kind} control is presented: {kind}")
    assert panel_window_mapped(window_kind), \
        ("refusing a historical hit target from a hidden panel", window_kind, kind)
    if window_kind == "Main":
        assert owned_focus(), "main companion controls must not steal input focus"
    else:
        settings_xid = x.window("Suzaku · Panel Settings")
        assert settings_xid is not None and x.focused_within(settings_xid)
    frame = window_frame(window_kind)
    point = rect_center(frame, control_rect(frame, kind))
    last_action = {"control": kind, "window": window_kind, "pointer": point}
    assert x.xt.XTestFakeMotionEvent(x.display, -1, *point, 0)
    assert x.xt.XTestFakeButtonEvent(x.display, 1, 1, 0)
    assert x.xt.XTestFakeButtonEvent(x.display, 1, 0, 0)
    x.x.XSync(x.display, 0)
    pump()


def persisted_layout():
    path = qa.root / "config" / "suzaku-panel" / "panel-settings.toml"
    if not path.exists():
        return None
    values = dict(line.split("=", 1) for line in path.read_text().splitlines()
                  if "=" in line)
    return values.get("panel_layout_mode")


def set_layout_from_settings(mode):
    assert seed_is("") and buffer_text() == committed == ""
    show_owned_main()
    detected = window_frame("Main").get("detected_layout_mode")
    assert detected in ("follow-caret", "bottom-dock")
    effective = detected if mode == "auto" else mode
    old_settings_line = window_frame("Settings").get("_qa_log_line", 0)
    click_control("SettingsToggle")
    wait(lambda: panel_window_mapped("Settings") and
         window_frame("Settings").get("_qa_log_line", 0) > old_settings_line,
         "owned settings window is mapped and presents a fresh frame")
    settings_xid = x.window("Suzaku · Panel Settings")
    x.focus(settings_xid)
    choice = {"auto": "Auto", "bottom-dock": "BottomDock", "follow-caret": "FollowCaret"}[mode]
    previous_frame_line = window_frame("Settings").get("_qa_log_line", 0)
    click_control(f"SetPanelLayoutMode({choice})", "Settings")
    wait(lambda: window_frame("Settings").get("_qa_log_line", 0) > previous_frame_line and
         window_frame("Settings").get("layout_mode") == effective and
         window_frame("Settings").get("layout_preference") == mode and
         window_frame("Settings").get("detected_layout_mode") == detected and
         persisted_layout() == mode, "real settings click applied and persisted layout")
    click_control("SettingsToggle", "Settings")
    wait(lambda: "Map State: IsViewable" not in qa.subprocess.run(
        ["xwininfo", "-id", str(settings_xid)], capture_output=True, text=True,
        timeout=2).stdout, "settings close control hides its window")
    x.focus(xid)
    view.grab_focus()
    wait(lambda: owned_focus() and seed_is("") and
         window_frame("Main").get("layout_mode") == effective and
         window_frame("Main").get("layout_preference") == mode and
         window_frame("Main").get("detected_layout_mode") == detected,
         "original empty TextView focus and selected layout restored")
    stable(lambda: seed_is(""), "settings interaction unexpectedly created a draft")


def pump():
    context = qa.GLib.MainContext.default()
    deadline = time.monotonic() + .01
    for _ in range(128):
        if not context.iteration(False) or time.monotonic() >= deadline:
            break
    if qa.watch is not None:
        qa.watch.drain()


def wait(check, label, timeout=4):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pump()
        if check():
            return
        time.sleep(.003)
    raise AssertionError(label)


def buffer_text():
    buffer = view.get_buffer()
    return buffer.get_text(buffer.get_start_iter(), buffer.get_end_iter(), True)


def owned_focus():
    return (window.get_visible() and x.focused_within(xid) and
            view.has_focus() and view.is_focus())


def seed_is(text):
    frame = qa.watch.latest
    return (frame is not None and frame["focused"] and not frame["private"] and
            frame["seed"] == text)


def expect_unchanged_buffer():
    assert owned_focus(), "native controls stole the owned TextView's focus"
    assert buffer_text() == committed, "draft/navigation unexpectedly changed the GTK buffer"


def stable(check, label, duration=.15):
    deadline = time.monotonic() + duration
    while time.monotonic() < deadline:
        pump()
        assert check(), label
        expect_unchanged_buffer()
        time.sleep(.003)


def key(symbol, expect=None):
    global last_action
    assert owned_focus(), "physical key target is not the owned GTK TextView"
    code = x.x.XKeysymToKeycode(x.display, symbol)
    assert code, "keysym missing from the private keyboard"
    last_action = {"key": symbol, "hardwarecode": code, "expected_seed": expect}
    keys = C.create_string_buffer(32)
    x.x.XQueryKeymap(x.display, keys)
    assert not any(keys.raw), "a previous physical key is still held"
    try:
        assert x.xt.XTestFakeKeyEvent(x.display, code, 1, 0)
        x.x.XSync(x.display, 0)
        deadline = time.monotonic() + .015
        while time.monotonic() < deadline:
            pump()
            time.sleep(.002)
    finally:
        released = x.xt.XTestFakeKeyEvent(x.display, code, 0, 0)
        x.x.XSync(x.display, 0)
        x.x.XQueryKeymap(x.display, keys)
        assert released and not any(keys.raw), "physical key release failed"
    if expect is not None:
        wait(lambda: seed_is(expect), "one physical key produced the exact draft")
    assert owned_focus(), "physical key processing changed application focus"


def type_text(text):
    expected = qa.watch.latest["seed"]
    for character in text:
        assert character.isascii() and (character.islower() or character == " ")
        expected += character
        key(ord(character), expect=expected)
        expect_unchanged_buffer()


def language(code):
    assert seed_is("")
    if qa.watch.latest["language"] != code:
        revision = qa.watch.latest["revision"]
        assert json.loads(qa.command("L" + code))["ok"]
        wait(lambda: qa.watch.latest["revision"] > revision and
             qa.watch.latest["language"] == code and seed_is(""), "language applied")


def clear_buffer():
    global committed
    assert seed_is("")
    view.get_buffer().set_text("")
    committed = ""
    insertions.clear()
    expect_unchanged_buffer()


def page_ready(start=None):
    frame = qa.companion_frame()
    page = frame.get("native_page")
    host = qa.watch.latest
    if (not page or page["busy"] or frame.get("native_revision") != host["revision"] or
            frame.get("native_context") != host["context"] or
            page["selected"] != host["selected"] or frame.get("seed") != host["seed"]):
        return False
    # RenderScene.draft_text is the selected candidate preview, not the raw
    # editable spelling shown by chrome.seed_text. Check both independently.
    expected_preview = host["candidates"][host["selected"]]["text"] if host["candidates"] else ""
    if frame.get("draft") != expected_preview:
        return False
    if start is not None and page["start"] != start:
        return False
    return frame if frame.get("position") is not None and frame.get("size") else False


def rect_center(frame, rect):
    assert len(rect) == 4
    left, top, width, height = rect
    assert width > 0 and height > 0
    assert (0 <= left < left + width <= frame["size"][0] + 1 and
            0 <= top < top + height <= frame["size"][1] + 1), \
        ("drawn control outside panel", rect, frame["size"])
    point = (round(frame["position"][0] + left + width / 2),
             round(frame["position"][1] + top + height / 2))
    assert 0 <= point[0] < screen_size[0] and 0 <= point[1] < screen_size[1]
    return point


def expect_bounds(frame):
    left, top = frame["position"]
    width, height = frame["size"]
    assert (0 <= left < left + width <= screen_size[0] and
            0 <= top < top + height <= screen_size[1]), \
        ("native panel outside private screen", frame["position"], frame["size"])


def capture_owned_display(name):
    # qa.start validated that this display belongs to the fresh Xvfb fixture.
    # No personal desktop or real input data is present on it.
    pixels = Gdk.pixbuf_get_from_window(Gdk.get_default_root_window(), 0, 0, *screen_size)
    assert pixels is not None
    pixels.savev(str(qa.root / (name + ".png")), "png", [], [])


def page(start=None):
    wait(lambda: page_ready(start), "current native candidate page was actually presented")
    frame = page_ready(start)
    host = qa.watch.latest
    current = frame["native_page"]
    assert current["total"] == len(host["candidates"])
    assert current["start"] == host["selected"] // 6 * 6
    expected_indices = list(range(current["start"], min(current["start"] + 6, current["total"])))
    assert [item["index"] for item in current["candidates"]] == expected_indices
    assert [item["number"] for item in current["candidates"]] == list(range(1, len(expected_indices) + 1))
    for item in current["candidates"]:
        assert item["text"] == host["candidates"][item["index"]]["text"]
        rect_center(frame, item["rect"])
    assert (current["previous"] is not None) == (current["start"] > 0)
    assert (current["next"] is not None) == (current["start"] + 6 < current["total"])
    for name in ("previous", "next"):
        if current[name] is not None:
            rect_center(frame, current[name])
    expect_bounds(frame)
    expect_unchanged_buffer()
    return frame


def pointer(frame, rect, button=1, down=True, up=True):
    global last_action
    assert owned_focus() and companion.poll() is None
    point = rect_center(frame, rect)
    last_action = {"pointer": point, "button": button, "down": down, "up": up,
                   "revision": frame["native_revision"], "page": frame["native_page"]["start"]}
    assert x.xt.XTestFakeMotionEvent(x.display, -1, *point, 0)
    if down:
        assert x.xt.XTestFakeButtonEvent(x.display, button, 1, 0)
    if up:
        assert x.xt.XTestFakeButtonEvent(x.display, button, 0, 0)
    x.x.XSync(x.display, 0)
    pump()


def draft(reading="hel", start=0):
    assert seed_is("") and buffer_text() == ""
    type_text(reading)
    frame = page(0)
    if reading == "hel":
        assert 6 < frame["native_page"]["total"] < 12, \
            "English fixture must exercise a full first page and partial final page"
    if start:
        key(IBus.KEY_Page_Down)
        frame = page(start)
    return frame


def cancel():
    key(IBus.KEY_Escape, expect="")
    stable(lambda: seed_is(""), "cancelled candidate draft reappeared")


def finish(text):
    global committed
    assert committed == "", "each click workflow starts from an empty owned buffer"
    committed += text
    wait(lambda: seed_is("") and buffer_text() == committed,
         "one native card click committed its exact full candidate")
    stable(lambda: seed_is("") and buffer_text() == committed,
           "native card click duplicated its commit or inserted a newline")
    assert insertions == [text], ("expected exactly one GTK insertion", insertions)
    clear_buffer()


def passed_case(label):
    global passed
    passed += 1
    print("PASS:", label, flush=True)


def candidate_budget_checks():
    language("zh-Hans")
    for reading in ("ceshi", "shiyan", "jianyan"):
        frame = draft(reading)
        host = qa.watch.latest
        assert host["focused"] and not host["private"] and host["seed"] == reading
        assert any(item["text"] == reading for item in host["candidates"])
        assert all(len(item[field].encode("utf-8")) <= 8192
                   for item in host["candidates"] for field in ("text", "label"))
        encoded = json.dumps(host, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        assert len(encoded) + 1 <= 65536, "full native frame, including newline, exceeds its budget"
        cards = frame["native_page"]["candidates"]
        if reading == "ceshi":
            candidate = next(item for item in cards if item["text"] == "测试")
            assert not any(item["text"].startswith("字" * 3000) for item in host["candidates"])
        else:
            candidate = next(item for item in cards if len(item["text"].encode("utf-8")) > 3000)
        text = candidate["text"]
        assert host["candidates"][candidate["index"]]["text"] == text
        def geometry(presented):
            return (presented["size"], presented["position"],
                    [(item["index"], item["rect"]) for item in presented["native_page"]["candidates"]])
        original_geometry = geometry(frame)
        key(IBus.KEY_1 + candidate["number"] - 1, expect=text)
        expect_unchanged_buffer()
        page(0)
        key(IBus.KEY_BackSpace, expect=reading)
        expect_unchanged_buffer()
        # A one-card adopted draft can shrink the window. Undo first presents
        # the restored list in that old height, then receives its resize ack.
        # Wait for the original full-page geometry, not merely the new revision;
        # a compressed interim card is not the target of this commit check.
        wait(lambda: (current := page_ready(0)) and geometry(current) == original_geometry,
             "undo restores the full pre-adoption candidate page geometry")
        restored = page(0)
        assert geometry(restored) == original_geometry
        candidate = next(item for item in restored["native_page"]["candidates"] if item["text"] == text)
        assert qa.watch.latest["candidates"][candidate["index"]]["text"] == text
        pointer(restored, candidate["rect"])
        finish(text)
        passed_case(f"valid long pack keeps native presentation and exact adopt/undo/click indices: {reading}")


def composition_identity(frame):
    # Geometry is independent from composition revision/selection/candidates.
    return {key: value for key, value in frame.items() if key != "cursor"}


def relocate_owned_input(position):
    assert owned_focus()
    old = dict(qa.watch.latest)
    previous_cursor = old.get("cursor")
    assert previous_cursor is not None, "GTK did not report an absolute caret rectangle"
    x.x.XMoveWindow(x.display, xid, *position)
    x.x.XSync(x.display, 0)
    # IBus GTK3 deduplicates the widget-relative rectangle, so translating only
    # the toplevel is not guaranteed to publish new absolute coordinates. Move
    # the real TextView caret via layout too (not a fake companion/IBus frame).
    # This gate covers reported-caret following, not a missing client signal.
    view.set_left_margin(view.get_left_margin() + 4)
    wait(lambda: qa.watch.latest.get("cursor") is not None and
         qa.watch.latest["cursor"] != previous_cursor, "relayout of owned input updates the host caret")
    cursor = qa.watch.latest["cursor"]
    assert cursor["x"] == position[0] + view.get_left_margin()
    assert cursor["y"] == position[1] + view.get_top_margin()
    assert composition_identity(qa.watch.latest) == composition_identity(old), \
        "geometry-only update changed composition, selection or generation"
    expect_unchanged_buffer()
    return old


def geometry_checks():
    draft()
    first = page(0)
    cursor = qa.watch.latest["cursor"]
    assert first["position"][1] >= cursor["y"] + cursor["height"], \
        "panel with ample space should appear below the input caret"
    before = composition_identity(qa.watch.latest)
    for position in [(200, 100), (screen_size[0] - 120, screen_size[1] - 90)]:
        old_position = page()["position"]
        relocate_owned_input(position)
        wait(lambda: page_ready() and page_ready()["position"] != old_position,
             "native panel follows the moved GTK caret without another key")
        current = page()
        assert composition_identity(qa.watch.latest) == before
        stable(lambda: composition_identity(qa.watch.latest) == before,
               "caret motion mutated the stable draft")
        cursor = qa.watch.latest["cursor"]
        if position[1] > screen_size[1] / 2:
            assert current["position"][1] + current["size"][1] <= cursor["y"], \
                "native panel should flip above a caret near the lower edge"
            assert current["position"][0] < cursor["x"], \
                "native panel should clamp left near the right screen edge"
            capture_owned_display("native-above-right-edge")
        passed_case("caret-only follow preserves focus/draft/revision and screen bounds: " + str(position))
    key(IBus.KEY_Page_Down)
    tail = page(6)
    cursor = qa.watch.latest["cursor"]
    assert tail["position"][1] + tail["size"][1] <= cursor["y"], \
        "short final page jumped below the lower-edge caret"
    key(IBus.KEY_Page_Up)
    page(0)
    passed_case("full and partial native pages retain safe upper placement at the lower-right edge")
    relocate_owned_input((60, 40))
    wait(lambda: page_ready() and page_ready()["position"][1] < 300,
         "native panel follows the caret back from the screen edge")
    cancel()


def bottom_anchor(frame):
    expect_bounds(frame)
    assert frame.get("layout_mode") == "bottom-dock"
    assert frame.get("layout_preference") == ("auto" if auto_layout else "bottom-dock")
    if auto_layout:
        assert frame.get("detected_layout_mode") == "bottom-dock"
    left, top = frame["position"]
    width, height = frame["size"]
    assert width >= screen_size[0] - 26, ("bottom panel did not widen", frame)
    assert abs(left + width / 2 - screen_size[0] / 2) <= 1, \
        ("bottom panel not horizontally centered", frame["position"], frame["size"])
    assert abs(top + height - (screen_size[1] - 12)) <= 1, \
        ("bottom panel lost its bottom anchor", frame["position"], frame["size"])
    return left, top + height, width


def wait_bottom(expanded=None):
    def ready_bottom():
        frame = page_ready()
        if not frame or frame.get("layout_mode") != "bottom-dock":
            return False
        if expanded is not None and frame.get("input_expanded") != expanded:
            return False
        left, top = frame["position"]
        width, height = frame["size"]
        return (width >= screen_size[0] - 26 and
                abs(left + width / 2 - screen_size[0] / 2) <= 1 and
                abs(top + height - (screen_size[1] - 12)) <= 1)
    wait(ready_bottom, "bottom layout resize/position was actually presented")
    frame = page()
    bottom_anchor(frame)
    return frame


def restart_owned_companion(preference, effective, detected):
    global companion
    assert seed_is("") and owned_focus() and buffer_text() == committed == ""
    old_pid = companion.pid
    # Only the exact process returned by qa.spawn is stopped. Every first frame
    # below comes from a fresh process; no historical log or startup key is used.
    companion.terminate()
    companion.wait(timeout=3)
    companion = qa.spawn([str(qa.bins / "panel")])
    wait(lambda: window_frame("Main").get("runtime_font") and
         window_frame("Main").get("layout_preference") == preference and
         window_frame("Main").get("detected_layout_mode") == detected and
         window_frame("Main").get("layout_mode") == effective,
         "fresh private panel reloads preference and resolves current session layout", timeout=30)
    assert companion.pid != old_pid and companion.poll() is None
    wait(lambda: owned_focus() and seed_is(""), "restart preserves the owned input focus")


def prepare_bottom_layout():
    global original_follow_width
    frame = draft()
    original_follow_width = frame["size"][0]
    cancel()
    set_layout_from_settings("bottom-dock")
    draft()
    frame = wait_bottom(expanded=True)
    assert frame["size"][0] > original_follow_width
    cancel()
    passed_case("actual settings controls select and persist bottom layout without a draft/commit")
    restart_owned_companion("bottom-dock", "bottom-dock",
                            window_frame("Main")["detected_layout_mode"])
    draft()
    wait_bottom(expanded=True)
    cancel()
    passed_case("fresh private panel process reloads bottom layout with no synthetic settings write")


def prepare_auto_layout():
    global original_follow_width
    initial = window_frame("Main")
    assert initial.get("layout_preference") == "auto"
    assert initial.get("detected_layout_mode") == "bottom-dock"
    assert initial.get("layout_mode") == "bottom-dock"
    draft()
    wait_bottom(expanded=True)
    capture_owned_display("auto-layout-fresh-phosh-default")
    cancel()
    passed_case("missing fresh settings default to Auto and the production Phosh detector chooses bottom layout")

    set_layout_from_settings("follow-caret")
    draft()
    wait(lambda: page_ready() and page_ready()["size"][0] == 900 and
         page_ready().get("layout_mode") == "follow-caret",
         "manual floating resize is actually presented")
    frame = page()
    original_follow_width = frame["size"][0]
    assert original_follow_width == 900, ("manual floating width", frame["size"])
    assert frame.get("layout_mode") == "follow-caret"
    assert frame.get("layout_preference") == "follow-caret"
    assert frame.get("detected_layout_mode") == "bottom-dock"
    capture_owned_display("auto-layout-manual-follow-override")
    cancel()
    assert persisted_layout() == "follow-caret"
    passed_case("real Follow caret setting overrides mobile detection and preserves its floating width")

    restart_owned_companion("follow-caret", "follow-caret", "bottom-dock")
    draft()
    wait(lambda: page_ready() and page_ready()["size"][0] == original_follow_width,
         "restarted manual floating width is actually presented")
    frame = page()
    assert frame["size"][0] == original_follow_width
    assert frame.get("layout_preference") == "follow-caret"
    assert frame.get("layout_mode") == "follow-caret"
    assert frame.get("detected_layout_mode") == "bottom-dock"
    cancel()
    passed_case("manual Follow caret preference survives a fresh process despite unchanged mobile detection")

    set_layout_from_settings("auto")
    draft()
    frame = wait_bottom(expanded=True)
    assert frame["size"][0] > original_follow_width
    assert persisted_layout() == "auto"
    cancel()
    passed_case("real Auto setting restores mobile bottom layout and saves auto rather than its resolved value")

    restart_owned_companion("auto", "bottom-dock", "bottom-dock")
    draft()
    wait_bottom(expanded=True)
    assert persisted_layout() == "auto"
    cancel()
    passed_case("fresh process reloads Auto and re-detects the mobile bottom layout without fixing the preference")


def bottom_geometry_checks():
    global committed
    draft()
    expanded = wait_bottom(expanded=True)
    keyboard = [item for item in expanded["controls"]
                if item["kind"].startswith("VirtualKeyboardKey(")]
    assert keyboard, "expanded bottom layout has no actual keyboard targets"
    assert max(card["rect"][1] + card["rect"][3]
               for card in expanded["native_page"]["candidates"]) <= \
        min(item["rect"][1] for item in keyboard), \
        "bottom layout candidate cards must appear above the keyboard"
    capture_owned_display("bottom-layout-expanded")
    passed_case("wide centered bottom panel presents all candidate cards above its touch keyboard")

    click_control("VirtualKeyboardKey(Character('l'))")
    wait(lambda: seed_is("hell"), "actual touch keyboard appends once to native host draft")
    wait_bottom(expanded=True)
    expect_unchanged_buffer()
    stable(lambda: seed_is("hell"), "touch key duplicated, committed or replaced the draft")
    cancel()
    draft()
    expanded = wait_bottom(expanded=True)
    passed_case("actual bottom touch key continues the IBus draft without submitting or losing focus")

    candidate = expanded["native_page"]["candidates"][0]
    pointer(expanded, candidate["rect"])
    committed = candidate["text"]
    wait(lambda: seed_is("") and buffer_text() == committed,
         "expanded bottom candidate commits exactly once")
    assert insertions == [committed]
    wait(lambda: window_frame("Main").get("seed") == "" and
         control_rect(window_frame("Main"), "VirtualKeyboardKey(Character('h'))"),
         "expanded keyboard is still presented after the empty committed frame")
    main_xid = x.window("Suzaku · Input")
    assert main_xid is not None and "Map State: IsViewable" in qa.subprocess.run(
        ["xwininfo", "-id", str(main_xid)], capture_output=True, text=True,
        timeout=2).stdout, "submitting hid the bottom touch keyboard"
    # This first letter MUST come from the screen key. No physical readiness
    # key, replacement snapshot or clear-buffer operation wakes the panel.
    click_control("VirtualKeyboardKey(Character('h'))")
    wait(lambda: seed_is("h"), "screen key starts a fresh host draft after committing")
    wait_bottom(expanded=True)
    stable(lambda: seed_is("h") and insertions == [committed],
           "fresh screen-key draft was lost, duplicated or unexpectedly committed")
    capture_owned_display("bottom-layout-touch-restart-after-commit")
    cancel()
    clear_buffer()
    draft()
    expanded = wait_bottom(expanded=True)
    passed_case("expanded keyboard remains mapped after commit and starts the next draft by touch alone")

    identity = composition_identity(qa.watch.latest)
    expanded_anchor = bottom_anchor(expanded)
    click_control("ToggleCompactMode")
    wait(lambda: window_frame("Main").get("size", [9999, 9999])[0] <= 120 and
         window_frame("Main").get("size", [9999, 9999])[1] <= 120 and
         control_rect(window_frame("Main"), "ToggleCompactMode"),
         "actual collapse control presents a small interactive orb")
    orb = window_frame("Main")
    expect_bounds(orb)
    assert not any(item["kind"].startswith("VirtualKeyboardKey(")
                   for item in orb["controls"]), "orb keeps hidden keyboard controls"
    # An explicit 180 ms UI debounce separates the two deliberate taps.
    stable(lambda: composition_identity(qa.watch.latest) == identity,
           "collapsing into an orb changed native input", duration=.22)
    capture_owned_display("bottom-layout-orb")
    click_control("ToggleCompactMode")
    expanded = wait_bottom(expanded=True)
    assert composition_identity(qa.watch.latest) == identity
    assert bottom_anchor(expanded) == expanded_anchor
    passed_case("real orb collapse/expand restores wide bottom alignment without changing the host draft")

    anchor = bottom_anchor(expanded)
    click_control("InputModesToggle")
    folded = wait_bottom(expanded=False)
    assert bottom_anchor(folded) == anchor
    assert folded["size"][1] < expanded["size"][1], "folding did not shrink the keyboard drawer"
    assert not any(item["kind"].startswith("VirtualKeyboardKey(")
                   for item in folded["controls"]), "folded keyboard retains hidden hit targets"
    pointer(folded, folded["native_page"]["next"])
    tail = page(6)
    bottom_anchor(tail)
    candidate = tail["native_page"]["candidates"][-1]
    capture_owned_display("bottom-layout-folded-tail")
    pointer(tail, candidate["rect"])
    finish(candidate["text"])
    passed_case("folded bottom drawer preserves anchor, page navigation and exact tail-card commit")

    draft()
    folded = wait_bottom(expanded=False)
    identity = composition_identity(qa.watch.latest)
    position = folded["position"]
    for destination in [(200, 100), (screen_size[0] - 120, screen_size[1] - 90)]:
        relocate_owned_input(destination)
        stable(lambda: composition_identity(qa.watch.latest) == identity and
               page_ready() and page_ready()["position"] == position,
               "same-monitor caret motion moved the dock or changed its draft")
        bottom_anchor(page())
        passed_case("same-screen caret relayout leaves bottom position and draft unchanged: " + str(destination))
    relocate_owned_input((60, 40))
    click_control("InputModesToggle")
    restored = wait_bottom(expanded=True)
    assert bottom_anchor(restored) == anchor
    cancel()


def restore_follow_layout():
    set_layout_from_settings("follow-caret")
    draft()
    frame = page()
    assert frame["size"][0] == original_follow_width, \
        ("follow-caret did not restore original floating width", frame["size"], original_follow_width)
    old_position = frame["position"]
    # stale_press_check leaves the input at (200, 100). Moving there again
    # only changes the left margin, which can stay screen-edge-clamped for a
    # 900px floating panel. Change the real vertical anchor as well.
    relocate_owned_input((60, 40))
    wait(lambda: page_ready() and page_ready()["position"] != old_position,
         "switching back restores caret-following behavior")
    frame = page()
    assert frame.get("layout_mode") == "follow-caret"
    cursor = qa.watch.latest["cursor"]
    assert frame["position"][1] >= cursor["y"] + cursor["height"]
    assert frame["position"][1] + frame["size"][1] < screen_size[1] - 12
    capture_owned_display("bottom-layout-restored-follow")
    cancel()
    passed_case("actual settings switch restores floating width and reported-caret following")


def stale_press_check():
    frame = draft()
    pointer(frame, frame["native_page"]["candidates"][0]["rect"], up=False)
    try:
        # Dispatch the press before changing the generation. No candidate APIs
        # or synthetic host snapshots are used to create this stale gesture.
        deadline = time.monotonic() + .04
        while time.monotonic() < deadline:
            pump()
            time.sleep(.002)
        key(IBus.KEY_l, expect="hell")
        changed = wait_bottom() if bottom_layout else page(0)
        assert changed["native_revision"] != frame["native_revision"]
        relocate_owned_input((200, 100))
        current_identity = composition_identity(qa.watch.latest)
        if bottom_layout:
            stable(lambda: page_ready() and page_ready()["position"] == changed["position"],
                   "geometry-only update moved dock while invalidating a stale press")
        else:
            wait(lambda: page_ready() and page_ready()["position"] != changed["position"],
                 "new geometry is presented after invalidating a pending candidate press")
        current = page()
        # Release over the *current* corresponding card; spatial miss must not
        # be the reason the obsolete press is rejected.
        pointer(current, current["native_page"]["candidates"][0]["rect"], down=False)
    finally:
        assert x.xt.XTestFakeButtonEvent(x.display, 1, 0, 0)
        x.x.XSync(x.display, 0)
    stable(lambda: composition_identity(qa.watch.latest) == current_identity,
           "geometry refresh revived a stale candidate click")
    assert seed_is("hell")
    cancel()
    passed_case("geometry-only redraw cannot revive a press from an obsolete candidate revision")


try:
    prepare_candidate_budget_pack()
    bus, x = qa.start()
    fake_context = bus.current_input_context()
    x.x.XQueryKeymap.argtypes = [C.c_void_p, C.c_void_p]
    x.x.XQueryKeymap.restype = C.c_int
    qa.gi.require_version("Gtk", "3.0")
    qa.gi.require_version("GdkX11", "3.0")
    from gi.repository import Gtk, Gdk, GdkX11

    Gtk.init([])
    screen = Gdk.Screen.get_default()
    assert (screen.get_width(), screen.get_height()) == screen_size
    window = Gtk.Window(title="Suzaku isolated native candidate QA")
    window.set_default_size(420, 220)
    window.move(60, 40)
    view = Gtk.TextView()
    view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
    view.set_left_margin(16)
    view.set_top_margin(16)
    for event in ("key-press-event", "key-release-event"):
        view.connect(event, lambda _view, item: events.append(
            {"event": str(item.type), "keysym": int(item.keyval),
             "hardwarecode": int(item.hardware_keycode)}) or False)
    view.get_buffer().connect("insert-text", lambda _buffer, _iter, text, _length:
                              insertions.append(text))
    window.add(view)
    window.show_all()
    xid = window.get_window().get_xid()
    x.focus(xid)
    view.grab_focus()
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    last_context, since = None, time.monotonic()

    def ready():
        global last_context, since
        current = (bus.current_input_context(),
                   None if qa.watch.latest is None else qa.watch.latest["context"])
        if current != last_context:
            last_context, since = current, time.monotonic()
        return (current[0] not in (None, fake_context, "/org/freedesktop/IBus/InputContext_1") and
                owned_focus() and seed_is("") and time.monotonic() - since >= .5)

    wait(ready, "stable focused owned GTK TextView", timeout=8)
    if auto_layout:
        assert not (qa.root / "config" / "suzaku-panel" / "panel-settings.toml").exists(), \
            "Auto default must begin without any panel settings file"
    companion = qa.spawn([str(qa.bins / "panel")])
    wait(lambda: qa.companion_frame().get("runtime_font"), "real native companion startup", timeout=30)
    assert companion.poll() is None and owned_focus()
    print("READY: real default native panel and GTK TextView on private 1024x768 display", flush=True)

    if auto_layout:
        prepare_auto_layout()
    elif bottom_layout:
        prepare_bottom_layout()

    frame = draft()
    total = frame["native_page"]["total"]
    assert len(frame["native_page"]["candidates"]) == 6
    assert [card["number"] for card in frame["native_page"]["candidates"][-2:]] == [5, 6]
    capture_owned_display("native-six-candidates")
    key(IBus.KEY_Page_Down)
    assert len(page(6)["native_page"]["candidates"]) == total - 6
    key(IBus.KEY_Page_Up)
    page(0)
    passed_case("six drawn first-page cards including 5/6 and a partial tail reached by physical PgDown/PgUp")

    for action in ("button", "wheel"):
        for start in (6, 0):
            frame = page()
            control = "next" if start else "previous"
            rect = (frame["native_page"][control] if action == "button" else
                    frame["native_page"]["candidates"][0]["rect"])
            pointer(frame, rect, button=1 if action == "button" else (5 if start else 4))
            page(start)
            assert seed_is("hel")
        passed_case("native " + action + " changes the same full/partial pages without commit or focus loss")
    for button in (4, 4):
        frame = page(0)
        identity = composition_identity(qa.watch.latest)
        pointer(frame, frame["native_page"]["candidates"][0]["rect"], button=button)
        stable(lambda: composition_identity(qa.watch.latest) == identity,
               "previous wheel crossed the first-page boundary")
    frame = page(0)
    pointer(frame, frame["native_page"]["next"])
    frame = page(6)
    identity = composition_identity(qa.watch.latest)
    pointer(frame, frame["native_page"]["candidates"][0]["rect"], button=5)
    stable(lambda: composition_identity(qa.watch.latest) == identity,
           "next wheel crossed the final-page boundary")
    cancel()
    passed_case("first/last native page wheel boundaries are inert and disabled arrows are not hit targets")

    for start, slot in [(0, 4), (0, 5), (6, total - 7)]:
        frame = draft(start=start)
        text = frame["native_page"]["candidates"][slot]["text"]
        revision = qa.watch.latest["revision"]
        key(IBus.KEY_1 + slot, expect=text)
        wait(lambda: qa.watch.latest["revision"] > revision,
             "physical number adoption acknowledged even if candidate equals the draft")
        expect_unchanged_buffer()
        key(IBus.KEY_BackSpace, expect="hel")
        expect_unchanged_buffer()
        page()
        cancel()
        passed_case(f"physical page-local number adopts and Backspace exactly undoes: page={start // 6 + 1}, slot={slot + 1}")

    for index in range(total):
        frame = draft(start=index // 6 * 6)
        assert frame["native_page"]["total"] == total
        candidate = frame["native_page"]["candidates"][index % 6]
        pointer(frame, candidate["rect"])
        finish(candidate["text"])
        passed_case(f"real card click commits exactly once: global index={index}, local number={index % 6 + 1}")

    language("zh-Hans")
    for slot in (0, 1):
        frame = draft("henqiang")
        candidate = frame["native_page"]["candidates"][slot]
        pointer(frame, candidate["rect"])
        finish(candidate["text"])
        passed_case(f"native Chinese word/sentence card preserves exact Unicode GTK insertion: slot={slot + 1}")
    candidate_budget_checks()
    language("en")
    if bottom_layout:
        bottom_geometry_checks()
    else:
        geometry_checks()
    stale_press_check()
    if bottom_layout:
        restore_follow_layout()
    expected_offset = 26 if auto_layout else (23 if bottom_layout else 16)
    assert passed == total + expected_offset, (passed, total)
    print(f"RESULT: {passed} native candidate {suite if bottom_layout else 'paging/caret'} workflows passed; "
          f"every {total} English card clicked, real GTK/IBus/XTest only", flush=True)
except Exception:
    print("NATIVE CANDIDATE DIAGNOSTIC:", json.dumps({
        "last_action": last_action,
        "host": None if qa.watch is None else qa.watch.latest,
        "presented": qa.companion_frame(),
        "buffer": None if view is None else buffer_text(),
        "expected_buffer": committed,
        "insertions": insertions,
        "keys": list(events),
        "focus_ancestry": None if x is None else x.focus_ancestry(),
        "owned_window": xid,
    }, ensure_ascii=False), flush=True)
    qa.diagnose()
    raise
finally:
    if window is not None:
        window.destroy()
    qa.close()
