"""Decide whose turn owns a PR from gatekeeper markers, worker response, and heads."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Literal

from comment_markers import (
    latest_worker_response,
    parse_markers,
)

Turn = Literal["worker", "gatekeeper", "human", "none"]

VERDICT_LABELS: dict[str, str] = {
    "changes-requested": "gatekeeper:changes-requested",
    "approve": "gatekeeper:approved",
    "escalate": "gatekeeper:escalated",
}

PRIMARY_VERDICT_LABELS = frozenset(VERDICT_LABELS.values())

# Shared order for overlapping PR/issue states (issue #2995): when several
# apply, the lowest rank decides what the loop reports and does.
OVERLAP_ORDER = ("needs-owner", "gatekeeper:escalated", "open-blocker", "merge-conflict", "verdict-label")


def overlap_rank(states: Iterable[str]) -> int | None:
    """Rank of the highest-priority state in `states`; None when none applies.

    `states` may contain any of the names in OVERLAP_ORDER (other names are
    ignored). A state with rank 0 (needs-owner) outranks an escalation, which
    outranks an open blocker, then a merge conflict, then a verdict-label fix.
    """
    names = set(states or ())
    return next((i for i, name in enumerate(OVERLAP_ORDER) if name in names), None)


def highest_overlap_state(states: Iterable[str]) -> str | None:
    """Name of the highest-priority state in `states` per OVERLAP_ORDER; None if none."""
    rank = overlap_rank(states)
    return OVERLAP_ORDER[rank] if rank is not None else None


def rebase_ready(blockers: list[dict], comments: list[dict], me: str, head_sha: str) -> bool:
    """True when every blocker is closed and the head matches the latest clean marker.

    Shared by `pr_status.py` and `review_queue.py` (issue #2995). `blockers` is
    the full blocked-by list from `issue_deps.pr_blockers` / `blocked_by`. The
    PR must carry at least one blocker link; the latest gatekeeper marker
    (verdicts including clean) must be a `clean` at the current head. After the
    push the head is new, so this is false until the gatekeeper reviews again.
    """
    if not blockers:
        return False
    if any(b["state"] == "open" for b in blockers):
        return False
    verdicts, _ = parse_markers(comments, me)
    return bool(verdicts) and verdicts[-1].get("verdict") == "clean" and verdicts[-1].get("head") == head_sha


@dataclass(frozen=True)
class PrTurn:
    turn: Turn
    currentVerdict: str | None
    verdictHead: str | None
    currentHead: str
    atVerdictHead: bool
    labelMismatch: list[str]
    reason: str


def label_mismatches(
    labels: list[str] | None,
    *,
    verdict: str | None,
    at_verdict_head: bool,
) -> list[str]:
    """Labels that disagree with the latest verdict at the current head."""
    if not labels or not verdict or not at_verdict_head:
        return []
    expected = VERDICT_LABELS.get(verdict)
    if not expected:
        return []
    mismatches: list[str] = []
    for name in PRIMARY_VERDICT_LABELS:
        if name != expected and name in labels:
            mismatches.append(f"remove stale label {name}")
    if expected not in labels:
        mismatches.append(f"add label {expected}")
    return mismatches


def decide_pr_turn(
    *,
    head_sha: str,
    mergeable: str | None,
    comments: list[dict],
    me: str,
    labels: list[str] | None = None,
    open_blockers: list[int] | None = None,
    rebase_ready: bool = False,
) -> PrTurn:
    """Return turn owner and verdict snapshot for worker and gatekeeper ticks."""
    if open_blockers:
        return PrTurn(
            turn="none",
            currentVerdict=None,
            verdictHead=None,
            currentHead=head_sha,
            atVerdictHead=False,
            labelMismatch=[],
            reason=f"blocked by open #{', #'.join(map(str, open_blockers))}",
        )
    if mergeable == "CONFLICTING":
        return PrTurn(
            turn="worker",
            currentVerdict=None,
            verdictHead=None,
            currentHead=head_sha,
            atVerdictHead=False,
            labelMismatch=[],
            reason="merge conflict",
        )
    if rebase_ready:
        return PrTurn(
            turn="worker",
            currentVerdict="clean",
            verdictHead=head_sha,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=[],
            reason="blockers closed; worker must rebase",
        )

    verdicts, responses = parse_markers(comments, me)
    # The newest marker is the current verdict, even when it is `clean`: a
    # clean at the current head supersedes any older changes-requested.
    last = verdicts[-1] if verdicts else None
    response = latest_worker_response(last, responses)

    if last is None:
        return PrTurn(
            turn="gatekeeper",
            currentVerdict=None,
            verdictHead=None,
            currentHead=head_sha,
            atVerdictHead=False,
            labelMismatch=label_mismatches(labels, verdict=None, at_verdict_head=False),
            reason="not reviewed yet",
        )

    verdict = last.get("verdict") or ""
    verdict_head = last.get("head")
    at_head = verdict_head == head_sha
    mismatches = label_mismatches(labels, verdict=verdict, at_verdict_head=at_head)

    if verdict == "escalate" and at_head:
        # The label is added before the `escalate` marker is posted, so a marker
        # with no label on GitHub means the owner removed it and resumed the PR
        # (#2996): the gatekeeper reviews again, from round 1, and must not
        # re-add the label. Unknown labels stay conservative: hand to the human.
        if labels is None or "gatekeeper:escalated" in labels:
            return PrTurn(
                turn="human",
                currentVerdict=verdict,
                verdictHead=verdict_head,
                currentHead=head_sha,
                atVerdictHead=True,
                labelMismatch=mismatches,
                reason="escalated; maintainer decision",
            )
        return PrTurn(
            turn="gatekeeper",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=[m for m in mismatches if not m.startswith("add label")],
            reason="owner resumed after gatekeeper:escalated; next round is 1",
        )

    if verdict == "approve" and at_head:
        return PrTurn(
            turn="human",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=mismatches,
            reason="approved at current head; human may merge",
        )

    if verdict == "changes-requested":
        if not at_head:
            return PrTurn(
                turn="gatekeeper",
                currentVerdict=verdict,
                verdictHead=verdict_head,
                currentHead=head_sha,
                atVerdictHead=False,
                labelMismatch=mismatches,
                reason="new head since changes-requested",
            )
        if response is not None:
            return PrTurn(
                turn="gatekeeper",
                currentVerdict=verdict,
                verdictHead=verdict_head,
                currentHead=head_sha,
                atVerdictHead=True,
                labelMismatch=mismatches,
                reason="worker responded to changes-requested",
            )
        return PrTurn(
            turn="worker",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=mismatches,
            reason="changes requested at current head",
        )

    if verdict == "clean" and at_head:
        return PrTurn(
            turn="gatekeeper",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=mismatches,
            reason="clean at head; gatekeeper posts approve when CI is green",
        )

    # Every fixed verdict at the current head is handled above, so the only
    # case left is a verdict at an older head: the gatekeeper re-reviews.
    return PrTurn(
        turn="gatekeeper",
        currentVerdict=verdict,
        verdictHead=verdict_head,
        currentHead=head_sha,
        atVerdictHead=False,
        labelMismatch=[],
        reason="new head since last verdict",
    )


def format_turn_report(turn: PrTurn, *, pr_number: int | None = None) -> str:
    """One-line stall-visible summary for tick reports."""
    prefix = f"#{pr_number} " if pr_number is not None else ""
    verdict = turn.currentVerdict or "none"
    reviewed = (turn.verdictHead or "none")[:7]
    current = turn.currentHead[:7]
    line = (
        f"{prefix}verdict={verdict} reviewedHead={reviewed} "
        f"currentHead={current} turn={turn.turn} ({turn.reason})"
    )
    if turn.labelMismatch:
        line += f" labelFix={'; '.join(turn.labelMismatch)}"
    return line
