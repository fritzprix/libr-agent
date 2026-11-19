"""Loopback E2E control HTTP client (debug builds + LIBRAGENT_E2E_CONTROL=1).

Polling contract (#2903). There is no event stream; harnesses poll this
surface. The contract below is what the client here and the Harbor poller
(`benchmarks/harbor/libragent_agent.py`) rely on:

Endpoints (all require the ``X-LibrAgent-E2E-Control`` capability header;
``:ref`` is a full session id or a unique short ref resolved server-side):

- ``GET  /api/e2e-control/status`` — ``{enabled, debugBuild, loopbackOnly,
  buildSha, buildDirty}``.
- ``GET  /api/e2e-control/sessions/:ref/interactions`` — snapshot
  ``{sessionId, pendingUserPrompt, pendingApprovals}`` where
  ``pendingUserPrompt`` is ``null`` or ``{requestId, kind, payload,
  createdAt}`` and ``pendingApprovals`` is a list of ``{toolCallId, toolName,
  reason, commandPreview, approvalKind, requestId?}``.
- ``POST /api/e2e-control/sessions/:ref/user-prompts`` — body
  ``{requestId, action: "answer"|"cancel", value?}`` → ``{delivered}``.
- ``POST /api/e2e-control/sessions/:ref/tool-approvals`` — body
  ``{toolCallId, approved}`` → ``{ok}``.
- ``POST /api/e2e-control/picker`` — body ``{path}`` or ``{cancel: true}``
  → ``{ok}``. Arms a one-shot override that resolves the *next* native
  open/save dialog (file attach, export, save-as) without a human. Arm it
  immediately before the action that opens the dialog; an unconsumed override
  is replaced by the next arm, never silently queued twice.

Interval and eventual consistency: poll every ``DEFAULT_POLL_INTERVAL_SEC``
(3.0 s, the bench default). A prompt or approval that just appeared or was
just answered may not be reflected in the snapshot until the next poll
cycle; treat one missed cycle as normal, not as a failure.

Timeout guidance: a waiting prompt is considered stuck when it survives more
than ``LIBRAGENT_POLL_TIMEOUT_SEC`` (scaled agent timeout) without being
answered; ``{delivered: false}`` means the prompt was already gone when the
answer arrived — re-poll and judge from the fresh snapshot instead of
retrying the same requestId.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

import httpx

E2E_HEADER = "x-libragent-e2e-control"
CAPABILITY_FILE = "e2e_control.cap"


def read_capability_token(data_dir: Path) -> str | None:
    """Read the capability token a debug LibrAgent instance wrote to its own data dir.

    Only the trial instance's data dir is consulted. There is no env-var token
    and no fallback to a production profile directory: the token must belong to
    the disposable instance under test.
    """
    cap = Path(data_dir) / CAPABILITY_FILE
    if cap.is_file():
        token = cap.read_text(encoding="utf-8").strip()
        if token:
            return token
    return None


def e2e_control_configured(data_dir: Path) -> bool:
    """True when opt-in env is set and the instance data dir carries a token."""
    return (
        os.environ.get("LIBRAGENT_E2E_CONTROL") == "1"
        and read_capability_token(data_dir) is not None
    )


class E2eControlClient:
    def __init__(self, api_url: str, *, token: str) -> None:
        self.api_url = api_url.rstrip("/")
        self.token = token

    def _headers(self) -> dict[str, str]:
        return {E2E_HEADER: self.token}

    async def status(self, client: httpx.AsyncClient) -> dict[str, Any]:
        res = await client.get(f"{self.api_url}/e2e-control/status", headers=self._headers())
        res.raise_for_status()
        return res.json()

    async def interactions(self, client: httpx.AsyncClient, session_id: str) -> dict[str, Any]:
        res = await client.get(
            f"{self.api_url}/e2e-control/sessions/{session_id}/interactions",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def respond_user_prompt(
        self,
        client: httpx.AsyncClient,
        session_id: str,
        request_id: str,
        *,
        action: str = "answer",
        value: str | None = None,
    ) -> dict[str, Any]:
        """Answer or cancel the session's waiting user prompt (#2543)."""
        body: dict[str, Any] = {"requestId": request_id, "action": action}
        if value is not None:
            body["value"] = value
        res = await client.post(
            f"{self.api_url}/e2e-control/sessions/{session_id}/user-prompts",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()

    async def respond_tool_approval(
        self,
        client: httpx.AsyncClient,
        session_id: str,
        tool_call_id: str,
        *,
        approved: bool,
    ) -> dict[str, Any]:
        res = await client.post(
            f"{self.api_url}/e2e-control/sessions/{session_id}/tool-approvals",
            headers=self._headers(),
            json={"toolCallId": tool_call_id, "approved": approved},
        )
        res.raise_for_status()
        return res.json()

    async def resume_circuit_break(self, client: httpx.AsyncClient, session_id: str) -> dict[str, Any]:
        """Drive the circuit-breaker Resume path (#2901).

        Injects the same ``ui__resumeCircuitBreak`` tool call the GUI Resume
        button sends. Distinct from ``POST /api/sessions/:id/resume``, which
        reloads a paused/crashed session and never touches the dialog.
        """
        res = await client.post(
            f"{self.api_url}/e2e-control/sessions/{session_id}/circuit-break-resume",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def arm_picker(
        self,
        client: httpx.AsyncClient,
        *,
        path: str | None = None,
        cancel: bool = False,
    ) -> dict[str, Any]:
        """Arm the one-shot native-dialog override (#2903).

        Give exactly one of ``path`` / ``cancel``. The next native open/save
        dialog (frontend attach/export or the Rust save path) resolves with
        it instead of waiting for a human.
        """
        if (path is not None) == bool(cancel):
            raise ValueError("provide exactly one of path or cancel=True")
        body: dict[str, Any] = {"cancel": True} if cancel else {"path": path}
        res = await client.post(
            f"{self.api_url}/e2e-control/picker",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()
