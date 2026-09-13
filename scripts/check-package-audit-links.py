#!/usr/bin/env python3
"""Check offline audit-report links in this repository's already-extracted packages.

This is not a generic Markdown renderer or an untrusted-archive validator. Source
code references may require the repository; linked audit reports must ship locally.
"""
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


def audit_links(text):
    # Ignore examples inside fenced code, but support both inline and reference
    # links, multiple links per line, relative directory prefixes and fragments.
    prose = []
    fence = None
    for line in text.splitlines():
        match = re.match(r"^\s*(`{3,}|~{3,})", line)
        if match:
            marker = match[1]
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence):
                fence = None
        elif fence is None:
            prose.append(line)
    text = "\n".join(prose)
    targets = re.findall(r"\]\(([^\s)]+)\)", text)
    targets += re.findall(r"^\[[^\]]+\]:\s+(\S+)\s*$", text, re.MULTILINE)
    for target in targets:
        uri = urlsplit(target)
        if uri.scheme or uri.netloc:
            continue
        path = Path(unquote(uri.path))
        if re.fullmatch(r"bug-audit-[\w.-]+\.md", path.name):
            yield path


def main():
    if len(sys.argv) < 2:
        raise SystemExit("Usage: check-package-audit-links.py DOC_ROOT [EXTRA_MARKDOWN ...]")
    root = Path(sys.argv[1])
    if not (root / "docs/functional-network.md").is_file():
        raise SystemExit("Missing packaged functional network")
    documents = sorted(root.glob("*.md")) + sorted((root / "docs").rglob("*.md"))
    documents += [Path(name) for name in sys.argv[2:]]
    missing = []
    checked = 0
    for document in documents:
        for target in audit_links(document.read_text(encoding="utf-8")):
            checked += 1
            resolved = document.parent / target
            if not resolved.is_file() or resolved.stat().st_size == 0:
                missing.append(f"Missing packaged audit document: {document}: {target}")
    if missing:
        raise SystemExit("\n".join(missing))
    if checked == 0:
        raise SystemExit("No packaged audit links were checked")
    print(f"PASS: {checked} packaged audit links resolve, including flattened guides.")


if __name__ == "__main__":
    main()
