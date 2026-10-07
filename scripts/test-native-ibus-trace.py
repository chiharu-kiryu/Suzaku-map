#!/usr/bin/env python3
"""Test native QA isolation and the diagnostic shim; never connect to an IBus bus."""
import ast
from contextlib import redirect_stderr, redirect_stdout
import functools
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, call, patch

sys.dont_write_bytecode = True
from fixtures.compose_fixture import require_private_compose
from fixtures.native_probe_cleanup import (
    PendingCallbacks, preserve_primary_failure, report_native_diagnostic,
)


class NativeTraceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-sync-qa.", dir="/tmp")
        self.addCleanup(self.temporary.cleanup)
        self.runtime = Path(self.temporary.name)
        self.trace = self.runtime / "activation-ibus.log"
        self.stub = self.runtime / "stub"
        self.stub.write_text("#!/bin/sh\nprintf 'xkb:us::eng\\n'\nprintf 'synthetic diagnostic\\n' >&2\nexit 7\n")
        self.stub.chmod(0o700)
        self.shim = self.runtime / "trace-ibus"
        shutil.copyfile(Path(__file__).parent / "fixtures/trace-native-ibus.sh", self.shim)
        self.shim.chmod(0o700)
        self.environment = dict(os.environ, SUZAKU_NATIVE_SYNC_QA="1",
                                XDG_RUNTIME_DIR=str(self.runtime),
                                IBUS_ADDRESS=f"unix:path={self.runtime}/ibus.sock",
                                SUZAKU_NATIVE_REAL_IBUS=str(self.stub))
        for name in ["DISPLAY", "WAYLAND_DISPLAY"]:
            self.environment.pop(name, None)

    def run_shim(self, *arguments, **overrides):
        return subprocess.run(["bash", str(self.shim), *arguments],
                              env=dict(self.environment, **overrides),
                              capture_output=True, text=True, timeout=2)

    def test_preserves_failed_exit_and_stdout_and_captures_stderr(self):
        result = self.run_shim("engine", "xkb:us::eng")
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, "xkb:us::eng\n")
        self.assertEqual(result.stderr, "")
        self.assertEqual(self.trace.read_text(),
                         "ibus engine xkb:us::eng\nsynthetic diagnostic\nexit=7\n")

    def test_success_does_not_become_a_failure(self):
        result = self.run_shim("engine", SUZAKU_NATIVE_REAL_IBUS="/usr/bin/true")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertEqual(self.trace.read_text(), "ibus engine\nexit=0\n")

    def test_invalid_scope_is_rejected_before_trace_or_command(self):
        for overrides in [
            {"SUZAKU_NATIVE_SYNC_QA": ""}, {"DISPLAY": ":1"}, {"WAYLAND_DISPLAY": "wayland-0"},
            {"IBUS_ADDRESS": "unix:path=/not-the-private-bus"},
            {"XDG_RUNTIME_DIR": str(self.runtime / "nested")},
            {"SUZAKU_NATIVE_REAL_IBUS": "relative-command"},
            {"SUZAKU_NATIVE_REAL_IBUS": "/does-not-exist"},
        ]:
            with self.subTest(overrides=overrides):
                result = self.run_shim("engine", **overrides)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
                self.assertFalse(self.trace.exists())

    def test_symlink_to_itself_is_rejected(self):
        alias = self.runtime / "alias"
        alias.symlink_to(self.shim.resolve())
        result = self.run_shim("engine", SUZAKU_NATIVE_REAL_IBUS=str(alias))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.trace.exists())

    def test_other_commands_are_rejected(self):
        for arguments in [[], ["restart"], ["engine", "xkb:us::eng", "extra"]]:
            with self.subTest(arguments=arguments):
                result = self.run_shim(*arguments)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
                self.assertFalse(self.trace.exists())


class ComposeIsolationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-compose-qa.", dir="/tmp")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source.XCompose"
        self.source.write_text("# synthetic source fixture\n")
        self.private = self.root / "compose.XCompose"
        self.environment = {"XCOMPOSEFILE": str(self.private)}

    def test_private_copy_can_be_migrated_without_changing_source(self):
        shutil.copyfile(self.source, self.private)
        path = require_private_compose(self.root, self.environment)
        path.write_text('# simulated IBus migration\ninclude "%L"\n')
        self.assertEqual(self.source.read_text(), "# synthetic source fixture\n")

    def test_missing_and_shared_source_paths_are_rejected(self):
        for environment in [{}, self.environment, {"XCOMPOSEFILE": str(self.source)}]:
            with self.subTest(environment=environment), self.assertRaises(ValueError):
                require_private_compose(self.root, environment)

    def test_symlink_and_hardlink_copies_are_rejected(self):
        self.private.symlink_to(self.source)
        with self.assertRaises(ValueError):
            require_private_compose(self.root, self.environment)
        self.private.unlink()
        os.link(self.source, self.private)
        with self.assertRaises(ValueError):
            require_private_compose(self.root, self.environment)

    def test_symlinked_and_non_tmp_roots_are_rejected(self):
        shutil.copyfile(self.source, self.private)
        alias = self.root.with_name(self.root.name + ".link")
        alias.symlink_to(self.root, target_is_directory=True)
        self.addCleanup(alias.unlink)
        nested = self.root / "nested"
        nested.mkdir()
        shutil.copyfile(self.source, nested / "compose.XCompose")
        for root in [alias, nested]:
            with self.subTest(root=root), self.assertRaises(ValueError):
                require_private_compose(root, {"XCOMPOSEFILE": str(root / "compose.XCompose")})


class FakeGLib:
    SOURCE_REMOVE = False

    def __init__(self):
        self.sources = {}
        self.removed = []
        self.next_id = 0

    def timeout_add(self, delay_ms, callback):
        self.next_id += 1
        self.sources[self.next_id] = (delay_ms, callback)
        return self.next_id

    def source_remove(self, source_id):
        self.removed.append(source_id)
        del self.sources[source_id]

    def dispatch(self, source_id):
        _, callback = self.sources.pop(source_id)
        try:
            result = callback()
        except Exception:
            # GI prints callback exceptions rather than raising from iteration().
            # The test must not rely on ordinary Python propagation here.
            return self.SOURCE_REMOVE
        if result is not self.SOURCE_REMOVE:
            raise AssertionError("one-shot callback requested another dispatch")
        return result


class NativeProbeCleanupTests(unittest.TestCase):
    def setUp(self):
        self.glib = FakeGLib()
        self.pending = PendingCallbacks(self.glib)

    def test_completed_callback_is_retired_and_cannot_request_a_repeat(self):
        calls = []
        source_id = self.pending.schedule(90, lambda: calls.append("ran") or True)
        self.assertEqual(self.glib.sources[source_id][0], 90)
        self.assertIs(self.glib.dispatch(source_id), self.glib.SOURCE_REMOVE)
        self.assertEqual(calls, ["ran"])
        self.pending.cancel_all()
        self.pending.raise_if_failed()
        self.assertEqual(self.glib.sources, {})
        self.assertEqual(self.glib.removed, [])

    def test_cancel_all_removes_only_pending_callbacks_and_is_idempotent(self):
        calls = []
        completed = self.pending.schedule(90, lambda: calls.append("completed"))
        self.glib.dispatch(completed)
        remaining = {self.pending.schedule(90, lambda: calls.append("unexpected"))
                     for _ in range(2)}
        self.pending.cancel_all()
        self.pending.cancel_all()
        self.pending.raise_if_failed()
        self.assertEqual(set(self.glib.removed), remaining)
        self.assertEqual(len(self.glib.removed), 2)
        self.assertEqual(self.glib.sources, {})
        self.assertEqual(calls, ["completed"])

    def test_failing_callback_is_retired_and_explicit_check_propagates_its_error(self):
        failure = RuntimeError("delayed engine switch failed")

        def fail():
            raise failure

        source_id = self.pending.schedule(90, fail)
        self.assertIs(self.glib.dispatch(source_id), self.glib.SOURCE_REMOVE)
        with self.assertRaises(RuntimeError) as raised:
            self.pending.raise_if_failed()
        self.assertIs(raised.exception, failure)
        self.pending.cancel_all()
        self.assertEqual(self.glib.sources, {})
        self.assertEqual(self.glib.removed, [])

    def test_later_callback_failures_do_not_replace_first_or_grow_notes_unboundedly(self):
        failures = [RuntimeError(f"switch failure {i}") for i in range(20)]
        for failure in failures:
            def fail(error=failure):
                raise error

            source_id = self.pending.schedule(90, fail)
            self.assertIs(self.glib.dispatch(source_id), self.glib.SOURCE_REMOVE)
        self.pending.cancel_all()
        with self.assertRaises(RuntimeError) as raised:
            self.pending.raise_if_failed()
        self.assertIs(raised.exception, failures[0])
        self.assertEqual(failures[0].__notes__, [
            "Additional delayed callback failure: RuntimeError: switch failure 1",
            "Additional delayed callback failure: RuntimeError: switch failure 2",
            "Additional delayed callback failure: RuntimeError: switch failure 3",
            "Further delayed callback failures omitted.",
        ])
        self.assertEqual(self.glib.sources, {})
        self.assertEqual(self.glib.removed, [])

    def test_cleanup_error_is_not_allowed_to_replace_primary_failure(self):
        failure = AssertionError("original engine mismatch")
        with self.assertRaises(AssertionError) as raised:
            try:
                raise failure
            finally:
                with preserve_primary_failure("release private panel"):
                    raise RuntimeError("connection closed")
        self.assertIs(raised.exception, failure)
        self.assertEqual(failure.__notes__,
                         ["release private panel: RuntimeError: connection closed"])

    def test_cleanup_error_without_primary_failure_propagates(self):
        failure = RuntimeError("connection closed")
        with self.assertRaises(RuntimeError) as raised:
            try:
                pass
            finally:
                with preserve_primary_failure("release private panel"):
                    raise failure
        self.assertIs(raised.exception, failure)

    def test_rechecking_primary_callback_error_does_not_add_a_self_note(self):
        failure = RuntimeError("delayed engine switch failed")
        with self.assertRaises(RuntimeError) as raised:
            try:
                raise failure
            finally:
                with preserve_primary_failure("check callback errors"):
                    raise failure
        self.assertIs(raised.exception, failure)
        self.assertFalse(getattr(failure, "__notes__", []))

    def test_successful_system_exit_does_not_hide_cleanup_failure(self):
        failure = RuntimeError("connection closed")
        with self.assertRaises(RuntimeError) as raised:
            try:
                raise SystemExit(0)
            finally:
                with preserve_primary_failure("release private panel"):
                    raise failure
        self.assertIs(raised.exception, failure)

    def test_successful_cleanup_preserves_normal_and_exceptional_control_flow(self):
        with preserve_primary_failure("release private panel"):
            pass
        for failure in [AssertionError("original engine mismatch"), SystemExit(0)]:
            with self.subTest(failure=type(failure).__name__):
                with self.assertRaises(type(failure)) as raised:
                    try:
                        raise failure
                    finally:
                        with preserve_primary_failure("release private panel"):
                            pass
                self.assertIs(raised.exception, failure)
                self.assertFalse(hasattr(failure, "__notes__"))


