#!/usr/bin/env python3
"""Select CI checks from the latest workflow run so older runs cannot fail a PR."""
from __future__ import annotations

import re
from collections import defaultdict

_RUN_ID_RE = re.compile(r"/actions/runs/(\d+)")


def check_state(check: dict) -> str:
    return (check.get("conclusion") or check.get("state") or check.get("status") or "").upper()


def check_run_ts(check: dict) -> str:
    """Started time, then created, then a non-zero completed time."""
    for field in ("startedAt", "createdAt", "completedAt"):
        val = str(check.get(field) or "")
        if val and not val.startswith("0001-01-01"):
            return val
    return ""


def workflow_run_id(check: dict) -> str | None:
    """Workflow run id from detailsUrl (/actions/runs/<id>/) or checkSuite."""
    match = _RUN_ID_RE.search(str(check.get("detailsUrl") or ""))
    if match:
        return match.group(1)
    suite = check.get("checkSuite")
    if isinstance(suite, dict):
        for key in ("id", "databaseId", "database_id"):
            val = suite.get(key)
            if val is not None and str(val):
                return str(val)
    return None


def _is_status_context(check: dict) -> bool:
    typ = check.get("__typename") or ""
    if typ == "StatusContext":
        return True
    if typ == "CheckRun":
        return False
    return bool(
        check.get("context")
        and not check.get("name")
        and not check.get("workflowName")
        and workflow_run_id(check) is None
    )


def _is_all_skipped(members: list[dict]) -> bool:
    return bool(members) and all(check_state(c) == "SKIPPED" for c in members)


def _run_sort_key(run_id: str, members: list[tuple[int, dict]]) -> tuple:
    times = [check_run_ts(c) for _, c in members if check_run_ts(c)]
    start = min(times) if times else ""
    try:
        rid = int(run_id)
    except ValueError:
        rid = 0
    return (start, rid, members[0][0])


def _latest_by_name(items: list[tuple[int, dict]], name_of) -> dict[int, dict]:
    latest: dict[str, tuple[str, int, dict]] = {}
    unnamed: list[tuple[int, dict]] = []
    for idx, check in items:
        name = name_of(check)
        if not name:
            unnamed.append((idx, check))
            continue
        ts = check_run_ts(check)
        prev = latest.get(name)
        if prev is None or (ts, idx) >= (prev[0], prev[1]):
            latest[name] = (ts, idx, check)
    kept = {idx: check for idx, check in unnamed}
    seen: set[str] = set()
    for idx, check in items:
        name = name_of(check)
        if not name or name in seen:
            continue
        seen.add(name)
        chosen_idx, chosen = latest[name][1], latest[name][2]
        kept[chosen_idx] = chosen
    return kept


def effective_checks(checks: list[dict]) -> list[dict]:
    """Keep checks from the latest workflow run of each workflow.

    Latest is the run that started last (min startedAt / createdAt / non-zero
    completedAt among its jobs; then run id). A newer in-progress run is the
    source of truth, even if an older run succeeded. A run whose jobs are all
    SKIPPED is not a result (label-triggered empty runs) and is ignored.
    Status contexts keep the latest result per context. Check runs with no run
    id fall back to the latest result per name within their workflow.
    """
    status_items: list[tuple[int, dict]] = []
    # workflowName -> run_id -> [(idx, check)]
    runs: dict[str, dict[str, list[tuple[int, dict]]]] = defaultdict(lambda: defaultdict(list))
    no_id: dict[str, list[tuple[int, dict]]] = defaultdict(list)
    kept: dict[int, dict] = {}

    for idx, check in enumerate(checks):
        if _is_status_context(check):
            status_items.append((idx, check))
            continue
        run_id = workflow_run_id(check)
        workflow = check.get("workflowName") or ""
        if run_id:
            runs[workflow][run_id].append((idx, check))
        elif check.get("name") or check.get("context"):
            no_id[workflow].append((idx, check))
        else:
            kept[idx] = check

    kept.update(_latest_by_name(status_items, lambda c: c.get("context") or ""))

    for by_run in runs.values():
        candidates = [
            (run_id, members)
            for run_id, members in by_run.items()
            if not _is_all_skipped([c for _, c in members])
        ]
        if not candidates:
            continue
        _, members = max(candidates, key=lambda item: _run_sort_key(item[0], item[1]))
        for idx, check in members:
            kept[idx] = check

    for workflow, items in no_id.items():
        if workflow in runs:
            continue
        kept.update(_latest_by_name(items, lambda c: c.get("name") or c.get("context") or ""))

    return [kept[idx] for idx in sorted(kept)]
