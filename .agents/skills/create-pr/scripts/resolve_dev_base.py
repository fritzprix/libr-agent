#!/usr/bin/env python3
"""Print the latest origin/dev/<version> branch name (e.g. dev/0.8.x)."""

from __future__ import annotations

import re
import subprocess
import sys


VERSION_RE = re.compile(
    r"^origin/dev/(?P<maj>\d+)\.(?P<min>\d+)\.(?P<pat>\d+|x)$"
)


def version_key(remote_ref: str) -> tuple[int, int, int] | None:
    match = VERSION_RE.match(remote_ref.strip())
    if not match:
        return None
    patch = match.group("pat")
    return (
        int(match.group("maj")),
        int(match.group("min")),
        10**9 if patch == "x" else int(patch),
    )


def list_dev_remotes() -> list[str]:
    result = subprocess.run(
        ["git", "branch", "-r", "--list", "origin/dev/*"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def main() -> int:
    scored: list[tuple[tuple[int, int, int], str]] = []
    for ref in list_dev_remotes():
        key = version_key(ref)
        if key is None:
            continue
        # Strip origin/ prefix for gh --base / human use
        scored.append((key, ref.removeprefix("origin/")))

    if not scored:
        print("No origin/dev/<version> branches found. Fetch first.", file=sys.stderr)
        return 1

    scored.sort(key=lambda item: item[0])
    print(scored[-1][1])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
