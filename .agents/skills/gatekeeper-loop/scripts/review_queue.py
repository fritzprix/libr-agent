#!/usr/bin/env python3
"""List open PRs by @me with their gatekeeper review status.

Verdicts and worker responses are PR comments authored by @me that carry a
hidden marker (see references/comment-format.md). Markers from any other
author are ignored.

status values:
  review     - needs review: new head since last verdict, a worker response,
               never reviewed, CI failed, or CI passed for an await-ci PR.
               Review diff & issue immediately regardless of CI status.
  await-ci   - current head has already been reviewed with zero open findings;
               waiting only for pending CI to complete before issuing approve.
               Excluded from the active review queue so it does not block ticks.
  wait-worker- changes requested at the current head without response, or merge
               conflicts. Worker's turn to fix.
  approved   - approve verdict exists for the current head.
  escalate   - round limit reached or unresolvable dispute; hand to the user.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

MAX_ROUNDS = 5
MARKER = re.compile(r"<!--\s*(gatekeeper-loop|worker-loop):\s*(.*?)\s*-->")
FAILED = {"FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"}
PENDING = {"PENDING", "QUEUED", "IN_PROGRESS", "WAITING", "EXPECTED", "REQUESTED"}
DEFAULT_STATE_FILE = Path(".libragent/work/gatekeeper-loop/state.json")


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def check_state(check: dict) -> str:
    return (check.get("conclusion") or check.get("state") or check.get("status") or "").upper()


def is_dev_base(base: str) -> bool:
    """True if CI runs on PRs targeting this base branch."""
    return base in ("main", "master") or base.startswith("dev/")


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


def load_state(path: Path | None = None) -> dict:
    state_path = path or DEFAULT_STATE_FILE
    if state_path.is_file():
        try:
            return json.loads(state_path.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def determine_status(
    *,
    mergeable: str | None,
    head: str,
    base: str,
    last_verdict: dict | None,
    worker_response: dict | None,
    total_verdicts: int,
    state_reviewed_head: str | None,
    state_pending_ci: bool,
    has_open_findings: bool,
    pending_checks: list[str],
    failing_checks: list[str],
) -> str:
    """Determine the review status of a PR based on review state and CI."""
    if mergeable == "CONFLICTING":
        return "wait-worker"

    if last_verdict and last_verdict.get("head") == head and last_verdict.get("verdict") == "approve":
        return "approved"

    if (
        last_verdict
        and last_verdict.get("head") == head
        and last_verdict.get("verdict") == "changes-requested"
        and worker_response is None
    ):
        return "wait-worker"

    if total_verdicts >= MAX_ROUNDS:
        return "escalate"

    # Stacked PRs (base is not dev/* or main) do not trigger GitHub Actions CI.
    # They do not wait for CI; once reviewed with 0 findings, they can be approved with retarget condition.
    if not is_dev_base(base):
        if state_reviewed_head == head and not has_open_findings:
            return "review"  # ready to issue approve with retarget condition
        return "review"

    # For dev-targeted PRs: current head was already reviewed with 0 open findings; waiting for CI.
    if state_reviewed_head == head and state_pending_ci and not has_open_findings:
        if pending_checks and not failing_checks:
            return "await-ci"
        # If CI completed green or failed, fall through to "review" (ready to approve or diagnose failure).

    return "review"


def summarize(number: int, me: str, state: dict | None = None) -> dict:
    pr = json.loads(
        gh("pr", "view", str(number), "--json",
           "number,title,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,"
           "statusCheckRollup,comments,labels")
    )
    base = pr["baseRefName"]
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

    pr_state = (state or {}).get("prs", {}).get(str(number), {})
    state_reviewed_head = pr_state.get("reviewedHead")
    state_pending_ci = pr_state.get("pendingApproveWaitCi", False)
    findings = pr_state.get("findings", {})
    has_open_findings = any(v == "open" for v in findings.values()) if isinstance(findings, dict) else bool(findings)

    status = determine_status(
        mergeable=pr.get("mergeable"),
        head=head,
        base=base,
        last_verdict=last,
        worker_response=response,
        total_verdicts=len(verdicts),
        state_reviewed_head=state_reviewed_head,
        state_pending_ci=state_pending_ci,
        has_open_findings=has_open_findings,
        pending_checks=pending,
        failing_checks=failing,
    )

    reviewed_head = (last.get("head") if last else None) or state_reviewed_head

    # Place review state first so the model and operator see review status before CI details.
    return {
        "number": pr["number"],
        "status": status,
        "round": len(verdicts) + (1 if status in ("review", "await-ci") else 0),
        "title": pr["title"],
        "head": pr["headRefName"],
        "headSha": head,
        "reviewedHead": reviewed_head,
        "lastVerdict": last,
        "workerResponse": response,
        "labels": sorted(label["name"] for label in pr.get("labels") or []),
        "url": pr["url"],
        "base": base,
        "draft": pr["isDraft"],
        "ciPending": pending,
        "ciFailing": failing,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, help="Only this PR number")
    parser.add_argument("--state-file", type=Path, default=DEFAULT_STATE_FILE, help="Path to state.json")
    args = parser.parse_args()

    state = load_state(args.state_file)
    me = gh("api", "user", "--jq", ".login").strip()
    if args.pr:
        numbers = [args.pr]
    else:
        listed = json.loads(gh("pr", "list", "--author", "@me", "--state", "open",
                               "--json", "number,isDraft"))
        numbers = [p["number"] for p in listed if not p["isDraft"]]

    print(json.dumps([summarize(n, me, state) for n in numbers], ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
