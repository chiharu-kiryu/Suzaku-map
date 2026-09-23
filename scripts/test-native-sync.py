#!/usr/bin/env python3
"""Real IBus + companion protocol QA, only on a fresh private D-Bus/IBus session.

Use scripts/test-linux-ci.sh ibus to build and supply the native test executable.
The fixture requires SUZAKU_NATIVE_SYNC_QA=1 and XDG_RUNTIME_DIR=/tmp/suzaku-sync-qa.*.
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
from gi.repository import IBus, GLib, Gio, Pango, PangoCairo

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
model_reply_gates = {}


class ModelFixture(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        model_requests.append(request)
        gate = model_reply_gates.get(request["model"])
        if gate is not None:
            gate["received"].set()
            assert gate["release"].wait(timeout=5), "synthetic model reply gate timed out"
        time.sleep(0.08)
        candidates = [
            {"text": text, "kind": kind} for text, kind in [
                ("helium", "word"), ("helmet", "word"), ("helix", "word"),
                ("hello world", "sentence"), ("hello everyone", "sentence"),
                ("hello internationalization compatibility verification works", "sentence"),
            ]
        ]
        composition = json.loads(request["messages"][1]["content"])["raw_composition"]
        if composition.startswith("please rec"):
            candidates = [{"text": text, "kind": "sentence"} for text in [
                "please reconsider the proposal.", "please reconsider the schedule."]]
        if composition.startswith("note ") and composition.endswith("hel"):
            candidates = [
                {"text": composition + "ioseismology", "kind": "word"},
                {"text": composition + "ioseismology is interesting.", "kind": "sentence"},
                {"text": composition + "x" * 161, "kind": "word"},
            ]
        body = json.dumps({"choices": [{"message": {"content": json.dumps({"candidates": candidates})}}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        if gate is not None:
            gate["finished"].set()


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
    # Type literal fixture text; unmodified candidate-number keys are tested separately.
    for char in seed:
        mask = IBus.ModifierType.MOD1_MASK if char in "0123456789" else 0
        assert context.process_key_event(IBus.unicode_to_keyval(char), 0, mask)
    pump()


def create_context(bus, name):
    context = bus.create_input_context(name)
    context.set_capabilities(IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.AUXILIARY_TEXT)
    return context


def check_post_process_preedit(bus):
    """Candidate-area drafts stay out of the client; opt-in inline drafts clear."""
    inline = os.environ.get("SUZAKU_IBUS_INLINE_PREEDIT") == "1"
    assert json.loads(command("P0"))["ok"]
    assert json.loads(command("Len"))["ok"]
    checked = 0
    for client_commit in [False, True]:
        for operation in ["enter", "escape", "backspace", "delete word", "reset", "focus out"]:
            context = create_context(bus, "suzaku-post-process-qa")
            context.set_client_commit_preedit(client_commit)
            context.set_post_process_key_event(True)
            preedit = {"text": "", "visible": False}
            auxiliary = {"text": "", "visible": False}
            commits = []
            updates = []

            def update(_, text, cursor, visible, *mode, state=preedit, history=updates):
                state.update(text=text.get_text(), visible=visible)
                history.append(dict(state))

            context.connect("update-preedit-text", update)
            context.connect("update-preedit-text-with-mode", update)
            context.connect("hide-preedit-text", lambda _, state=preedit: state.update(visible=False))
            context.connect("update-auxiliary-text", lambda _, text, visible, state=auxiliary:
                            state.update(text=text.get_text(), visible=visible))
            context.connect("hide-auxiliary-text", lambda _, state=auxiliary: state.update(visible=False))
            context.connect("commit-text", lambda _, text, output=commits: output.append(text.get_text()))
            context.focus_in()
            assert bus.set_global_engine("dev.suzaku.linux.ime")
            # Drain the initial FocusIn clear before synchronous post-processing;
            # an older queued signal must not be mistaken for the next key's result.
            wait(lambda: context.get_engine() is not None, "initial synchronous context ready")
            count, changed = len(updates), time.monotonic()

            def settled():
                nonlocal count, changed
                if len(updates) != count:
                    count, changed = len(updates), time.monotonic()
                return time.monotonic() - changed >= 0.15

            wait(settled, "initial context notifications drained")
            try:
                # One non-word letter keeps Backspace and Enter expectations exact.
                assert context.process_key_event(IBus.KEY_x, 0, 0)
                context.post_process_key_event()
                if inline:
                    wait(lambda: preedit == {"text": "x", "visible": True},
                         f"synchronous preedit before {operation}, client_commit={client_commit}")
                else:
                    wait(lambda: auxiliary["visible"] and auxiliary["text"].startswith("Suzaku · x\n"),
                         "candidate-area draft visible to synchronous client")
                    # IBus itself can publish empty resets while switching
                    # engines; only application-owned nonempty text is forbidden.
                    assert all(not item["text"] and not item["visible"] for item in updates)
                    assert preedit == {"text": "", "visible": False}
                if operation == "reset":
                    context.reset()
                elif operation == "focus out":
                    context.focus_out()
                else:
                    key = {"enter": IBus.KEY_Return, "escape": IBus.KEY_Escape,
                           "backspace": IBus.KEY_BackSpace, "delete word": IBus.KEY_BackSpace}[operation]
                    mask = IBus.ModifierType.CONTROL_MASK if operation == "delete word" else 0
                    assert context.process_key_event(key, 0, mask)
                    context.post_process_key_event()
                wait(lambda: preedit == {"text": "", "visible": False},
                     f"stale synchronous preedit after {operation}, client_commit={client_commit}")
                wait(lambda: not auxiliary["visible"], "candidate-area draft clears on boundary")
                if not inline:
                    assert all(not item["text"] and not item["visible"] for item in updates), \
                        "candidate-area mode must never populate the application's cache"
                assert commits == (["x"] if operation == "enter" else [])
                checked += 1
            except Exception:
                print("post-process QA:", client_commit, operation, updates, commits,
                      bus.current_input_context(), flush=True)
                raise
            finally:
                context.focus_out()
                context.destroy()
                pump()
    assert checked == 12
    print("PASS: 12 synchronous post-process draft clear/commit/cancel/reset/focus cases; inline:", inline)


def check_tray_activation(bus):
    executable = os.environ["SUZAKU_NATIVE_ACTIVATION_TEST"]
    test = "input_method::tests::native_activation_input_and_release_roundtrip"
    listing = subprocess.run([executable, "--list"], capture_output=True, text=True, timeout=5)
    assert listing.returncode == 0 and f"{test}: test" in listing.stdout.splitlines(), \
        "the native activation test must not silently become a zero-test run"
    arguments = [executable, test, "--exact", "--ignored", "--nocapture", "--test-threads=1"]
    unmarked = dict(os.environ)
    unmarked.pop("SUZAKU_NATIVE_SYNC_QA")
    guarded = subprocess.run(arguments, env=unmarked, capture_output=True, text=True, timeout=5)
    assert guarded.returncode != 0 and "private IBus fixture required" in guarded.stderr, \
        "activation QA must refuse to operate without the private-fixture marker"
    previous = "xkb:us::eng"
    assert previous in [engine.get_name() for engine in bus.list_engines()]
    assert bus.set_global_engine(previous)
    wait(lambda: bus.get_global_engine() is not None and bus.get_global_engine().get_name() == previous,
         "prepare the private activation restore target")
    result = subprocess.run(arguments, capture_output=True, text=True, timeout=20)
    assert result.returncode == 0, result.stdout + result.stderr
    assert bus.get_global_engine().get_name() == previous, "activation test did not restore its private engine"
    print(result.stdout.strip())


def check_whitespace_commit(context, watch, commits):
    """N03: the companion accepts this draft, so Enter must submit it once."""
    assert json.loads(command("P0"))["ok"]
    for language in ["en", "zh", "ja"]:
        assert json.loads(command("L" + language))["ok"]
        pump()
        assert not context.process_key_event(IBus.KEY_space, 0, 0), "empty-field Space must still pass through"
        for draft in [" ", "   ", "\u00a0", " \u3000 "]:
            for click in [False, True]:
                before = len(commits)
                assert action(watch.latest, "T" + draft)
                wait(lambda: watch.latest["seed"] == draft, "accepted whitespace was not mirrored")
                assert any(c["text"] == draft and c["kind"] == "literal" for c in watch.latest["candidates"])
                if click:
                    assert action(watch.latest, "K0")
                else:
                    assert context.process_key_event(IBus.KEY_Return, 0, 0), "N03: Enter did not submit whitespace"
                wait(lambda: commits[before:] == [draft] and not watch.latest["seed"],
                     "N03: whitespace must commit exactly once without changing its content")
                assert not context.process_key_event(IBus.KEY_Return, 0, 0)
                pump()
                assert commits[before:] == [draft], "empty Enter replayed the previous whitespace"
    assert json.loads(command("Len"))["ok"]
    pump()
    before = len(commits)
    assert action(watch.latest, "T   hello")
    wait(lambda: watch.latest["seed"] == "   hello", "prepare whitespace after word deletion")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, IBus.ModifierType.CONTROL_MASK)
    wait(lambda: watch.latest["seed"] == "   ", "word deletion must retain leading spaces")
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == ["   "] and not watch.latest["seed"], "commit whitespace after word deletion")
    commits.clear()
    print("PASS: N03 whitespace-only preedit in English/Chinese/Japanese, exact Enter/click commits, word deletion and empty-field pass-through")


def check_settings_file_boundaries(context, watch, commits):
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and config.is_file() and not config.is_symlink()
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    type_seed(context, "hel")
    wait(lambda: watch.latest["seed"] == "hel", "prepare failed-reload preedit")
    original = runtime / "qa-original-settings.json"
    fifo_target = runtime / "qa-settings.pipe"
    assert not original.exists() and not fifo_target.exists()
    config.rename(original)
    os.mkfifo(fifo_target, mode=0o600)
    latencies = []
    try:
        for kind in ["malformed", "oversized", "invalid-utf8", "directory", "socket", "fifo", "linked-fifo"]:
            listener = None
            try:
                if kind == "malformed":
                    config.write_text("{")
                elif kind == "oversized":
                    config.write_bytes(b" " * 65537)
                elif kind == "invalid-utf8":
                    config.write_bytes(b"\xff")
                elif kind == "directory":
                    config.mkdir(mode=0o700)
                elif kind == "socket":
                    listener = socket.socket(socket.AF_UNIX)
                    listener.bind(str(config))
                elif kind == "fifo":
                    os.mkfifo(config, mode=0o600)
                else:
                    config.symlink_to(fifo_target)
                metadata = config.lstat()
                baseline, before = watch.latest, len(commits)
                started = time.monotonic()
                result = json.loads(command("R"))
                latencies.append((kind, round((time.monotonic() - started) * 1000, 2)))
                assert not result["ok"] and result["settings"] == saved, kind
                assert command("Q") == b"1", f"{kind} blocked the native host"
                pump()
                assert watch.latest == baseline and len(commits) == before, f"{kind} changed the draft"
                assert config.lstat().st_ino == metadata.st_ino, f"{kind} was overwritten"
                assert context.process_key_event(IBus.KEY_x, 0, 0)
                wait(lambda: watch.latest["seed"] == "helx", f"{kind} blocked native keyboard input")
                assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
                wait(lambda: watch.latest["seed"] == "hel", "restore failed-reload seed")
            finally:
                if listener is not None:
                    listener.close()
                if config.is_dir():
                    config.rmdir()
                else:
                    config.unlink(missing_ok=True)
    finally:
        original.rename(config)
        fifo_target.unlink()
    assert json.loads(command("R"))["ok"]
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == ["hel"] and not watch.latest["seed"], "commit after settings recovery")
    # Keep the surrounding protocol tests' initial commit list empty.
    commits.clear()
    print(f"PASS: bad configuration reloads preserve settings/preedit and native keys; recoverable reads in ms: {latencies}")


class EnginePeer:
    """Own two real engine objects so late events can be ordered deterministically.

    These use only the fresh private IBus bus, never an engine on the desktop bus.
    """
    destination = "org.freedesktop.IBus.Suzaku"
    interface = "org.freedesktop.IBus.Engine"

    def __init__(self, bus):
        self.connection = bus.get_connection()
        self.destroyed = False
        self.path = self.connection.call_sync(
            self.destination, "/org/freedesktop/IBus/Factory", "org.freedesktop.IBus.Factory",
            "CreateEngine", GLib.Variant("(s)", ("dev.suzaku.linux.ime",)),
            None, Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
        self.commits = []
        def committed(_connection, _sender, _path, _interface, _signal, params, *_data):
            text = IBus.Serializable.deserialize_object(params.get_child_value(0).get_variant())
            self.commits.append(text.get_text())
        self.subscription = self.connection.signal_subscribe(
            self.destination, self.interface, "CommitText", self.path, None,
            Gio.DBusSignalFlags.NONE, committed)
        self.event("SetCapabilities", GLib.Variant("(u)", (int(
            IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE),)))

    def event(self, method, params=None):
        return self.connection.call_sync(self.destination, self.path, self.interface, method,
                                         params, None, Gio.DBusCallFlags.NONE, 2000, None).unpack()

    def process_key_event(self, keyval, keycode=0, state=0):
        return self.event("ProcessKeyEvent", GLib.Variant("(uuu)", (keyval, keycode, int(state))))[0]

    def click(self, index, button=1, state=0):
        self.event("CandidateClicked", GLib.Variant("(uuu)", (index, button, int(state))))

    def set_content_type(self, purpose, hints=0):
        self.connection.call_sync(
            self.destination, self.path, "org.freedesktop.DBus.Properties", "Set",
            GLib.Variant("(ssv)", (self.interface, "ContentType",
                                  GLib.Variant("(uu)", (int(purpose), int(hints))))),
            None, Gio.DBusCallFlags.NONE, 2000, None)

    def destroy(self):
        self.connection.call_sync(self.destination, self.path, "org.freedesktop.IBus.Service",
                                  "Destroy", None, None, Gio.DBusCallFlags.NONE, 2000, None)
        self.destroyed = True

    def close(self):
        if not self.destroyed:
            self.event("FocusOut")
            self.destroy()
        self.connection.signal_unsubscribe(self.subscription)


def check_native_draft_limits(bus):
    """Long native drafts remain lossless when the bounded panel view is hidden."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    watch = Watch()
    peer = EnginePeer(bus)
    limit = 8192
    operations = [
        ("letter", [(IBus.KEY_x, 0)], "x"),
        ("literal digit", [(IBus.KEY_9, IBus.ModifierType.MOD1_MASK)], "9"),
        ("space", [(IBus.KEY_space, 0)], " "),
        ("accent", [(IBus.KEY_dead_acute, 0), (IBus.KEY_e, 0)], "é"),
        ("long Compose", [(IBus.KEY_Multi_key, 0), (IBus.KEY_F11, 0), (IBus.KEY_3, 0)], "é" * 127),
        ("cancelled Compose", [(IBus.KEY_dead_acute, 0), (IBus.KEY_9, 0)], "9"),
    ]
    passed = 0
    try:
        for language in ["en", "zh-Hans", "ja"]:
            assert json.loads(command("L" + language))["ok"]
            for label, keys, suffix in operations:
                for overflow in [False, True]:
                    for recover in [False, True]:
                        peer.event("FocusIn")
                        pump()
                        # Include multibyte and JSON-escaped characters, but pad
                        # in bytes so the exact/over-limit distinction is real.
                        prefix = '"\\界😀é' * 500
                        budget = limit + int(overflow) - len(suffix.encode())
                        draft = prefix + "z" * (budget - len(prefix.encode()))
                        assert len(draft.encode()) == budget <= limit
                        assert action(watch.latest, "T" + draft)
                        wait(lambda: watch.latest["seed"] == draft, "prepare bounded native draft")
                        baseline, before = watch.latest, len(peer.commits)
                        assert not baseline["private"] and len(baseline["candidates"]) == 1
                        assert baseline["candidates"][0]["text"] == draft
                        assert baseline["candidates"][0]["kind"] == "literal"
                        # A rejected replacement must not truncate or clear an
                        # already accepted draft, including at the UTF-8 limit.
                        assert not action(baseline, "T" + "界" * 2731)
                        assert not action(baseline, "Tbad\ntext")
                        pump()
                        assert watch.latest == baseline
                        for key, mask in keys:
                            assert peer.process_key_event(key, 0, mask), (language, label)
                        expected = draft + suffix
                        wait(lambda: watch.latest["revision"] > baseline["revision"], "publish long edit")
                        pump()
                        frame = watch.latest
                        assert frame["context"] == baseline["context"]
                        assert frame["private"] == overflow, (language, label, overflow)
                        if overflow:
                            assert not frame["seed"] and not frame["candidates"]
                        else:
                            assert frame["seed"] == expected
                            assert frame["candidates"][frame["selected"]]["text"] == expected
                        assert not action(baseline, "Tstale"), "long edit retained an old revision"
                        assert peer.commits[before:] == [], "long edit unexpectedly committed"
                        if recover:
                            assert peer.process_key_event(IBus.KEY_BackSpace)
                            expected = expected[:-1]
                            wait(lambda: not watch.latest["private"] and watch.latest["seed"] == expected,
                                 "shortened draft must restore the exact public snapshot")
                            assert watch.latest["context"] == baseline["context"]
                        assert peer.process_key_event(IBus.KEY_Return)
                        wait(lambda: len(peer.commits) > before and not watch.latest["seed"],
                             "long draft must commit through native Enter")
                        assert peer.commits[before:] == [expected], (language, label, overflow, recover)
                        assert watch.latest["focused"] and not watch.latest["private"]
                        assert watch.latest["context"] != baseline["context"]
                        type_seed(peer, "hel")
                        wait(lambda: watch.latest["seed"] == "hel" and watch.latest["candidates"],
                             "normal native input must recover after a long commit")
                        assert not peer.process_key_event(IBus.KEY_x, 0, IBus.ModifierType.RELEASE_MASK)
                        assert peer.commits[before:] == [expected]
                        passed += 1
        assert passed == 72
        print("PASS: 72 native draft byte-limit cases preserve Unicode/Compose/space, reject invalid replacements, and recover/commit without truncation")
    finally:
        peer.close()
        assert json.loads(command("L" + saved["language"]))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_prediction_length_boundaries(bus):
    """Crossing the model budget cancels old results, not the editable draft."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original = config.read_text()
    watch = Watch()
    seed = "note " + "界😀é" * 82 + "z hel"
    assert len(seed) == 256 and len(seed.encode()) > 256 and seed.endswith("hel")
    operations = [
        ("letter", [(IBus.KEY_x, 0)], "x"),
        ("unicode", [(IBus.unicode_to_keyval("😀"), 0)], "😀"),
        ("space", [(IBus.KEY_space, 0)], " "),
        ("Compose", [(IBus.KEY_Multi_key, 0), (IBus.KEY_F11, 0), (IBus.KEY_3, 0)], "é" * 127),
    ]
    passed = 0
    try:
        for label, keys, suffix in operations:
            for phase in ["pending", "ready"]:
                name = f"synthetic-draft-budget-{label}-{phase}"
                gate = {key: threading.Event() for key in ["received", "release", "finished"]}
                if phase == "ready":
                    gate["release"].set()
                model_reply_gates[name] = gate
                peer = EnginePeer(bus)
                try:
                    settings = dict(saved, language="en", llm_enabled=False, llm_model=name,
                                    llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
                    config.write_text(json.dumps(settings))
                    assert json.loads(command("R"))["ok"]
                    peer.event("FocusIn")
                    type_seed(peer, "thank")
                    assert peer.process_key_event(IBus.KEY_Return)
                    wait(lambda: peer.commits == ["thank"] and not watch.latest["seed"], "prepare budget history")
                    assert json.loads(command("P1"))["ok"]
                    pump()
                    requested = len(model_requests)
                    assert action(watch.latest, "T" + seed)
                    wait(gate["received"].is_set, "model reached length-boundary reply gate")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == seed and payload["committed_context"] == "thank"
                    if phase == "ready":
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]),
                             "ready prediction at 256 Unicode characters")
                    baseline = watch.latest
                    for key, mask in keys:
                        assert peer.process_key_event(key, 0, mask)
                    longer = seed + suffix
                    wait(lambda: watch.latest["seed"] == longer, "retain over-budget draft")
                    assert watch.latest["context"] == baseline["context"] and not watch.latest["private"]
                    assert len(watch.latest["candidates"]) == 1
                    assert watch.latest["candidates"][0]["text"] == longer
                    assert json.loads(command("S"))["prediction"] == "Idle"
                    assert not action(baseline, "K0")
                    stable = watch.latest
                    gate["release"].set()
                    wait(gate["finished"].is_set, "release pre-budget model result")
                    until = time.monotonic() + 0.2
                    while time.monotonic() < until:
                        pump()
                        assert watch.latest == stable and peer.commits == ["thank"]
                        assert len(model_requests) == requested + 1
                        time.sleep(0.01)
                    if len(suffix) == 1:
                        assert peer.process_key_event(IBus.KEY_BackSpace)
                    else:
                        assert action(watch.latest, "T" + seed)
                    wait(lambda: watch.latest["seed"] == seed and
                         any(c["source"] == "model" for c in watch.latest["candidates"]),
                         "prediction must resume after shortening the same draft")
                    assert len(model_requests) == requested + 2
                    payload = json.loads(model_requests[-1]["messages"][1]["content"])
                    assert payload["raw_composition"] == seed and payload["committed_context"] == "thank"
                    word = seed + "ioseismology"
                    index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == word)
                    assert action(watch.latest, f"N{index}")
                    assert peer.process_key_event(IBus.KEY_Return)
                    wait(lambda: len(peer.commits) == 2 and not watch.latest["seed"], "commit recovered model word")
                    assert peer.commits == ["thank", word]
                    passed += 1
                finally:
                    gate["release"].set()
                    peer.close()
                    model_reply_gates.pop(name, None)
        assert passed == 8
        print("PASS: 8 pending/ready model length-boundary cases drop old predictions, keep exact Unicode drafts/history, and resume after shortening")
    finally:
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_commit_target_boundaries(bus):
    """N37: external commits end old work; acknowledged companion commits continue."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    watch = Watch()
    peer, stale = EnginePeer(bus), EnginePeer(bus)
    failures, passed = [], 0
    operations = ["enter", "keypad enter", "native click", "legacy send", "companion commit",
                  "shift enter", "space", "number choice", "release enter", "control enter",
                  "alt enter", "secondary click", "invalid click", "stale enter", "stale click",
                  "invalid send"]
    assert len(operations) == 16
    try:
        for language, seed in [("en", "hel"), ("zh-Hans", "nihao"), ("ja", "nihongo")]:
            assert json.loads(command("L" + language))["ok"]
            for operation in operations:
                for typed in [False, True]:
                    if operation.startswith("stale"):
                        stale.event("FocusIn")
                        type_seed(stale, "old")
                    peer.event("FocusIn")
                    assert command("Q") == b"1"
                    pump()
                    if typed:
                        type_seed(peer, seed)
                    baseline, before = watch.latest, len(peer.commits)
                    crossed = operation == "legacy send" or (typed and operation in ["enter", "keypad enter", "native click"])
                    expected_commits, expected_seed = [], baseline["seed"]
                    literal = f"synthetic old send {language} "
                    with socket.socket(socket.AF_UNIX) as pending:
                        pending.settimeout(2)
                        pending.connect(str(host_socket))
                        pending.sendall(("C" + literal).encode())
                        # Establish that the slow sender already owns the old
                        # target before the independent commit arrives.
                        assert command("Q") == b"1"
                        if operation in ["enter", "keypad enter"]:
                            key = IBus.KEY_Return if operation == "enter" else IBus.KEY_KP_Enter
                            assert peer.process_key_event(key) == typed
                            if typed:
                                expected_commits = [baseline["candidates"][baseline["selected"]]["text"]]
                                expected_seed = ""
                        elif operation == "native click":
                            peer.click(0)
                            if typed:
                                expected_commits, expected_seed = [baseline["candidates"][0]["text"]], ""
                        elif operation == "legacy send":
                            direct = f"synthetic direct {language} "
                            assert command("C" + direct) == b"1"
                            expected_commits, expected_seed = [direct], ""
                        elif operation == "companion commit":
                            assert action(baseline, "K0") == typed
                            if typed:
                                expected_commits, expected_seed = [baseline["candidates"][0]["text"]], ""
                        elif operation == "shift enter":
                            assert peer.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK) == typed
                            if typed:
                                expected_seed = baseline["candidates"][baseline["selected"]]["text"]
                        elif operation == "space":
                            assert peer.process_key_event(IBus.KEY_space) == typed
                            if typed:
                                expected_seed += " "
                        elif operation == "number choice":
                            assert peer.process_key_event(IBus.KEY_1) == typed
                            expected_seed = baseline["candidates"][0]["text"] if typed else ""
                        elif operation in ["release enter", "control enter", "alt enter"]:
                            mask = {"release enter": IBus.ModifierType.RELEASE_MASK,
                                    "control enter": IBus.ModifierType.CONTROL_MASK,
                                    "alt enter": IBus.ModifierType.MOD1_MASK}[operation]
                            assert not peer.process_key_event(IBus.KEY_Return, 0, mask)
                        elif operation == "secondary click":
                            peer.click(0, 2)
                        elif operation == "invalid click":
                            peer.click(10000)
                        elif operation == "stale enter":
                            assert not stale.process_key_event(IBus.KEY_Return)
                        elif operation == "stale click":
                            stale.click(0)
                        else:
                            assert operation == "invalid send"
                            assert command("Cbad\0text") == b"0"
                        assert command("Q") == b"1"
                        # Socket replies and D-Bus CommitText signals arrive
                        # independently, even after the host has completed its
                        # handler. Wait for the signal count, then check exact
                        # payloads so a wrong or extra commit still fails.
                        wait(lambda: len(peer.commits) >= before + len(expected_commits),
                             f"{language}/{operation}/typed={typed}: native commit signal")
                        assert peer.commits[before:] == expected_commits and not stale.commits, \
                            (language, operation, typed, peer.commits[before:], expected_commits, stale.commits)
                        assert watch.latest["seed"] == expected_seed, (operation, language, watch.latest["seed"], expected_seed)
                        changed = watch.latest["context"] != baseline["context"]
                        if crossed:
                            assert not action(baseline, "Tobsolete keyboard draft")
                            # Retype the same seed: latest-only readers need
                            # the new target, not a comparison of final text.
                            assert action(watch.latest, "T" + seed)
                            wait(lambda: watch.latest["seed"] == seed, "new composition after external commit")
                        final_frame = watch.latest
                        pending.shutdown(socket.SHUT_WR)
                        reply = pending.recv(2)
                    if reply == b"1":
                        wait(lambda: len(peer.commits) > before + len(expected_commits), "observe delayed send after commit")
                    else:
                        assert command("Q") == b"1"
                        pump()
                    allowed = not crossed
                    outcome_ok = reply == (b"1" if allowed else b"0") and \
                        peer.commits[before:] == expected_commits + ([literal] if allowed else [])
                    retained = allowed or watch.latest == final_frame
                    if changed == crossed and outcome_ok and retained:
                        passed += 1
                    else:
                        failure = (f"{language}/{operation}/typed={typed}: context_changed={changed}, reply={reply!r}, "
                                   f"commits={peer.commits[before:]!r}, new_draft_retained={retained}")
                        print("AUDIT: N37 " + failure)
                        failures.append(failure)
                    if crossed and outcome_ok:
                        assert command("Csynthetic fresh send") == b"1"
                        wait(lambda: peer.commits[before:] == expected_commits + ["synthetic fresh send"], "fresh send after external commit")
                    peer.event("FocusOut")
        print(f"AUDIT: N37 {passed}/96 commit target cases passed")
        assert not failures, "commit target boundaries:\n" + "\n".join(failures)
        print("PASS: N37 96 native/legacy/companion commits and edit/rejected-event controls isolate old sends from new compositions")
    finally:
        peer.close()
        stale.close()
        assert json.loads(command("L" + saved["language"]))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_cancel_target_boundaries(bus):
    """N36: an external cancellation must retire already accepted input work."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    watch = Watch()
    peer, stale = EnginePeer(bus), EnginePeer(bus)
    failures, passed = [], 0
    states = ["empty", "draft", "completion", "compose empty", "compose draft"]
    cases = [(operation, state) for operation in ["reset", "escape"] for state in states]
    cases += [(operation, "draft") for operation in [
        "stale reset", "stale escape", "panel clear", "backspace", "escape release",
        "escape control", "escape alt", "escape super"]]
    cases += [("backspace", state) for state in ["empty", "compose empty", "compose draft"]]
    assert len(cases) == 21
    try:
        for language, seed in [("en", "hel"), ("zh-Hans", "nihao"), ("ja", "nihongo")]:
            assert json.loads(command("L" + language))["ok"]
            for operation, state in cases:
                if operation.startswith("stale"):
                    stale.event("FocusIn")
                    type_seed(stale, "old")
                peer.event("FocusIn")
                assert command("Q") == b"1"
                pump()
                if state in ["draft", "completion", "compose draft"]:
                    type_seed(peer, seed)
                if state == "completion":
                    index = next(i for i, candidate in enumerate(watch.latest["candidates"])
                                 if candidate["text"] != seed)
                    assert action(watch.latest, "N" + str(index))
                    assert peer.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
                if state.startswith("compose"):
                    assert peer.process_key_event(IBus.KEY_dead_acute)
                assert command("Q") == b"1"
                pump()
                baseline, before = watch.latest, len(peer.commits)
                if state == "completion":
                    assert baseline["seed"] != seed, "completion fixture needs a real replacement"
                crossed = operation == "reset" or (operation == "escape" and state in ["draft", "completion"])
                expected = baseline["seed"]
                literal = f"synthetic pending {language} "
                with socket.socket(socket.AF_UNIX) as pending:
                    pending.settimeout(2)
                    pending.connect(str(host_socket))
                    pending.sendall(("C" + literal).encode())
                    # The later connection proves this request has already
                    # captured the old target, without an arbitrary sleep.
                    assert command("Q") == b"1"
                    if operation == "reset":
                        peer.event("Reset")
                        expected = ""
                    elif operation == "stale reset":
                        stale.event("Reset")
                    elif operation == "stale escape":
                        assert not stale.process_key_event(IBus.KEY_Escape)
                    elif operation == "panel clear":
                        assert action(baseline, "X")
                        expected = ""
                    elif operation == "escape":
                        assert peer.process_key_event(IBus.KEY_Escape) == (state != "empty")
                        if crossed:
                            expected = ""
                    elif operation == "backspace":
                        assert peer.process_key_event(IBus.KEY_BackSpace) == (state != "empty")
                        if state == "draft":
                            expected = expected[:-1]
                    else:
                        mask = {"escape release": IBus.ModifierType.RELEASE_MASK,
                                "escape control": IBus.ModifierType.CONTROL_MASK,
                                "escape alt": IBus.ModifierType.MOD1_MASK,
                                "escape super": IBus.ModifierType.SUPER_MASK}[operation]
                        assert not peer.process_key_event(IBus.KEY_Escape, 0, mask)
                    assert command("Q") == b"1"
                    pump()
                    assert watch.latest["seed"] == expected
                    assert len(peer.commits) == before and not stale.commits
                    changed = watch.latest["context"] != baseline["context"]
                    if state.startswith("compose"):
                        # Reset clears the whole draft; Esc/Backspace cancel
                        # only the unfinished dead key and keep the target.
                        type_seed(peer, "e")
                        assert watch.latest["seed"] == expected + "e"
                    if crossed:
                        assert not action(baseline, "Tobsolete keyboard draft")
                        # Same spelling after cancel must still be new work,
                        # even when readers coalesce away the empty frame.
                        restored = baseline["seed"] or seed
                        assert action(watch.latest, "T" + restored)
                        wait(lambda: watch.latest["seed"] == restored, "new draft after cancellation")
                    final_frame = watch.latest
                    pending.shutdown(socket.SHUT_WR)
                    reply = pending.recv(2)
                if reply == b"1":
                    wait(lambda: len(peer.commits) > before, "observe cancellation-boundary send")
                else:
                    assert command("Q") == b"1"
                    pump()
                allowed = not crossed
                outcome_ok = reply == (b"1" if allowed else b"0") and \
                    peer.commits[before:] == ([literal] if allowed else [])
                retained = allowed or watch.latest == final_frame
                if changed == crossed and outcome_ok and retained:
                    passed += 1
                else:
                    failure = (f"{language}/{operation}/{state}: context_changed={changed}, reply={reply!r}, "
                               f"commits={peer.commits[before:]!r}, new_draft_retained={retained}")
                    print("AUDIT: N36 " + failure)
                    failures.append(failure)
                if crossed and outcome_ok:
                    assert command("Csynthetic fresh send") == b"1"
                    wait(lambda: peer.commits[before:] == ["synthetic fresh send"], "fresh send after cancellation")
                peer.event("FocusOut")
        print(f"AUDIT: N36 {passed}/63 cancellation target cases passed")
        assert not failures, "cancellation target boundaries:\n" + "\n".join(failures)
        print("PASS: N36 63 Reset/Esc/Compose/stale-event/edit controls revoke canceled work without breaking continuity")
    finally:
        peer.close()
        stale.close()
        assert json.loads(command("L" + saved["language"]))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_draft_end_prediction_boundaries(bus):
    """N36/N37: cancel or commit a draft without losing same-field history."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original = config.read_text()
    watch = Watch()
    try:
        commit_operations = ["enter", "keypad enter", "native click", "companion commit"]
        for operation in ["reset", "escape", "compose escape", "compose backspace"] + commit_operations:
            for phase in ["pending", "ready"]:
                name = f"synthetic-draft-end-{operation}-{phase}"
                gate = {key: threading.Event() for key in ["received", "release", "finished"]}
                if phase == "ready":
                    gate["release"].set()
                model_reply_gates[name] = gate
                peer = EnginePeer(bus)
                try:
                    settings = dict(saved, language="en", llm_enabled=False, llm_model=name,
                                    llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
                    config.write_text(json.dumps(settings))
                    assert json.loads(command("R"))["ok"]
                    peer.event("FocusIn")
                    type_seed(peer, "thank")
                    assert peer.process_key_event(IBus.KEY_Return)
                    wait(lambda: peer.commits == ["thank"] and not watch.latest["seed"], "prepare draft-end model history")
                    assert json.loads(command("P1"))["ok"]
                    pump()
                    requested = len(model_requests)
                    assert action(watch.latest, "Thel")
                    wait(gate["received"].is_set, "model reached draft-end reply gate")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == "hel" and payload["committed_context"] == "thank"
                    if phase == "ready":
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "ready model before draft end")
                    if operation.startswith("compose"):
                        assert peer.process_key_event(IBus.KEY_dead_acute)
                        pump()
                    expected_commits = ["thank"]
                    if operation in commit_operations:
                        # Exercise exact model output when ready, and literal
                        # native output while the old model reply is held.
                        index = next(i for i, c in enumerate(watch.latest["candidates"])
                                     if (c["source"] == "model" if phase == "ready" else c["text"] == "hel"))
                        expected_commits.append(watch.latest["candidates"][index]["text"])
                        assert action(watch.latest, f"N{index}")
                        pump()
                    baseline = watch.latest
                    if operation == "reset":
                        peer.event("Reset")
                    elif operation == "native click":
                        peer.click(index)
                    elif operation == "companion commit":
                        assert action(baseline, f"K{index}")
                    elif operation in ["enter", "keypad enter"]:
                        key = IBus.KEY_Return if operation == "enter" else IBus.KEY_KP_Enter
                        assert peer.process_key_event(key)
                    else:
                        key = IBus.KEY_BackSpace if operation == "compose backspace" else IBus.KEY_Escape
                        assert peer.process_key_event(key)
                    # CommitText signals and companion snapshots use different
                    # transports; a socket ACK is not a D-Bus signal barrier.
                    wait(lambda: len(peer.commits) >= len(expected_commits), f"{operation}/{phase}: native commit signal")
                    whole_draft = not operation.startswith("compose")
                    retires_target = whole_draft and operation != "companion commit"
                    assert (watch.latest["context"] != baseline["context"]) == retires_target
                    assert watch.latest["seed"] == ("" if whole_draft else "hel")
                    assert peer.commits == expected_commits, (operation, phase, peer.commits, expected_commits)
                    if whole_draft:
                        assert not watch.latest["candidates"]
                        assert json.loads(command("S"))["prediction"] != "Pending"
                        assert not action(baseline, "K0")
                    cleared = watch.latest
                    gate["release"].set()
                    wait(gate["finished"].is_set, "release pre-boundary model result")
                    if whole_draft:
                        until = time.monotonic() + 0.2
                        while time.monotonic() < until:
                            pump()
                            assert watch.latest == cleared and peer.commits == expected_commits
                            time.sleep(0.01)
                    else:
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "Compose-only cancel retains valid prediction")
                        assert watch.latest["context"] == baseline["context"] and watch.latest["seed"] == "hel"
                    assert len(model_requests) == requested + 1
                    requested = len(model_requests)
                    assert action(watch.latest, "Tplease rec")
                    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "new predictions after draft end")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == "please rec" and payload["committed_context"] == "".join(expected_commits)
                    assert all(c["text"].startswith("please rec") for c in watch.latest["candidates"] if c["source"] == "model")
                    assert len(model_requests) == requested + 1 and peer.commits == expected_commits
                finally:
                    gate["release"].set()
                    peer.close()
                    model_reply_gates.pop(name, None)
        print("PASS: N36 8 pending/ready model cases discard canceled predictions, preserve Compose-only results and exact same-field history")
        print("PASS: N37 8 pending/ready commit cases retire old predictions, keep exact committed history and preserve companion continuation")
    finally:
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_language_target_boundaries(bus):
    """N35: a real language change revokes the target, including a round trip."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original, mode = config.read_text(), runtime.stat().st_mode & 0o777
    languages = [("en", "en_US", "hel"), ("zh-Hans", "zh_CN", "nihao"), ("ja", "ja_JP", "nihongo")]
    watch = Watch()
    peer = EnginePeer(bus)
    failures = []
    passed = 0

    def change_language(language, operation):
        if operation == "R":
            changed = json.loads(config.read_text())
            changed["language"] = language
            config.write_text(json.dumps(changed))
            reply = command("R")
        else:
            reply = command("L" + language)
        assert json.loads(reply)["ok"]

    try:
        for language, alias, seed in languages:
            cases = [(name, None, None, False) for name in [
                "same language", "same alias", "same reload", "provider reload",
                "unsupported language", "invalid reload", "conflict", "write failure"]]
            for target, _, _ in languages:
                if target != language:
                    cases.extend((f"{operation}/{target}/return={back}", target, operation, back)
                                 for operation in ["L", "R"] for back in [False, True])
            assert len(cases) == 16
            for label, target, operation, back in cases:
                for typed in [False, True]:
                    settings = dict(saved, language=language)
                    config.write_text(json.dumps(settings))
                    assert json.loads(command("R"))["ok"]
                    peer.event("FocusIn")
                    assert command("Q") == b"1"
                    pump()
                    if typed:
                        type_seed(peer, seed)
                    baseline, before = watch.latest, len(peer.commits)
                    assert baseline["language"] == language and baseline["seed"] == (seed if typed else "")
                    literal = f"synthetic pending {language} "
                    with socket.socket(socket.AF_UNIX) as pending:
                        pending.settimeout(2)
                        pending.connect(str(host_socket))
                        pending.sendall(("C" + literal).encode())
                        # The later probe is a listener-accept barrier: the
                        # delayed request already owns the original context.
                        assert command("Q") == b"1"
                        if target is not None:
                            change_language(target, operation)
                            if back:
                                change_language(language, operation)
                        elif label == "same language":
                            assert json.loads(command("L" + language))["ok"]
                        elif label == "same alias":
                            assert json.loads(command("L" + alias))["ok"]
                        elif label == "same reload":
                            assert json.loads(command("R"))["ok"]
                        elif label == "provider reload":
                            changed = dict(settings, llm_model="synthetic-language-reload")
                            config.write_text(json.dumps(changed))
                            assert json.loads(command("R"))["ok"]
                        elif label == "unsupported language":
                            assert not json.loads(command("Lunsupported-language"))["ok"]
                        elif label in ["invalid reload", "conflict"]:
                            preserved = config.read_text()
                            try:
                                if label == "invalid reload":
                                    config.write_text("{broken synthetic settings")
                                    response = command("R")
                                else:
                                    changed = dict(settings, llm_temperature_tenths=(settings["llm_temperature_tenths"] + 1) % 11)
                                    config.write_text(json.dumps(changed))
                                    response = command("L" + next(other for other, _, _ in languages if other != language))
                                assert not json.loads(response)["ok"]
                            finally:
                                config.write_text(preserved)
                        else:
                            assert label == "write failure"
                            try:
                                runtime.chmod(0o500)
                                response = command("L" + next(other for other, _, _ in languages if other != language))
                                assert not json.loads(response)["ok"]
                            finally:
                                runtime.chmod(mode)
                        assert command("Q") == b"1"
                        pump()
                        crossed = target is not None
                        assert watch.latest["language"] == (target if crossed and not back else language)
                        assert watch.latest["seed"] == ("" if crossed else baseline["seed"])
                        # Recreate the same spelling after a round trip. A
                        # latest-only consumer cannot infer the boundary from
                        # the final language/text; it needs a fresh target ID.
                        if crossed and typed:
                            assert action(watch.latest, "T" + seed)
                            wait(lambda: watch.latest["seed"] == seed, "type into the new language session")
                        final_frame = watch.latest
                        context_ok = (final_frame["context"] != baseline["context"]) == crossed
                        if crossed:
                            assert not action(baseline, "Tobsolete keyboard draft")
                        pending.shutdown(socket.SHUT_WR)
                        reply = pending.recv(2)
                    if reply == b"1":
                        wait(lambda: len(peer.commits) > before, "observe language-boundary send")
                    else:
                        assert command("Q") == b"1"
                        pump()
                    allowed = not crossed
                    outcome_ok = reply == (b"1" if allowed else b"0") and \
                        peer.commits[before:] == ([literal] if allowed else [])
                    retained = allowed or watch.latest == final_frame
                    if context_ok and outcome_ok and retained:
                        passed += 1
                    else:
                        failure = (f"{language}/{label}/typed={typed}: context_changed="
                                   f"{final_frame['context'] != baseline['context']}, reply={reply!r}, "
                                   f"commits={peer.commits[before:]!r}, new_draft_retained={retained}")
                        print("AUDIT: N35 " + failure)
                        failures.append(failure)
                    # A fresh explicit send in the new session remains valid;
                    # revocation must not leave the host permanently blocked.
                    if crossed and outcome_ok:
                        assert command("Csynthetic fresh send") == b"1"
                        wait(lambda: peer.commits[before:] == ["synthetic fresh send"], "fresh language-session send")
                    peer.event("FocusOut")
        print(f"AUDIT: N35 {passed}/96 language target cases passed")
        assert not failures, "language target boundaries:\n" + "\n".join(failures)
        print("PASS: N35 96 language-switch/reload/alias/failure cases reject old sends and preserve same-language controls")
    finally:
        runtime.chmod(mode)
        peer.close()
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_content_type_commit_boundaries(bus):
    """N33/N34: metadata boundaries revoke delayed sends, not ordinary typing."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    public = (IBus.InputPurpose.FREE_FORM, 0)
    private = (IBus.InputPurpose.FREE_FORM, 1 << 11)
    blocked = [IBus.InputPurpose.PASSWORD, IBus.InputPurpose.PIN,
               IBus.InputPurpose.DIGITS, IBus.InputPurpose.NUMBER, IBus.InputPurpose.PHONE]
    cases = [
        ("ordinary", public, [], True),
        ("same public metadata", public, [public], True),
        ("ordinary hint", public, [(IBus.InputPurpose.FREE_FORM, 1)], True),
        ("ordinary purpose", public, [(IBus.InputPurpose.EMAIL, 0)], True),
        ("stale private event", public, [], True),
        ("private", private, [], False),
        ("same private metadata", private, [private], False),
        ("public to private", public, [private], False),
        ("private to public", private, [public], False),
        ("private roundtrip", public, [private, public], False),
    ]
    for purpose in blocked:
        restricted = (purpose, 0)
        cases.extend([
            (f"bypass {int(purpose)}", restricted, [], False),
            (f"enter bypass {int(purpose)}", public, [restricted], False),
            (f"leave bypass {int(purpose)}", restricted, [public], False),
            (f"bypass roundtrip {int(purpose)}", public, [restricted, public], False),
            (f"private/bypass roundtrip {int(purpose)}", private, [restricted, private], False),
        ])
    assert len(cases) == 35
    failures = []
    passed = 0
    watch = Watch()
    peer, stale = EnginePeer(bus), EnginePeer(bus)

    def policy(content):
        purpose, hints = content
        bypass = purpose in blocked
        return bypass, bypass or bool(hints & (1 << 11))

    try:
        for language, seed, literal in [("en", "hel", "native café "),
                                         ("zh-Hans", "nihao", "你好 "),
                                         ("ja", "nihongo", "日本語 ")]:
            assert json.loads(command("L" + language))["ok"]
            for label, initial, changes, allowed in cases:
                peer.set_content_type(*initial)
                peer.event("FocusIn")
                pump()
                if not policy(initial)[0]:
                    type_seed(peer, seed)
                baseline, before = watch.latest, len(peer.commits)
                with socket.socket(socket.AF_UNIX) as pending:
                    pending.settimeout(2)
                    pending.connect(str(host_socket))
                    pending.sendall(("C" + literal).encode())
                    # This later connection can only be answered after the
                    # listener has accepted pending and captured its context.
                    assert command("Q") == b"1"
                    previous, boundaries = initial, 0
                    for content in changes:
                        boundaries += int(policy(previous) != policy(content))
                        peer.set_content_type(*content)
                        previous = content
                    if label == "stale private event":
                        stale.set_content_type(*public)
                        stale.set_content_type(*private)
                    assert command("Q") == b"1"
                    pump()
                    final_frame = watch.latest
                    assert final_frame["private"] == policy(previous)[1]
                    # Consumers may see only the final public frame and skip a
                    # brief private/blocked frame. Identity must still change.
                    changed = final_frame["context"] != baseline["context"]
                    context_ok = changed == bool(boundaries)
                    pending.shutdown(socket.SHUT_WR)
                    reply = pending.recv(2)
                if reply == b"1":
                    wait(lambda: len(peer.commits) > before, "observe legacy delivery")
                else:
                    assert command("Q") == b"1"
                    pump()
                outcome_ok = reply == (b"1" if allowed else b"0") and \
                    peer.commits[before:] == ([literal] if allowed else [])
                if context_ok and outcome_ok:
                    passed += 1
                else:
                    failure = (f"{language}/{label}: context_changed={changed}, boundaries={boundaries}, "
                               f"reply={reply!r}, commits={peer.commits[before:]!r}")
                    print("AUDIT: N33/N34 " + failure)
                    failures.append(failure)
                # Rejecting panel injection must not disable local conversion
                # in a PRIVATE (non-password/numeric) field.
                if label == "private" and outcome_ok:
                    assert peer.process_key_event(IBus.KEY_1)
                    assert peer.process_key_event(IBus.KEY_Return)
                    expected = {"en": "hel", "zh-Hans": "你好", "ja": "日本語"}[language]
                    wait(lambda: peer.commits[before:] == [expected], "private native conversion remains usable")
                    assert not watch.latest["seed"] and not watch.latest["candidates"]
                peer.event("FocusOut")
        print(f"AUDIT: N33/N34 {passed}/{len(cases) * 3} content-type/legacy-send cases passed")
        assert not failures, "content-type commit boundaries:\n" + "\n".join(failures)
        print("PASS: N33/N34 105 content-type/legacy-send cases revoke changed targets, reject PRIVATE injection and preserve ordinary/stale controls")
    finally:
        peer.close()
        stale.close()
        assert json.loads(command("L" + saved["language"]))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_content_type_prediction_boundaries(bus):
    """Late model replies cannot outlive a private/bypass metadata transition."""
    saved = json.loads(command("S"))["settings"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original = config.read_text()
    watch = Watch()
    contents = [(IBus.InputPurpose.FREE_FORM, 1 << 11)] + [
        (purpose, 0) for purpose in [IBus.InputPurpose.PASSWORD, IBus.InputPurpose.PIN,
                                    IBus.InputPurpose.DIGITS, IBus.InputPurpose.NUMBER,
                                    IBus.InputPurpose.PHONE]]
    try:
        for purpose, hints in contents:
            for phase in ["pending", "ready"]:
                name = f"synthetic-content-type-{int(purpose)}-{phase}"
                gate = {key: threading.Event() for key in ["received", "release", "finished"]}
                if phase == "ready":
                    gate["release"].set()
                model_reply_gates[name] = gate
                peer = EnginePeer(bus)
                try:
                    settings = dict(saved, language="en", llm_enabled=False, llm_model=name,
                                    llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
                    config.write_text(json.dumps(settings))
                    assert json.loads(command("R"))["ok"]
                    peer.event("FocusIn")
                    type_seed(peer, "thank")
                    assert peer.process_key_event(IBus.KEY_Return)
                    wait(lambda: peer.commits == ["thank"] and not watch.latest["seed"], "prepare pre-boundary context")
                    assert json.loads(command("P1"))["ok"]
                    pump()
                    requested = len(model_requests)
                    assert action(watch.latest, "Thel")
                    wait(gate["received"].is_set, "model reached content-type reply gate")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["committed_context"] == "thank"
                    if phase == "ready":
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "ready model before privacy")
                    baseline = watch.latest
                    peer.set_content_type(purpose, hints)
                    pump()
                    assert watch.latest["context"] > baseline["context"]
                    assert watch.latest["private"] and not watch.latest["seed"] and not watch.latest["candidates"]
                    assert json.loads(command("S"))["prediction"] == "Disabled"
                    assert not action(baseline, "K0") and command("Csynthetic denied") == b"0"
                    assert peer.commits == ["thank"]
                    count = len(model_requests)
                    if hints:
                        type_seed(peer, "private")
                        assert peer.process_key_event(IBus.KEY_Return)
                        wait(lambda: peer.commits == ["thank", "private"], "native private input remains usable")
                    else:
                        for key in [IBus.KEY_a, IBus.KEY_1, IBus.KEY_Return, IBus.KEY_dead_acute]:
                            assert not peer.process_key_event(key)
                    after, committed = watch.latest, list(peer.commits)
                    gate["release"].set()
                    wait(gate["finished"].is_set, "release pre-privacy model result")
                    until = time.monotonic() + 0.2
                    while time.monotonic() < until:
                        pump()
                        assert watch.latest == after and peer.commits == committed
                        assert len(model_requests) == count
                        time.sleep(0.01)
                    peer.set_content_type(IBus.InputPurpose.FREE_FORM)
                    pump()
                    assert watch.latest["context"] > after["context"]
                    assert not watch.latest["private"] and not watch.latest["seed"]
                    requested = len(model_requests)
                    assert action(watch.latest, "Thel")
                    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "public predictions resume")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == "hel" and not payload["committed_context"]
                    assert len(model_requests) == requested + 1 and peer.commits == committed
                finally:
                    gate["release"].set()
                    peer.close()
                    model_reply_gates.pop(name, None)
        print("PASS: N33/N34 12 pending/ready content-type model cases discard old results and context without disabling local PRIVATE input")
    finally:
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_engine_destruction(bus):
    """N31: destruction is a field boundary even without an earlier FocusOut."""
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"]
    watch = Watch()
    failures = []
    cases = 0

    def prepare(peer, phase, seed, word):
        peer.event("FocusIn")
        if phase == "empty":
            pump()
            assert not watch.latest["seed"]
            return ""
        draft = "caf" if phase == "compose" else seed
        type_seed(peer, draft)
        wait(lambda: watch.latest["seed"] == draft, "prepare destruction draft")
        if phase in ["selected", "adopted"]:
            index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == word)
            peer.event("CursorUp")
            for _ in range(index):
                peer.event("CursorDown")
            wait(lambda: watch.latest["selected"] == index, "select before destruction")
            if phase == "adopted":
                assert peer.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
                wait(lambda: watch.latest["seed"] == word, "adopt before destruction")
                draft = word
        elif phase == "compose":
            assert peer.process_key_event(IBus.KEY_dead_acute)
            pump()
        return draft

    try:
        for language, seed, word in [("en", "hel", "hello"), ("zh-Hans", "nihao", "你好"),
                                     ("ja", "nihongo", "日本語")]:
            assert json.loads(command("L" + language))["ok"]
            for phase in ["empty", "typed", "selected", "adopted", "compose"]:
                for focused in [True, False]:
                    cases += 1
                    peer, survivor = EnginePeer(bus), EnginePeer(bus)
                    try:
                        prepare(peer, phase, seed, word)
                        if not focused:
                            draft = prepare(survivor, phase, seed, word)
                        baseline = watch.latest
                        peer.destroy()
                        # The synchronous Destroy and this host barrier precede
                        # reading the existing subscription; no new focus event
                        # may mask the missing destruction publication.
                        assert command("Q") == b"1"
                        pump()
                        try:
                            if focused:
                                assert not watch.latest["focused"] and not watch.latest["seed"] and not watch.latest["candidates"], \
                                    "destroyed engine left a stale composition"
                                assert watch.latest["context"] > baseline["context"] and watch.latest["revision"] > baseline["revision"]
                                assert not action(baseline, "K0") and command("Cwrong target") == b"0"
                                # Existing and freshly connected panels agree.
                                late = Watch()
                                try:
                                    wait(lambda: late.latest is not None, "subscribe after destruction")
                                    assert late.latest == watch.latest
                                finally:
                                    watchers.remove(late)
                                    late.sock.close()
                            else:
                                assert watch.latest == baseline, "old engine destruction changed the new field"
                                key = IBus.KEY_BackSpace if phase == "adopted" else \
                                    IBus.KEY_space if phase == "selected" else IBus.KEY_e
                                expected = seed if phase == "adopted" else word + " " if phase == "selected" else \
                                    draft + ("é" if phase == "compose" else "e")
                                assert survivor.process_key_event(key)
                                wait(lambda: watch.latest["seed"] == expected, "old destruction broke continuation")
                            assert not peer.commits and not survivor.commits
                        except AssertionError as error:
                            failure = f"{language}/{phase}/focused={focused}: {error}; seed={watch.latest['seed']!r}"
                            print("AUDIT: N31 " + failure)
                            failures.append(failure)
                    finally:
                        peer.close()
                        survivor.close()
        assert cases == 30
        assert not failures, "engine destruction:\n" + "\n".join(failures)
        print("PASS: N31 30 focused/stale engine destructions clear only the owned field across English/Chinese/Japanese")
    finally:
        assert json.loads(command("L" + saved["language"]))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_engine_destruction_prediction(bus):
    """A controlled late reply must not revive a destroyed field."""
    saved = json.loads(command("S"))["settings"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original = config.read_text()
    watch = Watch()
    try:
        for phase in ["pending", "ready"]:
            for focused in [True, False]:
                name = f"synthetic-destruction-{phase}-{focused}"
                gate = {key: threading.Event() for key in ["received", "release", "finished"]}
                if phase == "ready":
                    gate["release"].set()
                model_reply_gates[name] = gate
                peer, survivor = EnginePeer(bus), EnginePeer(bus)
                try:
                    settings = dict(saved, language="en", llm_enabled=False, llm_model=name,
                                    llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
                    config.write_text(json.dumps(settings))
                    assert json.loads(command("R"))["ok"]
                    peer.event("FocusIn")
                    if not focused:
                        survivor.event("FocusIn")
                    target = peer if focused else survivor
                    type_seed(target, "thank")
                    assert target.process_key_event(IBus.KEY_Return)
                    wait(lambda: target.commits == ["thank"] and not watch.latest["seed"], "prepare owned context")
                    assert json.loads(command("P1"))["ok"]
                    pump()
                    requested = len(model_requests)
                    # One complete update avoids intermediate spelling requests
                    # even on a heavily loaded CI host.
                    assert action(watch.latest, "Thel")
                    wait(gate["received"].is_set, "model request reached reply gate")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == "hel" and payload["committed_context"] == "thank"
                    if phase == "ready":
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "model before destruction")
                    else:
                        assert json.loads(command("S"))["prediction"] == "Pending"
                    baseline, before = watch.latest, len(target.commits)
                    peer.destroy()
                    state = json.loads(command("S"))
                    pump()
                    if focused:
                        assert not watch.latest["focused"] and not watch.latest["seed"] and not watch.latest["candidates"]
                        assert watch.latest["context"] > baseline["context"] and watch.latest["revision"] > baseline["revision"]
                        assert state["prediction"] == "Idle", "destroyed field retained pending prediction"
                        assert not action(baseline, "K0")
                    else:
                        assert watch.latest == baseline, "old destruction invalidated the current model field"
                        assert state["prediction"] == ("Pending" if phase == "pending" else "Ready")
                    after = watch.latest
                    gate["release"].set()
                    wait(gate["finished"].is_set, "release synthetic model reply")
                    if focused:
                        # Cover several native prediction ticks after the reply;
                        # a destroyed field must never publish its old results.
                        until = time.monotonic() + 0.2
                        while time.monotonic() < until:
                            pump()
                            assert watch.latest == after
                            time.sleep(0.01)
                        requested = len(model_requests)
                        survivor.event("FocusIn")
                        pump()
                        assert action(watch.latest, "Thel")
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "new field model recovery")
                        payload = json.loads(model_requests[requested]["messages"][1]["content"])
                        assert payload["raw_composition"] == "hel" and not payload["committed_context"]
                        assert not survivor.commits
                    else:
                        wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "surviving field prediction")
                        index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == "helium")
                        assert action(watch.latest, f"N{index}")
                        assert survivor.process_key_event(IBus.KEY_space)
                        wait(lambda: watch.latest["seed"] == "helium ", "surviving model selection")
                        assert survivor.process_key_event(IBus.KEY_BackSpace)
                        wait(lambda: watch.latest["seed"] == "hel", "surviving model undo")
                    assert len(target.commits) == before, "destruction or late reply committed text"
                finally:
                    gate["release"].set()
                    peer.close()
                    survivor.close()
                    model_reply_gates.pop(name, None)
        print("PASS: N31 4 pending/ready model destruction cases cancel only the owned field and reject late results")
    finally:
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_committed_model_context(bus):
    """N32: model context is the exact recent native output, not editor joins."""
    saved = json.loads(command("S"))["settings"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    assert config == runtime / "ime.json" and not config.is_symlink()
    original = config.read_text()
    watch = Watch()
    failures = []
    cases = [
        ("en", "identifier", [("foo", "foo"), ("bar", "bar")]),
        ("en", "trailing-space", [("hello ", "hello "), ("world", "world")]),
        ("en", "leading-spaces", [("hello", "hello"), ("  world  ", "  world  ")]),
        ("en", "whitespace-chunks", [("  ", "  "), ("hello", "hello"), (" ", " "), ("world", "world")]),
        ("en", "unicode", [("café", "café"), ("\u3000", "\u3000"), ("日本😀", "日本😀")]),
        ("en", "bounded-tail", [("prefix" + "界😀" * 78, "prefix" + "界😀" * 78), (" café  ", " café  "), ("tail", "tail")]),
        ("zh-Hans", "conversion", [("nihao", "你好"), ("xiexie", "谢谢")]),
        ("zh-Hans", "literal", [("nihao", "nihao"), (" \u3000", " \u3000"), ("世界", "世界")]),
        ("ja", "conversion", [("nihongo", "日本語"), ("arigatou", "ありがとう")]),
        ("ja", "literal", [("にほん", "にほん"), (" \u3000", " \u3000"), ("世界", "世界")]),
    ]
    try:
        settings = dict(saved, llm_enabled=False, llm_model="synthetic-exact-context",
                        llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
        config.write_text(json.dumps(settings))
        assert json.loads(command("R"))["ok"]
        for language, label, chunks in cases:
            for method in ["enter", "panel-click"]:
                peer = EnginePeer(bus)
                try:
                    assert json.loads(command("P0"))["ok"]
                    assert json.loads(command("L" + language))["ok"]
                    peer.event("FocusIn")
                    pump()
                    for seed, output in chunks:
                        assert action(watch.latest, "T" + seed)
                        wait(lambda: watch.latest["seed"] == seed, "prepare native context chunk")
                        index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == output)
                        before = len(peer.commits)
                        if method == "enter":
                            assert action(watch.latest, f"N{index}")
                            assert peer.process_key_event(IBus.KEY_Return)
                        else:
                            assert action(watch.latest, f"K{index}")
                        wait(lambda: len(peer.commits) > before and not watch.latest["seed"], "native context chunk commit")
                        assert peer.commits[before:] == [output], "application output changed"
                    expected = "".join(peer.commits)[-160:]
                    assert json.loads(command("P1"))["ok"]
                    pump()
                    requested = len(model_requests)
                    assert action(watch.latest, "Thel")
                    wait(lambda: len(model_requests) > requested, "request following consecutive native commits")
                    payload = json.loads(model_requests[requested]["messages"][1]["content"])
                    assert payload["raw_composition"] == "hel" and payload["language"] == language
                    actual = payload["committed_context"]
                    if actual != expected:
                        failure = f"{language}/{label}/{method}: expected={expected!r}, model={actual!r}"
                        print("AUDIT: N32 " + failure)
                        failures.append(failure)
                    wait(lambda: json.loads(command("S"))["prediction"] != "Pending", "settle context probe")
                    assert peer.commits == [output for _, output in chunks], "prediction submitted text"
                finally:
                    peer.close()
        assert not failures, "native committed context:\n" + "\n".join(failures)
        print("PASS: N32 20 exact native/model-context cases preserve joins, whitespace, Unicode and the 160-character tail")
    finally:
        config.write_text(original)
        assert json.loads(command("R"))["ok"]
        watchers.remove(watch)
        watch.sock.close()


def check_engine_event_boundaries(bus):
    saved = json.loads(command("S"))["settings"]
    assert not saved["llm_enabled"], "lifecycle QA starts with prediction disabled"
    a, b = EnginePeer(bus), EnginePeer(bus)
    watch = Watch()
    failures = []
    try:
        def start(peer, seed):
            peer.event("FocusOut")
            peer.event("FocusIn")
            type_seed(peer, seed)
            wait(lambda: watch.latest is not None and watch.latest["seed"] == seed, "prepare engine peer")

        # New focus can arrive before the old peer's final queued events.
        for label, operation in [
            ("letter", lambda: a.process_key_event(IBus.KEY_x)),
            ("commit", lambda: a.process_key_event(IBus.KEY_Return)),
            ("completion", lambda: a.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)),
            ("number choice", lambda: a.process_key_event(IBus.KEY_2)),
            ("separator", lambda: a.process_key_event(IBus.KEY_space)),
            ("literal number", lambda: a.process_key_event(IBus.KEY_2, 0, IBus.ModifierType.MOD1_MASK)),
            ("navigation", lambda: a.event("CursorDown")),
            ("page navigation", lambda: a.event("PageDown")),
            ("candidate click", lambda: a.click(0)),
            ("reset", lambda: a.event("Reset")),
            ("enable", lambda: a.event("Enable")),
            ("disable", lambda: a.event("Disable")),
            ("focus-out", lambda: a.event("FocusOut")),
        ]:
            start(a, "old")
            previous = watch.latest["context"]
            b.event("FocusIn")
            type_seed(b, "hel")
            wait(lambda: watch.latest["context"] != previous and watch.latest["seed"] == "hel",
                 "focus moved to the second peer")
            baseline = watch.latest
            before = (len(a.commits), len(b.commits))
            result = operation()
            pump()
            if result is True or watch.latest != baseline or before != (len(a.commits), len(b.commits)):
                failures.append(f"late {label} changed the focused peer: result={result}, seed={watch.latest['seed']!r}")
            b.event("FocusOut")
            a.event("FocusOut")

        # Even an engine reused without FocusOut starts from an empty field.
        for target in [a, b]:
            start(a, "old")
            assert a.process_key_event(IBus.KEY_dead_acute)
            previous = watch.latest["context"]
            target.event("FocusIn")
            wait(lambda: watch.latest["context"] != previous and watch.latest["focused"],
                 "fresh focus session")
            if watch.latest["seed"] or watch.latest["candidates"]:
                failures.append("focus-in carried an earlier field's preedit/candidates")
            type_seed(target, "e")
            wait(lambda: watch.latest["seed"] == "e", "focus-in retained an old dead key")
            target.event("FocusOut")
            wait(lambda: not watch.latest["focused"], "unfocused engine peer")
            baseline, before = watch.latest, len(target.commits)
            target.event("Enable")  # Enable alone does not grant input focus.
            handled = target.process_key_event(IBus.KEY_x)
            target.click(0)
            pump()
            if handled or watch.latest != baseline or len(target.commits) != before:
                failures.append("an unfocused engine accepted input")

        # Non-primary and application-modified clicks must never select/commit.
        for page in [0, 1]:
            for button, mask in [(0, 0), (2, 0), (3, 0), (4, 0), (5, 0),
                                 (1, IBus.ModifierType.CONTROL_MASK),
                                 (1, IBus.ModifierType.MOD1_MASK),
                                 (1, IBus.ModifierType.MOD4_MASK),
                                 (1, IBus.ModifierType.SUPER_MASK),
                                 (1, IBus.ModifierType.META_MASK),
                                 (1, IBus.ModifierType.HYPER_MASK),
                                 (1, IBus.ModifierType.RELEASE_MASK)]:
                start(b, "hel")
                if page:
                    b.event("PageDown")
                    pump()
                    assert watch.latest["selected"] >= 6
                baseline, before = watch.latest, len(b.commits)
                b.click(0, button, mask)
                pump()
                if watch.latest != baseline or len(b.commits) != before:
                    failures.append(f"button={button}, mask={int(mask)}, page={page} unexpectedly committed")
            for mask in [0, IBus.ModifierType.BUTTON1_MASK,
                         IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.LOCK_MASK | IBus.ModifierType.MOD2_MASK]:
                start(b, "hel")
                if page:
                    b.event("PageDown")
                    pump()
                index = (watch.latest["selected"] // 6) * 6
                expected, before = watch.latest["candidates"][index]["text"], len(b.commits)
                b.click(0, 1, mask)
                wait(lambda: len(b.commits) > before and not watch.latest["seed"], "primary candidate click")
                assert b.commits[before:] == [expected]
                b.event("FocusOut")

        settings = dict(saved, llm_enabled=True, llm_model="synthetic-focus-model",
                        llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
        Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
        assert json.loads(command("R"))["ok"]
        start(a, "thank")
        assert a.process_key_event(IBus.KEY_Return)
        wait(lambda: not watch.latest["seed"], "commit focused context")
        before = len(model_requests)
        type_seed(a, "hel")
        wait(lambda: len(model_requests) > before, "same-field model request")
        payload = json.loads(model_requests[before]["messages"][1]["content"])
        assert payload["committed_context"] == "thank", "same-field context was lost"
        wait(lambda: any(" · AI" in c["label"] for c in watch.latest["candidates"]), "same-field result")
        before = len(model_requests)
        b.event("FocusIn")  # Deliberately no focus-out from a yet.
        type_seed(b, "hel")
        wait(lambda: len(model_requests) > before, "new-field model request")
        payload = json.loads(model_requests[before]["messages"][1]["content"])
        if payload["committed_context"]:
            failures.append("a previous field's committed context reached the new field's model request")
        wait(lambda: any(" · AI" in c["label"] for c in watch.latest["candidates"]), "new-field result")
        assert not failures, "engine event boundaries:\n" + "\n".join(failures)
        print("PASS: late key/click/navigation events, reordered focus/context isolation, primary-button-only clicks on both pages")
    finally:
        Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(saved))
        assert json.loads(command("R"))["ok"]
        b.close()
        a.close()
        watchers.remove(watch)
        watch.sock.close()


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


def check_draft_preview(context, watch, commits, lookup):
    inline = os.environ.get("SUZAKU_IBUS_INLINE_PREEDIT") == "1"
    # Drain the preceding language/focus publication before using an exact
    # revision-bound panel action. A synchronous native no-op is a bus barrier.
    assert not context.process_key_event(IBus.KEY_F1, 0, 0)
    assert command("Q") == b"1"
    pump()
    updates = []
    observer = context.connect("update-preedit-text", lambda _, text, _cursor, visible:
                               updates.append((text.get_text(), visible)))
    try:
        for seed in ["hello ", " ", "é" * 159 + "😀", "é" * 160 + "😀", "ab日本😀" * 100]:
            before = len(commits)
            assert action(watch.latest, "T" + seed), ("preview replacement", len(seed), watch.latest)
            wait(lambda: watch.latest["seed"] == seed, "complete draft remains in the host")
            if inline:
                wait(lambda: updates and updates[-1] == (seed, True), "opt-in inline draft remains complete")
            else:
                expected = "Suzaku · " + ("…" if len(seed) > 160 else "") + seed[-160:] + "\n"
                wait(lambda: lookup["aux_visible"] and lookup["aux"].startswith(expected),
                     "bounded Unicode draft tail remains visible in the candidate area")
                assert all(not text and not visible for text, visible in updates)
            assert context.process_key_event(IBus.KEY_Return, 0, 0)
            wait(lambda: commits[before:] == [seed] and not watch.latest["seed"] and
                    not lookup["aux_visible"], "preview bounds never truncate the commit")
        commits.clear()
        print("PASS: 5 Unicode/space/long draft previews and exact commits; inline:", inline)
    finally:
        context.disconnect(observer)


def check_candidate_presentation(watch, lookup, require_mix=True):
    frame = watch.latest
    wait(lambda: lookup["visible"] and lookup["aux_visible"] and
         lookup.get("text") == [c["ibus_label"] for c in frame["candidates"]], "IBus candidate metadata/labels")
    assert lookup["page_size"] == 6
    assert lookup["keys"] == ["¹", "²", "³", "⁴", "⁵", "⁶"]
    assert "1–6 选词续写" in lookup["aux"]
    assert "Alt+数字" in lookup["aux"] and "空格连写" in lookup["aux"]
    assert "Enter/点击提交" in lookup["aux"]
    if require_mix:
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
        assert context.process_key_event(IBus.KEY_space, 0, 0)
        wait(lambda: watch.latest["seed"] == prefix + " ", "plain Space preserved in preedit")
        type_seed(context, rest)
        wait(lambda: watch.latest["seed"] == prefix + " " + rest, "continue across spelling separator")
        assert len(commits) == before, "Space committed prematurely"
        assert any(c["text"] == converted for c in watch.latest["candidates"])
    for language, seed in [("en", "hel"), ("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(command("L" + language))["ok"]
        for slot in range(6):
            context.reset()
            wait(lambda: not watch.latest["seed"], "reset number candidate choice")
            type_seed(context, seed)
            check_candidate_presentation(watch, lookup)
            before = len(commits)
            if slot >= len(watch.latest["candidates"]):
                expected = seed
            else:
                expected = watch.latest["candidates"][slot]["text"]
            mask = [0, IBus.ModifierType.LOCK_MASK, IBus.ModifierType.MOD2_MASK][slot % 3]
            assert context.process_key_event(IBus.KEY_1 + slot, 0, mask)
            assert not context.process_key_event(IBus.KEY_1 + slot, 0, mask | IBus.ModifierType.RELEASE_MASK)
            wait(lambda: watch.latest["seed"] == expected, "number choice must remain editable")
            assert len(commits) == before, "number choice committed prematurely"
            if expected != seed:
                assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
                wait(lambda: watch.latest["seed"] == seed, "number choice must support spelling undo")
            # All three languages support numbers without committing or replacing a candidate.
            type_seed(context, "0123456789")
            wait(lambda: watch.latest["seed"] == seed + "0123456789", "Alt digits are literal in every language")
            assert len(commits) == before
            selected = watch.latest["candidates"][watch.latest["selected"]]["text"]
            assert context.process_key_event(IBus.KEY_Return, 0, 0)
            wait(lambda: commits[before:] == [selected] and not watch.latest["seed"], "Enter must commit exactly once")
            wait(lambda: not lookup["visible"] and not lookup["aux_visible"], "candidate and hint dismissal")
    assert json.loads(command("Len"))["ok"]
    type_seed(context, "zzzxq")
    baseline, before = watch.latest, len(commits)
    assert len(baseline["candidates"]) == 1
    for digit in "023456789":
        assert context.process_key_event(ord(digit), 0, 0)
        pump()
        assert watch.latest == baseline and len(commits) == before, "missing slot inserted a number"
    context.reset()
    wait(lambda: not watch.latest["seed"], "reset missing candidate slots")
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
    print("PASS: multilingual Space continuation, six-row number choices with undo, Alt literal digits, empty slots, metadata and Enter-only keyboard commits")


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
        # Setting an already active language is not a draft reset.
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset CJK completion fixture")
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
    for key, mask, expected in [(IBus.KEY_space, 0, "hello "),
                               (IBus.KEY_space, IBus.ModifierType.SHIFT_MASK, "hello "),
                               (IBus.KEY_Return, IBus.ModifierType.SHIFT_MASK, "hello"),
                               (IBus.KEY_KP_Enter, IBus.ModifierType.SHIFT_MASK, "hello")]:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset editable completion")
        type_seed(context, "hel")
        choose("hello")
        before = len(commits)
        assert context.process_key_event(key, 0, mask)
        assert not context.process_key_event(key, 0, mask | IBus.ModifierType.RELEASE_MASK)
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


def check_literal_choice_publication(context, watch, commits, lookup):
    """N28: adopting unchanged spelling still changes the selected candidate."""
    failures = []
    for language, seed in [("en", "hel"), ("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(command("L" + language))["ok"]
        for followup, mask in [
            ("commit", 0), ("space", IBus.ModifierType.LOCK_MASK),
            ("delete", IBus.ModifierType.MOD2_MASK),
            ("adopt and undo", IBus.ModifierType.LOCK_MASK | IBus.ModifierType.MOD2_MASK),
        ]:
            context.reset()
            wait(lambda: not watch.latest["seed"], "reset literal choice audit")
            type_seed(context, seed)
            check_candidate_presentation(watch, lookup)
            candidates = watch.latest["candidates"]
            literal = next(i for i, c in enumerate(candidates) if c["text"] == seed)
            page = (literal // 6) * 6
            other = next(i for i, c in enumerate(candidates) if page <= i < page + 6 and c["text"] != seed)
            for _ in range(other):
                assert context.process_key_event(IBus.KEY_Tab, 0, 0)
            wait(lambda: watch.latest["selected"] == other and lookup["selected"] == other,
                 "prepare a different highlighted candidate")
            baseline, before = watch.latest, len(commits)
            assert context.process_key_event(IBus.KEY_1 + literal % 6, 0, mask)
            assert not context.process_key_event(IBus.KEY_1 + literal % 6, 0, mask | IBus.ModifierType.RELEASE_MASK)
            try:
                wait(lambda: watch.latest["revision"] > baseline["revision"] and
                     watch.latest["selected"] == literal and lookup["selected"] == literal,
                     "literal adoption did not publish its selection")
                assert not action(baseline, "N" + str(other)), "pre-selection revision was still accepted"
            except AssertionError:
                failure = (f"N28 {language}/{followup}: selected="
                           f"{watch.latest['selected']}/{lookup['selected']}, expected={literal}; "
                           f"revision={watch.latest['revision']}, before={baseline['revision']}")
                print("AUDIT: " + failure)
                failures.append(failure)
            assert watch.latest["seed"] == seed and len(commits) == before
            assert watch.latest["candidates"] == candidates, "literal selection rebuilt or reordered candidates"
            if followup == "commit":
                assert context.process_key_event(IBus.KEY_Return, 0, 0)
                wait(lambda: commits[before:] == [seed] and not watch.latest["seed"],
                     "literal choice must match the actual Enter payload")
            elif followup == "space":
                assert context.process_key_event(IBus.KEY_space, 0, 0)
                wait(lambda: watch.latest["seed"] == seed + " ", "literal choice must continue unchanged")
            elif followup == "delete":
                assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
                wait(lambda: watch.latest["seed"] == seed[:-1], "unchanged spelling must not create completion undo")
            else:
                assert context.process_key_event(IBus.KEY_1 + other % 6, 0, 0)
                wait(lambda: watch.latest["seed"] == candidates[other]["text"], "adopt after literal selection")
                assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
                wait(lambda: watch.latest["seed"] == seed, "undo must still restore the original spelling")
            if followup != "commit":
                assert len(commits) == before, "literal selection/continuation committed unexpectedly"
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish literal selection audit")
    assert json.loads(command("Len"))["ok"]
    assert not failures, "literal selection publication: " + repr(failures)
    print("PASS: N28 12 English/Chinese/Japanese literal choices publish selection and preserve commit/Space/deletion/undo")


def check_unchanged_input_controls(context, watch, commits, lookup):
    """N29/N30: repeating a setting must preserve the live input state."""
    failures = []
    cases = 0

    def controls(language, alias):
        settings = json.loads(command("S"))["settings"]
        assert not settings["llm_enabled"]
        return ["L" + language, "L" + alias, "P0",
                "U" + json.dumps({"llm_temperature_tenths": settings["llm_temperature_tenths"]})]

    for language, alias, seed, word in [
        ("en", "en_US", "hel", "hello"),
        ("zh-Hans", "zh_CN", "nihao", "你好"),
        ("ja", "ja_JP", "nihongo", "日本語"),
    ]:
        assert json.loads(command("L" + language))["ok"]
        for request in controls(language, alias):
            for phase in ["selected", "adopted"]:
                cases += 1
                context.reset()
                wait(lambda: not watch.latest["seed"], "reset unchanged-control draft")
                type_seed(context, seed)
                index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == word)
                assert context.process_key_event(IBus.KEY_Up, 0, 0)
                for _ in range(index):
                    assert context.process_key_event(IBus.KEY_Tab, 0, 0)
                wait(lambda: watch.latest["selected"] == index and lookup["selected"] == index,
                     "select before repeating a setting")
                if phase == "adopted":
                    assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
                    wait(lambda: watch.latest["seed"] == word, "adopt before repeating a setting")
                baseline, before = watch.latest, len(commits)
                assert json.loads(command(request))["ok"], request
                wait(lambda: watch.latest["revision"] > baseline["revision"], "control acknowledgement publication")
                try:
                    assert watch.latest["seed"] == baseline["seed"], "same setting cleared the draft"
                    assert watch.latest["candidates"] == baseline["candidates"], "same setting rebuilt candidates"
                    assert watch.latest["selected"] == baseline["selected"] and lookup["selected"] == baseline["selected"]
                    revision = watch.latest["revision"]
                    key = IBus.KEY_space if phase == "selected" else IBus.KEY_BackSpace
                    assert context.process_key_event(key, 0, 0)
                    wait(lambda: watch.latest["revision"] > revision, "continue after unchanged control")
                    expected = word + " " if phase == "selected" else seed
                    assert watch.latest["seed"] == expected, f"expected {expected!r} after continuation"
                except AssertionError as error:
                    failure = f"{language}/{phase}/{request}: {error}; seed={watch.latest['seed']!r}"
                    print("AUDIT: N29/N30 " + failure)
                    failures.append(failure)
                assert len(commits) == before, "settings must not submit input"

    assert json.loads(command("Len"))["ok"]
    for request in controls("en", "en_US"):
        cases += 1
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset same-setting Compose check")
        type_seed(context, "caf")
        assert context.process_key_event(IBus.KEY_dead_acute, 0, 0)
        wait(lambda: lookup["aux"].endswith("Compose… · Esc / Backspace 取消"), "prepare pending accent")
        baseline, before = watch.latest, len(commits)
        assert json.loads(command(request))["ok"]
        wait(lambda: watch.latest["revision"] > baseline["revision"], "same-setting Compose publication")
        assert context.process_key_event(IBus.KEY_e, 0, 0)
        pump()
        if watch.latest["seed"] != "café":
            failure = f"en/compose/{request}: expected 'café', seed={watch.latest['seed']!r}"
            print("AUDIT: N29 " + failure)
            failures.append(failure)
        assert len(commits) == before
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish unchanged input controls")
    assert cases == 28
    assert not failures, "unchanged input controls:\n" + "\n".join(failures)
    print("PASS: N29/N30 28 unchanged-language/prediction/Tone cases preserve selection, undo and Compose")


def check_unchanged_control_failures(context, watch, commits, lookup):
    """Skipping a rebuild must not skip persistence/conflict validation."""
    assert json.loads(command("Len"))["ok"]
    config = Path(os.environ["SUZAKU_IME_CONFIG"])
    saved = json.loads(command("S"))["settings"]
    original = config.read_text()
    mode = runtime.stat().st_mode & 0o777
    requests = ["Len", "Len_US", "P0",
                "U" + json.dumps({"llm_temperature_tenths": saved["llm_temperature_tenths"]})]
    for request in requests:
        for failure in ["conflict", "write failure"]:
            context.reset()
            wait(lambda: not watch.latest["seed"], "reset rejected unchanged control")
            type_seed(context, "hel")
            assert context.process_key_event(IBus.KEY_Tab, 0, 0)
            wait(lambda: watch.latest["selected"] == 1 and lookup["selected"] == 1,
                 "select before rejected unchanged control")
            baseline, before = watch.latest, len(commits)
            try:
                if failure == "conflict":
                    changed = dict(saved)
                    changed["llm_temperature_tenths"] = (saved["llm_temperature_tenths"] + 1) % 11
                    config.write_text(json.dumps(changed))
                else:
                    runtime.chmod(0o500)
                disk_before = config.read_text()
                response = json.loads(command(request))
                assert not response["ok"] and response["settings"] == saved, (request, failure)
                assert config.read_text() == disk_before, "failed no-op control overwrote saved settings"
                pump()
                assert watch.latest == baseline and len(commits) == before, (request, failure)
            finally:
                runtime.chmod(mode)
                config.write_text(original)
            assert context.process_key_event(IBus.KEY_space, 0, 0)
            wait(lambda: watch.latest["seed"] == "hello ", "failed control discarded the selected completion")
            assert len(commits) == before
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish rejected unchanged controls")
    print("PASS: N29/N30 8 unchanged-control conflict/write failures preserve settings, draft and selection")


def check_unchanged_model_controls(context, watch, commits, lookup):
    settings = json.loads(command("S"))["settings"]
    assert settings["llm_enabled"] and settings["language"] == "en"
    requests = ["Len", "Len_US", "P1",
                "U" + json.dumps({"llm_temperature_tenths": settings["llm_temperature_tenths"]}),
                "U" + json.dumps({"llm_enabled": True, "llm_temperature_tenths": settings["llm_temperature_tenths"]})]
    for request in requests:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset unchanged model control")
        type_seed(context, "hel")
        wait(lambda: any(c["text"] == "helium" and c["source"] == "model" for c in watch.latest["candidates"]),
             "prepare model candidate before repeating a setting")
        index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == "helium")
        for _ in range(index):
            assert context.process_key_event(IBus.KEY_Tab, 0, 0)
        wait(lambda: watch.latest["selected"] == index and lookup["selected"] == index, "lock model candidate")
        baseline, before, requested = watch.latest, len(commits), len(model_requests)
        assert json.loads(command(request))["ok"]
        wait(lambda: watch.latest["revision"] > baseline["revision"], "unchanged model control publication")
        until = time.monotonic() + 0.35
        while time.monotonic() < until:
            pump()
            assert watch.latest["seed"] == "hel" and watch.latest["candidates"] == baseline["candidates"]
            assert watch.latest["selected"] == index and lookup["selected"] == index
            assert len(model_requests) == requested, "unchanged control restarted prediction"
            time.sleep(0.01)
        assert context.process_key_event(IBus.KEY_space, 0, 0)
        wait(lambda: watch.latest["seed"] == "helium ", "same-setting control unlocked model selection")
        assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: watch.latest["seed"] == "hel", "same-setting model continuation lost undo")
        assert len(commits) == before
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish unchanged model controls")
    type_seed(context, "hel")
    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "restore model reload fixture")
    print("PASS: N30 5 unchanged controls preserve locked model candidates without new requests")


def check_lossless_commit_chunks(context, watch, commits):
    assert json.loads(command("Len"))["ok"]
    pump()
    for text, key in [("first", None), ("  second  ", None),
                      ("\u3000third", IBus.KEY_Return), ("  fourth", IBus.KEY_space)]:
        assert action(watch.latest, "T" + text)
        wait(lambda: watch.latest["seed"] == text, "prepare exact commit text")
        before = len(commits)
        if key is None:
            assert action(watch.latest, "K0")
        else:
            assert context.process_key_event(key, 0, 0)
        expected = text + (" " if key == IBus.KEY_space else "")
        if key == IBus.KEY_space:
            wait(lambda: watch.latest["seed"] == expected, "Space lost exact draft whitespace")
            assert len(commits) == before, "Space committed a draft"
            assert context.process_key_event(IBus.KEY_Return, 0, 0)
        wait(lambda: len(commits) > before and not watch.latest["seed"], "exact text was not committed")
        assert commits[before:] == [expected], f"commit altered text: {commits[before:]!r}, expected {expected!r}"
    print("PASS: consecutive panel/keyboard commits preserve exact leading, trailing and Unicode spaces")


def check_writing_stream_prediction(context, watch, commits):
    def fresh_field():
        previous = watch.latest["context"]
        context.focus_out()
        # Global-engine IBus can immediately focus its fallback context on FocusOut.
        wait(lambda: watch.latest["context"] != previous and not watch.latest["seed"],
             "leave continuous writing field")
        previous = watch.latest["context"]
        context.focus_in()
        wait(lambda: watch.latest["focused"] and watch.latest["context"] != previous and not watch.latest["seed"],
             "new continuous writing field")

    fresh_field()
    before, request_start = len(commits), len(model_requests)
    type_seed(context, "hel")
    assert context.process_key_event(IBus.KEY_2, 0, 0)
    wait(lambda: watch.latest["seed"] == "hello", "accept word by number before model continuation")
    for _ in range(2):
        assert context.process_key_event(IBus.KEY_space, 0, 0)
    type_seed(context, "world!")
    draft = "hello  world!"
    wait(lambda: watch.latest["seed"] == draft, "preserve repeated spaces and punctuation")

    def requests_for(seed):
        return [payload for request in model_requests[request_start:]
                if (payload := json.loads(request["messages"][1]["content"]))["raw_composition"] == seed]

    wait(lambda: requests_for(draft), "LLM did not receive the full ongoing writing stream")
    assert all(payload["committed_context"] == "" for payload in requests_for(draft))
    assert len(commits) == before, "continuation created an application commit"
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == [draft] and not watch.latest["seed"], "commit the stream once")
    type_seed(context, "ne")
    wait(lambda: requests_for("ne"), "next stream did not request predictions")
    assert all(payload["committed_context"] == draft for payload in requests_for("ne"))
    fresh_field()
    before = len(commits)
    type_seed(context, "caf")
    assert context.process_key_event(IBus.KEY_dead_acute, 0, 0)
    assert context.process_key_event(IBus.KEY_e, 0, 0)
    type_seed(context, " please sen")
    composed = "café please sen"
    wait(lambda: requests_for(composed), "LLM did not receive the composed Unicode writing stream")
    assert all(payload["committed_context"] == "" for payload in requests_for(composed))
    assert watch.latest["seed"] == composed and len(commits) == before
    fresh_field()
    type_seed(context, "hel")
    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "restore AI fixture draft")
    print("PASS: LLM sees the full uncommitted stream; context advances only after explicit Enter")


