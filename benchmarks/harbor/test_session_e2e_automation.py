"""resolve_pending_interactions must record whether the answer was delivered.

``handle_user_prompt`` can answer 200 with ``{"delivered": false}`` when the
prompt was already gone. The evidence must carry that flag so the task
verifier rejects an undelivered answer instead of counting it as resolved.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import httpx
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
    # Pins the full documented snapshot shape (#2916 G1.4): every field the
    # control.py module docstring documents must pass through unchanged,
    # including pendingCircuitBreak (#2901).
    snapshot = {
        "sessionId": "sess-1",
        "pendingUserPrompt": {
            "requestId": "req-1",
            "kind": "ask_user",
            "payload": {},
            "createdAt": "2026-11-19T00:00:00Z",
        },
        "pendingApprovals": [
            {
                "toolCallId": "tc-1",
                "toolName": "workspace__runShell",
                "reason": "requires approval",
                "commandPreview": "sudo true",
                "approvalKind": "tool",
                "requestId": "req-2",
            }
        ],
        "pendingCircuitBreak": {
            "toolName": "workspace__writeFile",
            "repetitionCount": 5,
            "toolCallId": "tc-9",
        },
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


@pytest.mark.asyncio
async def test_circuit_break_resume_409_surfaces_as_http_status_error():
    """Pins the documented no-circuit-break response (#2916 G2.1).

    ``plan_circuit_break_resume`` answers HTTP 409 with ``{error}`` when no
    circuit break is waiting; the contract says the client surfaces that via
    ``raise_for_status()`` as ``httpx.HTTPStatusError`` — not as
    ``{ok: false}``. Uses a real ``httpx.AsyncClient`` over a mock transport
    so the actual ``raise_for_status`` path runs.
    """

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path.endswith(
            "/e2e-control/sessions/sess-1/circuit-break-resume"
        )
        assert request.headers[E2E_HEADER] == "t" * 48
        return httpx.Response(
            409,
            json={"error": "no circuit breaker is waiting for resume on this session"},
        )

    async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as http:
        with pytest.raises(httpx.HTTPStatusError) as excinfo:
            await _client().resume_circuit_break(http, "sess-1")

    assert excinfo.value.response.status_code == 409
    assert "no circuit breaker" in excinfo.value.response.json()["error"]


@pytest.mark.asyncio
async def test_picker_arm_uses_documented_endpoint_and_body():
    fake = RecordingClient({"/picker": {"ok": True}})

    result = await _client().arm_picker(fake, path="/tmp/e2e-out.bin")

    assert result == {"ok": True}
    (req,) = fake.requests
    assert req["method"] == "POST"
    assert req["url"].endswith("/e2e-control/picker")
    assert req["headers"] == {E2E_HEADER: "t" * 48}
    assert req["json"] == {"path": "/tmp/e2e-out.bin"}

    fake = RecordingClient({"/picker": {"ok": True}})
    await _client().arm_picker(fake, cancel=True)
    (req,) = fake.requests
    assert req["json"] == {"cancel": True}


@pytest.mark.asyncio
async def test_picker_arm_requires_exactly_one_choice():
    fake = RecordingClient({"/picker": {"ok": True}})
    with pytest.raises(ValueError):
        await _client().arm_picker(fake)
    with pytest.raises(ValueError):
        await _client().arm_picker(fake, path="/x", cancel=True)
    assert fake.requests == []


@pytest.mark.asyncio
async def test_set_window_uses_documented_endpoint_and_sends_only_given_fields():
    state = {"visible": True, "width": 800.0, "height": 600.0}
    fake = RecordingClient({"/window": state})

    result = await _client().set_window(fake, width=800, height=600)

    assert result == state
    (req,) = fake.requests
    assert req["method"] == "POST"
    assert req["url"].endswith("/e2e-control/window")
    assert req["headers"] == {E2E_HEADER: "t" * 48}
    assert req["json"] == {"width": 800, "height": 600}

    fake = RecordingClient({"/window": state})
    await _client().set_window(fake, visible=False)
    (req,) = fake.requests
    assert req["json"] == {"visible": False}


@pytest.mark.asyncio
async def test_set_window_rejects_empty_or_lone_dimension():
    fake = RecordingClient({"/window": {}})
    with pytest.raises(ValueError):
        await _client().set_window(fake)
    with pytest.raises(ValueError):
        await _client().set_window(fake, width=800)
    with pytest.raises(ValueError):
        await _client().set_window(fake, height=600)
    assert fake.requests == []


def test_poll_interval_matches_documented_default():
    # The contract documents 3.0 s as the default poll interval; the bench
    # poller must keep using exactly that constant.
    assert DEFAULT_POLL_INTERVAL_SEC == 3.0


@pytest.mark.asyncio
async def test_re_poll_prompt_before_answer_defers_first_poll_then_answers(tmp_path, monkeypatch):
    """2565 P-5 / X-5: first poll records userPromptObserved + refetches session
    without answering; second poll answers and sets survivedRepoll=True."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)

    class RepollClient:
        def __init__(self) -> None:
            self.gets: list[str] = []
            self.posts: list[tuple[str, dict[str, Any]]] = []

        async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
            self.gets.append(url)
            if url.endswith("/interactions"):
                return FakeResponse(
                    {"pendingUserPrompt": {"requestId": "req-p5"}, "pendingApprovals": []}
                )
            return FakeResponse({"id": "sess-p5", "status": "busy"})

        async def post(
            self,
            url: str,
            headers: dict[str, str] | None = None,
            json: dict[str, Any] | None = None,
        ) -> FakeResponse:
            self.posts.append((url, json or {}))
            return FakeResponse({"delivered": True})

    client = RepollClient()
    spec = SessionE2eAutomationSpec(
        re_poll_prompt_before_answer=True,
        answer_value='{"choice":"alpha"}',
    )

    first = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-p5",
        data_dir=data_dir,
        spec=spec,
        prior_actions=[],
    )
    assert len(first) == 1
    assert first[0]["kind"] == "userPromptObserved"
    assert first[0]["requestId"] == "req-p5"
    assert first[0]["sessionRefetched"] is True
    assert client.posts == []
    assert any(u.endswith("/sessions/sess-p5") for u in client.gets)

    second = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-p5",
        data_dir=data_dir,
        spec=spec,
        prior_actions=first,
    )
    assert len(second) == 1
    assert second[0]["kind"] == "userPrompt"
    assert second[0]["delivered"] is True
    assert second[0]["survivedRepoll"] is True
    assert second[0]["injectedValue"] == '{"choice":"alpha"}'
    assert len(client.posts) == 1


