#!/usr/bin/env python3
"""GitHub issue dependencies for worker-loop and gatekeeper-loop.

The only module that may call `gh` for blocked-by links (issue #2995).

- `blocked_by(number)` — the blockers of an issue: `{"number", "state"}` entries.
- `add_blocker(number, blocker_number)` — add a blocker link, idempotent.
- `open_blockers(number)` — the blocker numbers whose state is `open`.
- `closing_refs(pr)` — issue numbers closed by `pr` (title/body keywords + branch suffix).
- `pr_blockers(pr)` / `pr_open_blockers(pr)` — blockers across `pr`'s closing issues.

A missing item is an error: a 404 is `DepsError`, never an empty list. Writes
use the blocker's integer `id` (from `GET /repos/{owner}/{repo}/issues/N`),
not its number.

GitHub accepts the write for **issues only** (issue #2995): a PR number as
source or target is refused with HTTP 422 ("Source/Target issue may only be
an issue"). To park a PR, add the blocked-by link to the PR's closing issue
(`issue_deps.py add <closing-issue> <blocker-issue>`) and read it via
`pr_blockers(pr)` / `open_blockers(closing_issue)`.
"""
from __future__ import annotations

import json
import re
import subprocess
from typing import Callable

CLOSING_PATTERN = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)\b", re.I)
# Branch names end with the issue number (`fix/keychain-fallback-stale-2588`).
BRANCH_REF_PATTERN = re.compile(r"-(\d{3,})\b")


def closing_refs(pr: dict) -> set[int]:
    """Issues this PR closes: `Fixes #N` style keywords in title/body, plus the branch number.

    Single source of truth shared by `issue_candidates.py`, `tick_gate.py`,
    `pr_status.py`, and `review_queue.py`.
    """
    text = f"{pr.get('title') or ''} {pr.get('body') or ''}"
    refs = {int(n) for n in CLOSING_PATTERN.findall(text)}
    refs |= {int(n) for n in BRANCH_REF_PATTERN.findall(pr.get("headRefName") or "")}
    return refs


class DepsError(RuntimeError):
    """Raised when a dependency API call fails (including 404)."""


def _default_gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise DepsError(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def _call(run_gh: Callable[..., str], *args: str) -> str:
    """Run the injected `gh`, wrapping foreign errors (`GhError` etc.) as `DepsError`."""
    try:
        return run_gh(*args)
    except DepsError:
        raise
    except Exception as e:
        raise DepsError(str(e)) from e


def _is_not_found(error: DepsError) -> bool:
    return "HTTP 404" in str(error)


def _api_path(number: int, suffix: str) -> str:
    return f"repos/{{owner}}/{{repo}}/issues/{number}{suffix}"


def blocked_by(number: int, run_gh: Callable[..., str] = _default_gh) -> list[dict]:
    """Blockers of issue or PR `number`: `[{"number": int, "state": "open"|"closed"}]`."""
    try:
        out = _call(run_gh, "api", _api_path(number, "/dependencies/blocked_by"))
    except DepsError as e:
        if _is_not_found(e):
            raise DepsError(
                f"#{number} not found; a missing item is an error, not an empty blocker list"
            ) from e
        raise
    return [
        {"number": int(b["number"]), "state": b.get("state") or ""}
        for b in json.loads(out or "[]")
    ]


def open_blockers(number: int, run_gh: Callable[..., str] = _default_gh) -> list[int]:
    """Blocker numbers of issue `number` whose state is `open`."""
    return [b["number"] for b in blocked_by(number, run_gh) if b["state"] == "open"]


def pr_blockers(pr: dict, run_gh: Callable[..., str] = _default_gh) -> list[dict]:
    """Deduplicated blockers across all closing issues of `pr`."""
    seen: dict[int, dict] = {}
    for issue_num in sorted(closing_refs(pr)):
        for b in blocked_by(issue_num, run_gh):
            prev = seen.get(b["number"])
            if prev is None or b["state"] == "open":
                seen[b["number"]] = b
    return [seen[k] for k in sorted(seen)]


def pr_open_blockers(pr: dict, run_gh: Callable[..., str] = _default_gh) -> list[int]:
    """Open blocker numbers across all closing issues of `pr`."""
    return [b["number"] for b in pr_blockers(pr, run_gh) if b["state"] == "open"]


def add_blocker(number: int, blocker_number: int, run_gh: Callable[..., str] = _default_gh) -> None:
    """Record `blocker_number` as blocking `number`. Safe to call twice.

    The write needs the blocker's integer `id`, not its number; a PR shares the
    issue number space so `blocker_number` may be a PR number.
    """
    existing = {b["number"] for b in blocked_by(number, run_gh)}
    if blocker_number in existing:
        return
    try:
        blocker = json.loads(_call(run_gh, "api", _api_path(blocker_number, "")))
        blocker_id = blocker["id"]
    except DepsError as e:
        if _is_not_found(e):
            raise DepsError(f"blocker #{blocker_number} not found") from e
        raise
    if not isinstance(blocker_id, int):
        raise DepsError(f"blocker #{blocker_number} has no integer id: {blocker_id!r}")
    try:
        _call(
            run_gh,
            "api",
            "--method",
            "POST",
            _api_path(number, "/dependencies/blocked_by"),
            "-F",
            f"issue_id={blocker_id}",
        )
    except DepsError as e:
        if _is_not_found(e):
            raise DepsError(
                f"#{number} not found; a missing item is an error, not an empty blocker list"
            ) from e
        raise


def _main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="Shared GitHub issue-dependency helper (issue #2995).")
    sub = parser.add_subparsers(dest="command", required=True)
    p_add = sub.add_parser("add", help="Record that blocker ISSUE blocks NUMBER (idempotent)")
    p_add.add_argument("number", type=int)
    p_add.add_argument("issue", type=int)
    p_list = sub.add_parser("open", help="Print open blocker numbers of NUMBER, one per line")
    p_list.add_argument("number", type=int)
    p_show = sub.add_parser("list", help="Print all blockers of NUMBER as JSON")
    p_show.add_argument("number", type=int)
    args = parser.parse_args()
    if args.command == "add":
        add_blocker(args.number, args.issue)
    elif args.command == "open":
        for n in open_blockers(args.number):
            print(n)
    else:
        print(json.dumps(blocked_by(args.number), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    _main()
