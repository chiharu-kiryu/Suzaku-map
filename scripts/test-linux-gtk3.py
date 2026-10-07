#!/usr/bin/env python3
"""Strict first-key GTK3 regression on the runner's private Xvfb/IBus session.

Unlike the editor workflow, this in-process TextView fixture has no file-loading
probe: every typed character must arrive exactly once, including the first one.
Run with test-linux-apps.sh gtk3, never against the user's desktop.
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
spec.loader.exec_module(qa)  # Reject a non-owned display/config before opening GTK.
assert qa.os.environ["SUZAKU_APP_QA_SUITE"] == "gtk3"
assert qa.os.environ.get("GTK_IM_MODULE") == "ibus"
IBus = qa.IBus
bus, x, window, xid = None, None, None, None
views, insertions = [], []
committed = ["", ""]
events = deque(maxlen=16)
key_event_serial = 0
raw_key_events = deque(maxlen=32)
raw_key_event_serial = 0
gdk_observer_error = None
default_event_handler = None
event_handler_installed = False
focus_events = deque(maxlen=32)
started = time.monotonic()
last_key = None
passed = 0
active = 0
fake_context = None
tray_watcher = None
tray_layout = None
presentation_observer = None


def pump():
    # The GTK target and driver share a loop. Bound each pass so a stream of
    # queued events cannot silently extend a key's four-second deadline.
    context = qa.GLib.MainContext.default()
    deadline = time.monotonic() + .01
    for _ in range(128):
        if not context.iteration(False) or time.monotonic() >= deadline:
            break
    if gdk_observer_error is not None:
        raise gdk_observer_error
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


def buffer_text(view):
    buffer = view.get_buffer()
    return buffer.get_text(buffer.get_start_iter(), buffer.get_end_iter(), True)


def buffers():
    return [buffer_text(view) for view in views]


def seed_is(text):
    frame = qa.watch.latest
    return (frame is not None and frame["focused"] and not frame["private"] and
            frame["seed"] == text)


def target_is_owned():
    return (window.get_visible() and x.focused_within(xid) and
            views[active].has_focus() and views[active].is_focus())


def focus_state():
    # Record only owned widget state and XIDs, not other windows' contents.
    ancestry = () if x is None else x.focus_ancestry()
    return {"x11_ancestry": ancestry, "owned_window": xid, "active_view": active,
            "window_active": None if window is None else window.is_active(),
            "toplevel_focus": None if window is None else window.has_toplevel_focus(),
            "widget_focus": [{"has_focus": view.has_focus(), "is_focus": view.is_focus()}
                             for view in views]}


def observe_focus(_widget, _event, reason):
    focus_events.append({"time": round(time.monotonic() - started, 4),
                         "reason": reason, **focus_state()})
    return False


def expect_buffers():
    assert buffers() == committed, "GTK buffer changed without an explicit commit"


def settle_input(previous_context=None):
    last, since = None, time.monotonic()

    def ready():
        nonlocal last, since
        current = bus.current_input_context()
        frame = qa.watch.latest
        state = (current, None if frame is None else frame["context"],
                 x.focused(), views[active].has_focus(), views[active].is_focus())
        if state != last:
            last, since = state, time.monotonic()
        return (current and current not in (fake_context, "/org/freedesktop/IBus/InputContext_1") and
                current != previous_context and target_is_owned() and seed_is("") and
                time.monotonic() - since >= .5)

    wait(ready, "stable, non-fake GTK3 input context and focused TextView", timeout=8)


def key(symbol, *modifiers, expect=None):
    global last_key
    codes = [x.x.XKeysymToKeycode(x.display, item) for item in [*modifiers, symbol]]
    last_key = {"keysym": symbol, "hardwarecode": codes[-1],
                "modifiers": [{"keysym": item, "hardwarecode": code}
                              for item, code in zip(modifiers, codes)],
                "expected_seed": expect, "expected_buffers": list(committed)}
    assert target_is_owned(), "synthetic key target is not the focused owned TextView"
    assert all(codes), "keysym unavailable on private keyboard"
    state = C.create_string_buffer(32)
    x.x.XQueryKeymap(x.display, state)
    assert not any(state.raw), "a key from an earlier injection is still held"
    pressed = []
    try:
        for code in codes:
            assert target_is_owned(), "focus changed before synthetic key press"
            assert x.xt.XTestFakeKeyEvent(x.display, code, 1, 0)
            pressed.append(code)
        x.x.XSync(x.display, 0)
        deadline = time.monotonic() + .015
        while time.monotonic() < deadline:
            pump()
            time.sleep(.002)
    finally:
        released = True
        for code in reversed(pressed):
            # Attempt every release even if one fails; a final Enter must not
            # pass simply because no later key checks for a stuck injection.
            result = x.xt.XTestFakeKeyEvent(x.display, code, 0, 0)
            released = bool(result) and released
        x.x.XSync(x.display, 0)
        x.x.XQueryKeymap(x.display, state)
        assert released and not any(state.raw), "synthetic key release failed"
    if expect is not None:
        wait(lambda: seed_is(expect), "exact GTK3 draft after one physical key")
    assert target_is_owned(), "focus changed while processing synthetic key"


def type_text(text):
    expected = qa.watch.latest["seed"]
    for character in text:
        assert character.isascii() and (character.islower() or character == " ")
        expected += character
        key(ord(character), expect=expected)
        expect_buffers()


def choose(text):
    wait(lambda: any(c["text"] == text for c in qa.watch.latest["candidates"][:6]),
         "built-in candidate available for numeric adoption")
    slot = next(index for index, item in enumerate(qa.watch.latest["candidates"][:6])
                if item["text"] == text)
    key(IBus.KEY_1 + slot, expect=text)
    expect_buffers()


def language(code):
    assert seed_is("")
    if qa.watch.latest["language"] != code:
        revision = qa.watch.latest["revision"]
        assert json.loads(qa.command("L" + code))["ok"]
        wait(lambda: qa.watch.latest["revision"] > revision and
             qa.watch.latest["language"] == code and seed_is(""), "language applied")
    assert json.loads(qa.command("S"))["settings"]["language"] == code


def clear_buffers():
    assert seed_is("")
    for index, view in enumerate(views):
        view.get_buffer().set_text("")
        committed[index] = ""
        insertions[index].clear()
    expect_buffers()


def commit(text):
    assert seed_is(text)
    before = len(insertions[active])
    committed[active] += text
    key(IBus.KEY_Return, expect="")
    wait(lambda: buffers() == committed, "Enter commits the exact GTK buffer once")
    # A transient correct buffer must not hide a delayed duplicate or newline.
    deadline = time.monotonic() + .15
    while time.monotonic() < deadline:
        pump()
        assert seed_is("")
        expect_buffers()
        time.sleep(.003)
    assert insertions[active][before:] == [text], "Enter caused more than one GTK insertion"


def focus_view(index):
    global active
    previous = bus.current_input_context()
    previous_generation = qa.watch.latest["context"]
    active = index
    views[index].grab_focus()
    settle_input(previous)
    assert qa.watch.latest["context"] != previous_generation, "focus reused a stale draft generation"
    expect_buffers()


def passed_case(label):
    global passed
    passed += 1
    print("PASS:", label, flush=True)


def delivered_key(symbol, *modifiers, expect):
    """A same-draft expectation alone cannot acknowledge a no-op physical key."""
    previous = raw_key_event_serial
    hardwarecode = x.x.XKeysymToKeycode(x.display, symbol)
    key(symbol, *modifiers, expect=expect)
    wait(lambda: any(event["serial"] > previous and event["pressed"] and
                     event["hardwarecode"] == hardwarecode and event["view"] == active
                     for event in raw_key_events), "owned GTK input received the physical navigation key before IM filtering")


def boundary_key(symbol, *modifiers):
    snapshot = qa.watch.latest
    delivered_key(symbol, *modifiers, expect=snapshot["seed"])
    # As in commit(), observe beyond a transient correct state: async GTK input
    # must neither forward Tab into the document nor publish a late selection.
    deadline = time.monotonic() + .15
    while True:
        pump()
        assert qa.watch.latest == snapshot, "boundary navigation changed draft, candidates or revision"
        assert target_is_owned(), "boundary navigation moved the GTK focus"
        expect_buffers()
        if time.monotonic() >= deadline:
            return
        time.sleep(.003)


def navigate_row(symbol, selected):
    before = qa.watch.latest
    delivered_key(symbol, expect=before["seed"])
    wait(lambda: qa.watch.latest["revision"] > before["revision"] and
         qa.watch.latest["selected"] == selected, "real relative navigation selects the adjacent row")
    for field in ("host", "context", "focused", "private", "language", "seed", "candidates"):
        assert qa.watch.latest[field] == before[field], ("relative navigation changed", field)
    expect_buffers()


def check_candidate_row_boundaries():
    # Pending/cancellation is exercised by the private native HTTP suite. This
    # real GTK gate isolates physical-key delivery, local selection and undo.
    assert json.loads(qa.command("S"))["settings"]["llm_enabled"] is False
    for code, reading, word in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]:
        language(code)
        clear_buffers()
        type_text(reading)
        assert qa.watch.latest["selected"] == 0
        boundary_key(IBus.KEY_Up)
        boundary_key(IBus.KEY_Tab, IBus.KEY_Shift_L)
        key(IBus.KEY_space, expect=reading + " ")
        expect_buffers()
        key(IBus.KEY_Escape, expect="")
        expect_buffers()
        passed_case(code + " physical first-row Up/Shift+Tab preserve exact state and literal Space")

        type_text(reading)
        choose(word)
        assert qa.watch.latest["selected"] == 0
        boundary_key(IBus.KEY_Up)
        boundary_key(IBus.KEY_Tab, IBus.KEY_Shift_L)
        key(IBus.KEY_BackSpace, expect=reading)
        expect_buffers()
        key(IBus.KEY_Escape, expect="")
        expect_buffers()
        passed_case(code + " physical first-row boundaries retain original-spelling adoption undo")

        type_text(reading)
        assert qa.watch.latest["selected"] == 0 and len(qa.watch.latest["candidates"]) > 1
        selected_text = qa.watch.latest["candidates"][0]["text"]
        navigate_row(IBus.KEY_Down, 1)
        navigate_row(IBus.KEY_Up, 0)
        key(IBus.KEY_space, expect=selected_text + " ")
        expect_buffers()
        key(IBus.KEY_Escape, expect="")
        expect_buffers()
        passed_case(code + " real physical adjacent-row roundtrip still explicitly selects for Space")

    language("en")
    clear_buffers()
    type_text("qzxv")
    assert qa.watch.latest["selected"] == 0 and len(qa.watch.latest["candidates"]) == 1
    boundary_key(IBus.KEY_Down)
    boundary_key(IBus.KEY_Tab)
    key(IBus.KEY_Escape, expect="")
    expect_buffers()
    passed_case("single-candidate Down/Tab preserve exact state and GTK focus without inserting a tab")


def observe_key(view, event, index):
    global key_event_serial
    key_event_serial += 1
    events.append({"serial": key_event_serial, "pressed": event.type == Gdk.EventType.KEY_PRESS,
                   "time": round(time.monotonic() - started, 4),
                   "event": str(event.type), "view": index, "keysym": int(event.keyval),
                   "hardwarecode": int(event.hardware_keycode), "modifiers": int(event.state),
                   "has_focus": view.has_focus(), "is_focus": view.is_focus()})
    return False  # Observe only; GTK/IBus must process the original event.


def dispatch_gtk_event(event, *_args):
    default_event_handler(event)


def observe_gdk_event(event, *_args):
    """Observe delivery before GTK's IBus key snooper, then dispatch exactly once."""
    global raw_key_event_serial, gdk_observer_error
    try:
        if (event.type in (Gdk.EventType.KEY_PRESS, Gdk.EventType.KEY_RELEASE) and
                window is not None and xid is not None):
            event_window = event.get_window()
            if (event_window is not None and event_window.get_toplevel() == window.get_window() and
                    views[active].has_focus() and views[active].is_focus()):
                raw_key_event_serial += 1
                raw_key_events.append({"serial": raw_key_event_serial,
                    "pressed": event.type == Gdk.EventType.KEY_PRESS,
                    "time": round(time.monotonic() - started, 4), "event": str(event.type),
                    "view": active, "keysym": int(event.keyval),
                    "hardwarecode": int(event.hardware_keycode), "modifiers": int(event.state)})
    except Exception as error:
        # GI prints callback exceptions instead of propagating them through
        # MainContext.iteration. Preserve the first failure for pump() to raise.
        if gdk_observer_error is None:
            gdk_observer_error = error
    finally:
        try:
            # No copy, event_put, signal emission or second dispatch: this is
            # the same downstream handler installed by Gtk.init in our process.
            dispatch_gtk_event(event)
        except Exception as error:
            if gdk_observer_error is None:
                gdk_observer_error = error