@pytest.mark.asyncio
async def test_restart_while_prompt_waiting_emits_restart_without_answering(tmp_path, monkeypatch):
    """2565 P-8 / 2672 A-4: restart_while_prompt_waiting emits restartOnPrompt
    and never submits an answer or cancel."""
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)

    class WaitingClient:
        def __init__(self) -> None:
            self.posts: list[tuple[str, dict[str, Any]]] = []

        async def get(self, url: str, headers: dict[str, str] | None = None) -> FakeResponse:
            return FakeResponse(
                {"pendingUserPrompt": {"requestId": "req-p8"}, "pendingApprovals": []}
            )

        async def post(
            self,
            url: str,
            headers: dict[str, str] | None = None,
            json: dict[str, Any] | None = None,
        ) -> FakeResponse:
            self.posts.append((url, json or {}))
            return FakeResponse({"delivered": True})

    client = WaitingClient()
    actions = await resolve_pending_interactions(
        client,
        api_url="http://127.0.0.1:3031/api",
        session_id="sess-p8",
        data_dir=data_dir,
        spec=SessionE2eAutomationSpec(restart_while_prompt_waiting=True),
    )
    assert actions == [{"kind": "restartOnPrompt", "sessionId": "sess-p8", "requestId": "req-p8"}]
    assert client.posts == []


@pytest.mark.asyncio
async def test_picker_take_uses_documented_endpoint():
    fake = RecordingClient({"/picker/take": {"override": {"action": "path", "path": "/tmp/a"}}})

    result = await _client().take_picker(fake)

    assert result == {"override": {"action": "path", "path": "/tmp/a"}}
    (req,) = fake.requests
    assert req["method"] == "POST"
    assert req["url"].endswith("/e2e-control/picker/take")
    assert req["headers"] == {E2E_HEADER: "t" * 48}


@pytest.mark.asyncio
async def test_terminate_checks_persistent_shell_lifecycle(tmp_path, monkeypatch):
    """2565 P-6 Windows half (#2905): with check_persistent_shell_after_terminate,
    the terminateOnPrompt evidence records the persistent shell PID from the
    shared workspace, alive while the child prompt waits, gone after terminate,
    and still gone after the settle window."""
    from benchmarks.harbor.session_e2e import automation
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    workspace = data_dir / "workspaces" / "sess-parent"
    workspace.mkdir(parents=True)
    (workspace / "p6_shell_pid.txt").write_text("4242\n", encoding="ascii")

    alive_calls: list[int] = []

    def fake_alive(pid: int) -> bool:
        alive_calls.append(pid)
        return len(alive_calls) == 1  # alive only while the prompt waits

    monkeypatch.setattr(automation, "_process_is_alive", fake_alive)
    monkeypatch.setattr(automation, "PROCESS_GONE_TIMEOUT_SEC", 0.0)
    monkeypatch.setattr(automation, "PROCESS_SETTLE_SEC", 0.0)

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
            check_persistent_shell_after_terminate=True,
        ),
        child_session_ids=["sess-child"],
    )
    assert actions == [
        {
            "kind": "terminateOnPrompt",
            "sessionId": "sess-child",
            "requestId": "req-child",
            "terminated": True,
            "promptCleared": True,
            "shellPid": 4242,
            "shellAliveWhileWaiting": True,
            "shellGoneAfterTerminate": True,
            "shellStillGoneAfterSettle": True,
        }
    ]


