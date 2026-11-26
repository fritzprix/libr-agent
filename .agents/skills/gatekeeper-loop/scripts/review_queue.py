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
  await-ci   - current head has a `verdict=clean` marker (or state entry) and CI
               is not green yet: pending, not run, or a stacked PR (no CI until
               retargeted). Not reviewed again.
  wait-worker- changes requested at the current head without response, or merge
               conflicts. Worker's turn to fix.
  blocked    - the PR has an open blocker (blocked-by link to an open issue).
               Not reviewed and not diagnosed again until it closes (issue #2995).
  approved   - approve verdict exists for the current head, base unchanged, CI
               green at the head, no newer owner comment.
  escalate   - round limit (5) reached, or the answer is a product, security, or
               data decision for the user.

Each entry may include humanOnly: true when the diff touches loop-skill
paths (add `gatekeeper:needs-human` when posting approve; never skip review).

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

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

from comment_markers import START_MARKER, parse_markers  # noqa: E402
from issue_deps import DepsError, pr_blockers  # noqa: E402
from pr_turn import (  # noqa: E402
    decide_pr_turn,
    format_turn_report,
    highest_overlap_state,
    rebase_ready as is_rebase_ready,
)

MAX_ROUNDS = 5
ESCALATED_LABEL = "gatekeeper:escalated"
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
# Extra path hints for auth/credential surfaces that path-only policy regex misses (HTTP clients,
# release API routes, executor modules). Behavior review in the skill is still required.
SENSITIVE_BEHAVIOR_HINT = re.compile(
    r"(api[_-]?key|bearer|oauth|auth[_-]?guard|encrypted_secret|rust[_-]?executor|"
    r"llm[_-]?executor|openai[_-]?compat|session[_-]?api|routes\.rs|user[_-]?prompt|"
    r"prompt[_-]?kind|headless[_-]?runtime|liblet)",
    re.I,
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


# Application CI job names from LibrAgent CI/CD (matrix.os = macos-15).
# Do not match Loop skill tests' `test (macos-latest)` (#3070 G1.2).
REQUIRED_MACOS_CHECK_NAMES = ("Test (macos-15)", "Build (macos-15)")


def has_passed_macos_check(checks: list[dict]) -> bool:
    """True when both app-workflow macOS Test and Build checks succeeded (#3070)."""
    passed = {
        (check.get("name") or check.get("context") or "")
        for check in checks
        if check_state(check) == "SUCCESS"
    }
    return all(name in passed for name in REQUIRED_MACOS_CHECK_NAMES)


def is_dev_base(base: str) -> bool:
    """True if CI runs on PRs targeting this base branch."""
    return base in ("main", "master") or base.startswith("dev/")


def owner_comments_after(comments: list[dict], me: str, since: str | None) -> list[dict]:
    """Unmarked comments from the owner/@me newer than the last gatekeeper marker."""
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


def last_gatekeeper_marker_at(markers: list[dict]) -> str | None:
    """Timestamp of the newest gatekeeper marker (ignoring worker response markers)."""
    return max((m["createdAt"] for m in markers), default=None)


def rounds_since_restart(verdicts: list[dict]) -> int:
    """Verdicts counted toward the round limit, reset by the last `escalate` verdict.

    Verdict markers are the only input. A PR that carries `gatekeeper:escalated` is
    not reviewed, so the first verdict after an `escalate` marker can only be posted
    after the owner removed the label: that verdict is round 1.
    """
    for index in range(len(verdicts) - 1, -1, -1):
        if verdicts[index].get("verdict") == "escalate":
            return len(verdicts) - index - 1
    return len(verdicts)


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
    rounds_used: int,
    state_reviewed_head: str | None,
    state_pending_ci: bool,
    has_open_findings: bool,
    pending_checks: list[str],
    failing_checks: list[str],
    clean_head: str | None = None,
    reviewed_base: str | None = None,
    rebased: bool = False,
    owner_comment: bool = False,
    ci_ran: bool = True,
    macos_ci_passed: bool = False,
    escalate_label_present: bool = True,
    open_blockers: list[int] | None = None,
    rebase_ready: bool = False,
    blocker_lookup_error: str | None = None,
) -> tuple[str, str, int]:
    """Return (status, one-line reason, priority). Lower priority number is handled first.

    `rounds_used` counts verdicts since the last restart (see `rounds_since_restart`), so
    the limit of `MAX_ROUNDS` applies per owner-resumed run, never to the PR's lifetime.
    An approve needs green CI at the head. Stacked PRs (non-dev base) never run CI, so they
    stay at `clean` until they are retargeted and CI passes.

    A PR with an open blocker is `blocked` at priority 9: the gatekeeper does
    not review it and does not diagnose it again (issue #2995). The order
    uses `pr_turn.highest_overlap_state`: an unresolved escalation outranks an
    open blocker, which outranks a merge conflict.
    """
    # An escalated PR is not reviewed until the owner removes the label (#2996 G1.3).
    # Shared overlap precedence from `pr_turn.highest_overlap_state` (issue #2995).
    top_overlap = highest_overlap_state(
        (["gatekeeper:escalated"] if (last_verdict and last_verdict.get("verdict") == "escalate" and escalate_label_present) else [])
        + (["open-blocker"] if open_blockers else [])
        + (["merge-conflict"] if mergeable == "CONFLICTING" else [])
    )
    if top_overlap == "gatekeeper:escalated":
        return "escalate", f"escalated; waiting for owner to remove {ESCALATED_LABEL}", 9
    if blocker_lookup_error:
        # Fail closed (#2995 G3.1): without the blocker lookup this PR could be
        # parked, so do not review or diagnose it; skip it and report the error.
        return (
            "blocked-unknown",
            f"blocker lookup failed: {blocker_lookup_error}; skip this PR this tick and report the error",
            9,
        )
    if top_overlap == "open-blocker":
        return "blocked", f"open blocker #{', #'.join(map(str, open_blockers))}; parked until it closes", 9
    if top_overlap == "merge-conflict":
        return "wait-worker", "merge conflict; worker must rebase", 9
    if rebase_ready:
        return "wait-worker", "blockers closed; worker must rebase and push", 9

    ci_applies = is_dev_base(base)
    ci_failed = ci_applies and bool(failing_checks)
    # macOS is gated behind gatekeeper:review-clean on PRs (#3070). Approve only
    # when a passed macOS check exists at the head; ubuntu+windows alone is not green.
    ci_green = (
        ci_applies
        and ci_ran
        and not failing_checks
        and not pending_checks
        and macos_ci_passed
    )

    if owner_comment:
        return "review", "owner comment after last verdict; treat as required items", 0

    at_head = bool(last_verdict) and last_verdict.get("head") == head
    base_changed = bool(reviewed_base) and reviewed_base != base

    # An approve given in the last allowed round settles the PR; it is not an escalation.
    approved_at_head = at_head and last_verdict.get("verdict") == "approve"
    if rounds_used >= MAX_ROUNDS and not approved_at_head:
        return "escalate", f"round limit ({MAX_ROUNDS}) reached", 9

    # A clean marker for this head on GitHub means the code review of this head is
    # done, even after changes were requested at it (the worker answered without a
    # new commit, for example by editing the PR body). The caller only passes a clean
    # marker newer than both the last verdict and the worker's response to it.
    clean_at_head = clean_head == head

    if last_verdict and last_verdict.get("verdict") == "escalate":
        # The label gate above still parks it while the label is on; reaching here
        # means the owner removed the label, so the round count restarts (#2996).
        if not clean_at_head:
            return (
                "review",
                f"{ESCALATED_LABEL} removed by the owner; round count restarted at 0, next round is 1",
                0,
            )

    if at_head and last_verdict.get("verdict") == "approve":
        if base_changed:
            return "review", f"base changed after approve ({reviewed_base} -> {base})", 0
        if ci_failed:
            return "review", "CI failing after approve", 0
        if not ci_green:
            return "review", "approve without green CI at this head; replace it with a clean marker", 0
        return "approved", "approved at current head", 9

    if at_head and last_verdict.get("verdict") == "changes-requested" and not clean_at_head:
        if worker_response is None:
            return "wait-worker", "changes requested; waiting for worker response", 9
        return "review", "worker responded to the latest verdict", 0

    # Otherwise a clean marker (or, as a fallback, state.json) also covers a head
    # reached after verdicts at older heads.
    reviewed_clean = clean_at_head or (
        state_reviewed_head == head and state_pending_ci and not has_open_findings
    )
    if not reviewed_clean:
        if rebased:
            return "review", "head was rebased/force-pushed since last verdict; full review or range-diff", 0
        if last_verdict:
            return "review", "new head since last verdict", 0

    if reviewed_clean and ci_green:
        return "review", "reviewed clean; CI green at this head, issue the verdict", 1
    if reviewed_clean and ci_failed:
        return "review", "reviewed clean; CI failing, diagnose it", 1
    if reviewed_clean and not ci_applies:
        return "await-ci", "reviewed clean; stacked PR runs no CI until it is retargeted", 9
    if reviewed_clean and pending_checks:
        return "await-ci", "reviewed clean; waiting for pending CI", 9
    if reviewed_clean and ci_applies and not macos_ci_passed:
        return "await-ci", "reviewed clean; waiting for macOS CI", 9
    if reviewed_clean:
        return "await-ci", "reviewed clean; no CI run at this head yet", 9
    if failing_checks and ci_applies:
        return "review", "not reviewed yet; CI failing", 1
    return "review", "not reviewed yet", 1


def touches_human_only_paths(paths: list[str]) -> bool:
    return any(p.startswith(PROTECTED_SKILL_PREFIXES) for p in paths)


def classify_sensitive_paths(paths: list[str]) -> dict:
    """Split changed paths into policy-regex hits and broader behavior-hint hits."""
    policy = sorted(p for p in paths if SENSITIVE_PATH.search(p))
    behavior_hint = sorted(
        p for p in paths
        if p not in policy and SENSITIVE_BEHAVIOR_HINT.search(p)
    )
    combined = sorted(set(policy) | set(behavior_hint))
    return {
        "securitySensitive": combined,
        "securitySensitivePolicyPaths": policy,
        "securitySensitiveBehaviorHints": behavior_hint,
    }


def markers_at_head(markers: list[dict], head: str) -> list[dict]:
    """Gatekeeper markers (incl. clean) already posted for this PR head SHA."""
    return [m for m in markers if m.get("head") == head]


def posting_checks(*, head: str, markers: list[dict], last_verdict: dict | None) -> dict:
    """Signals the reviewer must satisfy immediately before posting a new marker."""
    at_head = markers_at_head(markers, head)
    last_at_head = last_verdict if (last_verdict or {}).get("head") == head else None
    return {
        "headSha": head,
        "markersAtHead": [
            {"verdict": m.get("verdict"), "round": m.get("round"), "createdAt": m.get("createdAt")}
            for m in at_head
        ],
        "lastVerdictAtHead": (
            {
                "verdict": last_at_head.get("verdict"),
                "round": last_at_head.get("round"),
                "url": last_at_head.get("url"),
            }
            if last_at_head
            else None
        ),
        "reFetchHeadBeforePost": True,
        "doNotDuplicateHeadVerdict": (
            "Before posting, refuse if an identical gatekeeper-loop marker already exists "
            "for this headSha and intended verdict (including clean)."
        ),
    }


def latest_clean_marker(cleans: list[dict], last: dict | None, response: dict | None) -> dict | None:
    """The newest clean marker posted after the last verdict and after the worker's
    response to it. A response that arrives after a clean marker asks for a new look,
    so that marker no longer counts."""
    return next(
        (c for c in reversed(cleans)
         if (last is None or c["createdAt"] > last["createdAt"])
         and (response is None or c["createdAt"] > response["createdAt"])),
        None,
    )


def determine_status(**kwargs) -> str:
    """Status only; see classify() for the arguments."""
    return classify(**kwargs)[0]


def gh_blocked_by(pr: dict) -> tuple[list[dict], str | None]:
    """Blockers of a PR's closing issues, via the shared helper (issue #2995).

    Returns `(blockers, error)`: a lookup failure is reported on this PR
    without aborting the rest of the queue.
    """
    try:
        return pr_blockers(pr, run_gh=gh), None
    except DepsError as e:
        return [], str(e)


def summarize(number: int, me: str, state: dict | None = None) -> dict:
    pr = json.loads(
        gh("pr", "view", str(number), "--json",
           "number,title,body,url,headRefName,headRefOid,baseRefName,isDraft,mergeable,createdAt,"
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
    clean = latest_clean_marker(cleans, last, response)

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

    last_gk_at = last_gatekeeper_marker_at(markers)
    owner = owner_comments_after(comments, me, last_gk_at)

    labels = sorted(label["name"] for label in pr.get("labels") or [])
    rounds_used = rounds_since_restart(verdicts)
    paths = [f["path"] for f in pr.get("files") or []]
    human_only = touches_human_only_paths(paths)
    sensitive_info = classify_sensitive_paths(paths)

    blockers, blocker_error = gh_blocked_by(pr)
    open_dep = [b["number"] for b in blockers if b["state"] == "open"]
    ready_to_rebase = is_rebase_ready(blockers, comments, me, head)

    status, reason, priority = classify(
        mergeable=pr.get("mergeable"),
        head=head,
        base=base,
        last_verdict=last,
        worker_response=response,
        rounds_used=rounds_used,
        state_reviewed_head=state_reviewed_head,
        state_pending_ci=state_pending_ci,
        has_open_findings=has_open_findings,
        pending_checks=pending,
        failing_checks=failing,
        clean_head=clean.get("head") if clean else None,
        reviewed_base=(last or {}).get("base"),
        rebased=rebased,
        owner_comment=bool(owner),
        ci_ran=bool(checks),
        macos_ci_passed=has_passed_macos_check(checks),
        escalate_label_present=ESCALATED_LABEL in labels,
        open_blockers=open_dep,
        rebase_ready=ready_to_rebase,
        blocker_lookup_error=blocker_error,
    )

    if status != "review" or not reviewed_head or reviewed_head == head:
        review_mode = "full" if status == "review" and not reviewed_head else "incremental"
    else:
        review_mode = "range-diff" if rebased else "incremental"

    turn = decide_pr_turn(
        head_sha=head,
        mergeable=pr.get("mergeable"),
        comments=comments,
        me=me,
        labels=labels,
        open_blockers=open_dep,
        rebase_ready=ready_to_rebase,
        blocker_lookup_error=blocker_error,
    )

    # Place review state first so the model and operator see review status before CI details.
    return {
        "number": pr["number"],
        "status": status,
        "humanOnly": human_only,
        "whoseTurn": turn.turn,
        "currentVerdict": turn.currentVerdict,
        "verdictHeadSha": turn.verdictHead,
        "atVerdictHead": turn.atVerdictHead,
        "labelMismatch": turn.labelMismatch,
        "turnReport": format_turn_report(turn, pr_number=pr["number"]),
        "reason": reason,
        "priority": priority,
        "reviewMode": review_mode,
        "round": rounds_used + (1 if status in ("review", "await-ci") else 0),
        "title": pr["title"],
        "head": pr["headRefName"],
        "headSha": head,
        "reviewedHead": reviewed_head,
        "rebased": rebased,
        "lastVerdict": last,
        "cleanMarker": clean,
        "workerResponse": response,
        "ownerComments": owner,
        "securitySensitive": sensitive_info["securitySensitive"],
        "securitySensitivePolicyPaths": sensitive_info["securitySensitivePolicyPaths"],
        "securitySensitiveBehaviorHints": sensitive_info["securitySensitiveBehaviorHints"],
        "postingChecks": posting_checks(head=head, markers=markers, last_verdict=last),
        "labels": labels,
        "staleApprovedLabel": "gatekeeper:approved" in labels and status not in ("approved", "blocked-unknown"),
        # Label is stale when the head moved since the clean marker that caused it (#3070).
        "staleReviewCleanLabel": (
            "gatekeeper:review-clean" in labels
            and (clean is None or clean.get("head") != head)
        ),
        "url": pr["url"],
        "base": base,
        "draft": pr["isDraft"],
        "createdAt": pr["createdAt"],
        "ciPending": pending,
        "ciFailing": failing,
        "blockerLookupError": blocker_error,
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
