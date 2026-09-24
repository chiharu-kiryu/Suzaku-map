#!/usr/bin/env python3
"""Opt-in synthetic Chrome input with a real loopback Ollama, never cloud/profile data.

The default checks input safety and responsiveness, not successful generation.
SUZAKU_MODEL_QA_REQUIRE_CANDIDATE=1 adds a strict real model-candidate adoption check
with a 5000 ms private QA budget. Neither mode proves general semantic quality.
"""
import importlib.util
import http.client
import json
import os
from pathlib import Path
import select
import socket
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("suzaku_cross_qa", Path(__file__).with_name("test-linux-cross-apps.py"))
cross = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cross)
qa, IBus = cross.qa, cross.IBus
assert os.environ["SUZAKU_APP_QA_SUITE"] == "model"
assert os.environ.get("SUZAKU_MODEL_LOCAL_QA") == "1"
model = os.environ["SUZAKU_MODEL_QA_MODEL"]
assert model and model != "auto"


class OllamaRelay:
    """Observe actual generation dispatch, not merely a pending debounce timer.

    Forward unmodified metadata/generation to the fixed loopback service. Never
    synthesize a model response, follow redirects, or leave an upstream socket
    open when the owned host cancels/times out and closes its downstream connection.
    Socket closure is observable; whether a server stops computing is not asserted.
    """
    def __init__(self):
        self.requests = []
        self.dispatches = []
        self.cancelled = 0
        relay = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def forward(self):
                upstream = http.client.HTTPConnection("127.0.0.1", 11434, timeout=3)
                record = None
                cancelled = False
                try:
                    self.connection.settimeout(2)
                    body = None
                    seed = None
                    if self.command == "POST" and self.path == "/api/chat":
                        length = int(self.headers.get("Content-Length", "0"))
                        assert 0 < length <= 131072
                        body = self.rfile.read(length)
                        payload = json.loads(body)
                        assert payload["model"] == model and payload["stream"] is False
                        seed = json.loads(payload["messages"][1]["content"])["raw_composition"]
                    elif self.command != "GET" or self.path != "/api/tags":
                        self.send_error(404)
                        return
                    upstream.request(self.command, self.path, body,
                                     {"Content-Type": "application/json", "Connection": "close"})
                    if seed is not None:
                        record = dict(seed=seed, started_at=time.monotonic(), cancelled_at=None, completed_at=None)
                        relay.dispatches.append(record)
                        relay.requests.append(seed)
                    deadline = time.monotonic() + 8
                    while time.monotonic() < deadline:
                        readable, _, _ = select.select([upstream.sock, self.connection], [], [], 0.05)
                        if self.connection in readable:
                            assert not self.connection.recv(1, socket.MSG_PEEK), "unexpected second QA request"
                            if seed is not None:
                                cancelled = True
                            return  # finally closes this request's upstream socket too.
                        if upstream.sock in readable:
                            response = upstream.getresponse()
                            data = response.read(131073)
                            assert len(data) <= 131072
                            self.send_response(response.status)
                            self.send_header("Content-Type", "application/json")
                            self.send_header("Content-Length", str(len(data)))
                            self.end_headers()
                            self.wfile.write(data)
                            if record is not None:
                                record["completed_at"] = time.monotonic()
                            return
                    self.send_error(504)
                except (OSError, ValueError, AssertionError, KeyError, http.client.HTTPException):
                    try:
                        self.send_error(502)
                    except OSError:
                        pass
                finally:
                    upstream.close()
                    if cancelled and record is not None:
                        record["cancelled_at"] = time.monotonic()
                        relay.cancelled += 1

            do_GET = forward
            do_POST = forward

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.endpoint = f"http://127.0.0.1:{self.server.server_port}/api/chat"
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)