class NativeDiagnosticTests(unittest.TestCase):
    def setUp(self):
        self.connection = Mock(spec=["is_closed"])
        self.connection.is_closed.return_value = True
        self.bus = Mock(spec=["get_connection", "is_connected"])
        self.bus.get_connection.return_value = self.connection
        self.bus.is_connected.return_value = False
        self.daemon = self.process(501, "ibus-daemon", -11)

    @staticmethod
    def process(pid, name, returncode=None):
        process = Mock(spec=["args", "pid", "poll"])
        process.args = [f"/private/fixture/{name}", "--synthetic-input", "do not print this text"]
        process.pid = pid
        process.poll.return_value = returncode
        return process

    def report(self, processes=(), **details):
        report_native_diagnostic("query-failed", phase="N45", started=100,
                                 bus=self.bus, daemon=self.daemon,
                                 processes=processes, **details)

    def test_closed_connection_and_signaled_daemon_use_only_local_observation(self):
        stderr = io.StringIO()
        with redirect_stderr(stderr), patch("fixtures.native_probe_cleanup.time.monotonic", return_value=103.125):
            self.report([self.process(502, "linux_ime_host")], purpose=8)
        line = stderr.getvalue()
        self.assertTrue(line.startswith("NATIVE DIAGNOSTIC: "))
        state = json.loads(line.removeprefix("NATIVE DIAGNOSTIC: "))
        self.assertEqual(state, {
            "event": "query-failed", "phase": "N45", "elapsed_s": 3.125,
            "bus_connected": False, "connection_closed": True,
            "daemon_pid": 501, "daemon_returncode": -11,
            "recent_children": [{"name": "linux_ime_host", "pid": 502, "returncode": None}],
            "purpose": 8,
        })
        self.assertEqual(self.bus.method_calls, [call.get_connection(), call.is_connected()])
        self.assertEqual(self.connection.method_calls, [call.is_closed()])

    def test_reports_only_executable_basename_not_path_or_remaining_arguments(self):
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.report([self.process(502, "linux_ime_probe", 1)])
        self.assertIn('"name": "linux_ime_probe"', stderr.getvalue())
        for private in ["/private/fixture", "--synthetic-input", "do not print this text"]:
            self.assertNotIn(private, stderr.getvalue())

    def test_observes_only_the_last_eight_children(self):
        children = [self.process(600 + i, f"child-{i}") for i in range(12)]
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.report(children)
        state = json.loads(stderr.getvalue().removeprefix("NATIVE DIAGNOSTIC: "))
        self.assertEqual([child["pid"] for child in state["recent_children"]], list(range(604, 612)))
        for child in children[:4]:
            child.poll.assert_not_called()
        for child in children[4:]:
            child.poll.assert_called_once_with()

    def test_broken_bus_accessor_does_not_replace_original_failure(self):
        self.bus.get_connection.side_effect = RuntimeError("accessor failed")
        failure = AssertionError("original native assertion")
        stderr = io.StringIO()
        with redirect_stderr(stderr), self.assertRaises(AssertionError) as raised:
            try:
                raise failure
            finally:
                self.report()
        self.assertIs(raised.exception, failure)
        self.assertEqual(stderr.getvalue(),
                         "NATIVE DIAGNOSTIC unavailable: RuntimeError: accessor failed\n")

    def test_unwritable_stderr_does_not_replace_original_failure(self):
        stderr = Mock(spec=["write", "flush"])
        stderr.write.side_effect = OSError("diagnostic destination closed")
        failure = AssertionError("original native assertion")
        with redirect_stderr(stderr), self.assertRaises(AssertionError) as raised:
            try:
                raise failure
            finally:
                self.report()
        self.assertIs(raised.exception, failure)


