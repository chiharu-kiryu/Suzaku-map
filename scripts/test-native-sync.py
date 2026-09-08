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
from gi.repository import IBus, GLib

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
        body = json.dumps({"choices": [{"message": {"content": "hello world\nhello everyone\nhello there"}}]}).encode()
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
    context.set_capabilities(IBus.Capabilite.FOCUS | IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE)
    return context


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
    lookup = {"count": 0, "selected": 0, "visible": False}
    context.connect("commit-text", lambda _, text: commits.append(text.get_text()))
    context.connect("update-lookup-table", lambda _, table, visible: lookup.update(count=table.get_number_of_candidates(), selected=table.get_cursor_pos(), visible=visible))
    context.connect("hide-lookup-table", lambda _: lookup.update(visible=False))
    context.focus_in()
    assert bus.set_global_engine("dev.suzaku.linux.ime")
    wait(lambda: context.get_engine() is not None and context.get_engine().get_name() == "dev.suzaku.linux.ime", "engine attach")
    watch = Watch()
    wait(lambda: watch.latest is not None and watch.latest["focused"], "initial subscription")
    assert not watch.latest["seed"] and not watch.latest["candidates"]
    assert json.loads(command("Len"))["ok"]
    type_seed(context, "hel")
    wait(lambda: watch.latest["seed"] == "hel" and lookup["visible"], "typed snapshot")
    old = watch.latest
    assert len(old["candidates"]) == lookup["count"]
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
    other.focus_in()
    wait(lambda: watch.latest["context"] > old["context"] and not watch.latest["seed"], "context transition")
    assert not action(old, "K0"), "old candidate reached a different input context"
    other_commits = []
    other.connect("commit-text", lambda _, text: other_commits.append(text.get_text()))
    for language, seed in [("zh-Hans", "nihao"), ("ja", "nihongo")]:
        assert json.loads(command("L" + language))["ok"]
        type_seed(other, seed)
        wait(lambda: watch.latest["seed"] == seed and watch.latest["language"] == language, "multilingual sync")
        expected = watch.latest["candidates"][0]["text"]
        assert action(watch.latest, "K0")
        wait(lambda: other_commits and other_commits[-1] == expected and not watch.latest["seed"], "multilingual commit")
    assert json.loads(command("Len"))["ok"]
    type_seed(other, "hel")
    # A deterministic local model fixture tests asynchronous publication, not model quality.
    settings = json.loads(command("S"))["settings"]
    settings.update(llm_enabled=True, llm_endpoint=f"http://127.0.0.1:{model.server_port}/v1/chat/completions")
    Path(os.environ["SUZAKU_IME_CONFIG"]).write_text(json.dumps(settings))
    assert json.loads(command("R"))["ok"]
    type_seed(other, "hel")
    wait(lambda: any(" · AI" in c["label"] for c in watch.latest["candidates"]), "asynchronous AI candidates were not pushed")
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
    print("PASS: push, late subscriber, exact candidates, selection, replacement, commit, stale revisions, context switches, English/Chinese/Japanese, asynchronous AI, private/password, Escape/focus-out and host restart")
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
