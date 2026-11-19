"""Restart-phase evidence for restart-dependent Harbor rows (#2903)."""

from __future__ import annotations

import json
import sqlite3
from pathlib import Path
from typing import Any

import pytest

import benchmarks.harbor.session_e2e.restart as restart_mod
from benchmarks.harbor.isolated_instance import InstanceError, InstanceLayout
from benchmarks.harbor.session_e2e.automation import load_session_e2e_spec
from benchmarks.harbor.session_e2e.restart import restart_and_snapshot, write_restart_evidence


# --- spec parsing ------------------------------------------------------------


def _write_task(logs_dir: Path, task_toml: str) -> None:
    task_dir = logs_dir.parent / "task"
    task_dir.mkdir(parents=True, exist_ok=True)
    (task_dir / "task.toml").write_text(task_toml, encoding="utf-8")
    (logs_dir.parent / "config.json").write_text(
        json.dumps({"task": {"path": str(task_dir)}}), encoding="utf-8"
    )


def test_spec_defaults_keep_restart_off(tmp_path: Path) -> None:
    logs = tmp_path / "logs"
    logs.mkdir()
    _write_task(logs, '[metadata.libragent_session_e2e]\nmatrix_id = "X-1"\n')

    spec = load_session_e2e_spec(logs)

    assert spec is not None
    assert spec.restart_after_completion is False
    assert spec.post_restart_http_probes == ()
    assert spec.post_restart_sql_probes == ()


def test_spec_parses_restart_probes_and_drops_malformed(tmp_path: Path) -> None:
    logs = tmp_path / "logs"
    logs.mkdir()
    _write_task(
        logs,
        """
[metadata.libragent_session_e2e]
restart_after_completion = true
post_restart_http_probes = [
  { name = "health", path = "/health" },
  { name = "broken" },
]
post_restart_sql_probes = [
  { name = "servers", query = "SELECT id FROM mcp_servers" },
]
""",
    )

    spec = load_session_e2e_spec(logs)

    assert spec is not None
    assert spec.restart_after_completion is True
    assert spec.post_restart_http_probes == ({"name": "health", "path": "/health"},)
    assert spec.post_restart_sql_probes == (
        {"name": "servers", "query": "SELECT id FROM mcp_servers"},
    )


# --- restart_and_snapshot ----------------------------------------------------


class _FakeResponse:
    def __init__(self, status: int, payload: Any) -> None:
        self.status_code = status
        self._payload = payload

    def json(self) -> Any:
        if self._payload is None:
            raise ValueError("not json")
        return self._payload


class _FakeClient:
    def __init__(self, behavior: dict[str, tuple[int, Any]], calls: list[dict[str, Any]]) -> None:
        self._behavior = behavior
        self.calls = calls

    async def __aenter__(self) -> _FakeClient:
        return self

    async def __aexit__(self, *args: object) -> bool:
        return False

    async def get(self, url: str, headers: dict[str, str] | None = None) -> _FakeResponse:
        self.calls.append({"url": url, "headers": headers or {}})
        for fragment, (status, payload) in self._behavior.items():
            if url.endswith(fragment):
                return _FakeResponse(status, payload)
        return _FakeResponse(404, None)


def _layout_with_db(tmp_path: Path) -> InstanceLayout:
    layout = InstanceLayout(tmp_path / "bench")
    layout.data_dir.mkdir(parents=True)
    conn = sqlite3.connect(layout.db_path)
    conn.execute("CREATE TABLE mcp_servers (id TEXT PRIMARY KEY, config TEXT)")
    conn.execute("INSERT INTO mcp_servers VALUES ('keep-1', '{\"secret\": true}')")
    conn.commit()
    conn.close()
    return layout


@pytest.fixture
def patched_restart(monkeypatch: pytest.MonkeyPatch):
    """Patch the process-level restart + httpx client; yield recorded state."""
    state: dict[str, Any] = {"calls": [], "restartArgs": None}

    def fake_restart(layout: InstanceLayout, app_bin: Path, *, timeout_sec: float) -> int:
        state["restartArgs"] = {"layout": layout, "app_bin": app_bin, "timeout": timeout_sec}
        return 4242

    monkeypatch.setattr(restart_mod, "restart", fake_restart)
    return state


