#!/usr/bin/env python3
"""Test the diagnostic shim with a fixed stub; never connect to any IBus bus."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


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


if __name__ == "__main__":
    unittest.main()
