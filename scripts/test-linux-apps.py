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
import sys
import time
import gi

sys.dont_write_bytecode = True

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
editor_observers = {}


def spawn(args, output=None, **kwargs):
    if args[0] == "gnome-text-editor":
        # Enable read-only buffer/save observation only in the owned editor.
        # The runner already created private display, bus and accessibility state.
        from fixtures.editor_observer import editor_environment
        kwargs["env"] = editor_environment(kwargs.pop("env", os.environ), root, subprocess.run)
        editor_config = root / "editor-config" / "gtk-4.0"
        editor_config.mkdir(parents=True, exist_ok=True)
        # Observe save completion without paying for a one-second progress-bar
        # fade on every assertion. This editor-only config never reaches the IME
        # panel, other applications or the user's normal desktop.
        (editor_config / "settings.ini").write_text("[Settings]\ngtk-enable-animations=false\n")
    elif args[0] == "/usr/libexec/ibus-ui-gtk3":
        # The controller observes accessibility; it must not export its own
        # GTK bridge onto that same bus. Enable the bridge only in the target.
        env = dict(kwargs.pop("env", os.environ))
        env.update(NO_AT_BRIDGE="0", GTK_MODULES="atk-bridge")
        env.pop("GTK_A11Y", None)
        kwargs["env"] = env
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


def companion_frame():
    """Read the latest rendered frame from the live, owned companion's opt-in log."""
    for index in range(len(processes) - 1, -1, -1):
        process = processes[index]
        if process.args == [str(bins / "panel")] and process.poll() is None:
            lines = (root / f"process-{index}.log").read_text().splitlines()
            for line in reversed(lines):
                if line.startswith("Suzaku frame: "):
                    try:
                        frame = json.loads(line.removeprefix("Suzaku frame: "))
                    except json.JSONDecodeError:
                        continue  # A concurrent write may not have finished yet.
                    if frame["window"] == "Main":
                        return frame
    return {}


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
    from fixtures.editor_observer import save_observed_document
    assert path in editor_observers, "only prepared, owned editor documents may be observed"
    save_observed_document(editor_observers[path], path, expected,
                           lambda: x.key(IBus.KEY_s, IBus.KEY_Control_L), wait)


