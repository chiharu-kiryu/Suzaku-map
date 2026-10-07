#!/usr/bin/env python3
"""Check source/catalog/shipping lists without building or installing packages.

Artifact content and clean-container installation retain their separate gates.
This check catches stale explicit lists when a data-only collection is added.
"""
from collections import Counter
from pathlib import Path
import re
import shlex
import unittest

ROOT = Path(__file__).resolve().parent.parent


def require_exact(actual, expected):
    if Counter(actual) != Counter(expected):
        raise ValueError(f"pack list differs: actual={actual!r}, expected={expected!r}")


def smoke_names(source):
    match = re.search(r"suzaku_package_test_pack_names=\((.*?)\)", source, re.S)
    if match is None:
        raise ValueError("missing explicit smoke-test pack list")
    return [name + ".json" for name in shlex.split(match[1])]


class ShippingLists(unittest.TestCase):
    def test_catalog_and_both_packaging_lists_cover_exactly_the_sources(self):
        expected = sorted(path.name for path in (ROOT / "data/offline-packs").glob("*.json"))
        self.assertTrue(expected)
        catalog = (ROOT / "src/lexicon/packs.rs").read_text()
        names = re.findall(r'include_str!\("../../data/offline-packs/([^"/]+\.json)"\)', catalog)
        require_exact(names, expected)
        shipping = (ROOT / "scripts/package-linux.sh").read_text()
        names = re.findall(r"data/offline-packs/([A-Za-z0-9-]+\.json)", shipping)
        require_exact(names, expected)
        smoke = (ROOT / "scripts/test-linux-package.sh").read_text()
        require_exact(smoke_names(smoke), expected)

    def test_missing_collection_is_rejected(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json"], ["en-family.json", "ja-family.json"])

    def test_duplicate_collection_is_not_hidden_by_set_comparison(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json", "en-family.json"], ["en-family.json"])

    def test_unknown_collection_is_rejected(self):
        with self.assertRaises(ValueError):
            require_exact(["en-family.json", "unknown.json"], ["en-family.json"])

    def test_missing_smoke_list_is_rejected(self):
        with self.assertRaises(ValueError):
            smoke_names("# no pack list")

    def test_smoke_list_keeps_order_and_duplicate_entries(self):
        source = "suzaku_package_test_pack_names=(en-family\n ja-family en-family)"
        self.assertEqual(smoke_names(source), ["en-family.json", "ja-family.json", "en-family.json"])


if __name__ == "__main__":
    unittest.main()
