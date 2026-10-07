#!/usr/bin/env python3
"""Test the GTK3 QA event observer without importing GI or opening a display."""
import ast
from collections import deque
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock


class GtkEventObserverTests(unittest.TestCase):
    def setUp(self):
        source = Path(__file__).with_name("test-linux-gtk3.py")
        names = {"dispatch_gtk_event", "observe_gdk_event", "pump", "delivered_key"}
        definitions = [node for node in ast.parse(source.read_text()).body
                       if isinstance(node, ast.FunctionDef) and node.name in names]
        self.assertEqual({node.name for node in definitions}, names)
        self.owned = object()
        self.view = SimpleNamespace(has_focus=Mock(return_value=True), is_focus=Mock(return_value=True))
        self.downstream = Mock()
        self.context = SimpleNamespace(iteration=Mock(return_value=False))
        self.watch = SimpleNamespace(drain=Mock())
        self.namespace = dict(
            Gdk=SimpleNamespace(EventType=SimpleNamespace(KEY_PRESS=8, KEY_RELEASE=9)),
            window=SimpleNamespace(get_window=lambda: self.owned), xid=42,
            views=[self.view], active=0, raw_key_event_serial=0, raw_key_events=deque(maxlen=32),
            gdk_observer_error=None, default_event_handler=self.downstream,
            time=SimpleNamespace(monotonic=lambda: 100.0, sleep=Mock()), started=99.0,
            qa=SimpleNamespace(GLib=SimpleNamespace(MainContext=SimpleNamespace(default=lambda: self.context)),
                               watch=self.watch),
            x=SimpleNamespace(x=SimpleNamespace(XKeysymToKeycode=Mock(return_value=111)), display=object()),
        )
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), "exec"), self.namespace)

    def event(self, kind=8, toplevel=None):
        target = self.owned if toplevel is None else toplevel
        return SimpleNamespace(type=kind, keyval=65362, hardware_keycode=111, state=0,
                               get_window=Mock(return_value=SimpleNamespace(get_toplevel=lambda: target)))

    def observe(self, event):
        self.namespace["observe_gdk_event"](event, None)

    def test_owned_press_is_observed_before_exactly_one_original_dispatch(self):
        event = self.event()
        original_fields = dict(vars(event))

        def dispatched(original):
            self.assertIs(original, event)
            self.assertEqual(self.namespace["raw_key_event_serial"], 1)
            self.assertEqual(len(self.namespace["raw_key_events"]), 1)

        self.downstream.side_effect = dispatched
        self.observe(event)
        self.downstream.assert_called_once_with(event)
        self.assertEqual(vars(event), original_fields)
        self.assertEqual(self.namespace["raw_key_events"][0], {
            "serial": 1, "pressed": True, "time": 1.0, "event": "8", "view": 0,
            "keysym": 65362, "hardwarecode": 111, "modifiers": 0})
        self.assertIsNone(self.namespace["gdk_observer_error"])

    def test_release_has_a_distinct_serial_but_is_not_a_press(self):
        self.observe(self.event())
        self.observe(self.event(9))
        self.assertEqual([(event["serial"], event["pressed"]) for event in
                          self.namespace["raw_key_events"]], [(1, True), (2, False)])
        self.assertEqual(self.downstream.call_count, 2)

    def test_other_windows_nonkeys_and_unfocused_views_are_forwarded_not_observed(self):
        foreign, nonkey, no_window = self.event(toplevel=object()), self.event(12), self.event()
        no_window.get_window.return_value = None
        for event in (foreign, nonkey, no_window):
            self.observe(event)
        for focus_property in ("has_focus", "is_focus"):
            getattr(self.view, focus_property).return_value = False
            self.observe(self.event())
            getattr(self.view, focus_property).return_value = True
        self.assertEqual(self.namespace["raw_key_event_serial"], 0)
        self.assertEqual(list(self.namespace["raw_key_events"]), [])
        self.assertEqual(self.downstream.call_count, 5)
        nonkey.get_window.assert_not_called()
        self.assertIsNone(self.namespace["gdk_observer_error"])

    def test_observer_failure_still_dispatches_once_and_pump_raises_original_error(self):
        error = RuntimeError("synthetic event inspection failure")
        event = self.event()
        event.get_window.side_effect = error
        self.observe(event)
        self.downstream.assert_called_once_with(event)
        self.assertIs(self.namespace["gdk_observer_error"], error)
        with self.assertRaises(RuntimeError) as raised:
            self.namespace["pump"]()
        self.assertIs(raised.exception, error)
        self.watch.drain.assert_not_called()

    def test_later_dispatch_failure_does_not_replace_observer_failure(self):
        original = RuntimeError("first failure")
        event = self.event()
        event.get_window.side_effect = original
        self.downstream.side_effect = ValueError("second failure")
        self.observe(event)
        self.downstream.assert_called_once_with(event)
        self.assertIs(self.namespace["gdk_observer_error"], original)

    def test_dispatch_failure_is_also_propagated_without_retrying_the_event(self):
        error = RuntimeError("synthetic dispatch failure")
        self.downstream.side_effect = error
        event = self.event()
        self.observe(event)
        self.downstream.assert_called_once_with(event)
        with self.assertRaises(RuntimeError) as raised:
            self.namespace["pump"]()
        self.assertIs(raised.exception, error)

    def test_restored_default_dispatch_has_no_observer_side_effects(self):
        event = self.event()
        self.namespace["dispatch_gtk_event"](event, None)
        self.downstream.assert_called_once_with(event)
        self.assertEqual(list(self.namespace["raw_key_events"]), [])
        self.namespace["pump"]()
        self.watch.drain.assert_called_once_with()

    def test_same_seed_widget_release_or_raw_release_cannot_acknowledge_a_press(self):
        self.namespace["events"] = [{"serial": 999, "pressed": True, "hardwarecode": 111, "view": 0}]
        self.namespace["key"] = Mock(side_effect=lambda *args, **kwargs: self.observe(self.event(9)))

        def wait(check, _label):
            self.assertFalse(check())
            raise AssertionError("navigation press was not received")

        self.namespace["wait"] = wait
        with self.assertRaisesRegex(AssertionError, "press was not received"):
            self.namespace["delivered_key"](65362, expect="hel")

    def test_delivery_acknowledges_only_a_new_raw_owned_hardware_press(self):
        self.namespace["key"] = Mock(side_effect=lambda *args, **kwargs: self.observe(self.event()))
        self.namespace["wait"] = lambda check, _label: self.assertTrue(check())
        self.namespace["delivered_key"](65362, expect="hel")
        self.namespace["key"].assert_called_once_with(65362, expect="hel")
        self.downstream.assert_called_once()


if __name__ == "__main__":
    unittest.main()