def check_long_model_completions(context, watch, commits):
    """N02: both complete payloads survive decoding, merging and native commit."""
    for length in [160, 256]:
        seed = "note " * ((length - 3) // 5) + " " * ((length - 3) % 5) + "hel"
        assert len(seed) == length
        word, sentence = seed + "ioseismology", seed + "ioseismology is interesting."
        for chosen in [word, sentence]:
            # Reset() is asynchronous on a different D-Bus channel. If the
            # previous commit already emptied the seed, merely waiting for an
            # empty seed can race the following Unix-socket replacement.
            revision = watch.latest["revision"]
            assert action(watch.latest, "X")
            wait(lambda: watch.latest["revision"] > revision and not watch.latest["seed"],
                 "acknowledged clear before long model draft")
            before = len(commits)
            assert action(watch.latest, "T" + seed)
            try:
                wait(lambda: watch.latest["seed"] == seed and any(c["source"] == "model" for c in watch.latest["candidates"]),
                     "N02: long draft did not receive a usable model result")
            except AssertionError as error:
                # Only synthetic fixture data; distinguish a request/decoder
                # failure from a snapshot/ranking failure when CI regresses.
                requests = [payload for request in model_requests
                            if (payload := json.loads(request["messages"][1]["content"]))["raw_composition"] == seed]
                raise AssertionError({
                    "length": length, "word_choice": chosen == word, "seed_length": len(watch.latest["seed"]),
                    "private": watch.latest["private"], "language": watch.latest["language"],
                    "requests": [(len(p["local_conversion"]), p["local_conversion"] == seed) for p in requests],
                    "candidates": [(len(c["text"]), c["text"][-35:], c["kind"], c["source"]) for c in watch.latest["candidates"]],
                }) from error
            candidates = watch.latest["candidates"]
            assert any(c["text"] == word and c["kind"] == "word" and c["source"] == "model" for c in candidates)
            assert any(c["text"] == sentence and c["kind"] == "sentence" and c["source"] == "model" for c in candidates)
            assert any(c["text"] == seed for c in candidates), "literal draft was lost"
            assert not any(c["text"] == seed + "x" * 161 for c in candidates), "generated-text budget was bypassed"
            index = next(i for i, c in enumerate(candidates) if c["text"] == chosen)
            if chosen == word:
                assert action(watch.latest, f"N{index}")
                wait(lambda: watch.latest["selected"] == index, "select the full long word completion")
                assert context.process_key_event(IBus.KEY_Return, 0, IBus.ModifierType.SHIFT_MASK)
                wait(lambda: watch.latest["seed"] == chosen, "adopted word lost its long prefix")
                assert len(commits) == before, "word adoption must not submit"
                assert context.process_key_event(IBus.KEY_Return, 0, 0)
            else:
                assert action(watch.latest, f"K{index}")
            wait(lambda: commits[before:] == [chosen] and not watch.latest["seed"],
                 "N02: long completion must commit its exact full payload once")
    type_seed(context, "hel")
    wait(lambda: any(c["source"] == "model" for c in watch.latest["candidates"]), "restore ordinary model draft")
    print("PASS: N02 160/256-character model word/sentence completions, bounded additions, editable adoption and exact commits")


def check_sentence_only_prediction(context, watch, commits, lookup):
    context.reset()
    wait(lambda: not watch.latest["seed"], "clear sentence-only fixture")
    type_seed(context, "please rec")
    word, sentence = "please reconsider", "please reconsider the proposal."

    def ready():
        return any(c["text"] == word and c["kind"] == "word" and c["source"] == "model"
                   for c in watch.latest["candidates"][:6])

    wait(ready, "sentence-only reply did not expose its first completed word")
    check_candidate_presentation(watch, lookup)
    assert any(c["text"] == sentence and c["kind"] == "sentence" for c in watch.latest["candidates"][:6])
    before = len(commits)
    for undo in [True, False]:
        wait(ready, "word completion disappeared after undo")
        index = next(i for i, c in enumerate(watch.latest["candidates"][:6]) if c["text"] == word)
        assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
        wait(lambda: watch.latest["seed"] == word, "choose the projected word, not the full sentence")
        assert len(commits) == before
        if undo:
            assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
            wait(lambda: watch.latest["seed"] == "please rec", "restore spelling after model-word adoption")
    type_seed(context, " again")
    wait(lambda: watch.latest["seed"] == "please reconsider again", "model word must stay editable")
    assert len(commits) == before
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == ["please reconsider again"] and not watch.latest["seed"], "submit continued model word")
    type_seed(context, "please rec")
    wait(ready, "restore sentence-only candidates")
    index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == sentence)
    before = len(commits)
    assert action(watch.latest, f"K{index}")
    wait(lambda: commits[before:] == [sentence] and not watch.latest["seed"], "full sentence must remain independently selectable")
    print("PASS: sentence-only model replies provide distinct word/sentence choices, numeric adoption/undo, continued drafting and exact commits")


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
        (prefix, [(ord(c), IBus.ModifierType.MOD1_MASK, True) for c in suffix], prefix + suffix)
        for prefix, suffix in [("hel", "2"), ("v", "123"), ("", "2026"), ("a", "0456789")]
    ]
    for key in [IBus.KEY_Shift_L, IBus.KEY_Shift_R, IBus.KEY_Caps_Lock,
                IBus.KEY_Control_L, IBus.KEY_ISO_Level3_Shift, IBus.KEY_F1]:
        cases.append(("hel", [(key, 0, False), (ord("L"), IBus.ModifierType.SHIFT_MASK, True)], "helL"))
    cases.append(("v", [(IBus.KEY_KP_1, 0, True)], "v1"))
    cases.append(("hel", [(IBus.KEY_2, IBus.ModifierType.MOD2_MASK | IBus.ModifierType.MOD1_MASK, True)], "hel2"))
    cases.append(("hel", [(IBus.KEY_2, IBus.ModifierType.SHIFT_MASK, True)], "hel2"))
    cases.append(("hel", [(IBus.KEY_2, IBus.ModifierType.MOD5_MASK, True)], "hel2"))
    for key in [IBus.KEY_0 + i for i in range(10)] + [IBus.KEY_KP_1, IBus.KEY_KP_9]:
        cases.append(("", [(key, 0, False)], ""))
    cases.append(("hel", [(ord(c), IBus.ModifierType.SHIFT_MASK, True) for c in "!@#$%^&*()"], "hel!@#$%^&*()"))
    cases.append(("hel", [(IBus.unicode_to_keyval(c), 0, True) for c in ".com/path?x=y, ok;_日本。"], "hel.com/path?x=y, ok;_日本。"))
    cases.append(("hel", [(IBus.KEY_L, IBus.ModifierType.LOCK_MASK, True)], "helL"))
    cases.append(("hel", [(IBus.KEY_x, IBus.ModifierType.MOD5_MASK, True)], "helx"))
    cases.append(("hel", [(IBus.KEY_c, IBus.ModifierType.CONTROL_MASK, False)], "hel"))
    for mask in [IBus.ModifierType.MOD4_MASK, IBus.ModifierType.SUPER_MASK,
                 IBus.ModifierType.META_MASK, IBus.ModifierType.HYPER_MASK]:
        for key in [IBus.KEY_x, IBus.KEY_Tab, IBus.KEY_Return, IBus.KEY_space]:
            cases.append(("hel", [(key, mask, False)], "hel"))
        cases.append(("hel", [(IBus.KEY_1, mask | IBus.ModifierType.MOD1_MASK, False)], "hel"))
    cases.append(("hel", [(IBus.KEY_x, IBus.ModifierType.RELEASE_MASK, False)], "hel"))
    for prefix, keys, expected in [
        ("caf", [IBus.KEY_dead_acute, IBus.KEY_e], "café"),
        ("", [IBus.KEY_dead_acute, IBus.KEY_E], "É"),
        ("", [IBus.KEY_Multi_key, IBus.KEY_apostrophe, IBus.KEY_e], "é"),
        ("pi", [IBus.KEY_Multi_key, IBus.KEY_s, IBus.KEY_s], "piß"),
        ("pay ", [IBus.KEY_Multi_key, IBus.KEY_1, IBus.KEY_2], "pay ½"),
        ("caf", [IBus.KEY_dead_acute, IBus.KEY_space], "caf'"),
        ("", [IBus.unicode_to_keyval("é")], "é"),
        ("", [IBus.KEY_dead_acute, IBus.KEY_dead_tilde, IBus.KEY_e], "ẽ́"),
        ("hel", [IBus.KEY_Multi_key, IBus.KEY_F11, IBus.KEY_1], "heltwo words"),
        ("hel", [IBus.KEY_Multi_key, IBus.KEY_F11, IBus.KEY_2], "hel"),
        ("hel", [IBus.KEY_Multi_key, IBus.KEY_F11, IBus.KEY_3], "hel" + "é" * 127),
    ]:
        cases.append((prefix, [(key, 0, True) for key in keys], expected))
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
            if not expected:
                assert not watch.latest["candidates"]
                assert not context.process_key_event(IBus.KEY_space, 0, 0)
                assert not context.process_key_event(IBus.KEY_Return, 0, 0)
                continue
            assert watch.latest["candidates"][0]["text"] == expected
            assert context.process_key_event(IBus.KEY_space, 0, 0)
            wait(lambda: watch.latest["seed"] == expected + " ", "Space must stay in the same draft")
            assert len(commits) == before, "Space committed literal input"
            assert context.process_key_event(IBus.KEY_Return, 0, 0)
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
    assert context.process_key_event(IBus.KEY_space, 0, 0)
    wait(lambda: watch.latest["seed"] == "hello ", "selected word must remain in the writing stream")
    type_seed(context, "world")
    assert context.process_key_event(ord("!"), 0, IBus.ModifierType.SHIFT_MASK)
    wait(lambda: watch.latest["seed"] == "hello world!", "punctuation must remain in the writing stream")
    assert len(commits) == before
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == ["hello world!"] and not watch.latest["seed"],
         "only Enter commits the complete writing stream")


