"""resolve_pending_interactions must record whether the answer was delivered.

``handle_user_prompt`` can answer 200 with ``{"delivered": false}`` when the
prompt was already gone. The evidence must carry that flag so the task
verifier rejects an undelivered answer instead of counting it as resolved.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from benchmarks.harbor.session_e2e.automation import (
    _clean_execution_mode,
    resolve_pending_interactions,
)


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
        self.post_urls: list[str] = []

    async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
        return FakeResponse(self._interactions)

    async def post(
        self,
        url: str,
        headers: dict[str, str] | None = None,
        json: dict[str, Any] | None = None,
    ) -> FakeResponse:
        self.respond_bodies.append(json or {})
        self.post_urls.append(url)
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
        {
            "kind": "userPrompt",
            "requestId": "req-1",
            "action": "answer",
            "delivered": False,
            "injectedValue": "e2e-synthetic-answer",
        }
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
        {
            "kind": "userPrompt",
            "requestId": "req-1",
            "action": "answer",
            "delivered": True,
            "injectedValue": "e2e-synthetic-answer",
        }
    ]


@pytest.mark.asyncio
async def test_injected_value_records_the_env_synthetic_secret(tmp_path, monkeypatch):
    # The verifier reads the exact injected value back from evidence
    # (`injectedValue`); the container does not see the host env var.
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    monkeypatch.setenv("LIBRAGENT_E2E_SYNTHETIC_SECRET", "ci-set-fixture-9f3a")
    client = FakeClient(_snapshot(), {"delivered": True})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(
            auto_answer_interactive=True,
            synthetic_secret_env="LIBRAGENT_E2E_SYNTHETIC_SECRET",
        ),
    )

    assert actions[0]["injectedValue"] == "ci-set-fixture-9f3a"
    assert client.respond_bodies[0]["value"] == "ci-set-fixture-9f3a"


@pytest.mark.asyncio
async def test_cancel_interactive_spec_cancels_the_prompt(tmp_path, monkeypatch):
    """cancel_interactive=true must drive the control-plane cancel path
    (2672 C-4 / 2565 A-3: dialog cancelled -> informational result -> the
    session continues). No value, no injectedValue on a cancel."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(_snapshot(), {"delivered": True})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(cancel_interactive=True),
    )

    assert actions == [
        {"kind": "userPrompt", "requestId": "req-1", "action": "cancel", "delivered": True}
    ]
    assert client.respond_bodies[0] == {"requestId": "req-1", "action": "cancel"}


@pytest.mark.asyncio
async def test_pending_circuit_break_is_resumed(tmp_path, monkeypatch):
    """A waiting ui__circuitBreak must be driven through the Resume endpoint (#2901)."""
    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(
        {
            "pendingUserPrompt": None,
            "pendingApprovals": [],
            "pendingCircuitBreak": {
                "toolName": "workspace__writeFile",
                "repetitionCount": 5,
                "toolCallId": "call-break",
            },
        },
        {"ok": True, "resumedTool": "workspace__writeFile", "repetitionCount": 5},
    )

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=None,
    )

    assert actions == [
        {
            "kind": "circuitBreakResume",
            "resumedTool": "workspace__writeFile",
            "repetitionCount": 5,
        }
    ]
    assert client.post_urls == [
        "http://127.0.0.1:3031/api/e2e-control/sessions/sess-1/circuit-break-resume"
    ]


@pytest.mark.asyncio
async def test_circuit_break_resume_disabled_by_spec(tmp_path, monkeypatch):
    """[metadata.libragent_session_e2e] resume_circuit_break=false must not resume."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(
        {
            "pendingUserPrompt": None,
            "pendingApprovals": [],
            "pendingCircuitBreak": {"toolName": "x", "repetitionCount": 5},
        },
        {"ok": True},
    )

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(resume_circuit_break=False),
    )

    assert actions == []
    assert client.post_urls == []


def test_execution_mode_override_parsing():
    """A task's execution_mode is trimmed/lowercased; blank or non-string means inherit."""
    assert _clean_execution_mode(" Normal ") == "normal"
    assert _clean_execution_mode("yolo") == "yolo"
    assert _clean_execution_mode("") is None
    assert _clean_execution_mode("   ") is None
    assert _clean_execution_mode(None) is None
    assert _clean_execution_mode(7) is None


def test_execution_mode_defaults_to_none_on_spec():
    """Absent metadata key must not change the run-level execution mode."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    assert SessionE2eAutomationSpec().execution_mode is None
