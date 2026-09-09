#!/usr/bin/env python3
"""Real IBus + companion protocol QA, only on a fresh private D-Bus/IBus session.

Run with SUZAKU_NATIVE_SYNC_QA=1 and XDG_RUNTIME_DIR=/tmp/suzaku-sync-qa.*.
No desktop capture, global input events or desktop engine changes.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
import gi

gi.require_version("IBus", "1.0")
gi.require_version("Pango", "1.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import IBus, GLib, Pango, PangoCairo

runtime = Path(os.environ["XDG_RUNTIME_DIR"])
assert os.environ.get("SUZAKU_NATIVE_SYNC_QA") == "1"
assert str(runtime).startswith("/tmp/suzaku-sync-qa.")
assert os.environ["IBUS_ADDRESS"] == f"unix:path={runtime}/ibus.sock"
assert not os.environ.get("DISPLAY") and not os.environ.get("WAYLAND_DISPLAY")
host_path = Path(__file__).resolve().parents[1] / "target/debug/linux_ime_host"
host_socket = runtime / "suzaku-ime/host.sock"
processes = []
watchers = []
model_requests = []


class ModelFixture(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        model_requests.append(json.loads(self.rfile.read(int(self.headers["Content-Length"]))))
        time.sleep(0.08)
        candidates = [
            {"text": text, "kind": kind} for text, kind in [
                ("helium", "word"), ("helmet", "word"), ("helix", "word"),
                ("hello world", "sentence"), ("hello everyone", "sentence"),
                ("hello internationalization compatibility verification works", "sentence"),
            ]
        ]
        body = json.dumps({"choices": [{"message": {"content": json.dumps({"candidates": candidates})}}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


model = HTTPServer(("127.0.0.1", 0), ModelFixture)
model_thread = threading.Thread(target=model.serve_forever, daemon=True)
model_thread.start()


class Watch:
    def __init__(self):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(2)
        self.sock.connect(str(host_socket))
        self.sock.sendall(b"W")
        self.sock.shutdown(socket.SHUT_WR)
        self.sock.setblocking(False)
        self.buffer = b""
        self.frames = []
        self.latest = None
        watchers.append(self)

    def drain(self):
        while True:
            try:
                chunk = self.sock.recv(65536)
            except BlockingIOError:
                break
            if not chunk:
                break
            self.buffer += chunk
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                frame = json.loads(line)
                assert frame["version"] == 1
                assert "committed_text" not in frame and "surrounding_text" not in frame
                if not frame["focused"] or frame["private"]:
                    assert not frame["seed"] and not frame["candidates"]
                self.frames.append(frame)
                self.latest = frame
            assert len(self.buffer) <= 65536


def pump():
    while GLib.MainContext.default().iteration(False):
        pass
    for watcher in watchers:
        watcher.drain()


def wait(check, label, timeout=5):
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
        result = b""
        while True:
            part = client.recv(8192)
            if not part:
                return result
            result += part


def action(frame, op):
    return command(f'A{frame["host"]} {frame["revision"]} {op}') == b"1"


def type_seed(context, seed):
    for char in seed:
        assert context.process_key_event(ord(char), 0, 0)
    pump()


def create_context(bus, name):
    context = bus.create_input_context(name)
    context.set_capabilities(IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.AUXILIARY_TEXT)
    return context


def observe_lookup(context):
    lookup = {"count": 0, "selected": 0, "visible": False, "aux_visible": False}
    def update(_, table, visible):
        labels = [table.get_label(i) for i in range(table.get_page_size())]
        lookup.update(count=table.get_number_of_candidates(), selected=table.get_cursor_pos(),
                      visible=visible, page_size=table.get_page_size(),
                      text=[table.get_candidate(i).get_text() for i in range(table.get_number_of_candidates())],
                      keys=[label.get_text() if label is not None else "" for label in labels])
    context.connect("update-lookup-table", update)
    context.connect("hide-lookup-table", lambda _: lookup.update(visible=False))
    context.connect("update-auxiliary-text", lambda _, text, visible: lookup.update(aux=text.get_text(), aux_visible=visible))
    context.connect("hide-auxiliary-text", lambda _: lookup.update(aux_visible=False))
    return lookup


def check_candidate_presentation(watch, lookup):
    frame = watch.latest
    wait(lambda: lookup["visible"] and lookup["aux_visible"] and
         lookup.get("text") == [c["ibus_label"] for c in frame["candidates"]], "IBus candidate metadata/labels")
    assert lookup["page_size"] == 6
    assert lookup["keys"] == ["¹", "²", "³", "⁴", "⁵", "⁶"]
    assert "Shift+Space" in lookup["aux"]
    assert {"word", "sentence"} <= {c["kind"] for c in frame["candidates"][:6]}
    assert any(c["text"] == frame["seed"] for c in frame["candidates"])
    for candidate in frame["candidates"]:
        assert candidate["source"] in ["local", "model"]
        assert 0 <= candidate["weight"] <= 100
        assert len(candidate["ibus_label"]) <= 42
        assert candidate["kind"] == "unknown" or any(marker in candidate["ibus_label"] for marker in ["ᵂ", "ˢ", "ᴿ"])
        assert ("ᴬᴵ" in candidate["ibus_label"]) == (candidate["source"] == "model")
    # Exercise the system's Pango/font fallback, not only Rust's GPU atlas.
    layout = Pango.Layout.new(PangoCairo.FontMap.get_default().create_context())
    for size in [10, 12, 18]:
        layout.set_font_description(Pango.FontDescription.from_string(f"Sans {size}"))
        layout.set_text(" ".join(lookup["keys"] + lookup["text"] + [lookup["aux"]]), -1)
        assert layout.get_unknown_glyphs_count() == 0, "IBus labels contain missing glyphs"


def check_mixed_keyboard(context, watch, commits, lookup):
    for language, prefix, rest, converted in [
        ("en", "please", "sen", "please send"),
        ("zh-Hans", "ni", "hao", "你好"),
        ("ja", "nihongo", "wobenkyoushitai", "日本語を勉強したい"),
    ]:
        assert json.loads(command("L" + language))["ok"]
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset continuous composition")
        before = len(commits)
        type_seed(context, prefix)
        assert context.process_key_event(IBus.KEY_space, 0, IBus.ModifierType.SHIFT_MASK)
        wait(lambda: watch.latest["seed"] == prefix + " ", "Shift+Space preserved in preedit")
        type_seed(context, rest)
        wait(lambda: watch.latest["seed"] == prefix + " " + rest, "continue across spelling separator")
        assert len(commits) == before, "Shift+Space committed prematurely"
        assert any(c["text"] == converted for c in watch.latest["candidates"])
    assert json.loads(command("Len"))["ok"]
    for slot in [0, 1, 5]:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset Alt candidate choice")
        type_seed(context, "hel")
        check_candidate_presentation(watch, lookup)
        before = len(commits)
        expected = watch.latest["candidates"][slot]["text"]
        assert context.process_key_event(IBus.KEY_1 + slot, 0, IBus.ModifierType.MOD1_MASK)
        wait(lambda: commits[before:] == [expected] and not watch.latest["seed"], "Alt digit selected wrong candidate")
        wait(lambda: not lookup["visible"] and not lookup["aux_visible"], "candidate and hint dismissal")
    # AltGr and application shortcuts must never turn into candidate commits.
    type_seed(context, "hel")
    before = len(commits)
    for mask in [IBus.ModifierType.MOD1_MASK | IBus.ModifierType.MOD5_MASK,
                 IBus.ModifierType.MOD1_MASK | IBus.ModifierType.CONTROL_MASK,
                 IBus.ModifierType.MOD1_MASK | IBus.ModifierType.SHIFT_MASK]:
        assert not context.process_key_event(IBus.KEY_1, 0, mask)
    pump()
    assert len(commits) == before and watch.latest["seed"] == "hel"
    context.reset()
    wait(lambda: not watch.latest["seed"], "clear mixed keyboard test")
    print("PASS: multilingual word/sentence metadata, six-row superscript labels, Shift+Space continuation, Alt candidate keys and hint dismissal")


def check_editable_completions(context, watch, commits, lookup):
    def choose(text):
        index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == text)
        # Up on row zero is an explicit selection too; it must not insert anything.
        assert context.process_key_event(IBus.KEY_Up, 0, 0)
        for _ in range(index):
            assert context.process_key_event(IBus.KEY_Tab, 0, 0)
        wait(lambda: watch.latest["selected"] == index, "select editable completion")

    for language, seed, word in [("zh-Hans", "woxihuanbei", "我喜欢北京"),
                                  ("ja", "watashihanihong", "私は日本語"),
                                  ("ja", "nihongowobenky", "日本語を勉強")]:
        assert json.loads(command("L" + language))["ok"]
        type_seed(context, seed)
        check_candidate_presentation(watch, lookup)
        assert any(c["text"] == word and c["kind"] == "word" for c in watch.latest["candidates"][:6])
        before = len(commits)
        choose(word)
        old = watch.latest
        assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
        wait(lambda: watch.latest["seed"] == word, "CJK completion remains in preedit")
        assert len(commits) == before
        assert not action(old, "K0"), "pre-completion revision was still accepted"
        assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: watch.latest["seed"] == seed, "CJK completion undo preserved exact spelling")
    assert json.loads(command("Len"))["ok"]
    for key, expected in [(IBus.KEY_space, "hello "), (IBus.KEY_Return, "hello"), (IBus.KEY_KP_Enter, "hello")]:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset editable completion")
        type_seed(context, "hel")
        choose("hello")
        before = len(commits)
        assert context.process_key_event(key, 0, IBus.ModifierType.SHIFT_MASK)
        assert not context.process_key_event(key, 0, IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.RELEASE_MASK)
        wait(lambda: watch.latest["seed"] == expected, "selected English word was discarded on continuation")
        assert len(commits) == before
        assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: watch.latest["seed"] == "hel", "immediate Backspace did not undo completion")
    choose("hello")
    before = len(commits)
    for mask in [IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.MOD5_MASK,
                 IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.CONTROL_MASK]:
        for key in [IBus.KEY_space, IBus.KEY_Return]:
            assert not context.process_key_event(key, 0, mask)
    pump()
    assert watch.latest["seed"] == "hel" and len(commits) == before
    assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
    wait(lambda: watch.latest["seed"] == "hello", "English completion before normal edit")
    type_seed(context, "x")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == "hello", "ordinary edit/Backspace")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == "hell", "undo remained armed after ordinary editing")

    # Continue in romaji/pinyin after accepting a converted word, then commit exactly once.
    for language, seed, converted, suffix, completed in [
        ("zh-Hans", "nihao", "你好", "shij", "你好世界"),
        ("ja", "nihongo", "日本語", "wobenky", "日本語を勉強"),
    ]:
        assert json.loads(command("L" + language))["ok"]
        type_seed(context, seed)
        before = len(commits)
        assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
        wait(lambda: watch.latest["seed"] == converted, "accept first CJK word")
        type_seed(context, suffix)
        choose(completed)
        assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
        wait(lambda: watch.latest["seed"] == completed, "continue after converted prefix")
        assert len(commits) == before
        assert context.process_key_event(IBus.KEY_Return, 0, 0)
        wait(lambda: commits[before:] == [completed] and not watch.latest["seed"], "commit completed phrase once")

    # Undo cannot resurrect a previous context, privacy state or dismissed composition.
    assert json.loads(command("Len"))["ok"]
    for boundary in ["reset", "escape", "language", "privacy", "focus"]:
        type_seed(context, "hel")
        choose("hello")
        assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
        wait(lambda: watch.latest["seed"] == "hello", "prepare completion boundary")
        if boundary == "reset":
            context.reset()
        elif boundary == "escape":
            assert context.process_key_event(IBus.KEY_Escape, 0, 0)
        elif boundary == "language":
            assert json.loads(command("Lja"))["ok"]
            assert json.loads(command("Len"))["ok"]
        elif boundary == "privacy":
            context.set_content_type(IBus.InputPurpose.FREE_FORM, 1 << 11)
            wait(lambda: watch.latest["private"], "privacy boundary")
            context.set_content_type(IBus.InputPurpose.FREE_FORM, 0)
            wait(lambda: not watch.latest["private"], "leave privacy boundary")
        else:
            previous = watch.latest["context"]
            context.focus_out()
            wait(lambda: watch.latest["context"] != previous and not watch.latest["seed"], "completion focus-out")
            context.focus_in()
            wait(lambda: watch.latest["focused"], "completion refocus")
        wait(lambda: not watch.latest["seed"], "clear completion boundary")
        before = len(commits)
        type_seed(context, "z")
        assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: not watch.latest["seed"], "old completion resurrected after boundary")
        assert len(commits) == before
    print("PASS: CJK unfinished-tail completion, Shift+Enter/Shift+Space editable choices, one-step spelling undo, exact phrase commits and reset/privacy/focus isolation")


def check_nonblocking_ipc(context, watch):
    context.reset()
    wait(lambda: not watch.latest["seed"], "reset IPC latency context")
    samples = []
    for payload in [b"", b"A"]:
        with socket.socket(socket.AF_UNIX) as slow:
            slow.settimeout(2)
            slow.connect(str(host_socket))
            if payload:
                slow.sendall(payload)
            time.sleep(0.05)
            started = time.monotonic()
            assert context.process_key_event(ord("x"), 0, 0)
            elapsed = time.monotonic() - started
            samples.append(elapsed * 1000)
            assert elapsed < 0.35, f"partial IPC request stalled native keys for {elapsed:.3f}s"
    context.reset()
    wait(lambda: not watch.latest["seed"], "clear IPC latency context")
    print(f"PASS: incomplete IPC clients do not block native key events ({samples!r} ms)")


def check_ipc_boundaries(bus, context, watch, commits):
    # EOF, not a partial packet, is the operation boundary.
    with socket.socket(socket.AF_UNIX) as partial:
        partial.settimeout(2)
        partial.connect(str(host_socket))
        partial.sendall(f'A{watch.latest["host"]} {watch.latest["revision"]} The'.encode())
        time.sleep(0.05)
        pump()
        assert not watch.latest["seed"] and not commits
        partial.sendall(b"llo")
        partial.shutdown(socket.SHUT_WR)
        assert partial.recv(1) == b"1"
    wait(lambda: watch.latest["seed"] == "hello", "fragmented replacement")
    context.reset()
    wait(lambda: not watch.latest["seed"], "reset fragmented replacement")

    assert command("C" + "x" * 65535) == b"0", "oversized input must not commit its prefix"
    assert command("Cbad\0text") == b"0", "embedded NUL must not truncate a request"
    with socket.socket(socket.AF_UNIX) as invalid_utf8:
        invalid_utf8.settimeout(2)
        invalid_utf8.connect(str(host_socket))
        invalid_utf8.sendall(b"C\xe6\x97")  # Truncated first codepoint of 日本.
        invalid_utf8.shutdown(socket.SHUT_WR)
        assert invalid_utf8.recv(1) == b"0", "invalid UTF-8 must not commit"
    pump()
    assert not commits and not watch.latest["seed"]
    with socket.socket(socket.AF_UNIX) as split_utf8:
        split_utf8.settimeout(2)
        split_utf8.connect(str(host_socket))
        split_utf8.sendall(b"C\xe6\x97")
        time.sleep(0.03)
        pump()
        assert not commits
        split_utf8.sendall("日本".encode()[2:])
        split_utf8.shutdown(socket.SHUT_WR)
        assert split_utf8.recv(1) == b"1"
    wait(lambda: commits == ["日本"], "UTF-8 codepoint split across reads")
    commits.clear()
    # Validate the byte boundary, not character count.
    exact_text = " \t" + "x" * (65534 - len(" \t\n日本".encode())) + "\n日本"
    assert len(exact_text.encode()) == 65534
    assert command("C" + exact_text) == b"1"
    wait(lambda: commits == [exact_text], "maximum-size commit with intact whitespace/Unicode")
    commits.clear()

    # A continuous trickle does not extend the absolute deadline.
    with socket.socket(socket.AF_UNIX) as trickle:
        trickle.settimeout(2)
        trickle.connect(str(host_socket))
        started = time.monotonic()
        closed = False
        for _ in range(14):
            try:
                trickle.sendall(b"C")
                time.sleep(0.1)
                try:
                    closed = trickle.recv(1, socket.MSG_DONTWAIT) == b""
                except BlockingIOError:
                    pass
            except (BrokenPipeError, ConnectionResetError):
                closed = True
            if closed:
                break
        assert closed and time.monotonic() - started < 1.5, "trickle extended IPC deadline"
    pump()
    assert not commits

    # Awaiting a legacy commit must not redirect it after focus changes.
    other = create_context(bus, "suzaku-transport-focus-qa")
    other_commits = []
    other.connect("commit-text", lambda _, text: other_commits.append(text.get_text()))
    with socket.socket(socket.AF_UNIX) as late:
        late.settimeout(2)
        late.connect(str(host_socket))
        late.sendall(b"Cmust not reach another app")
        time.sleep(0.05)
        previous = watch.latest["context"]
        context.focus_out()
        other.focus_in()
        wait(lambda: watch.latest["focused"] and watch.latest["context"] != previous, "switch pending-commit context")
        late.shutdown(socket.SHUT_WR)
        assert late.recv(1) == b"0"
    pump()
    assert not commits and not other_commits
    other.focus_out()
    context.focus_in()
    wait(lambda: watch.latest["focused"] and not watch.latest["seed"], "restore transport test context")

    # Bounded clients, including peers that abandon their response, release descriptors.
    fd_dir = Path(f"/proc/{processes[1].pid}/fd")
    baseline_fds = len(list(fd_dir.iterdir()))
    clients = []
    try:
        for _ in range(32):
            client = socket.socket(socket.AF_UNIX)
            clients.append(client)
            client.settimeout(2)
            client.connect(str(host_socket))
            # Give the listener time to accept/reject each peer: measure the
            # application cap, not the kernel's short Unix-socket backlog.
            time.sleep(0.005)
        time.sleep(0.05)
        started = time.monotonic()
        assert context.process_key_event(ord("x"), 0, 0)
        assert time.monotonic() - started < 0.35, "client limit stalled native keys"
        assert len(list(fd_dir.iterdir())) <= baseline_fds + 18, "unbounded pending clients"
    finally:
        for client in clients:
            client.close()
    wait(lambda: len(list(fd_dir.iterdir())) <= baseline_fds + 2, "pending clients leaked descriptors")
    for _ in range(32):
        with socket.socket(socket.AF_UNIX) as abandoned:
            abandoned.settimeout(2)
            abandoned.connect(str(host_socket))
            abandoned.sendall(b"S")
            abandoned.shutdown(socket.SHUT_WR)
        time.sleep(0.005)
    wait(lambda: len(list(fd_dir.iterdir())) <= baseline_fds + 2, "abandoned replies leaked descriptors")
    context.reset()
    wait(lambda: not watch.latest["seed"], "clear transport QA context")
    assert command("Q") == b"1"
    assert not commits
    print("PASS: fragmented requests, exact size/UTF-8 boundaries, trickle deadline, context-bound legacy commits and bounded client/descriptor cleanup")


def check_english_key_regressions(context, watch, commits):
    cases = [
        (prefix, [(ord(c), 0, True) for c in suffix], prefix + suffix)
        for prefix, suffix in [("hel", "2"), ("v", "123"), ("", "2026"), ("a", "0456789")]
    ]
    for key in [IBus.KEY_Shift_L, IBus.KEY_Shift_R, IBus.KEY_Caps_Lock,
                IBus.KEY_Control_L, IBus.KEY_ISO_Level3_Shift, IBus.KEY_F1]:
        cases.append(("hel", [(key, 0, False), (ord("L"), IBus.ModifierType.SHIFT_MASK, True)], "helL"))
    cases.append(("v", [(IBus.KEY_KP_1, 0, True)], "v1"))
    cases.append(("hel", [(IBus.KEY_c, IBus.ModifierType.CONTROL_MASK, False)], "hel"))
    cases.append(("hel", [(IBus.KEY_x, IBus.ModifierType.RELEASE_MASK, False)], "hel"))
    failures = []
    for prefix, keys, expected in cases:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset keyboard regression context")
        type_seed(context, prefix)
        before = len(commits)
        handled = [context.process_key_event(key, 0, mask) for key, mask, _ in keys]
        try:
            assert handled == [accepted for _, _, accepted in keys], f"handled={handled}"
            wait(lambda: watch.latest["seed"] == expected, f"expected preedit {expected!r}", timeout=1)
            assert len(commits) == before, f"unexpected commits: {commits[before:]!r}"
            assert watch.latest["candidates"][0]["text"] == expected
            assert context.process_key_event(IBus.KEY_space, 0, 0)
            wait(lambda: commits[before:] == [expected + " "] and not watch.latest["seed"],
                 f"literal commit changed {expected!r}")
        except AssertionError as error:
            failures.append(f"{prefix!r} + {[hex(k) for k, _, _ in keys]}: {error}; "
                            f"preedit={watch.latest['seed']!r}, commits={commits[before:]!r}")
    context.reset()
    wait(lambda: not watch.latest["seed"], "clear keyboard regression context")
    assert not failures, "English key regressions:\n" + "\n".join(failures)
    type_seed(context, "hel")
    assert context.process_key_event(IBus.KEY_Tab, 0, 0)
    wait(lambda: watch.latest["selected"] == 1, "Tab selects an English completion")
    before = len(commits)
    assert not context.process_key_event(IBus.KEY_Shift_L, 0, 0)
    assert not context.process_key_event(ord("!"), 0, IBus.ModifierType.SHIFT_MASK)
    wait(lambda: commits[before:] == ["hello"] and not watch.latest["seed"],
         "punctuation commits the selected word and is forwarded to the app")


try:
    processes.append(subprocess.Popen(["ibus-daemon", "--single", "--address=" + os.environ["IBUS_ADDRESS"], "--cache=none"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    wait(lambda: (runtime / "ibus.sock").exists(), "isolated IBus did not start")
    processes.append(subprocess.Popen([str(host_path)], stdout=subprocess.DEVNULL))
    wait(host_socket.exists, "native host did not start")
    IBus.init()
    bus = IBus.Bus.new()
    assert bus.is_connected()
    context = create_context(bus, "suzaku-sync-qa-a")
    commits = []
    lookup = observe_lookup(context)
    context.connect("commit-text", lambda _, text: commits.append(text.get_text()))
    context.focus_in()
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    wait(lambda: context.get_engine() is not None and context.get_engine().get_name() == "dev.suzaku.linux.ime", "engine attach")
    watch = Watch()
    wait(lambda: watch.latest is not None and watch.latest["focused"], "initial subscription")
    assert not watch.latest["seed"] and not watch.latest["candidates"]
    assert json.loads(command("Len"))["ok"]
    check_nonblocking_ipc(context, watch)
    check_ipc_boundaries(bus, context, watch, commits)
    type_seed(context, "hel")
    wait(lambda: watch.latest["seed"] == "hel" and lookup["visible"], "typed snapshot")
    old = watch.latest
    assert len(old["candidates"]) == lookup["count"]
    check_candidate_presentation(watch, lookup)
    late = Watch()
    wait(lambda: late.latest is not None, "late subscriber snapshot")
    assert late.latest == old
    assert context.process_key_event(IBus.KEY_Tab, 0, 0)
    wait(lambda: watch.latest["selected"] == 1 and lookup["selected"] == 1, "selection sync")
    assert not action(old, "K0"), "stale candidate was committed"
    assert not commits
    assert action(watch.latest, "Thello")
    wait(lambda: watch.latest["seed"] == "hello", "word completion preedit")
    expected = watch.latest["candidates"][0]["text"]
    assert action(watch.latest, "K0")
    wait(lambda: commits == [expected] and not watch.latest["seed"] and not lookup["visible"], "panel commit/dismissal")
    assert not watch.latest["candidates"]
    assert action(watch.latest, "Thel"), "on-screen typing could not restart composition"
    wait(lambda: watch.latest["seed"] == "hel", "restart composition")
    old = watch.latest
    context.focus_out()
    other = create_context(bus, "suzaku-sync-qa-b")
    other_lookup = observe_lookup(other)
    other.focus_in()
    wait(lambda: watch.latest["context"] > old["context"] and not watch.latest["seed"], "context transition")
    assert not action(old, "K0"), "old candidate reached a different input context"
    other_commits = []
    other.connect("commit-text", lambda _, text: other_commits.append(text.get_text()))
    for language, seed in [("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(command("L" + language))["ok"]
        type_seed(other, seed)
        wait(lambda: watch.latest["seed"] == seed and watch.latest["language"] == language, "multilingual sync")
        check_candidate_presentation(watch, other_lookup)
        expected = watch.latest["candidates"][0]["text"]
        assert action(watch.latest, "K0")
        wait(lambda: other_commits and other_commits[-1] == expected and not watch.latest["seed"], "multilingual commit")
        type_seed(other, seed)
        wait(lambda: watch.latest["seed"] == seed, "CJK number-key preedit")
        before = len(other_commits)
        assert other.process_key_event(IBus.KEY_1, 0, 0)
        wait(lambda: other_commits[before:] == [expected] and not watch.latest["seed"],
             "CJK numeric candidate selection changed")
    assert json.loads(command("Len"))["ok"]
    check_english_key_regressions(other, watch, other_commits)
    check_mixed_keyboard(other, watch, other_commits, other_lookup)
    check_editable_completions(other, watch, other_commits, other_lookup)
    type_seed(other, "hel")
    # A deterministic local model fixture tests asynchronous publication, not model quality.
    settings = json.loads(command("S"))["settings"]
    settings.update(llm_enabled=True, llm_model="synthetic-model", llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
    Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
    assert json.loads(command("R"))["ok"]
    # Tone must reach the actual native provider without clearing the composition.
    for temperature in [2, 4, 7]:
        other.reset()
        wait(lambda: not watch.latest["seed"], "reset tone test")
        type_seed(other, "hel")
        requested_before = len(model_requests)
        response = json.loads(command("U" + json.dumps({"llm_temperature_tenths": temperature})))
        assert response["ok"], response
        assert response["settings"]["llm_temperature_tenths"] == temperature
        assert response["settings"]["llm_endpoint"] == settings["llm_endpoint"]
        assert response["settings"]["llm_model"] == settings["llm_model"]
        saved = json.loads(Path(os.environ["SUZAKU_IME_CONFIG"]).read_text())
        assert saved["llm_temperature_tenths"] == temperature
        wait(lambda: any(abs(request["temperature"] - temperature / 10) < 1e-6 for request in model_requests[requested_before:]),
             "Tone did not reach the native model request")
        assert watch.latest["seed"] == "hel", "Tone cleared pending input"
    confirmed_settings = json.loads(command("S"))["settings"]
    for patch in [{"llm_temperature_tenths": 11}, {"llm_enabled": False, "llm_temperature_tenths": -1}, {"llm_model": "unexpected"}]:
        rejected = json.loads(command("U" + json.dumps(patch)))
        assert not rejected["ok"] and rejected["settings"] == confirmed_settings
    # A disk failure must not apply a value that cannot survive host restart.
    runtime.chmod(0o500)
    try:
        rejected = json.loads(command('U{"llm_temperature_tenths":2}'))
        assert not rejected["ok"] and rejected["settings"] == confirmed_settings
    finally:
        runtime.chmod(0o700)
    assert json.loads(Path(os.environ["SUZAKU_IME_CONFIG"]).read_text()) == confirmed_settings
    other.reset()
    wait(lambda: not watch.latest["seed"], "reset after tone test")
    type_seed(other, "hel")
    wait(lambda: any(" · AI" in c["label"] for c in watch.latest["candidates"]), "asynchronous AI candidates were not pushed")
    ai_index = max(i for i, c in enumerate(watch.latest["candidates"]) if " · AI" in c["label"])
    assert ai_index >= 6, "fixture must cover a candidate beyond the first lookup page"
    check_candidate_presentation(watch, other_lookup)
    expected = watch.latest["candidates"][ai_index]["text"]
    assert "…" in watch.latest["candidates"][ai_index]["ibus_label"], "fixture must cover a truncated preview"
    before = len(other_commits)
    for _ in range(ai_index):
        assert other.process_key_event(IBus.KEY_Tab, 0, 0)
    wait(lambda: watch.latest["selected"] == ai_index, "navigate to a later AI candidate")
    assert other.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
    wait(lambda: watch.latest["seed"] == expected, "full AI sentence did not remain editable")
    assert len(other_commits) == before, "accepting an AI completion committed it"
    assert other.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == "hel", "AI completion undo lost the original spelling")
    wait(lambda: any(c["text"] == expected for c in watch.latest["candidates"]), "AI suggestions after completion undo")
    ai_index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == expected)
    for _ in range(ai_index):
        assert other.process_key_event(IBus.KEY_Tab, 0, 0)
    wait(lambda: watch.latest["selected"] == ai_index, "restore later-page AI choice")
    assert other.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: other_commits[before:] == [expected] and not watch.latest["seed"], "commit later-page AI candidate")
    type_seed(other, "hel")
    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "model for page navigation")
    assert other.process_key_event(IBus.KEY_Page_Down, 0, 0)
    wait(lambda: watch.latest["selected"] == 6 and other_lookup["selected"] == 6, "second lookup page")
    assert other.process_key_event(IBus.KEY_Page_Up, 0, 0)
    wait(lambda: watch.latest["selected"] == 0, "first lookup page")
    assert other.process_key_event(IBus.KEY_Page_Down, 0, 0)
    wait(lambda: watch.latest["selected"] == 6, "restore second lookup page")
    expected = watch.latest["candidates"][6]["text"]
    before = len(other_commits)
    assert other.process_key_event(IBus.KEY_1, 0, IBus.ModifierType.MOD1_MASK)
    wait(lambda: other_commits[before:] == [expected] and not watch.latest["seed"], "page-local Alt+1 commit")
    type_seed(other, "hel")
    wait(lambda: any(" · AI" in c["label"] for c in watch.latest["candidates"]), "AI candidates after commit")
    model_count = len(model_requests)
    old = watch.latest
    other.set_content_type(IBus.InputPurpose.FREE_FORM, 1 << 11)
    wait(lambda: watch.latest["private"], "privacy transition")
    start = len(watch.frames)
    type_seed(other, "private")
    wait(lambda: len(watch.frames) > start, "private redacted frame")
    assert len(model_requests) == model_count, "private input triggered a model request"
    assert not action(old, "K0")
    assert all(not f["seed"] and not f["candidates"] for f in watch.frames[start:])
    other.set_content_type(IBus.InputPurpose.PASSWORD, 1 << 11)
    pump()
    assert not other.process_key_event(IBus.KEY_a, 0, 0)
    assert not action(old, "Thello")
    other.set_content_type(IBus.InputPurpose.FREE_FORM, 0)
    wait(lambda: not watch.latest["private"], "leave privacy")
    type_seed(other, "hel")
    assert other.process_key_event(IBus.KEY_Escape, 0, 0)
    wait(lambda: not watch.latest["seed"] and not watch.latest["candidates"], "Escape clears both views")
    type_seed(other, "hel")
    context_id = watch.latest["context"]
    other.focus_out()
    # IBus global-engine mode may focus its empty fallback context immediately.
    wait(lambda: not watch.latest["seed"] and watch.latest["context"] > context_id, "focus-out clears view")
    old_host = watch.latest["host"]
    processes[-1].terminate()
    processes[-1].wait(timeout=3)
    processes.append(subprocess.Popen([str(host_path)], stdout=subprocess.DEVNULL))
    def restarted():
        try:
            return command("Q") == b"1"
        except OSError:
            return False
    wait(restarted, "restarted host unavailable")
    assert json.loads(command("S"))["settings"] == confirmed_settings, "Tone did not survive host restart"
    fresh = Watch()
    wait(lambda: fresh.latest is not None, "reconnected subscription")
    assert fresh.latest["host"] != old_host
    other.focus_in()
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    wait(lambda: fresh.latest["focused"], "restarted focus")
    assert json.loads(command("P0"))["ok"]
    type_seed(other, "hel")
    wait(lambda: fresh.latest["seed"] == "hel", "restarted composition")
    forged = dict(fresh.latest, host=old_host)
    assert not action(forged, "K0"), "an earlier host's token was accepted"
    assert action(fresh.latest, "K0")
    wait(lambda: not fresh.latest["seed"], "reconnected commit")
    # Exercise the shipped probe too; its English AI path must not use digit shortcuts.
    for mode, enabled, expected in [("--complete", "P0", "hello "), ("--llm", "P1", "helium")]:
        assert json.loads(command(enabled))["ok"]
        probe = subprocess.run([str(host_path.with_name("linux_ime_probe")), mode, "hel"],
                               capture_output=True, text=True, timeout=15)
        assert probe.returncode == 0, probe.stderr
        assert f"committed={json.dumps(expected)}" in probe.stdout, probe.stdout
        print(probe.stdout.strip())
    print("PASS: push, late subscriber, exact candidates, selection, replacement, commit, stale revisions, context switches, English digits/modifiers/punctuation, CJK number keys, later-page AI selection, Tone requests/persistence/write failure, private/password, Escape/focus-out and host restart")
finally:
    model.shutdown()
    model.server_close()
    for watcher in watchers:
        watcher.sock.close()
    for process in reversed(processes):
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