class NativeWorkflowMetricsTests(unittest.TestCase):
    def setUp(self):
        source = Path(__file__).with_name("test-native-sync.py")
        phase = next(node for node in ast.parse(source.read_text()).body
                     if isinstance(node, ast.FunctionDef) and node.name == "native_check_phase")
        self.environment = {}
        self.clock = Mock(side_effect=[10.0, 10.25])
        self.namespace = dict(functools=functools, sys=sys,
                              preserve_primary_failure=preserve_primary_failure,
                              os=SimpleNamespace(environ=self.environment),
                              time=SimpleNamespace(monotonic=self.clock),
                              diagnostic_phase="native-workflows")
        exec(compile(ast.Module(body=[phase], type_ignores=[]), str(source), "exec"), self.namespace)
        self.wrap = self.namespace["native_check_phase"]

    def test_optional_timing_preserves_arguments_result_and_phase(self):
        self.environment["SUZAKU_NATIVE_TIMING"] = "1"
        def check_owned(value, *, suffix):
            self.assertEqual(self.namespace["diagnostic_phase"], "check_owned")
            return value + suffix
        wrapped = self.wrap(check_owned)
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            self.assertEqual(wrapped("a", suffix="b"), "ab")
        self.assertEqual(stdout.getvalue(), "TIMING: check_owned: 0.250s\n")
        self.assertEqual(wrapped.__name__, "check_owned")
        self.assertEqual(self.namespace["diagnostic_phase"], "native-workflows")

    def test_disabled_metrics_are_quiet(self):
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            self.assertEqual(self.wrap(lambda: 7)(), 7)
        self.assertEqual(stdout.getvalue(), "")
        self.clock.assert_called_once_with()

    def test_nested_checks_use_the_outer_phase_and_one_timer(self):
        self.environment["SUZAKU_NATIVE_TIMING"] = "1"
        def check_inner():
            self.assertEqual(self.namespace["diagnostic_phase"], "check_outer")
            return 7
        inner = self.wrap(check_inner)
        def check_outer():
            return inner()
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            self.assertEqual(self.wrap(check_outer)(), 7)
        self.assertEqual(stdout.getvalue(), "TIMING: check_outer: 0.250s\n")
        self.assertEqual(self.clock.call_count, 2)

    def test_failure_keeps_its_phase_and_original_exception(self):
        failure = AssertionError("owned workflow failed")
        def check_failure():
            raise failure
        with self.assertRaises(AssertionError) as raised:
            self.wrap(check_failure)()
        self.assertIs(raised.exception, failure)
        self.assertEqual(self.namespace["diagnostic_phase"], "check_failure")

    def test_failed_timing_output_does_not_replace_workflow_failure(self):
        self.environment["SUZAKU_NATIVE_TIMING"] = "1"
        failure = AssertionError("owned workflow failed")
        def check_failure():
            raise failure
        with patch("builtins.print", side_effect=OSError("closed output")), \
                self.assertRaises(AssertionError) as raised:
            self.wrap(check_failure)()
        self.assertIs(raised.exception, failure)
        self.assertEqual(failure.__notes__, ["native workflow timing: OSError: closed output"])
        self.assertEqual(self.namespace["diagnostic_phase"], "check_failure")


if __name__ == "__main__":
    unittest.main()