def check_numeric_field_routing(context, watch, commits):
    assert json.loads(command("Len"))["ok"]
    for purpose in [IBus.InputPurpose.DIGITS, IBus.InputPurpose.NUMBER, IBus.InputPurpose.PHONE]:
        context.set_content_type(IBus.InputPurpose.FREE_FORM, 0)
        context.reset()
        wait(lambda: not watch.latest["seed"] and not watch.latest["private"], "numeric fixture reset")
        type_seed(context, "hel")
        assert context.process_key_event(IBus.KEY_dead_acute, 0, 0)
        context.set_content_type(purpose, 0)
        wait(lambda: not watch.latest["seed"] and watch.latest["private"], "numeric transition clears the old draft")
        before, requested = len(commits), len(model_requests)
        keys = [ord(ch) for ch in "1234567890+-.()"] + [IBus.KEY_dead_acute, IBus.KEY_Multi_key, IBus.KEY_KP_1, IBus.KEY_BackSpace,
                IBus.KEY_Return, IBus.KEY_space, IBus.KEY_Tab]
        for key in keys:
            assert not context.process_key_event(key, 0, 0), (purpose, key)
            assert not context.process_key_event(key, 0, IBus.ModifierType.RELEASE_MASK)
        assert not action(watch.latest, "Twrong target")
        assert command("Cwrong target") == b"0"
        pump()
        assert not watch.latest["seed"] and not watch.latest["candidates"]
        assert len(commits) == before and len(model_requests) == requested
    context.set_content_type(IBus.InputPurpose.FREE_FORM, 0)
    wait(lambda: not watch.latest["private"], "return to normal text input")
    type_seed(context, "hel")
    assert context.process_key_event(IBus.KEY_2, 0, 0)
    wait(lambda: watch.latest["seed"] == "hello", "ordinary fields retain numeric candidate choices")
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish numeric field fixture")
    print("PASS: numeric/decimal/phone fields bypass candidate keys and panel injection; normal-field choices resume")


