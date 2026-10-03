#!/usr/bin/env python3
"""Deterministic focus-ownership checks; no display, bus or input required."""
import sys
import unittest
from unittest.mock import Mock

sys.dont_write_bytecode = True
from fixtures.x11_focus import focus_ancestry


class FocusAncestryTests(unittest.TestCase):
    def setUp(self):
        self.root, self.owned, self.child, self.foreign = 100, 200, 201, 300
        self.tree = {self.root: 0, self.owned: self.root,
                     self.child: self.owned, self.foreign: self.root}

    def test_top_level_window_is_owned(self):
        self.assertEqual(focus_ancestry(self.owned, self.tree.get),
                         (self.owned, self.root))

    def test_child_window_proves_exact_owned_ancestor(self):
        self.assertEqual(focus_ancestry(self.child, self.tree.get),
                         (self.child, self.owned, self.root))

    def test_foreign_sibling_does_not_inherit_ownership_from_shared_root(self):
        chain = focus_ancestry(self.foreign, self.tree.get)
        self.assertIn(self.root, chain)
        self.assertNotIn(self.owned, chain)

    def test_special_or_invalid_focus_never_queries_or_matches_a_window(self):
        for current in (0, 1, None, -1, True, False, "200", 200.0):
            with self.subTest(current=current):
                parent = Mock(side_effect=AssertionError("must not query"))
                self.assertEqual(focus_ancestry(current, parent), ())
                parent.assert_not_called()

    def test_null_or_invalid_parent_cannot_invent_an_ancestor(self):
        for value in (0, 1, None, -1, True, False, "200", 200.0):
            with self.subTest(parent=value):
                parent = Mock(return_value=value)
                self.assertEqual(focus_ancestry(self.foreign, parent), (self.foreign,))
                parent.assert_called_once_with(self.foreign)

    def test_query_failure_stops_without_inventing_ownership(self):
        parent = Mock(side_effect=OSError("window disappeared"))
        chain = focus_ancestry(self.foreign, parent)
        self.assertEqual(chain, (self.foreign,))
        self.assertNotIn(self.owned, chain)
        parent.assert_called_once_with(self.foreign)

    def test_proved_ancestor_survives_failure_above_that_window(self):
        parent = Mock(side_effect=[self.owned, OSError("upper tree unavailable")])
        self.assertEqual(focus_ancestry(self.child, parent), (self.child, self.owned))
        self.assertEqual(parent.call_count, 2)

    def test_self_cycle_stops_after_one_query(self):
        parent = Mock(return_value=self.child)
        self.assertEqual(focus_ancestry(self.child, parent), (self.child,))
        parent.assert_called_once_with(self.child)

    def test_multi_window_cycle_is_bounded_and_does_not_repeat_xids(self):
        parent = Mock(side_effect={self.child: self.foreign, self.foreign: self.child}.get)
        chain = focus_ancestry(self.child, parent)
        self.assertEqual(chain, (self.child, self.foreign))
        self.assertNotIn(self.owned, chain)
        self.assertEqual(parent.call_count, 2)

    def test_depth_limit_never_queries_or_accepts_an_unreached_ancestor(self):
        parent = Mock(side_effect=self.tree.get)
        chain = focus_ancestry(self.child, parent, max_depth=1)
        self.assertEqual(chain, (self.child,))
        self.assertNotIn(self.owned, chain)
        parent.assert_not_called()

    def test_default_depth_has_a_fixed_query_bound(self):
        parent = Mock(side_effect=lambda window: window + 1)
        self.assertEqual(focus_ancestry(10, parent), tuple(range(10, 26)))
        self.assertEqual(parent.call_count, 15)

    def test_invalid_depth_is_rejected_before_a_query(self):
        for depth in (0, -1, None, True, 1.5):
            with self.subTest(depth=depth):
                parent = Mock()
                with self.assertRaises(ValueError):
                    focus_ancestry(self.child, parent, max_depth=depth)
                parent.assert_not_called()

    def test_process_interrupts_are_not_swallowed(self):
        for interrupt in (KeyboardInterrupt, SystemExit):
            with self.subTest(interrupt=interrupt):
                with self.assertRaises(interrupt):
                    focus_ancestry(self.child, Mock(side_effect=interrupt))


if __name__ == "__main__":
    unittest.main()
