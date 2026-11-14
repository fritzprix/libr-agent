"""Shared parsing for worker-loop and gatekeeper-loop PR comment markers."""
from __future__ import annotations

import re

# Loop markers must appear at the very start of the comment (ignoring optional UTF-8 BOM).
START_MARKER = re.compile(
    r"^(?:\ufeff)?\s*<!--\s*(gatekeeper-loop|worker-loop):\s*(.*?)\s*-->"
)


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


def latest_non_clean_verdict(verdicts: list[dict]) -> dict | None:
    non_clean = [v for v in verdicts if v.get("verdict") != "clean"]
    if non_clean:
        return non_clean[-1]
    return verdicts[-1] if verdicts else None
