#!/usr/bin/env python3
"""XTest -> real GTK application -> IBus -> saved synthetic document.

Never run against an existing display/session. Use test-linux-apps.sh.
"""
import ctypes as C
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import gi

root = Path(os.environ["SUZAKU_APP_QA_ROOT"])
runtime = root / "runtime"
assert os.environ.get("SUZAKU_APP_QA") == "1"
assert root.parent == Path("/tmp") and root.name.startswith("suzaku-app-qa.")
assert not root.is_symlink() and root.stat().st_uid == os.getuid()
assert os.environ["XDG_RUNTIME_DIR"] == str(runtime)
assert os.environ["IBUS_ADDRESS"] == f"unix:path={runtime}/ibus.sock"
assert os.environ.get("DISPLAY") == os.environ["SUZAKU_APP_QA_DISPLAY"]
assert os.environ["DISPLAY"].startswith(":") and not os.environ.get("WAYLAND_DISPLAY")
assert os.environ["SUZAKU_IME_CONFIG"] == str(root / "ime.json")
gi.require_version("IBus", "1.0")
from gi.repository import IBus, GLib

bins = Path(os.environ["SUZAKU_APP_QA_BIN_DIR"])
host_socket = runtime / "suzaku-ime/host.sock"
processes, logs = [], []
watch = None


def spawn(args, output=None, **kwargs):
    log = (root / f"process-{len(processes)}.log").open("w")
    logs.append(log)
    process = subprocess.Popen(args, stdout=log if output is None else output, stderr=log, **kwargs)
    processes.append(process)
    return process


def pump():
    while GLib.MainContext.default().iteration(False):
        pass
    if watch is not None:
        watch.drain()


def wait(check, label, timeout=8):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        pump()
        if check():
            return
        time.sleep(0.01)
    raise AssertionError(label)


def command(request):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(2)
        client.connect(str(host_socket))
        client.sendall(request.encode())
        client.shutdown(socket.SHUT_WR)
        output = b""
        while part := client.recv(8192):
            output += part
        return output


class Watch:
    def __init__(self):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(2)
        self.sock.connect(str(host_socket))
        self.sock.sendall(b"W")
        self.sock.shutdown(socket.SHUT_WR)
        self.sock.setblocking(False)
        self.buffer = b""
        self.latest = None
        self.frames = []

    def drain(self):
        while True:
            try:
                chunk = self.sock.recv(65536)
            except BlockingIOError:
                return
            assert chunk, "native subscription disconnected"
            self.buffer += chunk
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                self.latest = json.loads(line)
                assert self.latest["version"] == 1
                self.frames.append(self.latest)
                if self.latest["private"] or not self.latest["focused"]:
                    assert not self.latest["seed"] and not self.latest["candidates"]