def check(bus, x, bridge, commits, settings, relay):
    qa.wait(lambda: bridge.state and x.window("Suzaku Browser QA"), "live-model owned browser ready", timeout=20)
    window = x.window("Suzaku Browser QA")
    x.focus(window)
    assert bus.set_global_engine("dev.suzaku.linux.ime")

    def focus(field):
        bridge.focus(field)
        qa.settle_input(bus, x, window)

    def status():
        return json.loads(qa.command("S"))["prediction"]

    def pending(seed, request_start):
        qa.wait(lambda: seed in relay.requests[request_start:] and qa.seed_is(seed) and status() == "Pending",
                "actual real-model generation dispatched before input boundary")
        return next(record for record in relay.dispatches[request_start:] if record["seed"] == seed)

    def cancellation(record, boundary_at):
        # A completed request is not evidence of in-flight cancellation. Require
        # both an early edit and actual socket closure well before the 1200 ms
        # host deadline; dispatch alone could include a nearly expired call.
        assert 0 <= boundary_at - record["started_at"] < 0.25, record
        qa.wait(lambda: record["cancelled_at"] is not None or record["completed_at"] is not None,
                "obsolete real request closes its upstream connection", timeout=0.7)
        assert record["cancelled_at"] is not None, ("request finished instead of being cancelled", record)
        elapsed = record["cancelled_at"] - boundary_at
        assert 0 <= elapsed < 0.5, ("old inference waited for its original deadline", elapsed)
        print("MODEL CANCEL:", record["seed"], round(elapsed * 1000, 1), "ms after boundary; upstream closed")

    def observe_stable(seconds, field, text):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            qa.pump()
            assert not qa.watch.latest["seed"] and not qa.watch.latest["candidates"]
            assert bridge.value(field) == text
            assert not bridge.state["composition"][field]
            time.sleep(0.02)

    def clear():
        focus("editor")
        assert qa.seed_is("")
        x.key(IBus.KEY_a, IBus.KEY_Control_L)
        x.key(IBus.KEY_BackSpace)
        bridge.expect("editor", "")

    focus("editor")
    assert json.loads(qa.command("P1"))["ok"]
    requested = len(relay.requests)
    x.type("hel")
    old_request = pending("hel", requested)
    changed = time.monotonic()
    latencies = []
    draft = "hel"
    for character in "lo world":
        started = time.monotonic()
        x.type(character)
        draft += character
        qa.wait(lambda: qa.seed_is(draft), "foreground draft continues during real inference", timeout=0.5)
        latencies.append(round((time.monotonic() - started) * 1000, 1))
    assert max(latencies) < 500, latencies
    cancellation(old_request, changed)
    bridge.expect("editor", "")
    qa.commit(x)
    bridge.expect("editor", "hello world")
    observe_stable(2.0, "editor", "hello world")
    cross.passed_case("real pending inference does not block typing or replay after Enter; per-key ms:", latencies)

    clear()
    requested = len(relay.requests)
    x.type("hel")
    qa.wait(lambda: "hel" in relay.requests[requested:] and qa.seed_is("hel") and status() in ("Ready", "Unavailable"),
            "real candidate request settles within bounded budget", timeout=5)
    outcome = status()
    print("MODEL OUTCOME:", outcome, "(Unavailable is fallback coverage, NOT model-candidate acceptance)")
    assert any(c["text"] == "hello" for c in qa.watch.latest["candidates"])
    if outcome == "Unavailable":
        assert all(c["source"] != "model" for c in qa.watch.latest["candidates"])
    qa.choose_number(x, "hello")
    x.key(IBus.KEY_BackSpace)
    qa.wait(lambda: qa.seed_is("hel"), "undo after real model outcome")
    qa.choose_number(x, "hello")
    qa.commit(x)
    bridge.expect("editor", "hello")
    cross.passed_case("bounded real request retains offline candidate adoption, undo and exact commit")

    clear()
    requested = len(relay.requests)
    x.type("please sen")
    old_request = pending("please sen", requested)
    before = len(commits.texts)
    changed = time.monotonic()
    x.key(IBus.KEY_Escape)
    cancellation(old_request, changed)
    qa.wait(lambda: qa.seed_is(""), "Escape while real model pending")
    bridge.expect("editor", "")
    observe_stable(2.0, "editor", "")
    assert len(commits.texts) == before
    cross.passed_case("Escape prevents late real inference from restoring or committing discarded input")

    requested = len(relay.requests)
    x.type("good m")
    old_request = pending("good m", requested)
    changed = time.monotonic()
    focus("password")
    cancellation(old_request, changed)
    qa.wait(lambda: not qa.watch.latest["seed"], "private focus cancels public draft")
    start = len(qa.watch.frames)
    requested = len(relay.requests)
    x.type("synthetic42")
    bridge.expect("password", "synthetic42")
    observe_stable(2.0, "password", "synthetic42")
    bridge.expect("editor", "")
    assert all(not f["seed"] and not f["candidates"] for f in qa.watch.frames[start:])
    assert len(commits.texts) == before
    assert len(relay.requests) == requested, "password input dispatched a model request"
    cross.passed_case("password creates neither model requests nor public snapshots while old real request settles")

    # A bound, non-listening owned socket refuses connections without stopping
    # the user's Ollama or racing another process for a freed ephemeral port.
    with socket.socket() as refused:
        refused.bind(("127.0.0.1", 0))
        failed_settings = dict(settings, llm_enabled=True,
            llm_endpoint=f"http://127.0.0.1:{refused.getsockname()[1]}/api/chat")
        (qa.root / "ime.json").write_text(json.dumps(failed_settings))
        assert json.loads(qa.command("R"))["ok"]
        focus("editor")
        x.type("hel")
        qa.wait(lambda: qa.seed_is("hel") and status() == "Unavailable", "owned refused endpoint fallback")
        qa.choose_number(x, "hello")
        qa.commit(x)
        bridge.expect("editor", "hello")
    cross.passed_case("refused owned endpoint leaves actual application input usable without stopping Ollama")

    (qa.root / "ime.json").write_text(json.dumps(dict(settings, llm_enabled=True)))
    assert json.loads(qa.command("R"))["ok"]
    clear()
    requested = len(relay.requests)
    x.type("public again")
    old_request = pending("public again", requested)
    changed = time.monotonic()
    assert json.loads(qa.command("P0"))["ok"]
    cancellation(old_request, changed)
    qa.commit(x)
    bridge.expect("editor", "public again")
    observe_stable(2.0, "editor", "public again")
    assert not commits.preedit_updates
    assert all("synthetic" not in seed for seed in relay.requests)
    cross.passed_case("restored real endpoint schedules new requests; disable/commit rejects late results")

    if os.environ.get("SUZAKU_MODEL_QA_REQUIRE_CANDIDATE") == "1":
        # Explicit positive acceptance, never pass on Unavailable/offline-only.
        # Only this isolated host gets the larger supported background budget.
        positive_settings = dict(settings, llm_enabled=True, llm_timeout_ms=5000)
        (qa.root / "ime.json").write_text(json.dumps(positive_settings))
        assert json.loads(qa.command("R"))["ok"]
        clear()
        requested = len(relay.requests)
        seed = "Could you sh"
        x.key(IBus.KEY_c, IBus.KEY_Shift_L)
        x.type(seed[1:])
        qa.wait(lambda: seed in relay.requests[requested:] and qa.seed_is(seed) and status() in ("Ready", "Unavailable"),
                "positive real-model request settles", timeout=7)
        assert status() == "Ready", "positive acceptance requires successful real generation"
        qa.wait(lambda: any(c["source"] == "model" for c in qa.watch.latest["candidates"][:6]),
                "real model candidate arrives on the independent snapshot channel", timeout=2)
        choices = [c for c in qa.watch.latest["candidates"][:6] if c["source"] == "model"]
        assert choices, "offline/corroborated candidates are not evidence of model-only adoption"
        adopted = choices[0]["text"]
        assert adopted.startswith(seed) and adopted != seed
        print("MODEL POSITIVE:", choices)
        before = len(commits.texts)
        qa.choose_number(x, adopted)
        bridge.expect("editor", "")
        x.key(IBus.KEY_BackSpace)
        qa.wait(lambda: qa.seed_is(seed), "undo real model candidate")
        # Re-adopt the actual model candidate, then continue while another model
        # request may be pending. Enter must commit exactly the visible draft.
        qa.wait(lambda: any(c["source"] == "model" for c in qa.watch.latest["candidates"][:6]),
                "model candidate available after undo", timeout=7)
        adopted = next(c["text"] for c in qa.watch.latest["candidates"][:6] if c["source"] == "model")
        assert adopted.startswith(seed) and adopted != seed
        qa.choose_number(x, adopted)
        x.type(" today")
        qa.wait(lambda: qa.seed_is(adopted + " today"), "continuation after real model adoption")
        assert len(commits.texts) == before
        qa.commit(x)
        bridge.expect("editor", adopted + " today")
        assert commits.texts[before:] == [adopted + " today"]
        assert json.loads(qa.command("P0"))["ok"]
        observe_stable(2.0, "editor", adopted + " today")
        cross.passed_case("real model-only candidate supports number adoption, undo, continuation and exact single commit")
    print("MODEL REQUESTS:", len(relay.requests), "actual dispatches; cancelled upstream connections:", relay.cancelled)


def main():
    bridge = None
    commits = None
    relay = None
    try:
        relay = OllamaRelay()
        settings = dict(language="en", llm_enabled=False, llm_scope="local", llm_protocol="ollama",
            llm_model=model, llm_endpoint=relay.endpoint, llm_timeout_ms=1200,
            llm_cloud_consent=False, llm_api_key_env=None)
        (qa.root / "ime.json").write_text(json.dumps(settings))
        bus, x = qa.start()
        bridge = cross.Bridge()
        commits = cross.EngineCommits(bus)
        cross.start_app("browser", bridge, 1)
        check(bus, x, bridge, commits, settings, relay)
        print(f"RESULT: {cross.passed} real-model workflows passed; strict adoption required="
              f"{os.environ.get('SUZAKU_MODEL_QA_REQUIRE_CANDIDATE') == '1'}; semantic quality is separate")
    except Exception:
        print("Live-model application state:", None if bridge is None else bridge.state)
        qa.diagnose()
        raise
    finally:
        if commits is not None:
            commits.close()
        qa.close()
        if bridge is not None:
            bridge.close()
        if relay is not None:
            relay.close()


if __name__ == "__main__":
    main()
