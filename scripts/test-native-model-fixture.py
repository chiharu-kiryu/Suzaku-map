#!/usr/bin/env python3
"""Exercise the native HTTP reply fixture without starting a model, bus or host."""
import ast
import io
import json
from pathlib import Path
import socket
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call


class NativeModelFixtureTests(unittest.TestCase):
    def setUp(self):
        source = Path(__file__).with_name("test-native-sync.py")
        fixture = next(node for node in ast.parse(source.read_text()).body
                       if isinstance(node, ast.ClassDef) and node.name == "ModelFixture")
        # Load only the handler definition. Importing the native suite would
        # require GI, start an HTTP server and later create an isolated IBus host.
        self.clock = SimpleNamespace(monotonic=Mock(return_value=100.0), sleep=Mock())
        self.select = Mock(return_value=([], [], []))
        self.requests, self.gates = [], {}
        namespace = dict(BaseHTTPRequestHandler=object, json=json, socket=socket,
                         time=self.clock, select=SimpleNamespace(select=self.select),
                         model_requests=self.requests, model_reply_gates=self.gates,
                         preference_sentence_batches={})
        exec(compile(ast.Module(body=[fixture], type_ignores=[]), str(source), "exec"), namespace)
        self.handler_type = namespace["ModelFixture"]

    def handler(self, model="synthetic-native-model", committed_context=None):
        payload = {"language": "en", "raw_composition": "hel"}
        if committed_context is not None:
            payload["committed_context"] = committed_context
        request = {"model": model, "messages": [{"content": "synthetic instruction"},
                   {"content": json.dumps(payload)}]}
        body = json.dumps(request).encode()
        handler = self.handler_type()
        handler.headers = {"Content-Length": str(len(body))}
        handler.rfile, handler.wfile = io.BytesIO(body), io.BytesIO()
        handler.connection = Mock()
        handler.send_response, handler.send_header, handler.end_headers = Mock(), Mock(), Mock()
        return handler, request

    def gate(self, model="synthetic-native-model", released=True):
        gate = {key: Mock() for key in ["received", "release", "finished"]}
        gate["release"].wait.return_value = released
        self.gates[model] = gate
        return gate

    def assert_success(self, handler, request):
        self.assertEqual(self.requests, [request])
        handler.send_response.assert_called_once_with(200)
        handler.end_headers.assert_called_once_with()
        body = handler.wfile.getvalue()
        handler.send_header.assert_has_calls([
            call("Content-Type", "application/json"), call("Content-Length", str(len(body)))])
        content = json.loads(json.loads(body)["choices"][0]["message"]["content"])
        self.assertEqual(content["candidates"], [
            {"text": text, "kind": kind} for text, kind in [
                ("helium", "word"), ("helmet", "word"), ("helix", "word"),
                ("hello world", "sentence"), ("hello everyone", "sentence"),
                ("hello internationalization compatibility verification works", "sentence")]])

    def test_ungated_reply_keeps_async_eighty_millisecond_delay(self):
        handler, request = self.handler()
        handler.do_POST()
        self.clock.sleep.assert_called_once_with(0.08)
        self.assert_success(handler, request)

    def test_ready_gate_has_no_second_artificial_delay(self):
        gate = self.gate()
        handler, request = self.handler()
        handler.do_POST()
        gate["received"].set.assert_called_once_with()
        gate["release"].wait.assert_called_once_with(timeout=0.01)
        gate["finished"].set.assert_called_once_with()
        self.select.assert_not_called()
        self.clock.sleep.assert_not_called()
        self.assertNotIn("cancelled_at", gate)
        self.assert_success(handler, request)

    def test_pending_gate_still_waits_for_explicit_release(self):
        gate = self.gate(released=False)
        gate["release"].wait.side_effect = [False, False, True]
        handler, request = self.handler()
        handler.do_POST()
        self.assertEqual(gate["release"].wait.call_args_list, [call(timeout=0.01)] * 3)
        self.assertEqual(self.select.call_args_list,
                         [call([handler.connection], [], [], 0)] * 2)
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()
        self.assert_success(handler, request)

    def test_exact_context_can_be_inspected_before_releasing_reply(self):
        name = "synthetic-exact-context"
        context = ("prefix" + "界😀" * 78 + " café  tail")[-160:]
        gate = self.gate(name, released=False)
        handler, request = self.handler(name, committed_context=context)

        def inspect_pending_payload(*, timeout):
            self.assertEqual(timeout, 0.01)
            gate["received"].set.assert_called_once_with()
            self.assertEqual(self.requests, [request])
            payload = json.loads(self.requests[0]["messages"][1]["content"])
            self.assertEqual(payload, {"language": "en", "raw_composition": "hel",
                                       "committed_context": context})
            gate["finished"].set.assert_not_called()
            handler.send_response.assert_not_called()
            self.assertEqual(handler.wfile.getvalue(), b"")
            self.assertNotIn("cancelled_at", gate)
            # Release only after inspecting the captured request, as N32 does.
            gate["release"].set()
            return True

        gate["release"].wait.side_effect = inspect_pending_payload
        handler.do_POST()
        gate["release"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()
        self.assert_success(handler, request)

    def test_closed_pending_client_still_records_cancellation_without_reply(self):
        gate = self.gate(released=False)
        handler, request = self.handler()
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b""
        self.clock.monotonic.side_effect = [100.0, 100.01, 100.02]
        handler.do_POST()
        self.assertEqual(self.requests, [request])
        self.assertEqual(gate["cancelled_at"], 100.02)
        handler.connection.recv.assert_called_once_with(1, socket.MSG_PEEK)
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        handler.send_response.assert_not_called()
        self.assertEqual(handler.wfile.getvalue(), b"")
        self.clock.sleep.assert_not_called()

    def test_unreleased_gate_retains_its_five_second_deadline(self):
        gate = self.gate(released=False)
        handler, _ = self.handler()
        self.clock.monotonic.side_effect = [100.0, 105.0]
        with self.assertRaisesRegex(AssertionError, "synthetic model reply gate timed out"):
            handler.do_POST()
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_not_called()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_unexpected_second_request_remains_an_assertion_failure(self):
        gate = self.gate(released=False)
        handler, _ = self.handler()
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b"x"
        with self.assertRaisesRegex(AssertionError, "unexpected second fixture request"):
            handler.do_POST()
        self.assertNotIn("cancelled_at", gate)
        gate["finished"].set.assert_not_called()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_fallback_error_keeps_empty_503_and_finished_acknowledgement(self):
        name = "synthetic-fallback-en-http-503"
        gate = self.gate(name)
        handler, request = self.handler(name)
        handler.do_POST()
        self.assertEqual(self.requests, [request])
        handler.send_response.assert_called_once_with(503)
        handler.send_header.assert_called_once_with("Content-Length", "0")
        handler.end_headers.assert_called_once_with()
        self.assertEqual(handler.wfile.getvalue(), b"")
        gate["received"].set.assert_called_once_with()
        gate["finished"].set.assert_called_once_with()
        self.clock.sleep.assert_not_called()

    def test_fallback_deadline_client_closure_does_not_send_503(self):
        name = "synthetic-fallback-en-timeout"
        gate = self.gate(name, released=False)
        handler, _ = self.handler(name)
        self.select.return_value = ([handler.connection], [], [])
        handler.connection.recv.return_value = b""
        handler.do_POST()
        self.assertIn("cancelled_at", gate)
        gate["finished"].set.assert_called_once_with()
        handler.send_response.assert_not_called()
        self.clock.sleep.assert_not_called()

    def test_peer_close_during_success_or_error_response_still_finishes_gate(self):
        for name in ["synthetic-native-model", "synthetic-fallback-en-http-503"]:
            for error in [BrokenPipeError, ConnectionResetError]:
                with self.subTest(model=name, error=error):
                    gate = self.gate(name)
                    handler, _ = self.handler(name)
                    handler.send_response.side_effect = error
                    handler.do_POST()
                    gate["received"].set.assert_called_once_with()
                    gate["finished"].set.assert_called_once_with()
                    self.assertEqual(handler.wfile.getvalue(), b"")
        self.clock.sleep.assert_not_called()


if __name__ == "__main__":
    unittest.main()