def module_state():
    return {"mode": qa.os.environ.get("IBUS_ENABLE_SYNC_MODE", "default (unset)"),
            "GTK_IM_MODULE": qa.os.environ.get("GTK_IM_MODULE"),
            "gtk-im-module": Gtk.Settings.get_default().get_property("gtk-im-module"),
            "TextView.im-module": [view.get_property("im-module")
                                   if view.find_property("im-module") else "unavailable"
                                   for view in views]}


class PrivateStatusNotifierWatcher:
    """Supply only the desktop watcher; the item and menu remain the real panel."""

    NAME = "org.kde.StatusNotifierWatcher"
    PATH = "/StatusNotifierWatcher"

    def __init__(self):
        from gi.repository import Gio

        self.items = []
        self.connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.info = Gio.DBusNodeInfo.new_for_xml("""
            <node><interface name="org.kde.StatusNotifierWatcher">
              <method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
              <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
              <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
              <property name="ProtocolVersion" type="i" access="read"/>
            </interface></node>
        """)
        # ksni 0.3.6 calls RegisterStatusNotifierItem and then reads
        # IsStatusNotifierHostRegistered before exposing its live tray service.
        self.registration = self.connection.register_object(
            self.PATH, self.info.interfaces[0], self.register_item, self.get_property, None)
        assert self.registration, "could not export the private tray watcher"
        result = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "RequestName", qa.GLib.Variant("(su)", (self.NAME, 4)), None,
            Gio.DBusCallFlags.NONE, 700, None).unpack()[0]
        if result != 1:
            self.connection.unregister_object(self.registration)
            raise AssertionError("private bus unexpectedly has another tray watcher")

    def register_item(self, _connection, _sender, _path, _interface, method, parameters,
                      invocation):
        if method != "RegisterStatusNotifierItem":
            invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method)
            return
        service = parameters.unpack()[0]
        if service not in self.items:
            self.items.append(service)
        invocation.return_value(qa.GLib.Variant("()", ()))

    def get_property(self, _connection, _sender, _path, _interface, name):
        return {
            "RegisteredStatusNotifierItems": qa.GLib.Variant(
                "as", [service + "/StatusNotifierItem" for service in self.items]),
            "IsStatusNotifierHostRegistered": qa.GLib.Variant("b", True),
            "ProtocolVersion": qa.GLib.Variant("i", 0),
        }.get(name)

    def close(self):
        from gi.repository import Gio

        self.connection.unregister_object(self.registration)
        self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "ReleaseName", qa.GLib.Variant("(s)", (self.NAME,)), None,
            Gio.DBusCallFlags.NONE, 700, None)