def check_english_compose_boundaries(context, watch, commits, lookup):
    before = len(commits)

    def start(prefix="hel"):
        context.reset()
        wait(lambda: not watch.latest["seed"], "clear compose fixture")
        type_seed(context, prefix)
        assert context.process_key_event(IBus.KEY_dead_acute, 0, 0)
        wait(lambda: lookup["aux_visible"] and lookup["aux"].endswith("Compose… · Esc / Backspace 取消"),
             "missing compose hint")
        assert not lookup["visible"] and watch.latest["seed"] == prefix

    for prefix in ["", "hel"]:
        for key in [IBus.KEY_BackSpace, IBus.KEY_Escape]:
            start(prefix)
            assert context.process_key_event(key, 0, 0)
            type_seed(context, "e")
            wait(lambda: watch.latest["seed"] == prefix + "e", "cancel must preserve the draft, not the accent")
    for mask in [IBus.ModifierType.CONTROL_MASK, IBus.ModifierType.MOD1_MASK,
                 IBus.ModifierType.SUPER_MASK, IBus.ModifierType.MOD4_MASK]:
        start()
        assert not context.process_key_event(IBus.KEY_c, 0, mask)
        type_seed(context, "e")
        wait(lambda: watch.latest["seed"] == "hele", "shortcut retained a pending accent")

    start("caf")
    assert not context.process_key_event(IBus.KEY_dead_acute, 0, IBus.ModifierType.RELEASE_MASK)
    assert not context.process_key_event(IBus.KEY_Shift_L, 0, 0)
    assert context.process_key_event(IBus.KEY_E, 0, IBus.ModifierType.SHIFT_MASK | IBus.ModifierType.LOCK_MASK)
    wait(lambda: watch.latest["seed"] == "cafÉ", "Shift/lock/release interrupted composition")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == "caf", "Backspace must edit a composed letter normally")
    for key, expected in [(IBus.KEY_b, "helb"), (IBus.KEY_9, "hel9")]:
        start()
        assert context.process_key_event(key, 0, 0)
        wait(lambda: watch.latest["seed"] == expected, "invalid sequence must retain its final printable key literally")

    start("hello world")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, IBus.ModifierType.CONTROL_MASK)
    type_seed(context, "e")
    wait(lambda: watch.latest["seed"] == "hello e", "draft word deletion retained an accent")
    start()
    assert action(watch.latest, "Tnew")
    type_seed(context, "e")
    wait(lambda: watch.latest["seed"] == "newe", "panel replacement retained an accent")
    for label, operation in [
        ("reset", lambda: context.reset()), ("language", lambda: command("Lja")),
        ("password", lambda: context.set_content_type(IBus.InputPurpose.PASSWORD, 0)),
        ("private", lambda: context.set_content_type(IBus.InputPurpose.FREE_FORM, 1 << 11)),
    ]:
        start()
        operation()
        wait(lambda: not watch.latest["seed"], "compose field boundary must clear the draft")
        if label == "password":
            assert not context.process_key_event(IBus.KEY_dead_acute, 0, 0)
            assert not context.process_key_event(IBus.KEY_e, 0, 0)
        context.set_content_type(IBus.InputPurpose.FREE_FORM, 0)
        if label == "language":
            type_seed(context, "e")
            wait(lambda: watch.latest["seed"] == "e", "language switch retained a pending accent")
            assert json.loads(command("Len"))["ok"]
        type_seed(context, "e")
        wait(lambda: watch.latest["seed"] == "e", "compose leaked through a reset/language/privacy boundary")
    assert len(commits) == before, "composition/cancellation committed unexpectedly"

    start()
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == ["hel"] and not watch.latest["seed"], "Enter must still submit only the existing draft")
    type_seed(context, "e")
    wait(lambda: watch.latest["seed"] == "e", "submit retained a pending accent")
    context.reset()
    wait(lambda: not watch.latest["seed"], "finish compose fixture")
    print("PASS: dead-key/Compose cancellation, modifiers, invalid sequences, panel edits, draft-only commit and reset/privacy boundaries")