class X11:
    def __init__(self):
        self.x = C.CDLL("libX11.so.6")
        self.xt = C.CDLL("libXtst.so.6")
        prototypes = {
            "XOpenDisplay": ([C.c_char_p], C.c_void_p),
            "XDefaultRootWindow": ([C.c_void_p], C.c_ulong),
            "XQueryTree": ([C.c_void_p, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_ulong),
                            C.POINTER(C.POINTER(C.c_ulong)), C.POINTER(C.c_uint)], C.c_int),
            "XFetchName": ([C.c_void_p, C.c_ulong, C.POINTER(C.c_void_p)], C.c_int),
            "XInternAtom": ([C.c_void_p, C.c_char_p, C.c_int], C.c_ulong),
            "XGetWindowProperty": ([C.c_void_p, C.c_ulong, C.c_ulong, C.c_long, C.c_long,
                                    C.c_int, C.c_ulong, C.POINTER(C.c_ulong), C.POINTER(C.c_int),
                                    C.POINTER(C.c_ulong), C.POINTER(C.c_ulong),
                                    C.POINTER(C.c_void_p)], C.c_int),
            "XFree": ([C.c_void_p], C.c_int),
            "XRaiseWindow": ([C.c_void_p, C.c_ulong], C.c_int),
            "XMoveWindow": ([C.c_void_p, C.c_ulong, C.c_int, C.c_int], C.c_int),
            "XSetInputFocus": ([C.c_void_p, C.c_ulong, C.c_int, C.c_ulong], C.c_int),
            "XGetInputFocus": ([C.c_void_p, C.POINTER(C.c_ulong), C.POINTER(C.c_int)], C.c_int),
            "XKeysymToKeycode": ([C.c_void_p, C.c_ulong], C.c_ubyte),
            "XFlush": ([C.c_void_p], C.c_int),
            "XSync": ([C.c_void_p, C.c_int], C.c_int),
            "XCloseDisplay": ([C.c_void_p], C.c_int),
        }
        for name, (args, result) in prototypes.items():
            function = getattr(self.x, name)
            function.argtypes, function.restype = args, result
        self.xt.XTestFakeKeyEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        self.xt.XTestFakeKeyEvent.restype = C.c_int
        self.xt.XTestFakeMotionEvent.argtypes = [C.c_void_p, C.c_int, C.c_int, C.c_int, C.c_ulong]
        self.xt.XTestFakeMotionEvent.restype = C.c_int
        self.xt.XTestFakeButtonEvent.argtypes = [C.c_void_p, C.c_uint, C.c_int, C.c_ulong]
        self.xt.XTestFakeButtonEvent.restype = C.c_int
        self.display = self.x.XOpenDisplay(os.environ["SUZAKU_APP_QA_DISPLAY"].encode())
        assert self.display
        self.root = self.x.XDefaultRootWindow(self.display)
        self.net_name = self.x.XInternAtom(self.display, b"_NET_WM_NAME", 0)

    def title(self, window):
        name = C.c_void_p()
        if self.x.XFetchName(self.display, window, C.byref(name)) and name.value:
            try:
                return C.string_at(name).decode("utf-8", "replace")
            finally:
                self.x.XFree(name)
        # Chromium supplies UTF-8 _NET_WM_NAME instead of legacy WM_NAME.
        actual, count, remaining, format_bits = C.c_ulong(), C.c_ulong(), C.c_ulong(), C.c_int()
        result = self.x.XGetWindowProperty(self.display, window, self.net_name, 0, 1024, 0, 0,
                                          C.byref(actual), C.byref(format_bits), C.byref(count),
                                          C.byref(remaining), C.byref(name))
        try:
            if result == 0 and format_bits.value == 8 and name.value:
                return C.string_at(name, count.value).decode("utf-8", "replace")
            return ""
        finally:
            if name.value:
                self.x.XFree(name)

    def windows(self):
        root_window, parent = C.c_ulong(), C.c_ulong()
        children, count = C.POINTER(C.c_ulong)(), C.c_uint()
        assert self.x.XQueryTree(self.display, self.root, C.byref(root_window), C.byref(parent),
                                C.byref(children), C.byref(count))
        windows = []
        try:
            for index in range(count.value):
                windows.append((children[index], self.title(children[index])))
        finally:
            if children:
                self.x.XFree(children)
        return windows

    def window(self, needle):
        matches = [window for window, title in self.windows() if needle in title]
        assert len(matches) <= 1, (needle, self.windows())
        return matches[0] if matches else None

    def focus(self, window):
        wait(lambda: "Map State: IsViewable" in subprocess.run(
            ["xwininfo", "-id", str(window)], capture_output=True, text=True, timeout=2).stdout,
            "application window must be mapped before focus")
        self.x.XRaiseWindow(self.display, window)
        self.x.XSetInputFocus(self.display, window, 1, 0)
        self.x.XSync(self.display, 0)
        wait(lambda: self.focused() == window, "X11 application focus")

    def focused(self):
        window, revert = C.c_ulong(), C.c_int()
        self.x.XGetInputFocus(self.display, C.byref(window), C.byref(revert))
        return window.value

    def key(self, keysym, modifier=None):
        modifiers = [] if modifier is None else (modifier if isinstance(modifier, list) else [modifier])
        for symbol in modifiers + [keysym]:
            code = self.x.XKeysymToKeycode(self.display, symbol)
            assert code, f"Key is unavailable on isolated keyboard: {symbol}"
            assert self.xt.XTestFakeKeyEvent(self.display, code, 1, 0)
        self.x.XSync(self.display, 0)
        time.sleep(0.015)
        for symbol in [keysym] + modifiers[::-1]:
            assert self.xt.XTestFakeKeyEvent(self.display, self.x.XKeysymToKeycode(self.display, symbol), 0, 0)
        self.x.XFlush(self.display)
        # These are application-delivered key events, not calls to the engine.
        time.sleep(0.025)
        pump()

    def type(self, text):
        for character in text:
            assert character.isascii() and (character.islower() or character in " ,.'0123456789")
            self.key(ord(character))

    def click(self, x, y, button=1):
        assert self.xt.XTestFakeMotionEvent(self.display, -1, x, y, 0)
        assert self.xt.XTestFakeButtonEvent(self.display, button, 1, 0)
        assert self.xt.XTestFakeButtonEvent(self.display, button, 0, 0)
        self.x.XSync(self.display, 0)
        time.sleep(0.1)
        pump()