class EnginePresentation:
    """Observe the owned host's actual IBus presentation signals, without routing keys.

    Engine signals are broadcast on the private IBus connection, as in the
    cross-application fixture. Do not replace GTK's context or capabilities.
    """

    def __init__(self):
        from gi.repository import Gio

        self.connection = bus.get_connection()
        self.states = {}
        self.path = None
        self.events = deque(maxlen=24)
        self.subscriptions = [self.connection.signal_subscribe(
            "org.freedesktop.IBus.Suzaku", "org.freedesktop.IBus.Engine", signal,
            None, None, Gio.DBusSignalFlags.NONE, self.observe) for signal in (
                "UpdateLookupTable", "ShowLookupTable", "HideLookupTable",
                "UpdateAuxiliaryText", "ShowAuxiliaryText", "HideAuxiliaryText")]

    def observe(self, _connection, _sender, path, _interface, signal, params, *_args):
        state = self.states.setdefault(path, {"lookup": None, "auxiliary": None, "text": ""})
        key = "lookup" if "LookupTable" in signal else "auxiliary"
        if signal.startswith("Update"):
            state[key] = params.get_child_value(1).get_boolean()
            if key == "auxiliary":
                text = IBus.Serializable.deserialize_object(params.get_child_value(0).get_variant())
                state["text"] = text.get_text()
                if state[key] and state["text"].startswith("Suzaku · "):
                    # Switching away and back can allocate a new engine object.
                    # Only this fixture types; follow its public draft signal,
                    # not a now-hidden object's remembered lookup state.
                    self.path = path
        else:
            state[key] = signal.startswith("Show")
        self.events.append({"path": path, "signal": signal, "visible": state[key]})

    def bind_visible_draft(self, draft):
        matches = [path for path, state in self.states.items()
                   if state["lookup"] is True and state["auxiliary"] is True and
                   state["text"].startswith("Suzaku · " + draft + "\n")]
        assert len(matches) <= 1, "more than one private engine presents the owned GTK draft"
        if matches:
            self.path = matches[0]
        return bool(matches)

    def both(self, visible):
        state = self.states.get(self.path, {})
        return state.get("lookup") is visible and state.get("auxiliary") is visible

    def close(self):
        for subscription in self.subscriptions:
            self.connection.signal_unsubscribe(subscription)


