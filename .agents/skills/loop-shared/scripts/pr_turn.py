"""Decide whose turn owns a PR from gatekeeper markers, worker response, and heads."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from comment_markers import (
    latest_worker_response,
    parse_markers,
)

Turn = Literal["worker", "gatekeeper", "human", "none", "error"]

VERDICT_LABELS: dict[str, str] = {
    "changes-requested": "gatekeeper:changes-requested",
    "approve": "gatekeeper:approved",
    "escalate": "gatekeeper:escalated",
}

PRIMARY_VERDICT_LABELS = frozenset(VERDICT_LABELS.values())


@dataclass(frozen=True)
class PrTurn:
    turn: Turn
    currentVerdict: str | None
    verdictHead: str | None
    currentHead: str
    atVerdictHead: bool
    labelMismatch: list[str]
    reason: str

    @property
    def noOwner(self) -> bool:
        return self.turn == "error"


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
) -> PrTurn:
    """Return turn owner and verdict snapshot for worker and gatekeeper ticks."""
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
        return PrTurn(
            turn="human",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=True,
            labelMismatch=mismatches,
            reason="escalated; maintainer decision",
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

    if not at_head:
        return PrTurn(
            turn="gatekeeper",
            currentVerdict=verdict,
            verdictHead=verdict_head,
            currentHead=head_sha,
            atVerdictHead=False,
            labelMismatch=[],
            reason="new head since last verdict",
        )

    return PrTurn(
        turn="error",
        currentVerdict=verdict,
        verdictHead=verdict_head,
        currentHead=head_sha,
        atVerdictHead=at_head,
        labelMismatch=mismatches,
        reason=f"no turn owner for verdict={verdict!r}",
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
    if turn.noOwner:
        line += " ERROR=no-owner"
    return line
