#!/usr/bin/env python3
"""List open issues labelled `approved` that a worker can take now.

Prints JSON with the candidates first (oldest first, issues assigned to me
first). Excluded issues are listed separately with the reason.

`approved` is the only approval. An issue without that label is never a
candidate, whoever filed it.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

import worker_state

EXCLUDED_LABELS = {"wontfix", "duplicate", "invalid", "question"}
APPROVED_LABEL = "approved"
CLOSING_PATTERN = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)\b", re.I)
# Branch names end with the issue number (`fix/keychain-fallback-stale-2588`).
BRANCH_REF_PATTERN = re.compile(r"-(\d{3,})\b")


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
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


def exclusion_reason(
    number: int,
    labels: set[str],
    assignees: list[str],
    me: str,
    closing_pr: dict[int, list[int]],
    claimed_by: str | None = None,
) -> str | None:
    """Why an open issue is not a candidate, or None when it is."""
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
    claims = worker_state.load_claims()
    me = gh("api", "user", "--jq", ".login").strip()
    issues = json.loads(
        gh("issue", "list", "--state", "open", "--limit", str(args.limit), "--json",
           "number,title,labels,assignees,url,updatedAt")
    )
    closing_pr = closing_pr_map(list_open_prs())

    candidates, excluded = [], []
    for issue in issues:
        number = issue["number"]
        labels = {label["name"] for label in issue["labels"]}
        assignees = [a["login"] for a in issue["assignees"]]

        reason = exclusion_reason(
            number, labels, assignees, me, closing_pr,
            claimed_by=worker_state.claimed_by_other(claims, f"issue:{number}", worker),
        )
        if reason:
            excluded.append({"number": number, "title": issue["title"], "reason": reason})
            continue
        candidates.append({
            "number": number,
            "title": issue["title"],
            "url": issue["url"],
            "labels": sorted(labels),
            "assignedToMe": me in assignees,
            "updatedAt": issue["updatedAt"],
        })

    # Issues assigned to me first, then oldest (lowest number) first.
    candidates.sort(key=lambda entry: (not entry["assignedToMe"], entry["number"]))
    excluded.sort(key=lambda entry: entry["number"])
    print(json.dumps({"candidates": candidates, "excluded": excluded}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