def check_english_writing_flow(context, watch, commits, lookup):
    assert json.loads(command("Len"))["ok"]
    for seed, expected, kind in [
        ("we’", "we’re", "word"), ("THEY’", "THEY’RE", "word"),
        ("He said 'please sen", "He said 'please send", "word"),
        ("'hel", "'hello, how are you?", "sentence"),
        ("how can I ", "how can I help", "word"),
        ("I’m w", "I’m working", "word"),
        ("could you sh", "could you share", "word"),
        ("I would like to ", "I would like to ask a question.", "sentence"),
    ]:
        context.reset()
        wait(lambda: not watch.latest["seed"], "reset quoted/contraction draft")
        type_seed(context, seed)
        check_candidate_presentation(watch, lookup, require_mix=kind == "sentence")
        first = watch.latest["candidates"][:6]
        index = next(i for i, c in enumerate(first) if c["text"] == expected and c["kind"] == kind)
        before = len(commits)
        assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
        wait(lambda: watch.latest["seed"] == expected, "quote/apostrophe changed on number choice")
        assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: watch.latest["seed"] == seed, "quote/apostrophe changed on undo")
        assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
        assert context.process_key_event(IBus.KEY_space, 0, 0)
        wait(lambda: watch.latest["seed"] == expected + " ", "quoted completion did not continue")
        assert len(commits) == before
        assert context.process_key_event(IBus.KEY_Return, 0, 0)
        wait(lambda: commits[before:] == [expected + " "] and not watch.latest["seed"], "quoted literal commit")
    context.reset()
    wait(lambda: not watch.latest["seed"], "start English writing flow")
    before = len(commits)
    type_seed(context, "please send")
    for suffix, expected_word, expected_sentence in [
        (" ", "please send me", "please send me the details."),
        ("m", "please send me", "please send me the details."),
        ("e the d", "please send me the details", "please send me the details."),
    ]:
        type_seed(context, suffix)
        check_candidate_presentation(watch, lookup)
        first = watch.latest["candidates"][:6]
        assert any(c["text"] == expected_word and c["kind"] == "word" for c in first), first
        assert any(c["text"] == expected_sentence and c["kind"] == "sentence" for c in first), first
        assert len(commits) == before
    # Number choice continues the draft; Backspace still restores its exact spelling.
    seed = watch.latest["seed"]
    index = next(i for i, c in enumerate(first) if c["text"] == expected_word)
    assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
    wait(lambda: watch.latest["seed"] == expected_word, "complete English word without committing")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == seed, "undo English completion")

    # Deleting a completed word must discard its old one-step completion undo.
    index = next(i for i, c in enumerate(watch.latest["candidates"][:6]) if c["text"] == expected_word)
    assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
    wait(lambda: watch.latest["seed"] == expected_word, "complete before deleting the word")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, IBus.ModifierType.CONTROL_MASK)
    wait(lambda: watch.latest["seed"] == "please send me the ", "delete completed word, not undo it")
    assert context.process_key_event(IBus.KEY_BackSpace, 0, 0)
    wait(lambda: watch.latest["seed"] == "please send me the", "ordinary Backspace must not restore an old completion")
    assert len(commits) == before

    # Long previews must identify the tail, while number/Enter retain full text.
    seed = "For the next release please review the current docu"
    expected = seed + "ment"
    assert action(watch.latest, "T" + seed)
    wait(lambda: watch.latest["seed"] == seed, "prepare long English draft")
    check_candidate_presentation(watch, lookup, require_mix=False)
    index = next(i for i, c in enumerate(watch.latest["candidates"]) if c["text"] == expected)
    assert index < 6
    label = watch.latest["candidates"][index]["ibus_label"]
    assert label.startswith("…") and "document" in label, label
    assert context.process_key_event(IBus.KEY_1 + index, 0, 0)
    wait(lambda: watch.latest["seed"] == expected, "long preview must not become replacement text")
    assert context.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: commits[before:] == [expected] and not watch.latest["seed"], "long English commit stays complete")

    control = IBus.ModifierType.CONTROL_MASK
    for text, shortened in [
        ("please send the details", "please send the "),
        ("please send the details  ", "please send the "),
        ("hello can't", "hello "), ("hello world!", "hello "),
        ("hello 日本😀", "hello "), ("  hello", "  "),
        ("word", ""), ("  ", ""),
    ]:
        assert action(watch.latest, "T" + text)
        wait(lambda: watch.latest["seed"] == text, "prepare English word deletion")
        before = len(commits)
        assert context.process_key_event(IBus.KEY_BackSpace, 0, control)
        assert not context.process_key_event(IBus.KEY_BackSpace, 0, control | IBus.ModifierType.RELEASE_MASK)
        wait(lambda: watch.latest["seed"] == shortened, "Ctrl+Backspace edits only the draft")
        assert len(commits) == before
    assert not context.process_key_event(IBus.KEY_BackSpace, 0, control), "empty draft stole application shortcut"
    for language in ["en", "zh-Hans", "ja"]:
        assert json.loads(command("L" + language))["ok"]
        type_seed(context, "hel")
        before = watch.latest
        masks = [control | flag for flag in [IBus.ModifierType.SHIFT_MASK, IBus.ModifierType.MOD1_MASK,
                                             IBus.ModifierType.MOD5_MASK, IBus.ModifierType.SUPER_MASK,
                                             IBus.ModifierType.MOD4_MASK, IBus.ModifierType.META_MASK,
                                             IBus.ModifierType.HYPER_MASK]]
        if language != "en":
            masks.append(control)
        for mask in masks:
            assert not context.process_key_event(IBus.KEY_BackSpace, 0, mask)
        pump()
        assert watch.latest == before
        context.reset()
        wait(lambda: not watch.latest["seed"], "clear English shortcut boundary")
    assert json.loads(command("Len"))["ok"]
    print("PASS: English word/sentence continuity, readable long previews, lossless acceptance/undo and draft-only Ctrl+Backspace")


