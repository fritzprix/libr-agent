"""resolve_pending_interactions must record whether the answer was delivered.

``handle_user_prompt`` can answer 200 with ``{"delivered": false}`` when the
prompt was already gone. The evidence must carry that flag so the task
verifier rejects an undelivered answer instead of counting it as resolved.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from benchmarks.harbor.libragent_agent import DEFAULT_POLL_INTERVAL_SEC
from benchmarks.harbor.session_e2e.automation import (
    _clean_execution_mode,
    resolve_pending_interactions,
)
from benchmarks.harbor.session_e2e.control import E2E_HEADER, E2eControlClient


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
            "sessionId": "sess-1",
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
            "sessionId": "sess-1",
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
        {
            "kind": "userPrompt",
            "sessionId": "sess-1",
            "requestId": "req-1",
            "action": "cancel",
            "delivered": True,
        }
    ]
    assert client.respond_bodies[0] == {"requestId": "req-1", "action": "cancel"}


@pytest.mark.asyncio
async def test_answer_value_overrides_the_synthetic_secret(tmp_path, monkeypatch):
    """A task's answer_value must be injected verbatim (multi-question
    ui__askUser expects a JSON object keyed by question id, 2565 A-1/A-2)."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(_snapshot(), {"delivered": True})
    answer = '{"city":"Seoul","toppings":["Cheese","Onion"],"note":"e2e-note"}'

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(answer_value=answer),
    )

    assert actions[0]["injectedValue"] == answer
    assert client.respond_bodies[0]["value"] == answer


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
            "sessionId": "sess-1",
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


@pytest.mark.asyncio
async def test_resolve_child_interactions_answers_child_session_first(tmp_path, monkeypatch):
    """2565 X-2 (smoke): with resolve_child_interactions, the child session's
    waiting prompt is answered **before** the primary's, and the child's
    injected value is suffixed. The -child suffix is a harness marker on the
    evidence records — it checks the harness plumbing, not product answer-
    channel isolation. The primary keeps the plain value."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    monkeypatch.setenv("LIBRAGENT_E2E_SYNTHETIC_SECRET", "x2-fixture-7c")
    data_dir = _instance_data_dir(tmp_path, monkeypatch)

    seen_sessions: list[str] = []

    class PerSessionClient(FakeClient):
        async def get(self, url, headers=None):  # type: ignore[override]
            sid = url.rsplit("/sessions/", 1)[1].split("/")[0]
            seen_sessions.append(sid)
            return FakeResponse(
                {"pendingUserPrompt": {"requestId": f"req-{sid}", "kind": "secret"},
                 "pendingApprovals": []}
            )

    client = PerSessionClient(_snapshot(), {"delivered": True})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-a",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(resolve_child_interactions=True),
        child_session_ids=["sess-b"],
    )

    assert [a["sessionId"] for a in actions] == ["sess-b", "sess-a"]  # child answered first
    assert actions[0]["injectedValue"] == "x2-fixture-7c-child"
    assert actions[1]["injectedValue"] == "x2-fixture-7c"
    assert client.respond_bodies[0]["value"] == "x2-fixture-7c-child"
    assert client.respond_bodies[1]["value"] == "x2-fixture-7c"


@pytest.mark.asyncio
async def test_resolve_child_interactions_off_ignores_child_ids(tmp_path, monkeypatch):
    """Without the flag, child_session_ids must be ignored entirely — one
    interaction read and one answer, for the primary session only."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    client = FakeClient(_snapshot(), {"delivered": True})

    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(resolve_child_interactions=False),
        child_session_ids=["sess-b", "sess-c"],
    )

    assert [a["sessionId"] for a in actions] == ["sess-1"]
    assert client.respond_bodies == [{"requestId": "req-1", "action": "answer",
                                      "value": "e2e-synthetic-answer"}]


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


