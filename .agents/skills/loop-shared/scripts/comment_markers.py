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


def parse_markers(
    comments: list[dict], me: str
) -> tuple[list[dict], list[dict]]:
    """Return (gatekeeper markers incl. clean, worker responses) authored by me."""
    verdicts: list[dict] = []
    responses: list[dict] = []
    for comment in comments:
        if (comment.get("author") or {}).get("login") != me:
            continue
        match = START_MARKER.match(comment.get("body") or "")
        if not match:
            continue
        fields = dict(
            kv.split("=", 1) for kv in match.group(2).split() if "=" in kv
        )
        entry = {**fields, "createdAt": comment["createdAt"], "url": comment.get("url")}
        if match.group(1) == "gatekeeper-loop":
            verdicts.append(entry)
        else:
            responses.append(entry)
    return verdicts, responses


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