try:
    processes.append(subprocess.Popen(["ibus-daemon", "--single", "--address=" + os.environ["IBUS_ADDRESS"], "--cache=none"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    wait(lambda: (runtime / "ibus.sock").exists(), "isolated IBus did not start")
    processes.append(subprocess.Popen([str(host_path)], stdout=subprocess.DEVNULL))
    wait(host_socket.exists, "native host did not start")
    IBus.init()
    bus = IBus.Bus.new()
    assert bus.is_connected()
    check_tray_activation(bus)
    check_post_process_preedit(bus)
    check_engine_event_boundaries(bus)
    check_native_draft_limits(bus)
    check_prediction_length_boundaries(bus)
    check_commit_target_boundaries(bus)
    check_cancel_target_boundaries(bus)
    check_draft_end_prediction_boundaries(bus)
    check_language_target_boundaries(bus)
    check_content_type_commit_boundaries(bus)
    check_content_type_prediction_boundaries(bus)
    check_engine_destruction(bus)
    check_engine_destruction_prediction(bus)
    check_committed_model_context(bus)
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
    check_draft_preview(context, watch, commits, lookup)
    check_whitespace_commit(context, watch, commits)
    check_settings_file_boundaries(context, watch, commits)
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
        wait(lambda: watch.latest["seed"] == expected, "CJK number choice must remain editable")
        assert len(other_commits) == before
        assert other.process_key_event(IBus.KEY_Return, 0, 0)
        wait(lambda: other_commits[before:] == [expected] and not watch.latest["seed"],
             "CJK numeric candidate selection changed")
    assert json.loads(command("Len"))["ok"]
    check_english_key_regressions(other, watch, other_commits)
    check_english_compose_boundaries(other, watch, other_commits, other_lookup)
    check_lossless_commit_chunks(other, watch, other_commits)
    check_mixed_keyboard(other, watch, other_commits, other_lookup)
    check_editable_completions(other, watch, other_commits, other_lookup)
    check_literal_choice_publication(other, watch, other_commits, other_lookup)
    check_unchanged_input_controls(other, watch, other_commits, other_lookup)
    check_unchanged_control_failures(other, watch, other_commits, other_lookup)
    check_english_writing_flow(other, watch, other_commits, other_lookup)
    check_numeric_field_routing(other, watch, other_commits)
    type_seed(other, "hel")
    # A deterministic local model fixture tests asynchronous publication, not model quality.
    settings = json.loads(command("S"))["settings"]
    settings.update(llm_enabled=True, llm_model="synthetic-model", llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
    Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
    assert json.loads(command("R"))["ok"]
    wait(lambda: watch.latest["seed"] == "hel" and any(" · AI" in c["label"] for c in watch.latest["candidates"]),
         "enabling a new provider lost the native composition")
    check_unchanged_model_controls(other, watch, other_commits, other_lookup)
    # Both unchanged and provider-changing reloads preserve Rust and native preedit.
    for model_name in ["synthetic-model", "synthetic-replacement"]:
        settings["llm_model"] = model_name
        Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
        requested_before, committed_before = len(model_requests), len(other_commits)
        assert json.loads(command("R"))["ok"]
        wait(lambda: any(request["model"] == model_name for request in model_requests[requested_before:]),
             "reloaded provider did not receive the preserved draft")
        for request in model_requests[requested_before:]:
            if request["model"] == model_name:
                payload = json.loads(request["messages"][1]["content"])
                assert payload["raw_composition"] == "hel"
                assert payload["committed_context"] == "", "old provider context leaked on reload"
        wait(lambda: watch.latest["seed"] == "hel" and any(" · AI" in c["label"] for c in watch.latest["candidates"]),
             "reload lost the draft or failed to publish new candidates")
        assert len(other_commits) == committed_before, "reload committed text unexpectedly"
        type_seed(other, "p")
        wait(lambda: watch.latest["seed"] == "help", "native input buffer diverged after reload")
        assert other.process_key_event(IBus.KEY_BackSpace, 0, 0)
        wait(lambda: watch.latest["seed"] == "hel", "native editing broke after reload")
    check_sentence_only_prediction(other, watch, other_commits, other_lookup)
    check_writing_stream_prediction(other, watch, other_commits)
    check_long_model_completions(other, watch, other_commits)
    # Loading a different language still clears the incompatible composition.
    settings["language"] = "ja"
    Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
    assert json.loads(command("R"))["ok"]
    wait(lambda: not watch.latest["seed"], "language reload retained an incompatible draft")
    settings["language"] = "en"
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
    assert other.process_key_event(IBus.KEY_1, 0, 0)
    wait(lambda: watch.latest["seed"] == expected, "second-page number choice must use full text")
    assert len(other_commits) == before
    assert other.process_key_event(IBus.KEY_Return, 0, 0)
    wait(lambda: other_commits[before:] == [expected] and not watch.latest["seed"], "commit page-local choice with Enter")
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
    # Exercise the shipped probe too, including Space-without-commit and later-page navigation.
    for mode, enabled, expected in [("--complete", "P0", "hello "), ("--llm", "P1", "helium")]:
        assert json.loads(command(enabled))["ok"]
        probe = subprocess.run([str(host_path.with_name("linux_ime_probe")), mode, "hel"],
                               capture_output=True, text=True, timeout=15)
        assert probe.returncode == 0, probe.stderr
        assert f"committed={json.dumps(expected)}" in probe.stdout, probe.stdout
        print(probe.stdout.strip())
    print("PASS: push, late subscriber, exact candidates, selection, replacement, commit, stale revisions, context switches, English digits/modifiers/punctuation, CJK number keys, later-page AI selection, model reload draft/context boundaries, Tone requests/persistence/write failure, private/password, Escape/focus-out and host restart")
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
