"""Post-restart snapshot of the isolated bench instance (#2903).

Restart-dependent #2731 rows (secrets/servers/settings read back after a
restart) need proof that the *same* disposable instance came back with its
state. The Harbor adapter calls :func:`restart_and_snapshot` after the agent
session completes: it restarts the instance in place (same data dir), runs
the task-declared probes, and returns a record the adapter writes to
``session_e2e_restart_evidence.json`` for the task verifier to grade.

Never raises for instance-level failures — the verifier decides the trial
outcome from ``restartExecuted`` and the probe records.
"""

from __future__ import annotations

import contextlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import httpx

from benchmarks.harbor.isolated_instance import (
    InstanceError,
    InstanceLayout,
    auth_headers,
    restart,
)


def _sql_probe(db_path: Path, query: str) -> list[list[Any]]:
    """Run one read-only SELECT; sqlite raises on anything else."""
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        return [list(row) for row in conn.execute(query).fetchall()]
    finally:
        conn.close()


async def restart_and_snapshot(
    layout: InstanceLayout,
    app_bin: Path,
    *,
    api_url: str,
    http_probes: tuple[dict[str, str], ...] = (),
    sql_probes: tuple[dict[str, str], ...] = (),
    session_id: str | None = None,
    timeout_sec: float = 120.0,
) -> dict[str, Any]:
    """Restart the bench instance in place and record a post-restart snapshot.

    ``restart`` rebuilds nothing: sessions, settings, and workspace files
    survive, which is exactly what the probes must observe. Returns a JSON
    record; on failure ``restartExecuted`` is False and ``error`` explains
    why, so the verifier fails the row with evidence instead of the harness
    crashing.
    """
    record: dict[str, Any] = {
        "restartExecuted": False,
        "dataDir": str(layout.data_dir.resolve()),
        "probedAt": datetime.now(timezone.utc).isoformat(),
        "http": [],
        "sql": [],
    }
    try:
        pid = restart(layout, app_bin, timeout_sec=timeout_sec)
    except (InstanceError, OSError, TimeoutError) as error:
        record["error"] = f"{type(error).__name__}: {error}"
        return record

    record["restartExecuted"] = True
    record["pid"] = pid

    headers = auth_headers(layout)
    async with httpx.AsyncClient(timeout=30.0) as client:
        for probe in http_probes:
            raw_path = probe["path"]
            resolved_path = (
                raw_path.replace("{session_id}", session_id)
                if session_id
                else raw_path
            )
            entry: dict[str, Any] = {"name": probe["name"], "path": resolved_path}
            try:
                res = await client.get(f"{api_url.rstrip('/')}{resolved_path}", headers=headers)
                entry["status"] = res.status_code
                with contextlib.suppress(ValueError):
                    entry["json"] = res.json()
            except (httpx.HTTPError, OSError) as error:
                entry["error"] = f"{type(error).__name__}: {error}"
            record["http"].append(entry)

    for probe in sql_probes:
        entry = {"name": probe["name"], "query": probe["query"]}
        try:
            entry["rows"] = _sql_probe(layout.db_path, probe["query"])
        except sqlite3.Error as error:
            entry["error"] = f"{type(error).__name__}: {error}"
        record["sql"].append(entry)

    return record


def write_restart_evidence(logs_dir: Path, record: dict[str, Any]) -> Path:
    """Persist the snapshot next to the other session E2E evidence files."""
    logs_dir.mkdir(parents=True, exist_ok=True)
    path = logs_dir / "session_e2e_restart_evidence.json"
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return path