def prepare_editor(x, path):
    from fixtures.editor_observer import GtkEditorObserver
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    assert path.parent == root and not path.is_symlink(), "not an owned QA document"
    editors = [process for process in processes if process.poll() is None and
               process.args[0] == "gnome-text-editor" and process.args[-1] == str(path)]
    assert len(editors) == 1, "one owned editor must be registered for this document"
    observer = GtkEditorObserver(Atspi, editors[0], path)
    wait(lambda: observer.state() is not None, "owned editor accessibility ready")
    editor_observers[path] = observer
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

    for seed, word in [("hello", "hello world"), ("please send", "please send me")]:
        clear_document(x, document)
        x.type(seed)
        wait(lambda: seed_is(seed), "whole-word prefix before Space")
        assert any(c["text"] == word and c["kind"] == "word" and "ᵂ" in c["ibus_label"]
                   for c in watch.latest["candidates"][:6])
        assert any(c["kind"] == "sentence" for c in watch.latest["candidates"][:6])
        choose_number(x, word)
        save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(seed), "next-word undo restores the prefix without an extra space")
        choose_number(x, word)
        x.type(" today")
        wait(lambda: seed_is(word + " today"), "continue from one-word suggestion")
        save_document(x, document, "")
        commit(x)
        save_document(x, document, word + " today\n")
    print("PASS: N50 complete-word prefixes retain word/sentence labels, adoption, exact undo and continuation")

    for token in ["hello123", "example.hello"]:
        clear_document(x, document)
        for char in token + " ":
            if char.isdigit():
                x.key(ord(char), IBus.KEY_Alt_L)
            else:
                x.type(char)
        literal = token + " "
        wait(lambda: seed_is(literal), "physical English protected context")
        assert [c["text"] for c in watch.latest["candidates"]] == [literal], watch.latest
        if token == "hello123":
            # Removing the literal digits restores the ordinary `hello` context.
            for _ in "123 ":
                x.key(IBus.KEY_BackSpace)
            wait(lambda: seed_is("hello"), "editing removes the identifier suffix")
            assert any(c["text"] == "hello world" and c["kind"] == "word"
                       for c in watch.latest["candidates"][:6]), watch.latest
            for digit in "123":
                x.key(ord(digit), IBus.KEY_Alt_L)
            x.key(IBus.KEY_space)
            wait(lambda: seed_is(literal), "restore the exact literal identifier")
            assert [c["text"] for c in watch.latest["candidates"]] == [literal], watch.latest
        x.type("please sen")
        seed = literal + "please sen"
        word = literal + "please send"
        sentence = word + " me the details."
        wait(lambda: seed_is(seed), "physical new phrase after an identifier")
        assert any(c["text"] == sentence and c["kind"] == "sentence"
                   for c in watch.latest["candidates"][:6]), watch.latest
        choose_number(x, word)
        save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(seed), "undo restores the whole identifier and new phrase")
        choose_number(x, word)
        x.key(IBus.KEY_space)
        wait(lambda: seed_is(word + " "), "Space continues without committing the identifier")
        choose_number(x, sentence)
        save_document(x, document, "")
        commit(x)
        save_document(x, document, sentence + "\n")
    print("PASS: N59 2 physical English context-boundary workflows: symbols/digits, edit recovery, fresh phrases, word/sentence adoption, undo and exact saved text")

    revision = watch.latest["revision"]
    assert json.loads(command("Lzh-Hans"))["ok"]
    wait(lambda: watch.latest["revision"] > revision, "Chinese tone-boundary language")
    for seed, converted in [
        ("ni3hao3!", "你好!"),
        ('"ni3hao3"', '"你好"'),
        ("hao3.5", "好3.5"),
        ("hao3:30", "好3:30"),
        ("hao3,000", "好3,000"),
    ]:
        clear_document(x, document)
        # An opening quote outside a draft belongs to the application. Start
        # that case from an owned companion draft, then use physical keys.
        start = 0
        if seed.startswith('"'):
            assert command(f'A{watch.latest["host"]} {watch.latest["revision"]} T"') == b"1"
            wait(lambda: seed_is('"'), "owned opening quote")
            start = 1
        for char in seed[start:]:
            if char.isdigit():
                x.key(ord(char), IBus.KEY_Alt_L)
            elif char in {'!', '"', ':'}:
                x.key({'!': IBus.KEY_1, '"': IBus.KEY_apostrophe, ':': IBus.KEY_semicolon}[char],
                      IBus.KEY_Shift_L)
            else:
                x.type(char)
        wait(lambda: seed_is(seed), "literal tone digits and punctuation stay in draft")
        assert watch.latest["candidates"][0]["text"] == converted, (seed, watch.latest)
        choose_number(x, converted)
        save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(seed), "tone-boundary undo restores exact digits and punctuation")
        choose_number(x, converted)
        x.type(" shi")
        wait(lambda: seed_is(converted + " shi"), "continue after punctuated Chinese choice")
        choose_number(x, converted + " 是")
        save_document(x, document, "")
        commit(x)
        save_document(x, document, converted + " 是\n")
    revision = watch.latest["revision"]
    assert json.loads(command("Len"))["ok"]
    wait(lambda: watch.latest["revision"] > revision, "English restored after tone-boundary check")
    print("PASS: N51 physical tone/number input preserves punctuation, numeric literals, undo and continuation")

    for language, prefix, tail, expected, suffix, continued in [
        ("zh-Hans", "你" * 127, "nihao", "你" * 127 + "你好", " de", " 的"),
        ("zh-Hans", "你" * 160, "bei j", "你" * 160 + "北京", " de", " 的"),
        ("zh-Hans", "你" * 251, "nihao", "你" * 251 + "你好", " de", " 的"),
        ("en", "note " * 49, "hel", "note " * 49 + "hello", " wo", " world"),
        ("en", "note " * 60, "please sen", "note " * 60 + "please send", " me the d", " me the details"),
        ("en", "note " * 1000, "hel", "note " * 1000 + "hello", " wo", " world"),
        ("zh-Hans", "你" * 252, "nihao", "你" * 252 + "你好", " de", " 的"),
        ("zh-Hans", "前文。" * 300 + "  ", "bei j", "前文。" * 300 + "  北京", " de", " 的"),
    ]:
        clear_document(x, document)
        revision = watch.latest["revision"]
        assert json.loads(command("L" + language))["ok"]
        wait(lambda: watch.latest["revision"] > revision, "long-tail language setting")
        assert command(f'A{watch.latest["host"]} {watch.latest["revision"]} T{prefix}') == b"1"
        wait(lambda: seed_is(prefix), "owned long prefix in real editor")
        # Only the prefix is supplied by the companion. Conversion, adoption,
        # undo, continuation and commit all use physical XTest key events.
        x.type(tail)
        wait(lambda: seed_is(prefix + tail), "long-prefix physical phonetic tail")
        if language == "zh-Hans":
            assert watch.latest["candidates"][0]["text"] == expected
        choose_number(x, expected)
        save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(prefix + tail), "restore complete long spelling")
        choose_number(x, expected)
        x.type(suffix)
        wait(lambda: seed_is(expected + suffix), "Space continues the long adopted draft")
        choose_number(x, expected + continued)
        save_document(x, document, "")
        commit(x)
        save_document(x, document, expected + continued + "\n")
    print("PASS: 8 long Chinese/English tails preserve numeric adoption, exact undo, continued typing and saved text")

    for language, prefix, reading, word, spacing, suffix in [
        ("zh-Hans", "你" * 252, "ni hao", "你好", "", "，很高兴认识你。"),
        ("zh-Hans", "你" * 254, "hui yi", "会议", " ", "什么时候开始？"),
        ("zh-Hans", "前文。", "wo xiang xue xi zhong wen", "我想学习中文", " ", "，请多指教。"),
        ("en", "note " * 60, "please sen", "please send", " ", "me the details."),
    ]:
        clear_document(x, document)
        revision = watch.latest["revision"]
        assert json.loads(command("L" + language))["ok"]
        wait(lambda: watch.latest["revision"] > revision, "adopted-sentence language setting")
        assert command(f'A{watch.latest["host"]} {watch.latest["revision"]} T{prefix}') == b"1"
        wait(lambda: seed_is(prefix), "owned prefix before physical word adoption")
        x.type(reading)
        wait(lambda: seed_is(prefix + reading), "physical spelling before threshold change")
        choose_number(x, prefix + word)
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(prefix + reading), "restore spelling across local-window threshold")
        choose_number(x, prefix + word)
        x.type(spacing)
        draft = prefix + word + spacing
        sentence = draft + suffix
        wait(lambda: seed_is(draft), "word adoption and Space preserve the draft")
        assert any(c["text"] == sentence and c["kind"] == "sentence" and c["source"] == "local"
                   for c in watch.latest["candidates"][:6])
        save_document(x, document, "")
        x.type(".")
        wait(lambda: seed_is(draft + "."), "physical punctuation remains editable")
        assert [c["text"] for c in watch.latest["candidates"]] == [draft + "."]
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(draft), "physical punctuation deletion restores draft")
        choose_number(x, sentence)
        save_document(x, document, "")
        x.key(IBus.KEY_BackSpace)
        wait(lambda: seed_is(draft), "sentence undo restores word and exact spacing")
        choose_number(x, sentence)
        commit(x)
        save_document(x, document, sentence + "\n")
    print("PASS: N54/N55 4 physical word-to-sentence workflows preserve threshold crossings, punctuation deletion, exact undo and saved text")

    revision = watch.latest["revision"]
    assert json.loads(command("Len"))["ok"]
    wait(lambda: watch.latest["revision"] > revision, "English restored after long-tail checks")

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
    wait(lambda: companion_frame().get("runtime_font") and companion_frame().get("draft") == "hel",
         "real companion rendered the native draft with system fonts", timeout=30)
    assert x.window("Suzaku · Input") is not None
    assert all("draft:" not in title and "committed:" not in title for _, title in x.windows())
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