def seed_is(text):
    return watch.latest is not None and watch.latest["focused"] and watch.latest["seed"] == text


def settle_input(bus, x, window):
    # A mapped GTK window may still be replacing its initial IM contexts. Wait
    # for the application's real context, not IBus's always-focused fake one.
    last, since = None, time.monotonic()

    def ready():
        nonlocal last, since
        context = bus.current_input_context()
        state = (context, None if watch.latest is None else watch.latest["context"], x.focused())
        if state != last:
            last, since = state, time.monotonic()
        return (context and context != "/org/freedesktop/IBus/InputContext_1" and
                x.focused() == window and watch.latest is not None and
                watch.latest["focused"] and time.monotonic() - since >= 0.5)

    wait(ready, "application IM context ready")


def save_document(x, path, expected):
    x.key(IBus.KEY_s, IBus.KEY_Control_L)
    wait(lambda: path.read_text() == expected, f"saved document differs: expected {expected!r}")


def prepare_editor(x, path):
    # GTK can focus the window before asynchronous file loading has finished.
    # Use only this fixture's empty file for a readiness probe; workflow assertions
    # begin AFTER the probe is discarded. This is not a startup-keystroke test.
    sent = 0.0

    def ready():
        nonlocal sent
        if seed_is("q"):
            return True
        assert watch.latest is None or not watch.latest["seed"], "unexpected startup probe text"
        if time.monotonic() - sent >= 0.5:
            sent = time.monotonic()
            x.key(IBus.KEY_q)
        return seed_is("q")

    wait(ready, "editor accepts real key input after file loading")
    x.key(IBus.KEY_Escape)
    wait(lambda: seed_is(""), "discard synthetic readiness probe")
    clear_document(x, path)


def clear_document(x, path):
    assert seed_is("")
    x.key(IBus.KEY_a, IBus.KEY_Control_L)
    x.key(IBus.KEY_BackSpace)
    save_document(x, path, "")


def commit(x):
    assert watch.latest["seed"]
    x.key(IBus.KEY_Return)
    wait(lambda: seed_is(""), "Enter clears preedit")


def choose_number(x, text):
    slot = next(i for i, candidate in enumerate(watch.latest["candidates"][:6])
                if candidate["text"] == text)
    x.key(IBus.KEY_1 + slot)
    wait(lambda: seed_is(text), "number adopts editable candidate")