class _TasklistDone:
    def __init__(self, stdout: str, returncode: int = 0) -> None:
        self.stdout = stdout
        self.returncode = returncode


def test_windows_process_probe_matches_pid_column_only(monkeypatch):
    """G1.2: the tasklist probe compares the PID column only — the session
    number "1" or the memory figure "12,340 K" must never match a PID."""
    from benchmarks.harbor.session_e2e import automation

    def fake_run(argv, **kwargs):
        assert argv[0] == "tasklist"
        assert "/FO" in argv and "CSV" in argv  # structured output, not whitespace
        return _TasklistDone('"powershell.exe","4242","Console","1","12,340 K"\r\n')

    monkeypatch.setattr(automation.subprocess, "run", fake_run)
    assert automation._windows_process_is_alive(4242) is True
    assert automation._windows_process_is_alive(1) is False  # session column
    assert automation._windows_process_is_alive(340) is False  # memory column


def test_windows_process_probe_not_found_and_failure_paths(monkeypatch):
    """G1.2: not-found is False; a failed probe (OSError, timeout, nonzero
    exit) is None — never silently "gone"."""
    from benchmarks.harbor.session_e2e import automation

    def not_found(argv, **kwargs):
        return _TasklistDone("INFO: No tasks are running which match the specified criteria.\r\n")

    monkeypatch.setattr(automation.subprocess, "run", not_found)
    assert automation._windows_process_is_alive(4242) is False

    def os_error(argv, **kwargs):
        raise OSError("tasklist.exe not found")

    monkeypatch.setattr(automation.subprocess, "run", os_error)
    assert automation._windows_process_is_alive(4242) is None

    def timeout(argv, **kwargs):
        raise automation.subprocess.TimeoutExpired(cmd="tasklist", timeout=15)

    monkeypatch.setattr(automation.subprocess, "run", timeout)
    assert automation._windows_process_is_alive(4242) is None

    def nonzero(argv, **kwargs):
        return _TasklistDone("", returncode=1)

    monkeypatch.setattr(automation.subprocess, "run", nonzero)
    assert automation._windows_process_is_alive(4242) is None


def test_read_persistent_shell_pid_failure_cases(tmp_path):
    """G1.2: missing file, non-ASCII bytes, and non-numeric content all
    yield None instead of raising; a valid PID parses."""
    from benchmarks.harbor.session_e2e.automation import _read_persistent_shell_pid

    assert _read_persistent_shell_pid(tmp_path) is None  # file missing
    pid_file = tmp_path / "p6_shell_pid.txt"
    pid_file.write_bytes(b"\xff\xfe\x00\x01")
    assert _read_persistent_shell_pid(tmp_path) is None  # non-ASCII bytes
    pid_file.write_text("not-a-pid\n", encoding="ascii")
    assert _read_persistent_shell_pid(tmp_path) is None  # non-numeric
    pid_file.write_text("4242\n", encoding="ascii")
    assert _read_persistent_shell_pid(tmp_path) == 4242


@pytest.mark.asyncio
async def test_probe_failure_is_recorded_not_silently_gone(tmp_path, monkeypatch):
    """G1.2: when the aliveness probe itself fails, the evidence must carry
    null flags — a failed probe is never reported as 'alive' or 'gone'."""
    from benchmarks.harbor.session_e2e import automation
    from benchmarks.harbor.session_e2e.automation import SessionE2eAutomationSpec

    data_dir = _instance_data_dir(tmp_path, monkeypatch)
    workspace = data_dir / "workspaces" / "sess-parent"
    workspace.mkdir(parents=True)
    (workspace / "p6_shell_pid.txt").write_text("4242\n", encoding="ascii")

    monkeypatch.setattr(automation, "_process_is_alive", lambda pid: None)
    monkeypatch.setattr(automation, "PROCESS_GONE_TIMEOUT_SEC", 0.0)
    monkeypatch.setattr(automation, "PROCESS_SETTLE_SEC", 0.0)

    class TermClient:
        def __init__(self) -> None:
            self.terminated = False

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
            check_persistent_shell_after_terminate=True,
        ),
        child_session_ids=["sess-child"],
    )
    (rec,) = actions
    assert rec["kind"] == "terminateOnPrompt"
    assert rec["shellPid"] == 4242
    assert rec["shellAliveWhileWaiting"] is None
    assert rec["shellGoneAfterTerminate"] is None
    assert rec["shellStillGoneAfterSettle"] is None
