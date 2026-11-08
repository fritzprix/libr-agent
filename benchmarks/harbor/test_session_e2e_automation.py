"""resolve_pending_interactions must record whether the answer was delivered.

``handle_user_prompt`` can answer 200 with ``{"delivered": false}`` when the
prompt was already gone. The evidence must carry that flag so the task
verifier rejects an undelivered answer instead of counting it as resolved.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from benchmarks.harbor.session_e2e.automation import resolve_pending_interactions


class FakeResponse:
    def __init__(self, payload: dict[str, Any]) -> None:
        self._payload = payload

    def raise_for_status(self) -> None:
        return None

    def json(self) -> dict[str, Any]:
        return self._payload


class FakeClient:
    def __init__(self, interactions: dict[str, Any], respond_payload: dict[str, Any]) -> None:
        self._interactions = interactions
        self._respond_payload = respond_payload
        self.respond_bodies: list[dict[str, Any]] = []

    async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
        return FakeResponse(self._interactions)

    async def post(
        self,
        url: str,
        headers: dict[str, str] | None = None,
        json: dict[str, Any] | None = None,
    ) -> FakeResponse:
        self.respond_bodies.append(json or {})
        return FakeResponse(self._respond_payload)


def _instance_data_dir(tmp_path: Path, monkeypatch) -> Path:
    monkeypatch.setenv("LIBRAGENT_E2E_CONTROL", "1")
    data_dir = tmp_path / "data"
    data_dir.mkdir()
    (data_dir / "e2e_control.cap").write_text("0" * 48, encoding="utf-8")
    return data_dir


def _snapshot() -> dict[str, Any]:
    return {
        "pendingUserPrompt": {"requestId": "req-1", "kind": "secret"},
        "pendingApprovals": [],
    }


@pytest.mark.asyncio
async def test_undelivered_answer_is_recorded_as_false(tmp_path, monkeypatch):
    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(_snapshot(), {"delivered": False})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=None,
    )

    assert actions == [
        {"kind": "userPrompt", "requestId": "req-1", "action": "answer", "delivered": False}
    ]
    # The verifier's filter for a resolved prompt must find nothing here.
    assert not [
        a for a in actions if a.get("delivered") is True and a.get("action") == "answer"
    ]


@pytest.mark.asyncio
async def test_delivered_answer_is_recorded_as_true(tmp_path, monkeypatch):
    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(_snapshot(), {"delivered": True})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=None,
    )

    assert actions == [
        {"kind": "userPrompt", "requestId": "req-1", "action": "answer", "delivered": True}
    ]