def start():
    global watch, x
    spawn(["ibus-daemon", "--single", "--panel=disable", "--config=disable",
           "--emoji-extension=disable",
           "--address=" + os.environ["IBUS_ADDRESS"], "--cache=none"])
    wait(lambda: (runtime / "ibus.sock").exists(), "private IBus startup")
    spawn([str(bins / "linux_ime_host")])
    wait(host_socket.exists, "private native host startup")
    IBus.init()
    bus = IBus.Bus.new()
    assert bus.is_connected()
    assert json.loads(command("Len"))["ok"]
    assert json.loads(command("P0"))["ok"]
    watch = Watch()
    x = X11()
    return bus, x


def check_gtk(bus, x):
    document = root / "suzaku-english-qa.txt"
    document.write_text("")
    editor = spawn(["gnome-text-editor", "--standalone", str(document)])
    wait(lambda: x.window(document.name), "GNOME Text Editor window")
    editor_window = x.window(document.name)
    x.focus(editor_window)
    x.click(200, 100)
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    settle_input(bus, x, editor_window)
    prepare_editor(x, document)
    print("READY: GNOME Text Editor through private GTK/IBus/XTest")
    x.type("hello world")
    wait(lambda: seed_is("hello world"), "real editor continuous English draft")
    save_document(x, document, "")
    assert seed_is("hello world"), "save shortcut prematurely ended the draft"
    commit(x)
    # GtkSourceView adds its conventional final newline when saving nonempty text.
    save_document(x, document, "hello world\n")
    print("PASS: real editor Space stays in preedit, Enter commits exact text")

    clear_document(x, document)
    x.type("hel")
    wait(lambda: seed_is("hel"), "candidate prefix")
    assert {"word", "sentence"} <= {c["kind"] for c in watch.latest["candidates"][:6]}
    choose_number(x, "hello")
    save_document(x, document, "")
    x.key(IBus.KEY_BackSpace)
    wait(lambda: seed_is("hel"), "Backspace undoes candidate adoption")
    choose_number(x, "hello")
    x.type(" world")
    wait(lambda: seed_is("hello world"), "continue after number adoption")
    commit(x)
    save_document(x, document, "hello world\n")
    print("PASS: word/sentence candidates, number adoption, spelling undo and continuation")

    clear_document(x, document)
    x.type("version ")
    for digit in "0123456789":
        x.key(ord(digit), IBus.KEY_Alt_L)
    x.key(IBus.KEY_1, IBus.KEY_Shift_L)
    wait(lambda: seed_is("version 0123456789!"), "Alt digits and layout-resolved Shift punctuation")
    commit(x)
    save_document(x, document, "version 0123456789!\n")
    print("PASS: literal numbers and Shift punctuation arrive without candidate labels")

    clear_document(x, document)
    x.type("hel")
    wait(lambda: seed_is("hel"), "Shift Enter prefix")
    x.key(IBus.KEY_Tab)
    wait(lambda: watch.latest["selected"] == 1, "Tab selects candidate")
    selected = watch.latest["candidates"][1]["text"]
    x.key(IBus.KEY_Return, IBus.KEY_Shift_L)
    wait(lambda: seed_is(selected), "Shift Enter adopts without commit")
    save_document(x, document, "")
    x.key(IBus.KEY_BackSpace)
    wait(lambda: seed_is("hel"), "Shift Enter undo")
    x.key(IBus.KEY_Escape)
    wait(lambda: seed_is(""), "Escape discards draft")
    save_document(x, document, "")
    print("PASS: Tab selection, Shift Enter adoption and Escape cancellation")

    x.type("left right")
    wait(lambda: seed_is("left right"), "caret test draft")
    commit(x)
    x.key(IBus.KEY_Home, IBus.KEY_Control_L)
    x.type("start ")
    wait(lambda: seed_is("start "), "insert at real editor caret")
    commit(x)
    save_document(x, document, "start left right\n")
    x.key(IBus.KEY_Home, IBus.KEY_Control_L)
    x.key(IBus.KEY_Right, [IBus.KEY_Control_L, IBus.KEY_Shift_L])
    x.type("new ")
    wait(lambda: seed_is("new "), "replace editor selection")
    commit(x)
    save_document(x, document, "new  left right\n")
    print("PASS: committed text inserts at caret and replaces the application's selection")

    clear_document(x, document)
    x.type("discardme")
    wait(lambda: seed_is("discardme"), "draft before application switch")
    with (root / "entry.txt").open("w") as output:
        entry = spawn(["zenity", "--entry", "--title=Suzaku QA Entry", "--text=Synthetic text only"], output=output)
        wait(lambda: x.window("Suzaku QA Entry"), "real entry window")
        entry_window = x.window("Suzaku QA Entry")
        x.focus(entry_window)
        settle_input(bus, x, entry_window)
        wait(lambda: seed_is(""), "old draft cleared on application switch")
        x.type("hello entry")
        wait(lambda: seed_is("hello entry"), "real entry preedit")
        commit(x)
        assert entry.poll() is None, "IME Enter also activated the dialog"
        x.key(IBus.KEY_Return)
        wait(lambda: entry.poll() is not None, "empty-preedit Enter activates dialog")
        assert entry.returncode == 0
    assert (root / "entry.txt").read_text() == "hello entry\n"
    x.focus(editor_window)
    settle_input(bus, x, editor_window)
    save_document(x, document, "")
    print("PASS: switching applications clears old preedit; entry submission does not duplicate commit")

    frame_start = len(watch.frames)
    with (root / "hidden.txt").open("w") as output:
        hidden = spawn(["zenity", "--password", "--title=Suzaku QA Hidden"], output=output)
        wait(lambda: x.window("Suzaku QA Hidden"), "hidden entry window")
        hidden_window = x.window("Suzaku QA Hidden")
        x.focus(hidden_window)
        wait(lambda: watch.latest is not None and watch.latest["private"],
             "declared password field protected by IBus")
        x.type("synthetic42")
        x.key(IBus.KEY_Return)
        wait(lambda: hidden.poll() is not None, "hidden entry submit")
        assert hidden.returncode == 0
    assert (root / "hidden.txt").read_text() == "synthetic42\n"
    assert all(not f["seed"] and not f["candidates"] for f in watch.frames[frame_start:])
    x.focus(editor_window)
    settle_input(bus, x, editor_window)
    x.type("public again")
    wait(lambda: seed_is("public again"), "public composition after hidden field")
    commit(x)
    save_document(x, document, "public again\n")
    print("PASS: hidden-entry text stays out of companion snapshots; public input resumes")

    clear_document(x, document)
    x.type("hel")
    wait(lambda: seed_is("hel"), "draft before real companion launch")
    panel = spawn([str(bins / "panel")])
    wait(lambda: any("Suzaku XR Candidate Panel" in title and "font: system-atlas" in title
                     and "draft: hel" in title for _, title in x.windows()),
         "real companion rendered the native draft with system fonts", timeout=30)
    assert x.focused() == editor_window, "companion launch stole application focus"
    x.type("lo world")
    wait(lambda: seed_is("hello world"), "typing with real companion open")
    commit(x)
    save_document(x, document, "hello world\n")
    assert x.focused() == editor_window and panel.poll() is None
    print("PASS: running the real no-focus companion preserves editor focus and input")


def diagnose():
    print("QA failure snapshot:", None if watch is None else watch.latest)
    if "x" in globals():
        print("QA windows:", x.windows(), "focus:", x.focused())
        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk
        Gdk.init([])
        screen = Gdk.get_default_root_window()
        picture = Gdk.pixbuf_get_from_window(screen, 0, 0, screen.get_width(), screen.get_height())
        picture.savev(str(root / "failure.png"), "png", [], [])


def close():
    if watch is not None:
        watch.sock.close()
    for process in processes[::-1]:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
    for log in logs:
        log.close()
    if "x" in globals():
        x.x.XCloseDisplay(x.display)


if __name__ == "__main__":
    try:
        bus, x = start()
        check_gtk(bus, x)
    except Exception:
        diagnose()
        raise
    finally:
        close()
