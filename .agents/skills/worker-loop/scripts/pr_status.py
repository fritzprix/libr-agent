#!/usr/bin/env python3
"""Summarize actionable state for the current user's open PRs.

Prints JSON: one entry per open PR authored by @me with merge state, failing
and pending checks, unresolved review threads, and PR comments from others
that arrived after the latest commit, plus the latest gatekeeper-loop verdict
(own marker comments only). A PR claimed by another worker yields `claimed`.
Bodies are truncated; fetch the full text only for items you act on.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import worker_claims
import worker_state

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

from comment_markers import START_MARKER  # noqa: E402
from pr_turn import decide_pr_turn, format_turn_report  # noqa: E402

THREADS_QUERY = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $cursor) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id isResolved isOutdated path line
          comments(first: 1) { nodes { author { login } body url } }
        }
      }
    }
  }
}
"""

FAILED = {"FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"}
PENDING = {"PENDING", "QUEUED", "IN_PROGRESS", "WAITING", "EXPECTED", "REQUESTED"}


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def short(text: str | None, limit: int = 240) -> str:
    text = " ".join((text or "").split())
    return text if len(text) <= limit else text[: limit - 1] + "…"


def check_state(check: dict) -> str:
    return (check.get("conclusion") or check.get("state") or check.get("status") or "").upper()


def gatekeeper_state(comments: list[dict], me: str, head_sha: str) -> dict | None:
    """Latest gatekeeper verdict (own comments only) and whether it awaits the worker."""
    verdict, responded_rounds = None, set()
    for c in comments:
        if (c.get("author") or {}).get("login") != me:
            continue
        match = START_MARKER.match(c.get("body") or "")
        if not match:
            continue
        fields = dict(kv.split("=", 1) for kv in match.group(2).split() if "=" in kv)
        if match.group(1) == "gatekeeper-loop":
            verdict = {**fields, "url": c.get("url"), "createdAt": c["createdAt"]}
        elif verdict and c["createdAt"] > verdict["createdAt"]:
            responded_rounds.add(fields.get("round"))
    if verdict is None:
        return None
    verdict["awaitingWorker"] = (
        verdict.get("verdict") == "changes-requested"
        and verdict.get("head") == head_sha
        and verdict.get("round") not in responded_rounds
    )
    return verdict


def select_next_action(
    *,
    turn: str,
    conflicting: bool,
    gatekeeper: dict | None,
    has_comments: bool,
    failing: list[str],
    pending: list[str],
) -> str:
    if turn == "error":
        return "no-owner"
    if conflicting:
        return "resolve-conflict"
    if turn == "worker" and gatekeeper and gatekeeper["awaitingWorker"]:
        return "address-gatekeeper"
    if turn == "gatekeeper":
        return "wait-gatekeeper"
    if turn == "human":
        return "wait-human"
    if has_comments:
        return "triage-comments"
    if failing:
        return "fix-ci"
    if pending:
        return "wait-ci"
    return "none"


