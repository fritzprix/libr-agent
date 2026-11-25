"""Shared parsing for worker-loop and gatekeeper-loop PR comment markers."""
from __future__ import annotations

import re

# Loop markers must appear at the very start of the comment (ignoring optional UTF-8 BOM).
START_MARKER = re.compile(
    r"^(?:\ufeff)?\s*<!--\s*(gatekeeper-loop|worker-loop):\s*(.*?)\s*-->"
)

NEEDS_OWNER_LABEL = "needs-owner"
# Besides `me` (the owner and the workers may share one account), only a
# repository owner, member, or collaborator can answer an owner question.
_ANSWER_ASSOCIATIONS = {"OWNER", "MEMBER", "COLLABORATOR"}

# Fixed marker kinds (issue #2994). These are the only kinds the loops act on:
# a marker whose kind or verdict is outside these sets is ignored entirely —
# not a verdict, not a worker response, not an owner comment.
# Gatekeeper verdict values (`verdict=` on a gatekeeper-loop marker):
GATEKEEPER_VERDICTS = frozenset({"changes-requested", "approve", "clean", "escalate"})
# Worker marker kinds (first word after `worker-loop:`):
WORKER_KINDS = frozenset({"response", "needs-owner", "ci-attempt", "reply"})


def _worker_marker_kind(comment: dict, me: str) -> str | None:
    """First word of a worker-loop marker written by me (`needs-owner`, `ci-attempt`, ...)."""
    if (comment.get("author") or {}).get("login") != me:
        return None
    match = START_MARKER.match(comment.get("body") or "")
    if not match or match.group(1) != "worker-loop":
        return None
    words = match.group(2).split()
    return words[0] if words else None


def owner_answer(comments: list[dict], me: str) -> dict | None:
    """The answer to the latest worker-loop `needs-owner` question, or None.

    The answer is the first later comment without a loop marker, written by `me`
    or by a repository owner, member, or collaborator.
    """
    asked_at = None
    for comment in comments:
        if _worker_marker_kind(comment, me) == "needs-owner":
            asked_at = comment["createdAt"]
    if asked_at is None:
        return None
    for comment in comments:
        if comment["createdAt"] <= asked_at or START_MARKER.match(comment.get("body") or ""):
            continue
        login = (comment.get("author") or {}).get("login")
        if login == me or comment.get("authorAssociation") in _ANSWER_ASSOCIATIONS:
            return {"author": login, "createdAt": comment["createdAt"], "url": comment.get("url")}
    return None


def ci_attempts(comments: list[dict], me: str) -> int:
    """CI-only fix attempts recorded on a PR with `<!-- worker-loop: ci-attempt n=K -->`."""
    return sum(1 for comment in comments if _worker_marker_kind(comment, me) == "ci-attempt")


def _marker_fields(marker_text: str) -> dict[str, str]:
    """Parse `key=value` fields from the inside of a loop marker comment."""
    return dict(kv.split("=", 1) for kv in marker_text.split() if "=" in kv)


def parse_markers(
    comments: list[dict], me: str
) -> tuple[list[dict], list[dict]]:
    """Return (gatekeeper markers incl. clean, worker responses) authored by me.

    Only the fixed marker kinds count (issue #2994): a gatekeeper marker whose
    `verdict=` is outside `GATEKEEPER_VERDICTS`, and a worker marker whose kind
    (first word) is outside `WORKER_KINDS`, are ignored entirely. Only `response`
    worker markers count as responses to a gatekeeper round. A `clean` marker
    carries no `blocked-by` field; any such field is dropped when parsing.
    """
    verdicts: list[dict] = []
    responses: list[dict] = []
    for comment in comments:
        if (comment.get("author") or {}).get("login") != me:
            continue
        match = START_MARKER.match(comment.get("body") or "")
        if not match:
            continue
        fields = _marker_fields(match.group(2))
        entry = {**fields, "createdAt": comment["createdAt"], "url": comment.get("url")}
        if match.group(1) == "gatekeeper-loop":
            if entry.get("verdict") not in GATEKEEPER_VERDICTS:
                continue
            if entry.get("verdict") == "clean":
                entry.pop("blocked-by", None)
            verdicts.append(entry)
        else:
            words = match.group(2).split()
            kind = words[0] if words else ""
            if kind not in WORKER_KINDS:
                continue
            entry["kind"] = kind
            if kind == "response":
                responses.append(entry)
    return verdicts, responses


def reply_targets(comments: list[dict], me: str) -> set[str]:
    """Comment ids answered by my worker-loop `reply` markers (`reply-to=<id>`).

    The worker posts a `reply` marker when it answers a triage comment without a
    push. A comment whose id is targeted by a later `reply` marker is not a
    triage item (issue #2994).
    """
    targets: set[str] = set()
    for comment in comments:
        if _worker_marker_kind(comment, me) != "reply":
            continue
        match = START_MARKER.match(comment.get("body") or "")
        if not match:
            continue
        fields = _marker_fields(match.group(2))
        if fields.get("reply-to"):
            targets.add(str(fields["reply-to"]))
    return targets


def latest_worker_response(
    last_verdict: dict | None, responses: list[dict]
) -> dict | None:
    """Newest worker-loop response for the last verdict's round, after that verdict."""
    if not last_verdict:
        return None
    target_round = last_verdict.get("round")
    for response in reversed(responses):
        if response.get("round") != target_round:
            continue
        if response["createdAt"] <= last_verdict["createdAt"]:
            continue
        return response
    return None