def unchanged_presentation(snapshot, visible, duration=0):
    """Lease operations cannot edit/revise the draft, select or commit text."""
    deadline = time.monotonic() + duration
    while True:
        pump()
        assert target_is_owned(), "presentation change stole the GTK TextView focus"
        assert qa.watch.latest == snapshot, "presentation lease changed native draft/revision/selection"
        assert presentation_observer.both(visible), "presentation ownership changed unexpectedly"
        expect_buffers()
        if time.monotonic() >= deadline:
            return
        time.sleep(.005)


def owned_tray_action(companion, labels):
    """Exercise the real panel's menu on the runner's private session bus."""
    from gi.repository import Gio

    session = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    target = None
    shown = set()

    def call(destination, path, interface, method, arguments=None):
        return session.call_sync(destination, path, interface, method, arguments, None,
                                 Gio.DBusCallFlags.NONE, 700, None).unpack()

    def ready():
        global tray_layout
        nonlocal target
        assert companion.poll() is None, "owned companion exited before tray action"
        assert target_is_owned(), "companion stole the TextView focus before tray action"
        names = call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                     "org.freedesktop.DBus", "ListNames")[0]
        names = [name for name in names
                 if name.startswith(f"org.kde.StatusNotifierItem-{companion.pid}-")]
        assert len(names) <= 1, "owned companion exported ambiguous tray services"
        if not names:
            return False
        destination = names[0]
        if destination not in shown:
            call(destination, "/MenuBar", "com.canonical.dbusmenu", "AboutToShow",
                 qa.GLib.Variant("(i)", (0,)))
            shown.add(destination)
        tray_layout = call(destination, "/MenuBar", "com.canonical.dbusmenu", "GetLayout",
                           qa.GLib.Variant("(iias)", (0, 1, ["label", "enabled"])))[1]
        actions = [child for child in tray_layout[2] if child[1].get("label") in labels]
        assert len(actions) <= 1, "owned tray action is ambiguous"
        if not actions:
            return False  # Visibility acknowledgements can still be queued.
        if not actions[0][1].get("enabled", True):
            return False
        target = destination, actions[0][0]
        return True

    wait(ready, "owned panel's enabled menu action: " + repr(labels))
    destination, item = target
    call(destination, "/MenuBar", "com.canonical.dbusmenu", "Event",
         qa.GLib.Variant("(isvu)", (item, "clicked", qa.GLib.Variant("i", 0), 0)))


