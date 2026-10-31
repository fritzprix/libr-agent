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

EXCLUDED_LABELS = {"wontfix", "duplicate", "invalid", "question"}
DEPRIORITIZED_LABELS = {"backlog"}
REF_PATTERN = re.compile(r"(?<![\w/])#(\d+)\b")
EXTERNAL_REF_PATTERN = re.compile(r"\b[\w.-]+/[\w.-]+#\d+\b")
CLOSING_PATTERN = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)\b", re.I)
BLOCKED_PATTERN = re.compile(r"\b(blocked by|depends on|waiting (?:for|on)|after #\d+ (?:merges|lands))\b", re.I)
UMBRELLA_PATTERN = re.compile(r"\b(umbrella|tracking issue|epic)\b", re.I)


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=100, help="Candidate issues limit (default: 100)")
    args = parser.parse_args()

    me = gh("api", "user", "--jq", ".login").strip()
    issues = json.loads(
        gh("issue", "list", "--state", "open", "--limit", str(args.limit), "--json",
           "number,title,labels,assignees,body,createdAt,updatedAt,url")
    )
    open_prs = json.loads(
        gh("pr", "list", "--state", "open", "--limit", "100", "--json", "number,title,body,headRefName")
    )
    open_issue_numbers = {i["number"] for i in issues}
    open_pr_numbers = {p["number"] for p in open_prs}

    closing_pr: dict[int, list[int]] = {}
    mentioned_by_pr: dict[int, list[int]] = {}
    for pr in open_prs:
        text = f"{pr['title']} {pr.get('body') or ''}"
        closes = {int(n) for n in CLOSING_PATTERN.findall(text)}
        closes |= {int(n) for n in re.findall(r"-(\d{3,})\b", pr["headRefName"])}
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

        reason = None
        if labels & EXCLUDED_LABELS:
            reason = f"label: {', '.join(sorted(labels & EXCLUDED_LABELS))}"
        elif assignees and me not in assignees:
            reason = f"assigned to {', '.join(assignees)}"
        elif number in closing_pr:
            reason = f"closed by open PR #{', #'.join(map(str, closing_pr[number]))}"

        entry = {
            "number": number,
            "title": issue["title"],
            "url": issue["url"],
            "labels": sorted(labels),
            "assignedToMe": me in assignees,
            "checkboxes": {"total": len(checkboxes), "done": sum(c.lower() == "x" for c in checkboxes)},
            "bodyChars": len(body),
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
