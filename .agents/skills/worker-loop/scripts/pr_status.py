#!/usr/bin/env python3
"""Summarize actionable state for the current user's open PRs.

Prints JSON: one entry per open PR authored by @me with merge state, failing
and pending checks, unresolved review threads, and PR comments from others
that arrived after the latest commit, plus the latest gatekeeper-loop verdict
(own marker comments only). A PR claimed by another worker yields `claimed`;
a PR labelled `needs-owner` with no owner answer yet yields `wait-owner`.
`ciAttempts` counts the `<!-- worker-loop: ci-attempt n=K -->` comments.
A PR whose base moved far enough (see base_drift_needs_rebase) yields
`rebase-base`. `mergeable == UNKNOWN` is refetched with a short backoff; if it
stays UNKNOWN, `mergeableUnknown` is true and the turn report says so.
Bodies are truncated; fetch the full text only for items you act on.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections.abc import Callable, Iterable
from pathlib import Path

import worker_claims
import worker_state

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

from comment_markers import NEEDS_OWNER_LABEL, START_MARKER, ci_attempts, owner_answer  # noqa: E402
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
PR_FIELDS = (
    "number,title,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,"
    "mergeStateStatus,reviewDecision,statusCheckRollup,commits,comments,labels,files"
)
# Base commits missing from the PR before the worker rebases it. GitHub reports
# mergeStateStatus BEHIND only when branch protection requires up-to-date
# branches, so drift is measured with the compare API as well.
BASE_DRIFT_COMMITS = 10
UNKNOWN_RETRY_DELAYS = (2.0, 4.0, 8.0)


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
        elif verdict and c["createdAt"] > verdict["createdAt"] and "round" in fields:
            responded_rounds.add(fields["round"])
    if verdict is None:
        return None
    verdict["awaitingWorker"] = (
        verdict.get("verdict") == "changes-requested"
        and verdict.get("head") == head_sha
        and verdict.get("round") not in responded_rounds
    )
    return verdict


def base_drift_needs_rebase(
    *,
    merge_state: str | None,
    base_ahead_by: int,
    base_files: Iterable[str],
    pr_files: Iterable[str],
    threshold: int = BASE_DRIFT_COMMITS,
) -> bool:
    """True when the base moved enough that CI on the PR head is out of date.

    That is: GitHub reports BEHIND, the base has `threshold` or more commits the
    PR lacks, or a base commit since the merge base touched a file the PR changes.
    """
    if base_ahead_by <= 0:
        return False
    if merge_state == "BEHIND" or base_ahead_by >= threshold:
        return True
    return not set(base_files).isdisjoint(pr_files)


def fetch_until_mergeable_known(
    fetch: Callable[[], dict],
    sleep: Callable[[float], None] = time.sleep,
    delays: Iterable[float] = UNKNOWN_RETRY_DELAYS,
) -> dict:
    """Refetch while GitHub is still computing `mergeable`; return the last PR JSON."""
    pr = fetch()
    for delay in delays:
        if pr.get("mergeable") != "UNKNOWN":
            break
        sleep(delay)
        pr = fetch()
    return pr


def select_next_action(
    *,
    turn: str,
    conflicting: bool,
    gatekeeper: dict | None,
    has_comments: bool,
    failing: list[str],
    pending: list[str],
    waiting_owner: bool = False,
    base_drift: bool = False,
) -> str:
    if waiting_owner:
        return "wait-owner"
    if turn == "error":
        return "no-owner"
    if conflicting:
        return "resolve-conflict"
    if base_drift:
        return "rebase-base"
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


def base_drift(owner: str, name: str, pr: dict) -> tuple[int, list[str]]:
    """Commits on the base that the PR head lacks, and the files they changed."""
    compare = json.loads(gh(
        "api", f"repos/{owner}/{name}/compare/{pr['headRefOid']}...{pr['baseRefName']}",
        "--jq", "{ahead_by, files: [.files[]?.filename]}",
    ))
    return compare.get("ahead_by") or 0, compare.get("files") or []


def summarize(number: int, owner: str, name: str, me: str, claimed_by: str | None = None) -> dict:
    pr = fetch_until_mergeable_known(lambda: json.loads(gh("pr", "view", str(number), "--json", PR_FIELDS)))
    mergeable_unknown = pr.get("mergeable") == "UNKNOWN"
    base_ahead_by, base_files = base_drift(owner, name, pr)
    needs_rebase = base_drift_needs_rebase(
        merge_state=pr.get("mergeStateStatus"),
        base_ahead_by=base_ahead_by,
        base_files=base_files,
        pr_files=[f["path"] for f in pr.get("files") or []],
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

    answer = owner_answer(comments, me) if NEEDS_OWNER_LABEL in labels else None
    next_action = select_next_action(
        turn=turn.turn,
        conflicting=pr.get("mergeable") == "CONFLICTING",
        gatekeeper=gatekeeper,
        has_comments=bool(threads or new_comments),
        failing=failing,
        pending=pending,
        waiting_owner=NEEDS_OWNER_LABEL in labels and answer is None,
        base_drift=needs_rebase,
    )
    if claimed_by:
        next_action = "claimed"
    turn_report = format_turn_report(turn, pr_number=pr["number"])
    if mergeable_unknown:
        turn_report += "; mergeable still UNKNOWN after retries, recheck next tick"

    return {
        "number": pr["number"],
        "title": pr["title"],
        "url": pr["url"],
        "head": pr["headRefName"],
        "base": pr["baseRefName"],
        "draft": pr["isDraft"],
        "mergeable": pr.get("mergeable"),
        "mergeState": pr.get("mergeStateStatus"),
        "mergeableUnknown": mergeable_unknown,
        "baseAheadBy": base_ahead_by,
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
        "turnReport": turn_report,
        "ciAttempts": ci_attempts(comments, me),
        "ownerAnswer": answer,
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
