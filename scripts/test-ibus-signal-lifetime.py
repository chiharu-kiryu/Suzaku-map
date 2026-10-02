#!/usr/bin/env python3
"""Real floating C signals and Python lifetimes; no display, bus or IME needed."""
import ctypes
import gc
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock

sys.dont_write_bytecode = True
from fixtures.ibus_signal_lifetime import IBusSignalObjects
from gi.repository import GLib, GObject


class Emitter(GObject.GObject):
    __gsignals__ = {"borrowed-object": (GObject.SignalFlags.RUN_LAST, None, (GObject.Object,))}


class SignalLifetimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="suzaku-signal-qa.", dir="/tmp")
        cls.addClassCleanup(cls.temporary.cleanup)
        library = Path(cls.temporary.name) / "borrowed-signal.so"
        flags = subprocess.run(["pkg-config", "--cflags", "--libs", "gobject-2.0"],
                               check=True, capture_output=True, text=True, timeout=5)
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                        str(Path(__file__).parent / "fixtures/borrowed-signal.c"),
                        "-o", str(library), *shlex.split(flags.stdout)], check=True, timeout=15)
        cls.library = ctypes.CDLL(str(library))
        cls.library.borrowed_signal_emit.argtypes = [ctypes.c_void_p]
        cls.library.borrowed_signal_emit.restype = ctypes.c_int
        cls.library.borrowed_signal_live.argtypes = []
        cls.library.borrowed_signal_live.restype = ctypes.c_uint

    def setUp(self):
        self.guard = IBusSignalObjects()
        self.emitter = Emitter()
        self.observed = []
        self.emitter.connect("borrowed-object", self.guard.hold)
        # Read but do not retain the Python wrapper, like text-only observers.
        self.emitter.connect("borrowed-object", lambda _, payload: self.observed.append(type(payload)))
        self.loop = GLib.MainContext.default()
        self.guard.drain(self.loop)
        self.assertEqual(self.library.borrowed_signal_live(), 0)
        self.addCleanup(self.guard.drain, self.loop)

    def emit(self):
        capsule = ctypes.pythonapi.PyCapsule_GetPointer
        capsule.argtypes = [ctypes.py_object, ctypes.c_char_p]
        capsule.restype = ctypes.c_void_p
        pointer = capsule(self.emitter.__gpointer__, None)
        self.assertEqual(self.library.borrowed_signal_emit(pointer), 1,
                         "C emitter observed a prematurely destroyed signal payload")

    def test_synchronous_signals_stay_alive_until_c_returns_then_are_released(self):
        for _ in range(32):
            self.emit()
        gc.collect()
        self.assertEqual(self.library.borrowed_signal_live(), 32)
        self.guard.drain(self.loop)
        gc.collect()
        self.assertEqual(self.library.borrowed_signal_live(), 0)
        self.assertEqual(len(self.observed), 32)

    def test_main_loop_releases_each_dispatch_without_accumulating_a_history(self):
        counts = []

        def dispatch():
            counts.append(self.library.borrowed_signal_live())
            self.emit()
            self.assertEqual(self.library.borrowed_signal_live(), 1)
            return len(counts) < 128

        source = GLib.idle_add(dispatch)
        self.guard.drain(self.loop)
        # No duplicate per-signal idles, and no old payload survives a dispatch.
        self.assertEqual(counts, [0] * 128)
        self.assertEqual(self.library.borrowed_signal_live(), 0)
        self.assertIsNone(self.loop.find_source_by_id(source))

    def test_observer_retention_is_not_revoked_by_guard_cleanup(self):
        saved = []
        self.emitter.connect("borrowed-object", lambda _, payload: saved.append(payload))
        self.emit()
        self.guard.drain(self.loop)
        self.assertEqual(self.library.borrowed_signal_live(), 1)
        saved.clear()
        gc.collect()
        self.assertEqual(self.library.borrowed_signal_live(), 0)

    def test_all_object_bearing_ibus_signals_are_guarded(self):
        context = Mock()
        self.guard.watch(context)
        self.assertEqual([call.args[0] for call in context.connect.call_args_list], [
            "commit-text", "update-preedit-text", "update-preedit-text-with-mode",
            "update-auxiliary-text", "update-lookup-table", "register-properties", "update-property"])
        self.assertTrue(all(call.args[1] == self.guard.hold for call in context.connect.call_args_list))


if __name__ == "__main__":
    unittest.main()
