#!/usr/bin/env python3
"""List open PRs by @me with their gatekeeper review status.

Verdicts and worker responses are PR comments authored by @me that carry a
hidden marker (see references/comment-format.md). Markers from any other
author are ignored. Unmarked comments from the owner (or @me) posted after the
last marker are surfaced as `ownerComments` and force a review.

status values:
  review     - needs review: new head/rebase/base change since last verdict, a
               worker response, an owner comment, never reviewed, CI failed
               (also after approve), or CI passed for a clean-marked PR.
  await-ci   - current head has a `verdict=clean` marker (or state entry) and
               only pending CI remains. Not reviewed again.
  wait-worker- changes requested at the current head without response, or merge
               conflicts. Worker's turn to fix.
  approved   - approve verdict exists for the current head, base unchanged, CI
               not failing, no newer owner comment.
  escalate   - round limit reached or unresolvable dispute; hand to the user.
  human-only - touches the gatekeeper's own skill; left to the human.

The list is sorted by priority (worker response / new head / owner comment
first), then oldest PR first. Every entry carries a one-line `reason`.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

MAX_ROUNDS = 5
# Loop markers must appear at the very start of the comment (ignoring optional UTF-8 BOM and leading whitespace).
# Quoted markers or mentions later in the comment are ignored.
START_MARKER = re.compile(r"^(?:\ufeff)?\s*<!--\s*(gatekeeper-loop|worker-loop):\s*(.*?)\s*-->")
FAILED = {"FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"}
PENDING = {"PENDING", "QUEUED", "IN_PROGRESS", "WAITING", "EXPECTED", "REQUESTED"}
PROTECTED_SKILL_PREFIXES = (
    ".agents/skills/gatekeeper-loop/",
    ".agents/skills/worker-loop/",
)
OWNER_ASSOCIATIONS = {"OWNER", "MEMBER", "COLLABORATOR"}
SENSITIVE_PATH = re.compile(
    r"(shell[_-]?policy|tool[_-]?approval|permission|secret|credential|keychain|sandbox)", re.I
)


def get_default_state_file() -> Path:
    env = os.environ.get("LOOP_STATE_DIR")
    if env:
        p = Path(env)
        base = p.parent if p.name == "state.json" else p
        return (base if base.name == "gatekeeper-loop" else base / "gatekeeper-loop") / "state.json"
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            capture_output=True, text=True, check=True
        ).stdout.strip()
        common = Path(out).resolve()
        repo_root = common.parent if common.name == ".git" else common
        return repo_root / ".libragent" / "work" / "gatekeeper-loop" / "state.json"
    except Exception:
        return Path(".libragent/work/gatekeeper-loop/state.json")


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
    """Return (gatekeeper markers incl. `clean`, worker responses) authored by me."""
    verdicts, responses = [], []
    for c in comments:
        if (c.get("author") or {}).get("login") != me:
            continue
        match = START_MARKER.match(c.get("body") or "")
        if not match:
            continue
        fields = dict(kv.split("=", 1) for kv in match.group(2).split() if "=" in kv)
        entry = {**fields, "createdAt": c["createdAt"], "url": c.get("url")}
        (verdicts if match.group(1) == "gatekeeper-loop" else responses).append(entry)
    return verdicts, responses


def owner_comments_after(comments: list[dict], me: str, since: str | None) -> list[dict]:
    """Unmarked comments from the owner/@me newer than the last marker comment."""
    found = []
    for c in comments:
        login = (c.get("author") or {}).get("login") or ""
        if login.endswith("[bot]") or START_MARKER.match(c.get("body") or ""):
            continue
        if login != me and c.get("authorAssociation") not in OWNER_ASSOCIATIONS:
            continue
        if since is None or c["createdAt"] > since:
            found.append({"author": login, "createdAt": c["createdAt"], "url": c.get("url"),
                          "body": (c.get("body") or "")[:500]})
    return found


def is_rebased(reviewed_head: str | None, head: str, is_ancestor) -> bool:
    """True if the reviewed head is not an ancestor of head (rebase/force-push or unknown object)."""
    if not reviewed_head or reviewed_head == head:
        return False
    return not is_ancestor(reviewed_head, head)


def git_is_ancestor(old: str, new: str) -> bool:
    return subprocess.run(["git", "merge-base", "--is-ancestor", old, new],
                          capture_output=True).returncode == 0


def load_state(path: Path | None = None) -> dict:
    state_path = path or get_default_state_file()
    if state_path.is_file():
        try:
            return json.loads(state_path.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def classify(
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
    clean_head: str | None = None,
    reviewed_base: str | None = None,
    rebased: bool = False,
    owner_comment: bool = False,
) -> tuple[str, str, int]:
    """Return (status, one-line reason, priority). Lower priority number is handled first."""
    ci_applies = is_dev_base(base)
    ci_failed = ci_applies and bool(failing_checks)

    if mergeable == "CONFLICTING":
        return "wait-worker", "merge conflict; worker must rebase", 9

    if owner_comment:
        return "review", "owner comment after last verdict; treat as required items", 0

    if total_verdicts >= MAX_ROUNDS:
        return "escalate", f"round limit ({MAX_ROUNDS}) reached", 9

    at_head = bool(last_verdict) and last_verdict.get("head") == head
    base_changed = bool(reviewed_base) and reviewed_base != base

    if at_head and last_verdict.get("verdict") == "approve":
        if base_changed:
            return "review", f"base changed after approve ({reviewed_base} -> {base})", 0
        if ci_failed:
            return "review", "CI failing after approve", 0
        return "approved", "approved at current head", 9

    if at_head and last_verdict.get("verdict") == "changes-requested":
        if worker_response is None:
            return "wait-worker", "changes requested; waiting for worker response", 9
        return "review", "worker responded to the latest verdict", 0

    if rebased:
        return "review", "head was rebased/force-pushed since last verdict; full review or range-diff", 0
    if last_verdict:
        return "review", "new head since last verdict", 0

    # Never given a verdict at this head. A clean marker on GitHub (or, as a
    # fallback, state.json) means the code review is already done.
    reviewed_clean = clean_head == head or (
        state_reviewed_head == head and state_pending_ci and not has_open_findings
    )
    if reviewed_clean and ci_applies and pending_checks and not failing_checks:
        return "await-ci", "reviewed clean; waiting for pending CI", 9
    if reviewed_clean:
        return "review", "reviewed clean; CI done or not applicable, issue the verdict", 1
    if failing_checks and ci_applies:
        return "review", "not reviewed yet; CI failing", 1
    return "review", "not reviewed yet", 1


def determine_status(**kwargs) -> str:
    """Status only; see classify() for the arguments."""
    return classify(**kwargs)[0]


def summarize(number: int, me: str, state: dict | None = None) -> dict:
    pr = json.loads(
        gh("pr", "view", str(number), "--json",
           "number,title,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,createdAt,"
           "statusCheckRollup,comments,labels,files")
    )
    base = pr["baseRefName"]
    checks = pr.get("statusCheckRollup") or []
    failing = [c.get("name") or c.get("context") for c in checks if check_state(c) in FAILED]
    pending = [c.get("name") or c.get("context") for c in checks if check_state(c) in PENDING]
    comments = pr.get("comments") or []
    markers, responses = parse_markers(comments, me)
    verdicts = [m for m in markers if m.get("verdict") != "clean"]
    cleans = [m for m in markers if m.get("verdict") == "clean"]

    head = pr["headRefOid"]
    last = verdicts[-1] if verdicts else None
    response = None
    if last:
        response = next(
            (r for r in reversed(responses)
             if r.get("round") == last.get("round") and r["createdAt"] > last["createdAt"]),
            None,
        )
    clean = next((c for c in reversed(cleans)
                  if last is None or c["createdAt"] > last["createdAt"]), None)

    pr_state = (state or {}).get("prs", {}).get(str(number), {})
    state_reviewed_head = pr_state.get("reviewedHead")
    state_pending_ci = pr_state.get("pendingApproveWaitCi", False)
    findings = pr_state.get("findings", {})
    has_open_findings = any(v == "open" for v in findings.values()) if isinstance(findings, dict) else bool(findings)

    reviewed_head = (clean or last or {}).get("head") or state_reviewed_head
    if reviewed_head and reviewed_head != head:
        subprocess.run(["git", "fetch", "origin", f"refs/pull/{number}/head", "--quiet"],
                       capture_output=True)
    rebased = is_rebased(reviewed_head, head, git_is_ancestor)

    last_marker_at = max((m["createdAt"] for m in [*markers, *responses]), default=None)
    owner = owner_comments_after(comments, me, last_marker_at)

    labels = sorted(label["name"] for label in pr.get("labels") or [])
    paths = [f["path"] for f in pr.get("files") or []]
    own_skill = any(p.startswith(PROTECTED_SKILL_PREFIXES) for p in paths)
    sensitive = sorted(p for p in paths if SENSITIVE_PATH.search(p))

    status, reason, priority = classify(
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
        clean_head=clean.get("head") if clean else None,
        reviewed_base=(last or {}).get("base"),
        rebased=rebased,
        owner_comment=bool(owner),
    )
    if own_skill:
        status, reason, priority = "human-only", "changes loop skill files; left to the human", 9

    if status != "review" or not reviewed_head or reviewed_head == head:
        review_mode = "full" if status == "review" and not reviewed_head else "incremental"
    else:
        review_mode = "range-diff" if rebased else "incremental"

    # Place review state first so the model and operator see review status before CI details.
    return {
        "number": pr["number"],
        "status": status,
        "reason": reason,
        "priority": priority,
        "reviewMode": review_mode,
        "round": len(verdicts) + (1 if status in ("review", "await-ci") else 0),
        "title": pr["title"],
        "head": pr["headRefName"],
        "headSha": head,
        "reviewedHead": reviewed_head,
        "rebased": rebased,
        "lastVerdict": last,
        "cleanMarker": clean,
        "workerResponse": response,
        "ownerComments": owner,
        "securitySensitive": sensitive,
        "labels": labels,
        "staleApprovedLabel": "gatekeeper:approved" in labels and status != "approved",
        "url": pr["url"],
        "base": base,
        "draft": pr["isDraft"],
        "createdAt": pr["createdAt"],
        "ciPending": pending,
        "ciFailing": failing,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, help="Only this PR number")
    parser.add_argument("--state-file", type=Path, default=None, help="Path to state.json")
    args = parser.parse_args()

    state = load_state(args.state_file)
    me = gh("api", "user", "--jq", ".login").strip()
    if args.pr:
        numbers = [args.pr]
    else:
        listed = json.loads(gh("pr", "list", "--author", "@me", "--state", "open",
                               "--json", "number,isDraft"))
        numbers = [p["number"] for p in listed if not p["isDraft"]]

    summaries = [summarize(n, me, state) for n in numbers]
    summaries.sort(key=lambda x: (x["priority"], x["createdAt"]))
    print(json.dumps(summaries, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
