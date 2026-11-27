#!/usr/bin/env python3
"""Select the latest CI check run per name so superseded cancels do not fail a PR."""
from __future__ import annotations


def check_state(check: dict) -> str:
    return (check.get("conclusion") or check.get("state") or check.get("status") or "").upper()


def check_run_ts(check: dict) -> str:
    """Started time, then created, then a non-zero completed time."""
    for field in ("startedAt", "createdAt", "completedAt"):
        val = str(check.get(field) or "")
        if val and not val.startswith("0001-01-01"):
            return val
    return ""


def _check_key(check: dict) -> tuple[str, str] | None:
    name = check.get("name") or check.get("context")
    if not name:
        return None
    return (check.get("workflowName") or "", name)


def effective_checks(checks: list[dict]) -> list[dict]:
    """Keep only the latest run of each named check.

    The key is ``(workflowName, name or context)`` so the same job name in two
    workflows stays independent. Latest is ``startedAt``, then ``createdAt``,
    then a non-zero ``completedAt``. A newer queued or in-progress run is the
    latest, even if an older run succeeded. Unnamed checks are kept as-is.
    """
    latest: dict[tuple[str, str], tuple[str, int, dict]] = {}
    for idx, check in enumerate(checks):
        key = _check_key(check)
        if key is None:
            continue
        ts = check_run_ts(check)
        prev = latest.get(key)
        if prev is None or (ts, idx) >= (prev[0], prev[1]):
            latest[key] = (ts, idx, check)
    out: list[dict] = []
    seen: set[tuple[str, str]] = set()
    for check in checks:
        key = _check_key(check)
        if key is None:
            out.append(check)
            continue
        if key not in seen:
            seen.add(key)
            out.append(latest[key][2])
    return out
