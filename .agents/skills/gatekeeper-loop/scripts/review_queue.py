#!/usr/bin/env python3
"""List open PRs by @me with their gatekeeper review status.

Verdicts and worker responses are PR comments authored by @me that carry a
hidden marker (see references/comment-format.md). Markers from any other
author are ignored.

status values:
  review     - new head since the last verdict, a worker response to it, or
               never reviewed. Approve only once ciPending is empty.
  wait-worker- changes requested at the current head, or CI failing / conflicts
  approved   - approve verdict exists for the current head
  escalate   - round limit reached; hand to the user
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

MAX_ROUNDS = 5
MARKER = re.compile(r"<!--\s*(gatekeeper-loop|worker-loop):\s*(.*?)\s*-->")
FAILED = {"FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"}
PENDING = {"PENDING", "QUEUED", "IN_PROGRESS", "WAITING", "EXPECTED", "REQUESTED"}


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def check_state(check: dict) -> str:
    return (check.get("conclusion") or check.get("state") or check.get("status") or "").upper()


def parse_markers(comments: list[dict], me: str) -> tuple[list[dict], list[dict]]:
    verdicts, responses = [], []
    for c in comments:
        if (c.get("author") or {}).get("login") != me:
            continue
        match = MARKER.search(c.get("body") or "")
        if not match:
            continue
        fields = dict(kv.split("=", 1) for kv in match.group(2).split() if "=" in kv)
        entry = {**fields, "createdAt": c["createdAt"], "url": c.get("url")}
        (verdicts if match.group(1) == "gatekeeper-loop" else responses).append(entry)
    return verdicts, responses


def summarize(number: int, me: str) -> dict:
    pr = json.loads(
        gh("pr", "view", str(number), "--json",
           "number,title,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,"
           "statusCheckRollup,comments,labels")
    )
    checks = pr.get("statusCheckRollup") or []
    failing = [c.get("name") or c.get("context") for c in checks if check_state(c) in FAILED]
    pending = [c.get("name") or c.get("context") for c in checks if check_state(c) in PENDING]
    verdicts, responses = parse_markers(pr.get("comments") or [], me)

    head = pr["headRefOid"]
    last = verdicts[-1] if verdicts else None
    response = None
    if last:
        response = next(
            (r for r in reversed(responses)
             if r.get("round") == last.get("round") and r["createdAt"] > last["createdAt"]),
            None,
        )

    if pr.get("mergeable") == "CONFLICTING" or failing:
        status = "wait-worker"
    elif last and last.get("head") == head and last.get("verdict") == "approve":
        status = "approved"
    elif last and last.get("head") == head and response is None:
        status = "wait-worker"
    elif len(verdicts) >= MAX_ROUNDS:
        status = "escalate"
    else:
        status = "review"

    return {
        "number": pr["number"],
        "title": pr["title"],
        "url": pr["url"],
        "head": pr["headRefName"],
        "headSha": head,
        "base": pr["baseRefName"],
        "draft": pr["isDraft"],
        "labels": sorted(label["name"] for label in pr.get("labels") or []),
        "status": status,
        "round": len(verdicts) + (1 if status == "review" else 0),
        "lastVerdict": last,
        "reviewedHead": last.get("head") if last else None,
        "workerResponse": response,
        "ciPending": pending,
        "ciFailing": failing,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, help="Only this PR number")
    args = parser.parse_args()

    me = gh("api", "user", "--jq", ".login").strip()
    if args.pr:
        numbers = [args.pr]
    else:
        listed = json.loads(gh("pr", "list", "--author", "@me", "--state", "open",
                               "--json", "number,isDraft"))
        numbers = [p["number"] for p in listed if not p["isDraft"]]

    print(json.dumps([summarize(n, me) for n in numbers], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
