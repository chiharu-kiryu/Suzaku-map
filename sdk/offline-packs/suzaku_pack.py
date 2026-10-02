#!/usr/bin/env python3
"""Suzaku data-pack SDK (Python 3.10+, standard library only).

Keep the authoritative schema/decoder checks in `suzaku_tool pack validate`.
No model, internet, input history or executable code is stored in a pack.
"""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile


class PackBuilder:
    """A small fluent authoring API; array order is vocabulary priority."""

    def __init__(self, *, identifier: str, language: str, name: str,
                 description: str, topics: list[str], authors: list[str],
                 license: str, version: str = "1.0.0"):
        self.document = {
            "format_version": 1,
            "manifest": {"id": identifier, "version": version, "name": name,
                         "description": description, "language": language,
                         "topics": list(topics), "authors": list(authors), "license": license},
            "lexicon": {"format_version": 1, "language": language},
        }

    def words(self, layer: str, words: list[str], *, next_words=None, sentences=None):
        """English: explicit forms, (context, [next words]), complete sentences."""
        self.document["lexicon"].setdefault("word_layers", []).append({
            "id": layer, "words": list(words),
            "next_words": list(next_words or []), "sentences": list(sentences or []),
        })
        return self

    def reading(self, reading: str, text: str, *, kind="word", require_separators=False):
        """Chinese: apostrophe-delimited Pinyin (v for ü); Japanese: Hiragana."""
        self.document["lexicon"].setdefault("readings", []).append({
            "reading": reading, "text": text, "kind": kind,
            "require_separators": require_separators,
        })
        return self

    def continuation(self, context: str, *sentences: str):
        """Chinese/Japanese: each full sentence must start with the literal context."""
        self.document["lexicon"].setdefault("continuations", []).append([context, list(sentences)])
        return self

    def phrase_ending(self, context: str, *endings: str):
        """English: suffixes, not full sentences."""
        self.document["lexicon"].setdefault("phrase_endings", []).append([context, list(endings)])
        return self

    def build(self, destination, *, tool="suzaku_tool") -> Path:
        return build(self.document, destination, tool=tool)


def build(document: dict, destination, *, tool="suzaku_tool") -> Path:
    """Validate and publish without overwriting. Never installs/enables the result.

    The validator executable is explicit; it receives only our private temporary
    file. Failures leave an existing destination unchanged. No shell is invoked.
    """
    data = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    if len(data) > 256 * 1024:
        raise ValueError("offline pack exceeds 256 KiB")
    destination = Path(destination)
    # Parent must already exist: avoid creating a directory tree on a typo.
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".suzaku-pack-", suffix=".json",
                                         dir=destination.parent, delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        result = subprocess.run([str(tool), "pack", "validate", str(temporary)],
                                capture_output=True, text=True, check=False, timeout=30)
        if result.returncode:
            raise ValueError(result.stderr.strip() or "Suzaku pack validation failed")
        # Atomic no-clobber, including concurrent builders and destination links.
        os.link(temporary, destination)
        return destination
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON field: {key}")
        value[key] = item
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Authored pack JSON")
    parser.add_argument("destination", type=Path, help="New distributable JSON; must not exist")
    parser.add_argument("--tool", default="suzaku_tool", help="Path to suzaku_tool / suzaku-tool")
    args = parser.parse_args()
    try:
        # Do not follow a final symlink or block indefinitely on a FIFO on Linux.
        flags = os.O_RDONLY | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_NOFOLLOW", 0)
        with os.fdopen(os.open(args.source, flags), "rb") as stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                raise ValueError("source must be a regular JSON file")
            raw = stream.read(256 * 1024 + 1)
        if len(raw) > 256 * 1024:
            raise ValueError("offline pack exceeds 256 KiB")
        document = json.loads(raw, object_pairs_hook=unique_object)
        print(build(document, args.destination, tool=args.tool))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
