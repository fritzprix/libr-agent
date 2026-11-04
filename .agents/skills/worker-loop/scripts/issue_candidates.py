#!/usr/bin/env python3
"""List open issues with the signals needed to pick the next one to work on.

Prints JSON sorted with the most likely candidates first. The ordering is a
hint only; read the top candidates before choosing. Excluded issues are listed
separately with the reason.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

import worker_state

EXCLUDED_LABELS = {"wontfix", "duplicate", "invalid", "question"}
# Issues filed by producer-loop need a human `approved` label; `validated` alone is not enough.
PRODUCER_LABEL = "auto:producer"
APPROVED_LABEL = "approved"
DEPRIORITIZED_LABELS = {"backlog"}
REF_PATTERN = re.compile(r"(?<![\w/])#(\d+)\b")
EXTERNAL_REF_PATTERN = re.compile(r"\b[\w.-]+/[\w.-]+#\d+\b")
CLOSING_PATTERN = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)\b", re.I)
BLOCKED_PATTERN = re.compile(r"\b(blocked by|depends on|waiting (?:for|on)|after #\d+ (?:merges|lands))\b", re.I)
UMBRELLA_PATTERN = re.compile(r"\b(umbrella|tracking issue|epic)\b", re.I)
# "Depends on #N" / "Blocked by #N" clause; refs are read only up to the end of the sentence so
# "Depends on #1. It does not depend on #2." yields [1]. Negated forms ("does not depend on") never match.
DEPENDS_PATTERN = re.compile(r"\b(?:depends on|blocked by)\b:?(?P<refs>[^\n.]*)", re.I)


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def depends_on(body: str, number: int) -> list[int]:
    """Issue/PR numbers named in this issue's own `Depends on` / `Blocked by` clauses."""
    refs: set[int] = set()
    for match in DEPENDS_PATTERN.finditer(body):
        refs |= {int(n) for n in REF_PATTERN.findall(match.group("refs"))}
    refs.discard(number)
    return sorted(refs)


def open_dependencies(
    deps: list[int],
    open_issue_numbers: set[int],
    open_pr_numbers: set[int],
    closed_by_merged_pr: set[int],
) -> list[int]:
    """Dependencies still unresolved. A closed issue, a merged/closed PR, or an open issue whose
    closing PR has merged (merges into dev/* do not auto-close issues) counts as resolved."""
    return [
        d for d in deps
        if d in open_pr_numbers or (d in open_issue_numbers and d not in closed_by_merged_pr)
    ]


def closing_refs(pr: dict) -> set[int]:
    text = f"{pr['title']} {pr.get('body') or ''}"
    refs = {int(n) for n in CLOSING_PATTERN.findall(text)}
    refs |= {int(n) for n in re.findall(r"-(\d{3,})\b", pr["headRefName"])}
    return refs


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
    if labels & EXCLUDED_LABELS:
        return f"label: {', '.join(sorted(labels & EXCLUDED_LABELS))}"
    if PRODUCER_LABEL in labels and APPROVED_LABEL not in labels:
        state = "validated" if "validated" in labels else "rejected" if "rejected" in labels else "not yet validated"
        return f"{PRODUCER_LABEL} issue ({state}) waits for a human '{APPROVED_LABEL}' label"
    if assignees and me not in assignees:
        return f"assigned to {', '.join(assignees)}"
    if number in closing_pr:
        return f"closed by open PR #{', #'.join(map(str, closing_pr[number]))}"
    return None


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
           "number,title,labels,assignees,body,createdAt,updatedAt,url")
    )
    open_prs = json.loads(
        gh("pr", "list", "--state", "open", "--limit", "100", "--json", "number,title,body,headRefName")
    )
    merged_prs = json.loads(
        gh("pr", "list", "--state", "merged", "--limit", "100", "--json", "number,title,body,headRefName")
    )
    open_issue_numbers = {i["number"] for i in issues}
    open_pr_numbers = {p["number"] for p in open_prs}
    closed_by_merged_pr = set().union(*(closing_refs(pr) for pr in merged_prs))

    closing_pr: dict[int, list[int]] = {}
    mentioned_by_pr: dict[int, list[int]] = {}
    for pr in open_prs:
        text = f"{pr['title']} {pr.get('body') or ''}"
        closes = closing_refs(pr)
        for ref in closes:
            closing_pr.setdefault(ref, []).append(pr["number"])
        for ref in {int(n) for n in REF_PATTERN.findall(text)} - closes:
            mentioned_by_pr.setdefault(ref, []).append(pr["number"])

    candidates, excluded = [], []
    for issue in issues:
        number = issue["number"]
        body = issue.get("body") or ""
        labels = {label["name"] for label in issue["labels"]}
        assignees = [a["login"] for a in issue["assignees"]]
        refs = sorted({int(n) for n in REF_PATTERN.findall(body)} - {number})
        checkboxes = re.findall(r"^\s*[-*] \[( |x|X)\]", body, re.M)

        reason = exclusion_reason(
            number, labels, assignees, me, closing_pr,
            claimed_by=worker_state.claimed_by_other(claims, f"issue:{number}", worker),
        )
        deps = depends_on(body, number)
        unresolved = open_dependencies(deps, open_issue_numbers, open_pr_numbers, closed_by_merged_pr)

        entry = {
            "number": number,
            "title": issue["title"],
            "url": issue["url"],
            "labels": sorted(labels),
            "assignedToMe": me in assignees,
            "checkboxes": {"total": len(checkboxes), "done": sum(c.lower() == "x" for c in checkboxes)},
            "bodyChars": len(body),
            "dependsOn": deps,
            "openDependencies": unresolved,
            # Open PRs implementing an unresolved dependency: stack on one of them instead of skipping.
            "stackOn": sorted(
                {p for d in unresolved for p in closing_pr.get(d, [])}
                | {d for d in unresolved if d in open_pr_numbers}
            ),
            "startable": not unresolved,
            "refsOpenIssues": [r for r in refs if r in open_issue_numbers],
            "refsOpenPRs": [r for r in refs if r in open_pr_numbers],
            "mentionedByOpenPRs": mentioned_by_pr.get(number, []),
            "externalRefs": sorted(set(EXTERNAL_REF_PATTERN.findall(body))),
            "blockedHint": bool(BLOCKED_PATTERN.search(body)),
            "umbrellaHint": bool(UMBRELLA_PATTERN.search(issue["title"])),
            "updatedAt": issue["updatedAt"],
        }
        if reason:
            excluded.append({"number": number, "title": issue["title"], "reason": reason})
            continue
        candidates.append(entry)

    def rank(entry: dict) -> tuple:
        return (
            not entry["assignedToMe"],
            not entry["startable"],
            entry["blockedHint"],
            entry["umbrellaHint"],
            bool(entry["refsOpenPRs"] or entry["externalRefs"]),
            bool(set(entry["labels"]) & DEPRIORITIZED_LABELS),
            entry["checkboxes"]["total"] == 0,
            "bug" not in entry["labels"],
            entry["bodyChars"],
        )

    candidates.sort(key=rank)
    print(json.dumps({"candidates": candidates, "excluded": excluded}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