def summarize(number: int, owner: str, name: str, me: str, claimed_by: str | None = None) -> dict:
    pr = json.loads(
        gh(
            "pr", "view", str(number), "--json",
            "number,title,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,"
            "mergeStateStatus,reviewDecision,statusCheckRollup,commits,comments,labels",
        )
    )
    comments = pr.get("comments") or []
    head_sha = pr["headRefOid"]
    labels = sorted(label["name"] for label in pr.get("labels") or [])
    gatekeeper = gatekeeper_state(comments, me, head_sha)
    turn = decide_pr_turn(
        head_sha=head_sha,
        mergeable=pr.get("mergeable"),
        comments=comments,
        me=me,
        labels=labels,
    )
    checks = pr.get("statusCheckRollup") or []
    failing = [c.get("name") or c.get("context") for c in checks if check_state(c) in FAILED]
    pending = [c.get("name") or c.get("context") for c in checks if check_state(c) in PENDING]

    threads = []
    cursor = None
    while True:
        args = ["api", "graphql", "-f", f"query={THREADS_QUERY}", "-F", f"owner={owner}",
                "-F", f"name={name}", "-F", f"number={number}"]
        if cursor:
            args.extend(["-F", f"cursor={cursor}"])
        threads_raw = json.loads(gh(*args))
        review_threads = (
            threads_raw.get("data", {}).get("repository", {}).get("pullRequest", {}).get("reviewThreads") or {}
        )
        nodes = review_threads.get("nodes") or []
        for node in nodes:
            if node["isResolved"]:
                continue
            first = (node["comments"]["nodes"] or [{}])[0]
            threads.append({
                "threadId": node["id"],
                "outdated": node["isOutdated"],
                "path": node["path"],
                "line": node["line"],
                "author": (first.get("author") or {}).get("login"),
                "body": short(first.get("body")),
                "url": first.get("url"),
            })
        page_info = review_threads.get("pageInfo") or {}
        if not page_info.get("hasNextPage"):
            break
        cursor = page_info.get("endCursor")

    commits = pr.get("commits") or []
    last_commit_at = commits[-1]["committedDate"] if commits else ""
    # Filter comments: keep non-bot comments posted after the latest commit that do NOT
    # carry loop markers. This includes human owner comments even when the owner shares login @me.
    new_comments = []
    for c in pr.get("comments") or []:
        author_login = (c.get("author") or {}).get("login") or ""
        body = c.get("body") or ""
        if author_login.endswith("[bot]") or START_MARKER.match(body):
            continue
        if c.get("createdAt", "") > last_commit_at:
            new_comments.append({
                "author": author_login,
                "createdAt": c["createdAt"],
                "body": short(body),
                "url": c.get("url"),
            })

    next_action = select_next_action(
        turn=turn.turn,
        conflicting=pr.get("mergeable") == "CONFLICTING",
        gatekeeper=gatekeeper,
        has_comments=bool(threads or new_comments),
        failing=failing,
        pending=pending,
    )
    if claimed_by:
        next_action = "claimed"

    return {
        "number": pr["number"],
        "title": pr["title"],
        "url": pr["url"],
        "head": pr["headRefName"],
        "base": pr["baseRefName"],
        "draft": pr["isDraft"],
        "mergeable": pr.get("mergeable"),
        "mergeState": pr.get("mergeStateStatus"),
        "reviewDecision": pr.get("reviewDecision"),
        "failingChecks": failing,
        "pendingChecks": pending,
        "unresolvedThreads": threads,
        "newComments": new_comments,
        "gatekeeper": gatekeeper,
        "whoseTurn": turn.turn,
        "currentVerdict": turn.currentVerdict,
        "verdictHeadSha": turn.verdictHead,
        "currentHeadSha": head_sha,
        "labelMismatch": turn.labelMismatch,
        "noOwnerError": turn.noOwner,
        "turnReport": format_turn_report(turn, pr_number=pr["number"]),
        "claimedBy": claimed_by,
        "nextAction": next_action,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, help="Only this PR number")
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID or 'worker')")
    args = parser.parse_args()
    worker = worker_state.worker_name(args.worker)
    claims = worker_claims.load_claims()

    repo = json.loads(gh("repo", "view", "--json", "owner,name"))
    owner, name = repo["owner"]["login"], repo["name"]
    me = gh("api", "user", "--jq", ".login").strip()

    if args.pr:
        numbers = [args.pr]
    else:
        listed = json.loads(gh("pr", "list", "--author", "@me", "--state", "open", "--json", "number,isDraft"))
        numbers = [p["number"] for p in listed if not p.get("isDraft")]

    summaries = [
        summarize(n, owner, name, me, worker_claims.claimed_by_other(claims, f"pr:{n}", worker))
        for n in numbers
    ]
    print(json.dumps(summaries, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
