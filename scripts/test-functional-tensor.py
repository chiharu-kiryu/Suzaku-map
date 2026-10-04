#!/usr/bin/env python3
"""Deterministic stdlib-only tensor checks using disposable local fixtures."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout


sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "functional_tensor", Path(__file__).with_name("functional-tensor.py"))
TENSOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TENSOR)


class TensorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="suzaku-tensor-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name, content in (("src/core.rs", "fn core() {}\n"),
                              ("tests/core.rs", "test\n"), ("docs/run.md", "evidence\n")):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.data = {
            "schema_version": 1,
            "snapshot": {"date": "2026-10-03", "head": "baseline", "dirty": True,
                         "source_sha256": "", "note": "Static fixture snapshot"},
            "semantics": {
                "axes": "Architecture, function, implementation, criterion",
                "missing": "Missing coordinates and null are not zero",
                "score_zero": "Explicitly absent or failed",
                "aggregation": "No averages",
                "evidence": "History does not certify today's snapshot",
                "coverage": "Declared relationships only",
                "exclusions": ["No implicit current delivery"]
            },
            "architectures": [{"id": "A01", "title": "Core"}],
            "criteria": [{"id": key, "title": key,
                          "levels": [f"level {number}" for number in range(5)]}
                         for key in TENSOR.CRITERIA],
            "implementations": [{"id": "core", "a": "A01", "title": "Engine",
                                 "scope": "Unit fixture", "paths": ["src/core.rs"],
                                 "tests": ["tests/core.rs"]}],
            "functions": [{"id": "F01", "title": "Input", "parents": []}],
            "evidence": [{"id": "E01", "kind": "source", "paths": ["src/core.rs"],
                          "scope": "Static", "note": "Not an execution claim"}],
            "cells": [{"a": "A01", "f": "F01", "i": "core", "m": [2, 2, None, 0],
                       "evidence": ["E01"], "limit": "No real desktop acceptance"}],
        }
        self.refresh_digest()

    def refresh_digest(self):
        self.data["snapshot"]["source_sha256"] = TENSOR.source_digest(self.data, self.root)

    def invalid(self, pattern):
        with self.assertRaisesRegex(TENSOR.TensorError, pattern):
            TENSOR.validate(self.data, self.root)

    def test_valid_dirty_snapshot_preserves_null_distinct_from_zero(self):
        TENSOR.validate(self.data, self.root)
        entries = list(TENSOR.coo(self.data))
        self.assertEqual([entry["m"] for entry in entries], ["I", "T", "V", "O"])
        self.assertIsNone(entries[2]["value"])
        self.assertEqual(entries[3]["value"], 0)
        self.assertIsNone(TENSOR.slice_cells(self.data, "F01")[0]["m"]["V"])

    def test_duplicate_coordinates_are_rejected(self):
        self.data["cells"].append(copy.deepcopy(self.data["cells"][0]))
        self.invalid("duplicate coordinate")

    def test_catalog_ids_are_unique_within_each_axis(self):
        for name in ("architectures", "criteria", "implementations", "functions", "evidence"):
            with self.subTest(catalog=name):
                original = self.data[name][:]
                self.data[name].append(copy.deepcopy(self.data[name][0]))
                self.invalid("duplicate id")
                self.data[name] = original

    def test_catalog_ids_cannot_collide_across_slice_axes(self):
        original = copy.deepcopy(self.data)
        for catalog, collision in (("architectures", "F01"),
                                   ("architectures", "core"),
                                   ("implementations", "F01")):
            with self.subTest(catalog=catalog, collision=collision):
                self.data = copy.deepcopy(original)
                self.data[catalog][0]["id"] = collision
                self.invalid("axis catalog collision")

    def test_evidence_and_criteria_keep_separate_id_namespaces(self):
        self.data["evidence"][0]["id"] = "F01"
        self.data["cells"][0]["evidence"] = ["F01"]
        self.data["architectures"][0]["id"] = "I"
        self.data["implementations"][0]["a"] = "I"
        self.data["cells"][0]["a"] = "I"
        TENSOR.validate(self.data, self.root)
        self.assertEqual(len(TENSOR.slice_cells(self.data, "I")), 1)

    def test_dangling_cell_ids_and_evidence_are_rejected(self):
        for field in ("a", "f", "i"):
            with self.subTest(field=field):
                original = self.data["cells"][0][field]
                self.data["cells"][0][field] = "missing"
                self.invalid("unknown")
                self.data["cells"][0][field] = original
        self.data["cells"][0]["evidence"] = ["missing"]
        self.invalid("unknown evidence")

    def test_implementation_architecture_must_match_cell(self):
        self.data["architectures"].append({"id": "A02", "title": "Host"})
        self.data["cells"][0]["a"] = "A02"
        self.invalid("architecture does not match")
        self.data["cells"][0]["a"] = "A01"
        self.data["implementations"][0]["a"] = "missing"
        self.invalid("unknown architecture")

    def test_scores_are_integers_or_null_never_booleans(self):
        for invalid in (True, False, -1, 5, 2.0, "2"):
            with self.subTest(value=invalid):
                self.data["cells"][0]["m"][0] = invalid
                self.invalid("integer 0..4 or null")
        self.data["cells"][0]["m"][0] = None
        TENSOR.validate(self.data, self.root)

    def test_high_scores_require_referenced_evidence_of_the_right_kind(self):
        original = copy.deepcopy(self.data)
        for position, value, kind in ((1, 3, "historical_run"),
                                      (3, 1, "delivery_contract"),
                                      (3, 2, "delivery_contract")):
            with self.subTest(kind=kind):
                self.data = copy.deepcopy(original)
                self.data["cells"][0]["m"][position] = value
                self.invalid(f"requires {kind}")
                self.data["evidence"].append({"id": "E02", "kind": kind,
                                             "paths": ["docs/run.md"], "scope": "Fixture",
                                             "note": "Exact controlled scope"})
                self.invalid(f"requires {kind}")
                self.data["cells"][0]["evidence"].append("E02")
                TENSOR.validate(self.data, self.root)

    def test_history_and_contracts_cannot_claim_current_snapshot_high_scores(self):
        for number, kind in enumerate(("historical_run", "manual_acceptance", "delivery_contract"), 2):
            identifier = f"E{number:02}"
            self.data["evidence"].append({"id": identifier, "kind": kind,
                                         "paths": ["docs/run.md"], "scope": "Historical fixture",
                                         "note": "Not current snapshot acceptance"})
            self.data["cells"][0]["evidence"].append(identifier)
        original = self.data["cells"][0]["m"][:]
        for position, value in ((0, 4), (1, 4), (2, 1), (2, 4), (3, 3), (3, 4)):
            with self.subTest(criterion=TENSOR.CRITERIA[position], value=value):
                self.data["cells"][0]["m"] = original[:]
                self.data["cells"][0]["m"][position] = value
                self.invalid("schema v1 cannot verify current-snapshot")

    def test_semantics_and_snapshot_notes_are_validated(self):
        self.data["snapshot"]["note"] = ""
        self.invalid("snapshot.note")
        self.data["snapshot"]["note"] = "No current acceptance"
        self.data["semantics"]["missing"] = False
        self.invalid("semantics.missing")
        self.data["semantics"]["missing"] = "Unknown is not zero"
        self.data["semantics"]["exclusions"] = [1]
        self.invalid("semantics.exclusions")

    def test_unknown_evidence_kind_and_empty_paths_are_rejected(self):
        self.data["evidence"][0]["kind"] = "trust_me"
        self.invalid("unknown kind")
        self.data["evidence"][0]["kind"] = "source"
        self.data["evidence"][0]["paths"] = []
        self.invalid("must cite a file")

    def test_paths_reject_parent_absolute_windows_missing_and_directories(self):
        for name in ("../outside", str(self.root / "src/core.rs"),
                     "C:/outside.txt", "..\\outside", "missing.rs", "src"):
            with self.subTest(path=name):
                self.data["implementations"][0]["paths"] = [name]
                self.invalid("path")

    def test_symlink_escape_is_rejected_but_internal_file_link_is_safe(self):
        with tempfile.TemporaryDirectory(prefix="suzaku-tensor-outside-") as outside:
            target = Path(outside) / "outside.txt"
            target.write_text("outside", encoding="utf-8")
            (self.root / "escape").symlink_to(target)
            self.data["implementations"][0]["paths"] = ["escape"]
            self.invalid("symlink escapes")
        (self.root / "inside").symlink_to(self.root / "src/core.rs")
        self.data["implementations"][0]["paths"] = ["inside"]
        self.refresh_digest()
        TENSOR.validate(self.data, self.root)

    def test_digest_uses_sorted_unique_source_and_test_paths_not_evidence(self):
        expected = hashlib.sha256()
        for name in ("src/core.rs", "tests/core.rs"):
            content_hash = hashlib.sha256((self.root / name).read_bytes()).hexdigest().encode("ascii")
            expected.update(name.encode() + b"\0" + content_hash)
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        self.data["implementations"][0]["paths"].extend(["tests/core.rs", "src/core.rs"])
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        (self.root / "docs/run.md").write_text("new evidence", encoding="utf-8")
        self.assertEqual(TENSOR.source_digest(self.data, self.root), expected.hexdigest())
        (self.root / "src/core.rs").write_text("changed source", encoding="utf-8")
        self.invalid("digest drift")

    def test_digest_frames_adversarial_content_and_path_boundaries(self):
        first = {"a": b"foo", "b": b"b\0bar"}
        second = {"a": b"foob\0", "b": b"bar"}

        def old_unframed_bytes(contents):
            return b"".join(name.encode() + b"\0" + contents[name] for name in sorted(contents))

        self.assertEqual(old_unframed_bytes(first), old_unframed_bytes(second))
        self.data["implementations"][0]["paths"] = ["a", "b"]
        self.data["implementations"][0]["tests"] = []
        digests = []
        for contents in (first, second):
            for name, content in contents.items():
                (self.root / name).write_bytes(content)
            digests.append(TENSOR.source_digest(self.data, self.root))
        self.assertNotEqual(*digests)

    def test_parent_references_cycles_and_missing_cells_are_rejected(self):
        self.data["functions"][0]["parents"] = ["F99"]
        self.invalid("unknown parent")
        self.data["functions"][0]["parents"] = ["F01"]
        self.invalid("parent cycle")
        self.data["functions"][0]["parents"] = []
        self.data["functions"].append({"id": "F69", "title": "Future", "parents": ["F01"]})
        self.invalid("functions without cells")
        cell = copy.deepcopy(self.data["cells"][0])
        cell["f"] = "F69"
        self.data["cells"].append(cell)
        TENSOR.validate(self.data, self.root)

    def test_fixed_criteria_order_and_snapshot_types(self):
        self.data["criteria"].reverse()
        self.invalid("ordered I, T, V, O")
        self.data["criteria"].reverse()
        self.data["snapshot"]["dirty"] = 1
        self.invalid("expected a boolean")
        self.data["snapshot"]["dirty"] = True
        self.data["schema_version"] = True
        self.invalid("integer 1")

    def test_slice_by_any_axis_and_unknown_id(self):
        for identifier in ("F01", "A01", "core"):
            result = TENSOR.slice_cells(self.data, identifier)
            self.assertEqual(len(result), 1)
            self.assertEqual(result[0]["paths"], ["src/core.rs"])
            self.assertEqual(result[0]["evidence"][0]["id"], "E01")
        with self.assertRaisesRegex(TENSOR.TensorError, "unknown"):
            TENSOR.slice_cells(self.data, "missing")

    def test_cli_default_check_digest_and_coo_are_read_only(self):
        path = self.root / "docs/functional-tensor.json"
        path.write_text(json.dumps(self.data), encoding="utf-8")
        before = path.read_bytes()
        base = ["--root", str(self.root)]
        for flags in ([], ["--check"], ["--digest"], ["--coo"], ["--slice", "F01"]):
            with self.subTest(flags=flags), redirect_stdout(io.StringIO()) as output:
                self.assertEqual(TENSOR.main(base + flags), 0)
                self.assertTrue(output.getvalue())
                self.assertEqual(path.read_bytes(), before)
        self.data["snapshot"]["source_sha256"] = ""
        path.write_text(json.dumps(self.data), encoding="utf-8")
        with redirect_stdout(io.StringIO()):
            self.assertEqual(TENSOR.main(base + ["--digest"]), 0)
        with redirect_stderr(io.StringIO()) as error:
            self.assertEqual(TENSOR.main(base), 1)
        self.assertIn("source_sha256", error.getvalue())

    def test_duplicate_json_fields_are_not_silently_overwritten(self):
        path = self.root / "duplicate.json"
        path.write_text('{"schema_version": 1, "schema_version": 1}', encoding="utf-8")
        with redirect_stderr(io.StringIO()) as error:
            self.assertEqual(TENSOR.main(["--root", str(self.root), "--data", str(path)]), 1)
        self.assertIn("duplicate object field", error.getvalue())


if __name__ == "__main__":
    unittest.main()
