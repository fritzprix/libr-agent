#!/usr/bin/env python3
"""Verify the checkout's .agents/skills matches origin/<base> before a loop tick."""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

SKILLS_PREFIX = ".agents/skills"

# CLI exit codes.
EXIT_FRESH = 0
EXIT_STALE = 1
EXIT_ERROR = 2

# Exit codes the loop gates return to their own process (tick_gate / producer_gate).
GATE_STALE_EXIT = 5
GATE_ERROR_EXIT = 3

# Tests set this to make the shared gate skip the check; the CLI never skips.
SKIP_GATE_ENV = "LIBRAGENT_SKILLS_FRESHNESS_SKIP"


class SkillsFreshnessError(RuntimeError):
    """The check could not run (dev base unresolvable or a git command failed).

    This is an error, not a stale verdict: network or git failures must not
    be reported as "skills stale".
    """


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
    raise SkillsFreshnessError(
        "could not resolve the dev base branch "
        "(create-pr/scripts/resolve_dev_base.py failed); pass --base explicitly"
    )


def check_skills_fresh(
    base: str,
    *,
    fetch: bool = True,
) -> tuple[bool, str]:
    """Return (True, message) when HEAD skills tree matches origin/base.

    Raise SkillsFreshnessError when the check itself cannot run (fetch or
    diff failure); that is an error, not a stale verdict.
    """
    root = _repo_root()
    if fetch:
        fetch_proc = subprocess.run(
            ["git", "fetch", "origin", base, "--quiet"],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if fetch_proc.returncode != 0:
            raise SkillsFreshnessError(
                f"git fetch origin {base} failed: {fetch_proc.stderr.strip()}"
            )

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
    raise SkillsFreshnessError(diff.stderr.strip() or "git diff failed")


def gate_skills_freshness() -> int | None:
    """Shared loop-tick gate: the exit code when the tick must stop, or None.

    Stale skills exit GATE_STALE_EXIT; a check that could not run (unresolved
    dev base, failed fetch, _repo_root CalledProcessError) exits GATE_ERROR_EXIT.
    """
    if os.environ.get(SKIP_GATE_ENV):
        return None
    try:
        fresh, message = check_skills_fresh(resolve_base_branch(None), fetch=True)
    except (SkillsFreshnessError, subprocess.CalledProcessError) as exc:
        print(f"skills freshness check error: {exc}", file=sys.stderr)
        return GATE_ERROR_EXIT
    if fresh:
        return None
    print(message, file=sys.stderr)
    return GATE_STALE_EXIT


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
    try:
        base = resolve_base_branch(args.base)
        fresh, message = check_skills_fresh(base, fetch=not args.no_fetch)
    except (SkillsFreshnessError, subprocess.CalledProcessError) as exc:
        print(str(exc), file=sys.stderr)
        return EXIT_ERROR
    if fresh:
        print(message)
        return EXIT_FRESH
    print(message, file=sys.stderr)
    return EXIT_STALE


if __name__ == "__main__":
    raise SystemExit(main())
