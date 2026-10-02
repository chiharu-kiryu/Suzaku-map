#!/usr/bin/env python3
"""Test native QA isolation and the diagnostic shim; never connect to an IBus bus."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
from fixtures.compose_fixture import require_private_compose


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


if __name__ == "__main__":
    unittest.main()
