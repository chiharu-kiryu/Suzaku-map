"""Reject shared Compose files before an isolated native test starts IBus.

IBus may migrate XCOMPOSEFILE in place and create a backup next to it. The
runner must supply a private copy, not a source fixture, symlink or hard link.
This guard never reads or changes the user's Compose configuration.
"""
import os
from pathlib import Path


def require_private_compose(root, environment):
    root = Path(root)
    if (root.parent != Path("/tmp") or not root.is_dir() or root.is_symlink()
            or root.stat().st_uid != os.getuid()):
        raise ValueError("Compose QA root must be an owned private directory")
    path = Path(environment.get("XCOMPOSEFILE", ""))
    if path != root / "compose.XCompose" or path.is_symlink() or not path.is_file():
        raise ValueError("Compose QA requires its private compose.XCompose copy")
    metadata = path.stat()
    if metadata.st_uid != os.getuid() or metadata.st_nlink != 1:
        raise ValueError("Compose QA file must be owned and must not be hard-linked")
    return path
