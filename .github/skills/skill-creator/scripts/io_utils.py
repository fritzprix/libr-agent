"""Cross-platform UTF-8 helpers for skill-creator scripts."""

from __future__ import annotations

import sys
from pathlib import Path

UTF8 = "utf-8"


def configure_stdio() -> None:
    """Prefer UTF-8 on stdout/stderr when the stream supports reconfiguration."""
    for stream in (sys.stdout, sys.stderr):
        if stream is None:
            continue
        reconfigure = getattr(stream, "reconfigure", None)
        if not callable(reconfigure):
            continue
        try:
            reconfigure(encoding=UTF8, errors="replace")
        except (OSError, ValueError):
            pass


def read_text(path: Path | str) -> str:
    """Read a text file as UTF-8."""
    return Path(path).read_text(encoding=UTF8)


def write_text(path: Path | str, content: str) -> None:
    """Write a text file as UTF-8 with LF newlines."""
    Path(path).write_text(content, encoding=UTF8, newline="\n")