def activate_from_owned_tray(companion):
    owned_tray_action(companion, ("Activate Suzaku", "激活 Suzaku 输入法"))


def diagnose():
    frame = None if qa.watch is None else qa.watch.latest
    print("GTK3 failure:", json.dumps({
        "last_key": last_key, "events": list(events), "buffers": buffers(),
        "raw_key_events": list(raw_key_events), "event_observer_error": repr(gdk_observer_error),
        "expected_buffers": committed,
        "native": None if frame is None else {name: frame.get(name) for name in
                  ("seed", "focused", "private", "context", "revision", "language")},
        "input_context": None if bus is None else bus.current_input_context(),
        "fake_context": fake_context, "x11_focus": None if x is None else x.focused(),
        "owned_window": xid, "active_view": active,
        "focus": focus_state(), "focus_events": list(focus_events),
        "im": module_state() if window is not None else None,
        "tray_layout": tray_layout,
        "tray_registered_items": None if tray_watcher is None else tray_watcher.items,
        "presentation": None if presentation_observer is None else {
            "path": presentation_observer.path, "states": presentation_observer.states,
            "events": list(presentation_observer.events)},
    }, ensure_ascii=False), flush=True)
    qa.diagnose()


try:
    # Match the earlier owned GTK3 experiment's startup order. This is a test
    # boundary, not an assertion that startup order caused the desktop failure.
    bus, x = qa.start()
    fake_context = bus.current_input_context()
    x.x.XQueryKeymap.argtypes = [C.c_void_p, C.c_void_p]
    x.x.XQueryKeymap.restype = C.c_int
    qa.gi.require_version("Gtk", "3.0")
    qa.gi.require_version("GdkX11", "3.0")
    from gi.repository import Gtk, Gdk, GdkX11

    Gtk.init([])
    # Local API contract: Gdk-3.0.gir:36643 permits this wrapper; Gtk-3.0.gir:218169
    # documents key snoopers inside main_do_event, before widget key signals.
    # This private process has no earlier application-installed GDK handler.
    default_event_handler = Gtk.main_do_event
    Gdk.event_handler_set(observe_gdk_event, None)
    event_handler_installed = True
    window = Gtk.Window(title="Suzaku isolated GTK3 input QA")
    window.set_default_size(720, 360)
    for event in ("focus-in-event", "focus-out-event", "notify::is-active",
                  "notify::has-toplevel-focus"):
        window.connect(event, observe_focus, event)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
    for index in range(2):
        view = Gtk.TextView()
        view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        view.connect("key-press-event", observe_key, index)
        view.connect("key-release-event", observe_key, index)
        view.connect("notify::has-focus", observe_focus, f"view-{index}-focus")
        insertions.append([])
        view.get_buffer().connect("insert-text", lambda _buffer, _iter, text, _length, i:
                                  insertions[i].append(text), index)
        views.append(view)
        box.pack_start(view, True, True, 0)
    window.add(box)
    window.show_all()
    xid = window.get_window().get_xid()
    x.focus(xid)
    views[0].grab_focus()
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    settle_input()
    print("READY: isolated GTK3", json.dumps(module_state()), flush=True)

    type_text("please annotate this para")
    expect_buffers()
    key(IBus.KEY_Escape, expect="")
    expect_buffers()
    passed_case("first key, adjacent repeated letters and Space preserve the exact English draft")

    for code, reading, word, suffix, final in [
        ("en", "hel", "hello", " world", "hello world"),
        ("zh-Hans", "nihao", "你好", " de", "你好 的"),
    ]:
        language(code)
        clear_buffers()
        type_text(reading)
        choose(word)
        key(IBus.KEY_BackSpace, expect=reading)
        expect_buffers()
        choose(word)
        type_text(suffix)
        if not seed_is(final):
            choose(final)
        commit(final)
        passed_case(code + " numeric adoption, exact Backspace undo, Space continuation and one Enter commit")

    check_candidate_row_boundaries()

    language("en")
    clear_buffers()
    type_text("hel")
    for code in ("zh-Hans", "en"):
        revision = qa.watch.latest["revision"]
        key(IBus.KEY_space, IBus.KEY_Control_L, IBus.KEY_Shift_L, expect="hel")
        wait(lambda: qa.watch.latest["revision"] > revision and
             qa.watch.latest["language"] == code, "language shortcut acknowledgement")
        assert json.loads(qa.command("S"))["settings"]["language"] == code
        assert seed_is("hel")
        expect_buffers()
    choose("hello")
    commit("hello")
    passed_case("Ctrl+Shift+Space preserves the draft across English/Chinese/English")

    for code, reading, word in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好")]:
        for adopted in (False, True):
            language(code)
            clear_buffers()
            type_text(reading)
            if adopted:
                choose(word)
            focus_view(1)
            type_text(reading)
            choose(word)
            commit(word)
            focus_view(0)
            assert seed_is("")
            expect_buffers()
            passed_case(code + " TextView switch discards " +
                        ("adopted" if adopted else "raw") + " preedit without replay or stray commit")

    language("zh-Hans")
    assert json.loads(qa.command("S"))["settings"]["llm_enabled"] is False
    for reading, word, sentences in [
        ("henqiang", "很强", {"很强的学习能力。", "很强，继续加油！"}),
        ("henlihai", "很厉害", {"很厉害的表现。", "很厉害，向你学习。"}),
    ]:
        clear_buffers()
        type_text(reading)

        def fallback_ready():
            candidates = qa.watch.latest["candidates"][:6]
            return (any(c["text"] == word and c["kind"] == "word" and c["source"] == "local"
                        for c in candidates) and
                    any(c["text"] in sentences and c["kind"] == "sentence" and c["source"] == "local"
                        for c in candidates))

        wait(fallback_ready, reading + " has local word and authored sentence candidates with LLM disabled")
        choose(word)
        commit(word)
        passed_case(reading + " real GTK keys expose local word/sentence fallback and commit numeric adoption once")

    language("en")
    clear_buffers()
    presentation_observer = EnginePresentation()
    type_text("hel")
    wait(lambda: presentation_observer.bind_visible_draft("hel"),
         "IBus lookup and auxiliary are visible before a companion takes ownership")
    passed_case("stock IBus lookup and auxiliary remain available without a presentation owner")
    before_presentation = dict(qa.watch.latest)
    tray_watcher = PrivateStatusNotifierWatcher()
    companion = qa.spawn([str(qa.bins / "panel")])
    wait(lambda: qa.companion_frame().get("runtime_font") and
         qa.companion_frame().get("draft") == "hel", "owned companion renders GTK3 draft", timeout=30)
    assert companion.poll() is None and target_is_owned(), "companion stole GTK3 input focus"
    wait(lambda: presentation_observer.both(False),
         "drawn companion claims presentation and hides both stock IBus surfaces")
    # Longer than the host's 1200ms lease: a real UI worker must renew, not merely
    # send a one-shot test claim. Renewals must not manufacture new snapshots.
    unchanged_presentation(before_presentation, False, duration=1.6)
    passed_case("real drawn companion renews exclusive presentation without changing the draft")
    type_text("lo world")
    commit("hello world")
    assert companion.poll() is None and target_is_owned()
    passed_case("owned no-focus companion preserves TextView focus, physical continuation and exact commit")

    # Reproduce the old desktop experiment's activation order without touching
    # that desktop: an already-focused GTK input, a running companion, tray
    # activation from another real engine, then a Chinese-to-English IPC change.
    language("zh-Hans")
    fallback = "xkb:us::eng"
    assert fallback in {engine.get_name() for engine in bus.list_engines()}, \
        "private IBus has no US keyboard engine for tray activation regression"
    assert bus.set_global_engine(fallback)
    wait(lambda: bus.get_global_engine() is not None and
         bus.get_global_engine().get_name() == fallback, "private US keyboard selected")
    assert target_is_owned(), "switching the private engine changed the TextView focus"
    activate_from_owned_tray(companion)
    wait(lambda: bus.get_global_engine() is not None and
         bus.get_global_engine().get_name() == "dev.suzaku.linux.ime",
         "real tray activation selected Suzaku")
    settle_input()
    language("en")
    clear_buffers()
    type_text("please annotate this para")
    expect_buffers()
    key(IBus.KEY_Escape, expect="")
    expect_buffers()
    assert companion.poll() is None and target_is_owned()
    passed_case("real tray activation and Chinese-to-English change preserve every key in an already-focused TextView")

    type_text("hel")
    wait(lambda: presentation_observer.both(False), "companion owns a new GTK draft")
    before_presentation = dict(qa.watch.latest)
    owned_tray_action(companion, ("Hide panel", "隐藏面板"))
    wait(lambda: presentation_observer.both(True), "hiding the companion restores stock lookup and auxiliary")
    unchanged_presentation(before_presentation, True, duration=1.6)
    passed_case("real tray Hide restores stock presentation without editing or committing the draft")

    owned_tray_action(companion, ("Show panel", "显示面板"))
    wait(lambda: presentation_observer.both(False), "redisplayed companion reclaims presentation")
    unchanged_presentation(before_presentation, False, duration=1.6)
    passed_case("real tray Show reclaims stock presentation only after drawing the current draft")

    # Only the process spawned above on the fixture's private display is killed.
    # No shutdown/release is sent: fallback must follow TTL expiry without a key.
    assert companion in qa.processes and companion.poll() is None
    companion.kill()
    companion.wait(timeout=3)
    wait(lambda: presentation_observer.both(True),
         "companion crash restores stock lookup and auxiliary without another key")
    unchanged_presentation(before_presentation, True, duration=.2)
    assert bus.get_global_engine().get_name() == "dev.suzaku.linux.ime"
    commit("hel")
    passed_case("crashed private companion expires to stock presentation with exact draft and one commit")

    assert passed == 24
    print(f"RESULT: {passed} strict in-process GTK3 input/presentation workflows passed", flush=True)
except Exception:
    try:
        diagnose()
    except Exception as error:
        print("GTK3 diagnostic failed:", repr(error), flush=True)
    raise
finally:
    if event_handler_installed:
        Gdk.event_handler_set(dispatch_gtk_event, None)
        event_handler_installed = False
    if presentation_observer is not None:
        presentation_observer.close()
    if window is not None:
        window.destroy()
    try:
        qa.close()
    finally:
        if tray_watcher is not None:
            tray_watcher.close()