@pytest.mark.asyncio
async def test_restart_snapshot_records_health_and_probes(tmp_path: Path, monkeypatch, patched_restart) -> None:
    layout = _layout_with_db(tmp_path)
    layout.api_key_file.write_text("libr_key_for_post_restart\n", encoding="utf-8")
    behavior = {"/health": (200, {"status": "ok"}), "/mcp-servers-status": (200, {"keep-1": "connected"})}
    monkeypatch.setattr(
        restart_mod.httpx,
        "AsyncClient",
        lambda **kw: _FakeClient(behavior, patched_restart["calls"]),
    )

    record = await restart_and_snapshot(
        layout,
        tmp_path / "appbin",
        api_url="http://127.0.0.1:3031/api",
        http_probes=(
            {"name": "health", "path": "/health"},
            {"name": "servers", "path": "/mcp-servers-status"},
        ),
        sql_probes=(
            {"name": "kept", "query": "SELECT id FROM mcp_servers"},
            {"name": "bad", "query": "UPDATE mcp_servers SET config = 'x'"},
        ),
    )

    assert record["restartExecuted"] is True
    assert record["pid"] == 4242
    assert record["dataDir"] == str(layout.data_dir.resolve())
    assert patched_restart["restartArgs"]["layout"].root == layout.root

    by_name = {entry["name"]: entry for entry in record["http"]}
    assert by_name["health"]["status"] == 200
    assert by_name["health"]["json"] == {"status": "ok"}
    assert by_name["servers"]["json"] == {"keep-1": "connected"}
    # The harness must authenticate with the *post-restart* key.
    assert all(
        call["headers"].get("Authorization") == "Bearer libr_key_for_post_restart"
        for call in patched_restart["calls"]
    )

    sql = {entry["name"]: entry for entry in record["sql"]}
    assert sql["kept"]["rows"] == [["keep-1"]]
    assert "error" in sql["bad"]  # read-only violation recorded, not raised


@pytest.mark.asyncio
async def test_restart_failure_is_recorded_not_raised(tmp_path: Path, monkeypatch) -> None:
    layout = _layout_with_db(tmp_path)

    def failing_restart(layout: InstanceLayout, app_bin: Path, *, timeout_sec: float) -> int:
        raise InstanceError("instance did not become healthy")

    monkeypatch.setattr(restart_mod, "restart", failing_restart)

    record = await restart_and_snapshot(
        layout, tmp_path / "appbin", api_url="http://127.0.0.1:3031/api"
    )

    assert record["restartExecuted"] is False
    assert "did not become healthy" in record["error"]
    assert record["http"] == [] and record["sql"] == []


def test_write_restart_evidence_lands_next_to_the_other_evidence(tmp_path: Path) -> None:
    logs = tmp_path / "logs"
    path = write_restart_evidence(logs, {"restartExecuted": True})

    assert path.name == "session_e2e_restart_evidence.json"
    assert path.parent == logs
    assert json.loads(path.read_text(encoding="utf-8")) == {"restartExecuted": True}


@pytest.mark.asyncio
async def test_restart_snapshot_substitutes_session_id_in_http_probes(
    tmp_path: Path, monkeypatch, patched_restart
) -> None:
    layout = _layout_with_db(tmp_path)
    layout.api_key_file.write_text("libr_key_post_restart\n", encoding="utf-8")
    behavior = {
        "/sessions/sess-42/messages": (200, {"messages": [{"role": "tool", "content": []}]}),
    }
    monkeypatch.setattr(
        restart_mod.httpx,
        "AsyncClient",
        lambda **kw: _FakeClient(behavior, patched_restart["calls"]),
    )

    record = await restart_and_snapshot(
        layout,
        tmp_path / "appbin",
        api_url="http://127.0.0.1:3031/api",
        http_probes=({"name": "messages", "path": "/sessions/{session_id}/messages"},),
        session_id="sess-42",
    )

    assert record["restartExecuted"] is True
    assert record["http"][0]["path"] == "/sessions/sess-42/messages"
    assert record["http"][0]["status"] == 200
    assert record["http"][0]["json"]["messages"][0]["role"] == "tool"
