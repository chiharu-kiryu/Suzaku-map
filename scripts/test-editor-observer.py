#!/usr/bin/env python3
"""Deterministic save-barrier checks; no display, input or user files needed."""
from types import SimpleNamespace
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock

sys.dont_write_bytecode = True
from fixtures.editor_observer import (
    EditorState, GtkEditorObserver, editor_environment, nodes, save_observed_document,
    visible_nodes,
)


def bounded_wait(check, label):
    for _ in range(8):
        if check():
            return
    raise AssertionError(label)


class SaveBarrierTests(unittest.TestCase):
    def path(self, text):
        return SimpleNamespace(name="owned.txt", read_text=Mock(return_value=text))

    def test_waits_for_buffer_and_idle_even_when_file_already_matches(self):
        events = []
        states = iter([None, EditorState("hello", True), EditorState("hello", False),
                       EditorState("hello", True), EditorState("hello", False)])

        def observe():
            state = next(states)
            events.append(state)
            return state

        path = self.path("hello\n")
        save_observed_document(SimpleNamespace(state=observe), path, "hello\n",
                               lambda: events.append("save"), bounded_wait)
        self.assertEqual(events, [None, EditorState("hello", True), EditorState("hello", False),
                                  "save", EditorState("hello", True), EditorState("hello", False)])
        self.assertEqual(path.read_text.call_count, 1)

    def test_wrong_buffer_is_not_hidden_by_matching_old_file(self):
        send = Mock()
        observer = SimpleNamespace(state=lambda: EditorState("hell", False))
        with self.assertRaisesRegex(AssertionError, "before save.*observed=.*hell"):
            save_observed_document(observer, self.path("hello\n"), "hello\n", send, bounded_wait)
        send.assert_not_called()

    def test_busy_editor_never_receives_save_chord(self):
        send = Mock()
        observer = SimpleNamespace(state=lambda: EditorState("你好", True))
        with self.assertRaisesRegex(AssertionError, "before save.*busy=True"):
            save_observed_document(observer, self.path(""), "你好\n", send, bounded_wait)
        send.assert_not_called()

    def test_failed_save_is_not_retried_or_replaced(self):
        send = Mock()
        observer = SimpleNamespace(state=lambda: EditorState("你好", False))
        with self.assertRaisesRegex(AssertionError, "after save.*saved='old"):
            save_observed_document(observer, self.path("old\n"), "你好\n", send, bounded_wait)
        send.assert_called_once_with()

    def test_buffer_change_after_save_does_not_pass_on_matching_file(self):
        send = Mock()

        def observe():
            return EditorState("hello" if not send.called else "extra hello", False)

        with self.assertRaisesRegex(AssertionError, "after save.*extra hello"):
            save_observed_document(SimpleNamespace(state=observe), self.path("hello\n"),
                                   "hello\n", send, bounded_wait)
        send.assert_called_once_with()

    def test_missing_accessibility_is_a_failure_not_a_fallback_to_file_only(self):
        send = Mock()
        with self.assertRaisesRegex(AssertionError, "before save.*observed=None"):
            save_observed_document(SimpleNamespace(state=lambda: None), self.path(""),
                                   "", send, bounded_wait)
        send.assert_not_called()

    def test_only_sourceview_final_newline_is_implicit(self):
        for text, serialized in [("", ""), ("hello", "hello\n"), ("hello\n", "hello\n"),
                                 ("你好  ", "你好  \n"), ("  ", "  \n"), ("é😀\t", "é😀\t\n"),
                                 ("hello\n\n", "hello\n\n")]:
            with self.subTest(text=text):
                self.assertEqual(EditorState(text, False).saved_text(), serialized)

    def test_exited_editor_is_rejected_before_any_desktop_query(self):
        api = Mock()
        observer = GtkEditorObserver(api, SimpleNamespace(poll=lambda: 0), self.path(""))
        with self.assertRaisesRegex(AssertionError, "owned editor exited"):
            observer.state()
        api.get_desktop.assert_not_called()

    def test_only_owned_focused_document_is_read_not_another_process_or_tab(self):
        def node(role, name="", states=(), children=()):
            value = Mock()
            value.get_role.return_value = role
            value.get_name.return_value = name
            value.get_state_set.return_value.contains.side_effect = lambda state: state in states
            value.get_child_count.return_value = len(children)
            value.get_child_at_index.side_effect = lambda index: children[index]
            return value

        background = node("text", states=("showing", "multiline"))
        target = node("text", states=("showing", "multiline", "focused"))
        progress = node("progress", states=("showing",))
        frame = node("frame", "owned.txt (/tmp/owned) - Text Editor", ("showing",),
                     (background, target, progress))
        app = node("application", children=(frame,))
        app.get_process_id.return_value = 42
        foreign = Mock()
        foreign.get_process_id.return_value = 43
        desktop = node("desktop", children=(foreign, app))
        api = SimpleNamespace(
            get_desktop=lambda _: desktop,
            Cache=SimpleNamespace(NONE=0),
            Role=SimpleNamespace(FRAME="frame", TEXT="text", PROGRESS_BAR="progress"),
            StateType=SimpleNamespace(SHOWING="showing", MULTI_LINE="multiline", FOCUSED="focused"),
            Text=SimpleNamespace(get_text=Mock(return_value="你好")))
        observer = GtkEditorObserver(api, SimpleNamespace(poll=lambda: None, pid=42), self.path(""))
        self.assertEqual(observer.state(), EditorState("你好", True))
        app.set_cache_mask.assert_called_once_with(0)
        api.Text.get_text.assert_called_once_with(target, 0, -1)
        foreign.get_child_count.assert_not_called()
        desktop.clear_cache.assert_not_called()
        # One observation may reuse its own topology/role/state reads, but the
        # next observation must query again (including focus and save progress).
        for value in [app, frame, background, target, progress]:
            value.get_child_count.assert_called_once_with()
            value.get_role.assert_called_once_with()
        for value in [frame, background, target, progress]:
            value.get_state_set.assert_called_once_with()
        progress.get_state_set.return_value.contains.side_effect = lambda _: False
        target.get_state_set.return_value.contains.side_effect = lambda state: state in ("showing", "multiline")
        background.get_state_set.return_value.contains.side_effect = lambda state: state in ("showing", "multiline", "focused")
        api.Text.get_text.return_value = "new buffer"
        self.assertEqual(observer.state(), EditorState("new buffer", False))
        api.Text.get_text.assert_called_with(background, 0, -1)
        for value in [app, frame, background, target, progress]:
            self.assertEqual(value.get_child_count.call_count, 2)
            self.assertEqual(value.get_role.call_count, 2)
        for value in [frame, background, target, progress]:
            self.assertEqual(value.get_state_set.call_count, 2)
        self.assertEqual(app.set_cache_mask.call_count, 2)

    def test_missing_owned_process_does_not_observe_foreign_text(self):
        foreign = Mock()
        foreign.get_process_id.return_value = 43
        desktop = Mock()
        desktop.get_child_count.return_value = 1
        desktop.get_child_at_index.return_value = foreign
        observer = GtkEditorObserver(SimpleNamespace(get_desktop=lambda _: desktop),
                                     SimpleNamespace(poll=lambda: None, pid=42), self.path(""))
        self.assertIsNone(observer.state())
        foreign.get_child_count.assert_not_called()

    def test_accessibility_aliases_and_cycles_are_visited_once(self):
        root, child = Mock(), Mock()
        root.get_child_count.return_value = 3
        root.get_child_at_index.side_effect = [child, None, child]
        child.get_child_count.return_value = 1
        child.get_child_at_index.return_value = root
        self.assertEqual(list(nodes(root)), [root, child])
        child.get_child_count.assert_called_once_with()

    def test_owned_popup_visibility_is_refreshed_before_each_read(self):
        root = Mock()
        root.get_child_count.return_value = 0
        visibility = iter([True, False])
        root.clear_cache.side_effect = lambda: setattr(root, "showing", next(visibility))
        root.get_state_set.return_value.contains.side_effect = lambda state: root.showing
        self.assertEqual(visible_nodes(root, "showing"), [root])
        self.assertEqual(visible_nodes(root, "showing"), [])
        self.assertEqual(root.clear_cache.call_count, 2)

    def test_editor_settings_are_scoped_verified_and_do_not_mutate_parent_environment(self):
        root = Path("/tmp/suzaku-app-qa.fixture")
        base = {"XDG_CONFIG_HOME": str(root / "config"), "GSETTINGS_BACKEND": "memory",
                "XDG_CONFIG_DIRS": "/fixture/system-settings"}
        run = Mock(return_value=SimpleNamespace(stdout="uint32 300\n"))
        env = editor_environment(base, root, run)
        self.assertEqual(base["GSETTINGS_BACKEND"], "memory")
        self.assertEqual(base["XDG_CONFIG_DIRS"], "/fixture/system-settings")
        self.assertEqual(env["GSETTINGS_BACKEND"], "keyfile")
        self.assertEqual(env["XDG_CONFIG_HOME"], base["XDG_CONFIG_HOME"])
        self.assertEqual(env["XDG_CONFIG_DIRS"], str(root / "editor-config") + ":/fixture/system-settings")
        self.assertEqual(run.call_count, 2)
        self.assertEqual(run.call_args_list[0].args[0],
                         ["gsettings", "set", "org.gnome.TextEditor", "auto-save-delay", "300"])
        self.assertEqual(run.call_args_list[1].args[0],
                         ["gsettings", "get", "org.gnome.TextEditor", "auto-save-delay"])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["env"]["XDG_CONFIG_HOME"], str(root / "config"))
            self.assertTrue(call.kwargs["check"])
            self.assertEqual(call.kwargs["timeout"], 3)

    def test_unisolated_or_unapplied_editor_settings_fail_closed(self):
        root = Path("/tmp/suzaku-app-qa.fixture")
        run = Mock(return_value=SimpleNamespace(stdout="uint32 3\n"))
        with self.assertRaisesRegex(AssertionError, "not isolated"):
            editor_environment({"XDG_CONFIG_HOME": "/not-the-fixture"}, root, run)
        run.assert_not_called()
        with self.assertRaisesRegex(AssertionError, "not applied"):
            editor_environment({"XDG_CONFIG_HOME": str(root / "config")}, root, run)


if __name__ == "__main__":
    unittest.main()
