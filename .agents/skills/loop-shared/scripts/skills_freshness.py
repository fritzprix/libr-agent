#!/usr/bin/env python3
"""Verify the checkout's .agents/skills matches origin/<base> before a loop tick."""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

SKILLS_PREFIX = ".agents/skills"

EXIT_FRESH = 0
EXIT_STALE = 1
EXIT_ERROR = 2


def _repo_root() -> Path:
    out = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True,
    )
    return Path(out.stdout.strip())


def resolve_base_branch(base: str | None) -> str:
    if base:
        return base
    resolve = _repo_root() / ".agents/skills/create-pr/scripts/resolve_dev_base.py"
    if resolve.is_file():
        proc = subprocess.run(
            [sys.executable, str(resolve)],
            capture_output=True,
            text=True,
        )
        if proc.returncode == 0 and proc.stdout.strip():
            return proc.stdout.strip()
    return "main"


def check_skills_fresh(
    base: str,
    *,
    fetch: bool = True,
) -> tuple[bool, str]:
    """Return (True, message) when HEAD skills tree matches origin/base."""
    root = _repo_root()
    if fetch:
        fetch_proc = subprocess.run(
            ["git", "fetch", "origin", base, "--quiet"],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if fetch_proc.returncode != 0:
            return False, f"git fetch origin {base} failed: {fetch_proc.stderr.strip()}"

    remote = f"origin/{base}"
    diff = subprocess.run(
        ["git", "diff", "--quiet", "HEAD", remote, "--", SKILLS_PREFIX],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if diff.returncode == 0:
        return True, f"skills match {remote}"
    if diff.returncode == 1:
        return (
            False,
            f"skills under {SKILLS_PREFIX} differ from {remote}; "
            f"merge or reset skills before this tick",
        )
    return False, diff.stderr.strip() or "git diff failed"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base",
        default=None,
        help="Base branch on origin (default: latest dev/* from resolve_dev_base.py)",
    )
    parser.add_argument(
        "--no-fetch",
        action="store_true",
        help="Skip git fetch (caller already fetched)",
    )
    args = parser.parse_args(argv)
    base = resolve_base_branch(args.base)
    try:
        fresh, message = check_skills_fresh(base, fetch=not args.no_fetch)
    except subprocess.CalledProcessError as exc:
        print(exc.stderr or str(exc), file=sys.stderr)
        return EXIT_ERROR
    if fresh:
        print(message)
        return EXIT_FRESH
    print(message, file=sys.stderr)
    return EXIT_STALE


if __name__ == "__main__":
    raise SystemExit(main())