@pytest.mark.asyncio
async def test_wait_for_overlapping_child_prompt_holds_until_both_pending(tmp_path, monkeypatch):
    """2565 X-2: hold answers while only one session has a prompt; once both
    have pending prompts in the same poll, answer child first and mark both
    records with simultaneousPending=True."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    monkeypatch.setenv("LIBRAGENT_E2E_SYNTHETIC_SECRET", "x2-overlap-secret")
    data_dir = _instance_data_dir(tmp_path, monkeypatch)

    class RoutingClient:
        def __init__(self, snapshots: dict[str, dict[str, Any]]) -> None:
            self.snapshots = snapshots
            self.posts: list[tuple[str, dict[str, Any]]] = []

        async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
            for sid, snap in self.snapshots.items():
                if f"/sessions/{sid}/interactions" in url:
                    return FakeResponse(snap)
            return FakeResponse({"pendingUserPrompt": None, "pendingApprovals": []})

        async def post(
            self,
            url: str,
            headers: dict[str, str] | None = None,
            json: dict[str, Any] | None = None,
        ) -> FakeResponse:
            self.posts.append((url, json or {}))
            return FakeResponse({"delivered": True})

    spec = SessionE2eAutomationSpec(
        resolve_child_interactions=True,
        wait_for_overlapping_child_prompt=True,
    )

    # Only primary pending -> held, no prompt answered yet.
    client_one = RoutingClient(
        {
            "sess-a": {"pendingUserPrompt": {"requestId": "req-a"}, "pendingApprovals": []},
            "sess-b": {"pendingUserPrompt": None, "pendingApprovals": []},
        }
    )
    held = await resolve_pending_interactions(
        client_one,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-a",
        data_dir=data_dir,
        spec=spec,
        child_session_ids=["sess-b"],
    )
    assert held == []
    assert client_one.posts == []

    # Both pending -> child answered first, primary second, both flagged simultaneousPending.
    client_both = RoutingClient(
        {
            "sess-a": {"pendingUserPrompt": {"requestId": "req-a"}, "pendingApprovals": []},
            "sess-b": {"pendingUserPrompt": {"requestId": "req-b"}, "pendingApprovals": []},
        }
    )
    actions = await resolve_pending_interactions(
        client_both,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-a",
        data_dir=data_dir,
        spec=spec,
        child_session_ids=["sess-b"],
    )
    assert [a["sessionId"] for a in actions] == ["sess-b", "sess-a"]
    assert all(a.get("simultaneousPending") is True for a in actions)
    assert actions[0]["injectedValue"] == "x2-overlap-secret-child"
    assert actions[1]["injectedValue"] == "x2-overlap-secret"


@pytest.mark.asyncio
async def test_distinct_secret_per_prompt_increments_version_suffix(tmp_path, monkeypatch):
    """2565 B-8 / M-8: successive prompts in the same session get distinct
    injected values (-v1, -v2) when distinct_secret_per_prompt is enabled."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    monkeypatch.setenv("LIBRAGENT_E2E_SYNTHETIC_SECRET", "b8-secret")
    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    spec = SessionE2eAutomationSpec(distinct_secret_per_prompt=True)

    client1 = FakeClient(_snapshot(), {"delivered": True})
    first = await resolve_pending_interactions(
        client1,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=spec,
        prior_actions=[],
    )
    assert first[0]["injectedValue"] == "b8-secret-v1"

    client2 = FakeClient(
        {"pendingUserPrompt": {"requestId": "req-2", "kind": "secret"}, "pendingApprovals": []},
        {"delivered": True},
    )
    second = await resolve_pending_interactions(
        client2,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=spec,
        prior_actions=first,
    )
    assert second[0]["injectedValue"] == "b8-secret-v2"
    assert first[0]["injectedValue"] != second[0]["injectedValue"]


