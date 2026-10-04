#!/usr/bin/env python3
"""Read-only validation and sparse queries for the functional maturity tensor.

The digest covers sorted, unique implementation/test paths and their content
hashes, not evidence documents or Git cleanliness. No commands, network calls
or writes run. Each record is UTF-8 path + NUL + the file SHA-256 as 64 ASCII hex
bytes; fixed-width content hashes prevent ambiguous cross-file boundaries.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import sys


CRITERIA = ("I", "T", "V", "O")
EVIDENCE_KINDS = {"source", "historical_run", "manual_acceptance", "delivery_contract"}


class TensorError(ValueError):
    """An invalid tensor or a source snapshot that no longer matches."""


def require(condition, message):
    if not condition:
        raise TensorError(message)


def object_fields(value, fields, location):
    require(isinstance(value, dict), f"{location}: expected an object")
    actual, expected = set(value), set(fields)
    require(actual == expected,
            f"{location}: missing fields {sorted(expected - actual)}; "
            f"unknown fields {sorted(actual - expected)}")


def string(value, location, allow_empty=False):
    require(isinstance(value, str) and (allow_empty or bool(value.strip())),
            f"{location}: expected a nonempty string")


def array(value, location):
    require(isinstance(value, list), f"{location}: expected an array")


def ids(value, location):
    array(value, location)
    for item in value:
        string(item, location)
    require(len(set(value)) == len(value), f"{location}: duplicate reference")


def repo_file(root, value):
    string(value, "path")
    path = PurePosixPath(value)
    require("\x00" not in value and "\\" not in value and not path.is_absolute()
            and not PureWindowsPath(value).drive
            and ".." not in path.parts and path.parts,
            f"path {value!r}: expected a repository-relative path without '..'")
    try:
        resolved = (root / value).resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise TensorError(f"path {value!r}: missing or unreadable: {error}") from error
    require(resolved.is_relative_to(root), f"path {value!r}: symlink escapes repository")
    require(resolved.is_file(), f"path {value!r}: expected a regular file")
    return resolved


def paths(root, value, location):
    array(value, location)
    for name in value:
        try:
            repo_file(root, name)
        except TensorError as error:
            raise TensorError(f"{location}: {error}") from error


def catalog(data, name, fields):
    array(data[name], name)
    entries = {}
    for index, item in enumerate(data[name]):
        location = f"{name}[{index}]"
        object_fields(item, fields, location)
        string(item["id"], f"{location}.id")
        require(item["id"] not in entries, f"{name}: duplicate id {item['id']!r}")
        entries[item["id"]] = item
    return entries


def source_digest(data, root):
    """SHA256(concat(path_utf8 + NUL + SHA256(contents).hexdigest_ascii64))."""
    root = Path(root).resolve()
    names = sorted({name for item in data["implementations"]
                    for key in ("paths", "tests") for name in item[key]})
    digest = hashlib.sha256()
    for name in names:
        digest.update(name.encode("utf-8"))
        digest.update(b"\0")
        try:
            contents = repo_file(root, name).read_bytes()
            digest.update(hashlib.sha256(contents).hexdigest().encode("ascii"))
        except OSError as error:
            raise TensorError(f"path {name!r}: cannot read: {error}") from error
    return digest.hexdigest()


def validate(data, root, check_digest=True):
    root = Path(root).resolve()
    object_fields(data, ("schema_version", "snapshot", "semantics", "architectures", "criteria",
                         "implementations", "functions", "evidence", "cells"), "root")
    require(type(data["schema_version"]) is int and data["schema_version"] == 1,
            "schema_version: expected integer 1")
    snapshot = data["snapshot"]
    object_fields(snapshot, ("date", "head", "dirty", "source_sha256", "note"), "snapshot")
    string(snapshot["date"], "snapshot.date")
    try:
        date = datetime.date.fromisoformat(snapshot["date"])
    except ValueError as error:
        raise TensorError("snapshot.date: expected YYYY-MM-DD") from error
    require(date.isoformat() == snapshot["date"], "snapshot.date: expected YYYY-MM-DD")
    string(snapshot["head"], "snapshot.head")
    string(snapshot["note"], "snapshot.note")
    require(type(snapshot["dirty"]) is bool, "snapshot.dirty: expected a boolean")
    string(snapshot["source_sha256"], "snapshot.source_sha256", allow_empty=True)
    semantics = data["semantics"]
    semantic_fields = ("axes", "missing", "score_zero", "aggregation", "evidence", "coverage")
    object_fields(semantics, (*semantic_fields, "exclusions"), "semantics")
    for key in semantic_fields:
        string(semantics[key], f"semantics.{key}")
    array(semantics["exclusions"], "semantics.exclusions")
    for exclusion in semantics["exclusions"]:
        string(exclusion, "semantics.exclusions")

    architectures = catalog(data, "architectures", ("id", "title"))
    criteria = catalog(data, "criteria", ("id", "title", "levels"))
    require(tuple(criteria) == CRITERIA, "criteria: expected ordered I, T, V, O")
    implementations = catalog(data, "implementations",
                              ("id", "a", "title", "scope", "paths", "tests"))
    functions = catalog(data, "functions", ("id", "title", "parents"))
    evidence = catalog(data, "evidence", ("id", "kind", "paths", "scope", "note"))
    axis_owners = {}
    for name, entries in (("architectures", architectures), ("functions", functions),
                          ("implementations", implementations)):
        for identifier in entries:
            require(identifier not in axis_owners,
                    f"axis catalog collision: id {identifier!r} reused by "
                    f"{axis_owners.get(identifier)} and {name}; --slice must be unambiguous")
            axis_owners[identifier] = name
    for name, entries in (("architectures", architectures), ("criteria", criteria),
                          ("implementations", implementations), ("functions", functions)):
        require(bool(entries), f"{name}: must not be empty")
        for identifier, item in entries.items():
            string(item["title"], f"{name}.{identifier}.title")
    for identifier, item in criteria.items():
        array(item["levels"], f"criteria.{identifier}.levels")
        require(len(item["levels"]) == 5, f"criteria.{identifier}: expected five levels")
        for level in item["levels"]:
            string(level, f"criteria.{identifier}.levels")
    for identifier, item in implementations.items():
        string(item["a"], f"implementations.{identifier}.a")
        require(item["a"] in architectures,
                f"implementation {identifier}: unknown architecture {item['a']!r}")
        string(item["scope"], f"implementations.{identifier}.scope")
        for key in ("paths", "tests"):
            paths(root, item[key], f"implementations.{identifier}.{key}")
    for identifier, item in functions.items():
        require(re.fullmatch(r"F\d{2,}", identifier) is not None,
                f"function {identifier!r}: expected an F-number identifier")
        ids(item["parents"], f"functions.{identifier}.parents")
        for parent in item["parents"]:
            require(parent in functions, f"function {identifier}: unknown parent {parent!r}")
    visited, pending = set(), set()

    def visit(identifier):
        require(identifier not in pending, f"functions: parent cycle at {identifier}")
        if identifier not in visited:
            pending.add(identifier)
            for parent in functions[identifier]["parents"]:
                visit(parent)
            pending.remove(identifier)
            visited.add(identifier)

    for identifier in functions:
        visit(identifier)
    for identifier, item in evidence.items():
        string(item["kind"], f"evidence.{identifier}.kind")
        require(item["kind"] in EVIDENCE_KINDS,
                f"evidence {identifier}: unknown kind {item['kind']!r}")
        string(item["scope"], f"evidence.{identifier}.scope")
        string(item["note"], f"evidence.{identifier}.note")
        paths(root, item["paths"], f"evidence.{identifier}.paths")
        require(bool(item["paths"]), f"evidence {identifier}: must cite a file")

    array(data["cells"], "cells")
    coordinates, covered = set(), set()
    for index, cell in enumerate(data["cells"]):
        location = f"cells[{index}]"
        object_fields(cell, ("a", "f", "i", "m", "evidence", "limit"), location)
        for key, entries in (("a", architectures), ("f", functions), ("i", implementations)):
            string(cell[key], f"{location}.{key}")
            require(cell[key] in entries, f"{location}: unknown {key} id {cell[key]!r}")
        implementation = implementations[cell["i"]]
        require(cell["a"] == implementation["a"],
                f"{location}: architecture does not match implementation {cell['i']}")
        coordinate = (cell["a"], cell["f"], cell["i"])
        require(coordinate not in coordinates, f"{location}: duplicate coordinate {coordinate}")
        coordinates.add(coordinate)
        covered.add(cell["f"])
        string(cell["limit"], f"{location}.limit")
        ids(cell["evidence"], f"{location}.evidence")
        for reference in cell["evidence"]:
            require(reference in evidence, f"{location}: unknown evidence {reference!r}")
        kinds = {evidence[reference]["kind"] for reference in cell["evidence"]}
        array(cell["m"], f"{location}.m")
        require(len(cell["m"]) == 4, f"{location}.m: expected four values for I, T, V, O")
        for criterion, value in zip(CRITERIA, cell["m"]):
            require(value is None or type(value) is int and 0 <= value <= 4,
                    f"{location}.{criterion}: expected integer 0..4 or null (not boolean)")
        implementation_score, testing, verification, delivery = cell["m"]
        # v1 evidence has no exact-snapshot identity or structured acceptance
        # result. A historical run, manual note or delivery contract cannot
        # certify today's source or artifact merely by being cited in a cell.
        for criterion, value, maximum in zip(CRITERIA, cell["m"], (3, 3, 0, 2)):
            require(value is None or value <= maximum,
                    f"{location}.{criterion}={value}: schema v1 cannot verify current-snapshot "
                    "acceptance/delivery; extend the schema with exact snapshot evidence "
                    "before assigning this level")
        if implementation_score is not None and implementation_score > 0:
            require(bool(implementation["paths"]), f"{location}: I>0 requires implementation paths")
        if testing is not None and testing > 0:
            require(bool(implementation["tests"]), f"{location}: T>0 requires test paths")
        if testing is not None and testing >= 3:
            require("historical_run" in kinds, f"{location}: T>=3 requires historical_run evidence")
        if verification is not None and verification > 0:
            require("manual_acceptance" in kinds,
                    f"{location}: V>0 requires manual_acceptance evidence")
        if delivery is not None and delivery > 0:
            require("delivery_contract" in kinds,
                    f"{location}: O>0 requires delivery_contract evidence")
    require(covered == set(functions), f"functions without cells: {sorted(set(functions) - covered)}")
    actual_digest = source_digest(data, root)
    if check_digest:
        require(re.fullmatch(r"[0-9a-f]{64}", snapshot["source_sha256"]) is not None,
                "snapshot.source_sha256: expected 64 lowercase hex digits; run --digest")
        require(snapshot["source_sha256"] == actual_digest,
                f"snapshot source digest drift: expected {snapshot['source_sha256']}, "
                f"actual {actual_digest}; review changes before updating --digest")
    return actual_digest


def slice_cells(data, identifier):
    catalogs = {name: {item["id"]: item for item in data[name]}
                for name in ("architectures", "functions", "implementations", "evidence")}
    require(any(identifier in catalogs[name]
                for name in ("architectures", "functions", "implementations")),
            f"slice: unknown architecture/function/implementation id {identifier!r}")
    result = []
    for cell in data["cells"]:
        if identifier not in (cell["a"], cell["f"], cell["i"]):
            continue
        implementation = catalogs["implementations"][cell["i"]]
        result.append({
            "a": cell["a"], "f": cell["f"], "i": cell["i"],
            "titles": {"a": catalogs["architectures"][cell["a"]]["title"],
                       "f": catalogs["functions"][cell["f"]]["title"],
                       "i": implementation["title"]},
            "scope": implementation["scope"],
            "paths": implementation["paths"], "tests": implementation["tests"],
            "m": dict(zip(CRITERIA, cell["m"])), "limit": cell["limit"],
            "evidence": [catalogs["evidence"][key] for key in cell["evidence"]],
        })
    return result


def coo(data):
    for cell in data["cells"]:
        for criterion, value in zip(CRITERIA, cell["m"]):
            yield {"a": cell["a"], "f": cell["f"], "i": cell["i"],
                   "m": criterion, "value": value}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"JSON: duplicate object field {key!r}")
        result[key] = value
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--data", type=Path, default=Path("docs/functional-tensor.json"))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="validate schema, paths and source digest (default)")
    mode.add_argument("--digest", action="store_true", help="compute digest, permitting an empty saved hash")
    mode.add_argument("--slice", metavar="ID", help="show cells for a function, architecture or implementation")
    mode.add_argument("--coo", action="store_true", help="emit one JSON coordinate per criterion, including null")
    args = parser.parse_args(argv)
    try:
        root = args.root.resolve()
        path = args.data if args.data.is_absolute() else root / args.data
        data = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        digest = validate(data, root, check_digest=not args.digest)
        if args.digest:
            print(digest)
        elif args.slice:
            print(json.dumps(slice_cells(data, args.slice), ensure_ascii=False, indent=2))
        elif args.coo:
            for entry in coo(data):
                print(json.dumps(entry, ensure_ascii=False))
        else:
            print(f"OK: {len(data['functions'])} functions, {len(data['cells'])} cells; "
                  f"source_sha256={digest}; snapshot dirty={data['snapshot']['dirty']}")
        return 0
    except (TensorError, OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"functional-tensor: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
