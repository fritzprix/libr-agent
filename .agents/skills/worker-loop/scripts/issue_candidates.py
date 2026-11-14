#!/usr/bin/env python3
"""List open issues labelled `approved` that a worker can take now.

Prints JSON with the candidates first (oldest first, issues assigned to me
first). Excluded issues are listed separately with the reason.

`approved` is the only approval. An issue without that label is never a
candidate, whoever filed it. An approved issue still waits while it carries
`needs-owner` without an owner answer, or while an issue it depends on is open.
A `needs-owner` issue whose question was answered is a candidate with
`ownerAnswer` set: remove the label and follow that comment.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Callable

import worker_claims
import worker_state

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

from comment_markers import NEEDS_OWNER_LABEL, owner_answer  # noqa: E402

EXCLUDED_LABELS = {"wontfix", "duplicate", "invalid", "question", "epic", "auto:e2e"}
APPROVED_LABEL = "approved"
CLOSING_PATTERN = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)\b", re.I)
# Branch names end with the issue number (`fix/keychain-fallback-stale-2588`).
BRANCH_REF_PATTERN = re.compile(r"-(\d{3,})\b")
# `Depends on #1`, `Blocked by #1, #2, and #3`, `depends on #1 / #2`. The list
# ends at the first item that is not `#N`, so write dependencies as a plain list.
DEPENDENCY_PATTERN = re.compile(
    r"\b(?:depends\s+on|blocked\s+by)\s*:?\s*(#\d+(?:\s*(?:,|/|&|and)\s*(?:and\s+)?#\d+)*)", re.I
)


class GhError(RuntimeError):
    """Raised when GitHub CLI execution fails."""


def _is_not_found(error: GhError) -> bool:
    return "HTTP 404" in str(error)


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def closing_refs(pr: dict) -> set[int]:
    """Issues this PR closes: `Fixes #N` style keywords in title/body, plus the branch number.

    Shared with `tick_gate.py`, which must agree on what counts as an open PR for an issue.
    """
    text = f"{pr['title']} {pr.get('body') or ''}"
    refs = {int(n) for n in CLOSING_PATTERN.findall(text)}
    refs |= {int(n) for n in BRANCH_REF_PATTERN.findall(pr["headRefName"])}
    return refs


def closing_pr_map(prs: list[dict]) -> dict[int, list[int]]:
    """Issue number -> open PR numbers that close it."""
    closing: dict[int, list[int]] = {}
    for pr in prs:
        for ref in closing_refs(pr):
            closing.setdefault(ref, []).append(pr["number"])
    return closing


def dependency_refs(body: str | None) -> list[int]:
    """Issue numbers named after `Depends on` / `Blocked by` in an issue body, in order."""
    refs: list[int] = []
    for group in DEPENDENCY_PATTERN.findall(body or ""):
        for number in re.findall(r"#(\d+)", group):
            if int(number) not in refs:
                refs.append(int(number))
    return refs


def exclusion_reason(
    number: int,
    labels: set[str],
    assignees: list[str],
    me: str,
    closing_pr: dict[int, list[int]],
    claimed_by: str | None = None,
) -> str | None:
    """Why an open issue is not a candidate, from data already listed; None when it may be."""
    if claimed_by:
        return f"claimed by {claimed_by}"
    if APPROVED_LABEL not in labels:
        return f"no '{APPROVED_LABEL}' label"
    if labels & EXCLUDED_LABELS:
        return f"label: {', '.join(sorted(labels & EXCLUDED_LABELS))}"
    if assignees and me not in assignees:
        return f"assigned to {', '.join(assignees)}"
    if number in closing_pr:
        return f"closed by open PR #{', #'.join(map(str, closing_pr[number]))}"
    return None


def _open_blocked_by(number: int, run_gh: Callable[..., str]) -> list[int]:
    """Open issues GitHub records as blocking this one; empty where the API is unavailable (404)."""
    try:
        out = run_gh(
            "api", f"repos/{{owner}}/{{repo}}/issues/{number}/dependencies/blocked_by",
            "--jq", '[.[] | select(.state == "open") | .number]',
        )
    except GhError as e:
        if _is_not_found(e):
            return []
        raise
    return [int(n) for n in json.loads(out or "[]")]


def waiting_reason(
    issue: dict, labels: set[str], me: str, run_gh: Callable[..., str]
) -> tuple[str | None, dict | None]:
    """(reason, ownerAnswer) for an issue that passed `exclusion_reason`.

    A reason means the issue waits on the owner or on an open dependency.
    `ownerAnswer` is set when a `needs-owner` question has been answered.
    """
    number = issue["number"]
    answer = None
    if NEEDS_OWNER_LABEL in labels:
        comments = json.loads(run_gh("issue", "view", str(number), "--json", "comments"))["comments"]
        answer = owner_answer(comments, me)
        if answer is None:
            return f"{NEEDS_OWNER_LABEL}: waiting for the owner's answer", None

    blockers = []
    for dep in dependency_refs(issue.get("body")):
        try:
            state = run_gh("api", f"repos/{{owner}}/{{repo}}/issues/{dep}", "--jq", ".state").strip()
        except GhError as e:
            if _is_not_found(e):
                return f"dependency #{dep} not found", answer
            raise
        if state == "open":
            blockers.append(dep)
    blockers += [n for n in _open_blocked_by(number, run_gh) if n not in blockers]
    if blockers:
        return f"blocked by open #{', #'.join(map(str, blockers))}", answer
    return None, answer


def list_open_prs() -> list[dict]:
    return json.loads(
        gh("pr", "list", "--state", "open", "--limit", "100", "--json", "number,title,body,headRefName")
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=100, help="Candidate issues limit (default: 100)")
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID or 'worker')")
    args = parser.parse_args()

    worker = worker_state.worker_name(args.worker)
    claims = worker_claims.load_claims()
    try:
        me = gh("api", "user", "--jq", ".login").strip()
        issues = json.loads(
            gh("issue", "list", "--state", "open", "--limit", str(args.limit), "--json",
               "number,title,body,labels,assignees,url,updatedAt")
        )
        closing_pr = closing_pr_map(list_open_prs())

        candidates, excluded = [], []
        for issue in issues:
            number = issue["number"]
            labels = {label["name"] for label in issue["labels"]}
            assignees = [a["login"] for a in issue["assignees"]]

            reason = exclusion_reason(
                number, labels, assignees, me, closing_pr,
                claimed_by=worker_claims.claimed_by_other(claims, f"issue:{number}", worker),
            )
            answer = None
            if reason is None:
                reason, answer = waiting_reason(issue, labels, me, gh)
            if reason:
                excluded.append({"number": number, "title": issue["title"], "reason": reason})
                continue
            candidates.append({
                "number": number,
                "title": issue["title"],
                "url": issue["url"],
                "labels": sorted(labels),
                "assignedToMe": me in assignees,
                "ownerAnswer": answer,
                "updatedAt": issue["updatedAt"],
            })
    except GhError as e:
        sys.exit(str(e))

    # Issues assigned to me first, then oldest (lowest number) first.
    candidates.sort(key=lambda entry: (not entry["assignedToMe"], entry["number"]))
    excluded.sort(key=lambda entry: entry["number"])
    print(json.dumps({"candidates": candidates, "excluded": excluded}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