@pytest.mark.asyncio
async def test_terminate_child_on_prompt_terminates_and_checks_cleared(tmp_path, monkeypatch):
    """2565 P-6: when terminate_child_on_prompt is set, a child session waiting
    on a prompt is terminated via POST /sessions/:id/terminate and verified cleared."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)

    class TermClient:
        def __init__(self) -> None:
            self.terminated = False
            self.post_urls: list[str] = []

        async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
            if "/sessions/sess-child/interactions" in url:
                if self.terminated:
                    return FakeResponse({"pendingUserPrompt": None, "pendingApprovals": []})
                return FakeResponse(
                    {"pendingUserPrompt": {"requestId": "req-child"}, "pendingApprovals": []}
                )
            return FakeResponse({"pendingUserPrompt": None, "pendingApprovals": []})

        async def post(
            self,
            url: str,
            headers: dict[str, str] | None = None,
            json: dict[str, Any] | None = None,
        ) -> FakeResponse:
            self.post_urls.append(url)
            if url.endswith("/sessions/sess-child/terminate"):
                self.terminated = True
                return FakeResponse({"status": "terminated"})
            return FakeResponse({"delivered": True})

    client = TermClient()
    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-parent",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(
            resolve_child_interactions=True,
            terminate_child_on_prompt=True,
        ),
        child_session_ids=["sess-child"],
    )
    assert client.post_urls == ["http://127.0.0.1:3031/api/sessions/sess-child/terminate"]
    assert actions == [
        {
            "kind": "terminateOnPrompt",
            "sessionId": "sess-child",
            "requestId": "req-child",
            "terminated": True,
            "promptCleared": True,
        }
    ]


@pytest.mark.asyncio
async def test_tool_approval_records_tool_name_and_command_preview(tmp_path, monkeypatch):
    """2672 M-3: toolApproval evidence records preserve toolName and
    commandPreview so verifiers can assert preview masking."""
    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    snapshot = {
        "pendingUserPrompt": None,
        "pendingApprovals": [
            {
                "toolCallId": "call-mcp-1",
                "toolName": "tool__registerServer",
                "approvalKind": "hard",
                "commandPreview": "url=https://127.0.0.1:9999/mcp?literal=***&k={{secret:KEY}}#***",
            }
        ],
    }
    client = FakeClient(snapshot, {"approved": True})
    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-1",
        data_dir=data_dir,
        spec=None,
    )
    assert actions == [
        {
            "kind": "toolApproval",
            "sessionId": "sess-1",
            "toolCallId": "call-mcp-1",
            "approvalKind": "hard",
            "approved": True,
            "toolName": "tool__registerServer",
            "commandPreview": "url=https://127.0.0.1:9999/mcp?literal=***&k={{secret:KEY}}#***",
        }
    ]


# --- Polling contract (#2903) ------------------------------------------------
#
# The module docstring of ``benchmarks/harbor/session_e2e/control.py`` is the
# documented polling contract: endpoint paths, the capability header, the
# snapshot shape, and the poll interval. These tests pin the client half of
# that contract so a silent drift between docstring and code fails here.


class RecordingClient(FakeClient):
    """FakeClient that also records every request URL and header."""

    def __init__(self, responses: dict[str, dict[str, Any]]) -> None:
        super().__init__({}, {})
        self._responses = responses
        self.requests: list[dict[str, Any]] = []

    def _match(self, url: str) -> dict[str, Any]:
        for fragment, payload in self._responses.items():
            if fragment in url:
                return payload
        raise AssertionError(f"unexpected url: {url}")

    async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
        self.requests.append({"method": "GET", "url": url, "headers": headers})
        return FakeResponse(self._match(url))

    async def post(
        self,
        url: str,
        headers: dict[str, str] | None = None,
        json: dict[str, Any] | None = None,
    ) -> FakeResponse:
        self.requests.append({"method": "POST", "url": url, "headers": headers, "json": json})
        return FakeResponse(self._match(url))


def _client() -> E2eControlClient:
    return E2eControlClient("http://127.0.0.1:3031/api", token="t" * 48)


@pytest.mark.asyncio
async def test_interactions_uses_documented_path_and_capability_header():
    snapshot = {
        "sessionId": "sess-1",
        "pendingUserPrompt": {
            "requestId": "req-1",
            "kind": "ask_user",
            "payload": {},
            "createdAt": "2026-11-19T00:00:00Z",
        },
        "pendingApprovals": [],
    }
    fake = RecordingClient({"/interactions": snapshot})

    result = await _client().interactions(fake, "sess-1")

    assert result == snapshot  # snapshot passes through unchanged
    (req,) = fake.requests
    assert req["method"] == "GET"
    assert req["url"].endswith("/e2e-control/sessions/sess-1/interactions")
    assert req["headers"] == {E2E_HEADER: "t" * 48}


@pytest.mark.asyncio
async def test_user_prompt_response_sends_documented_body():
    fake = RecordingClient({"/user-prompts": {"delivered": True}})

    result = await _client().respond_user_prompt(fake, "sess-1", "req-1", value="42")

    assert result == {"delivered": True}
    (req,) = fake.requests
    assert req["method"] == "POST"
    assert req["url"].endswith("/e2e-control/sessions/sess-1/user-prompts")
    assert req["headers"] == {E2E_HEADER: "t" * 48}
    assert req["json"] == {"requestId": "req-1", "action": "answer", "value": "42"}


@pytest.mark.asyncio
async def test_cancel_action_and_gone_prompt_passthrough():
    fake = RecordingClient({"/user-prompts": {"delivered": False}})

    result = await _client().respond_user_prompt(
        fake,
        "sess-1",
        "req-1",
        action="cancel",
    )

    # {delivered: false} means the prompt was already gone; the client must
    # pass it through untouched so the poller re-polls instead of retrying.
    assert result == {"delivered": False}
    (req,) = fake.requests
    assert req["json"] == {"requestId": "req-1", "action": "cancel"}
    assert "value" not in req["json"]


@pytest.mark.asyncio
async def test_tool_approval_uses_documented_endpoint():
    fake = RecordingClient({"/tool-approvals": {"ok": True}})

    result = await _client().respond_tool_approval(fake, "sess-1", "call-9", approved=True)

    assert result == {"ok": True}
    (req,) = fake.requests
    assert req["url"].endswith("/e2e-control/sessions/sess-1/tool-approvals")
    assert req["json"] == {"toolCallId": "call-9", "approved": True}


def test_poll_interval_matches_documented_default():
    # The contract documents 3.0 s as the default poll interval; the bench
    # poller must keep using exactly that constant.
    assert DEFAULT_POLL_INTERVAL_SEC == 3.0
