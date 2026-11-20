"""Loopback E2E control HTTP client (debug builds + LIBRAGENT_E2E_CONTROL=1).

Polling contract (#2903). There is no event stream; harnesses poll this
surface. The contract below is what the client here and the Harbor poller
(`benchmarks/harbor/libragent_agent.py`) rely on:

Endpoints (all require the ``X-LibrAgent-E2E-Control`` capability header;
``:ref`` is a full session id or a unique short ref resolved server-side):

- ``GET  /api/e2e-control/status`` — ``{enabled, debugBuild, loopbackOnly,
  buildSha, buildDirty}``.
- ``GET  /api/e2e-control/sessions/:ref/interactions`` — snapshot
  ``{sessionId, pendingUserPrompt, pendingApprovals, pendingCircuitBreak}``
  where ``pendingUserPrompt`` is ``null`` or ``{requestId, kind, payload,
  createdAt}``, ``pendingApprovals`` is a list of ``{toolCallId, toolName,
  reason, commandPreview, approvalKind, requestId?}``, and
  ``pendingCircuitBreak`` is ``null`` or ``{toolName, repetitionCount,
  toolCallId}`` (a waiting ``ui__circuitBreak``, #2901).
- ``POST /api/e2e-control/sessions/:ref/user-prompts`` — body
  ``{requestId, action: "answer"|"cancel", value?}`` → ``{delivered}``.
- ``POST /api/e2e-control/sessions/:ref/tool-approvals`` — body
  ``{toolCallId, approved}`` → ``{ok}``.
- ``POST /api/e2e-control/sessions/:ref/circuit-break-resume`` — no body →
  ``{ok, resumedTool, repetitionCount}``. Resumes the session's waiting
  circuit break (#2901). When none is waiting the route answers **HTTP 409**
  with ``{error}`` (``"no circuit breaker is waiting for resume on this
  session"``); the client calls ``raise_for_status()``, so a harness sees
  ``httpx.HTTPStatusError`` — not ``{ok: false}``.
- ``POST /api/e2e-control/picker`` — body ``{path}`` or ``{cancel: true}``
  → ``{ok}``. Arms a one-shot override that resolves the *next* native
  open/save dialog (file attach, export, save-as) without a human. Arm it
  immediately before the action that opens the dialog; an unconsumed override
  is replaced by the next arm, never silently queued twice.
- ``POST /api/e2e-control/window`` — body ``{visible?, width?, height?}``
  (at least one field; ``width``/``height`` are logical pixels and must be
  given together) → ``{visible, width, height}`` reflecting the *actual*
  post-apply state (window managers clamp requests). Show/hide and resize
  of the isolated instance's main window (#2903 AC2).
- ``GET  /api/e2e-control/ui/screenshot`` — query ``?includeBase64=true|false``
  → ``{path, width, height, bytes, base64?}``. Captures the isolated
  instance's own window (selected strictly by PID, never by window title) and
  writes the PNG under ``<LIBRAGENT_DATA_DIR>/e2e_captures/`` (#2902).
- ``POST /api/e2e-control/ui/navigate`` — body ``{path?, sessionId?, panel?}``
  (at least one field; ``path`` and ``sessionId`` are mutually exclusive;
  ``panel`` is ``"workspace"|"processes"|"planning"|"close"``) →
  ``{ok, path, panel}`` (#2902).
- ``POST /api/e2e-control/ui/elements`` — body ``{selector?, timeoutMs?}``
  → ``{elements, count}`` where each element carries ``{role, tag, label,
  id?, testId?, inputType?, disabled, bounds, rect, visible}`` (#2902).
- ``POST /api/e2e-control/ui/input`` — body ``{action, selector?, testId?,
  x?, y?, text?, key?, modifiers?, scrollAmount?, axis?, button?}`` →
  ``{ok, result}``. Delivers window-scoped input (click, double_click,
  right_click, middle_click, move, hover, type, key, scroll) to the isolated
  instance window (#2902).
- ``POST /api/e2e-control/ui/restart`` — no body →
  ``{ok, restarting, note}``. Restarts the isolated instance in place (#2902).
- ``POST /api/e2e-control/ui/wait`` — body ``{condition, sessionId?,
  timeoutMs?, pollMs?}`` (``condition`` is ``"session-idle"|"prompt-pending"|
  "approval-pending"|"circuit-break-pending"|"dialog-open"``) →
  ``{condition, satisfied, timedOut, waitedMs, state}`` (#2902).

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

    async def take_picker(self, client: httpx.AsyncClient) -> dict[str, Any]:
        """Consume the currently armed one-shot picker override (#2903).

        Drives the same ``take_picker_override()`` store as the native save/open
        consumers and returns ``{"override": {"action": "path", "path": ...} |
        {"action": "cancel"} | null}``.
        """
        res = await client.post(
            f"{self.api_url}/e2e-control/picker/take",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def set_window(
        self,
        client: httpx.AsyncClient,
        *,
        visible: bool | None = None,
        width: float | None = None,
        height: float | None = None,
    ) -> dict[str, Any]:
        """Show/hide and resize the isolated instance's main window (#2903 AC2).

        Give at least one field; ``width``/``height`` are logical pixels and
        must be given together. The reply carries the *actual* post-apply
        state — window managers clamp requests — so verify against the
        response, not the request.
        """
        if visible is None and width is None and height is None:
            raise ValueError("provide visible and/or width+height")
        if (width is None) != (height is None):
            raise ValueError("width and height must be given together")
        body: dict[str, Any] = {}
        if visible is not None:
            body["visible"] = visible
        if width is not None and height is not None:
            body["width"] = width
            body["height"] = height
        res = await client.post(
            f"{self.api_url}/e2e-control/window",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()

    async def ui_screenshot(
        self,
        client: httpx.AsyncClient,
        *,
        include_base64: bool = False,
    ) -> dict[str, Any]:
        """Capture the isolated instance's window to PNG (#2902).

        Selects the window strictly by the instance's own PID (never by
        title). Writes the PNG under ``<LIBRAGENT_DATA_DIR>/e2e_captures/``
        and returns ``{path, width, height, bytes, base64?}``.
        """
        url = f"{self.api_url}/e2e-control/ui/screenshot"
        if include_base64:
            url = f"{url}?includeBase64=true"
        res = await client.get(url, headers=self._headers())
        res.raise_for_status()
        return res.json()

    async def ui_navigate(
        self,
        client: httpx.AsyncClient,
        *,
        path: str | None = None,
        session_id: str | None = None,
        panel: str | None = None,
    ) -> dict[str, Any]:
        """Navigate to an in-app route, focus a session, and/or open a panel (#2902)."""
        if path is None and session_id is None and panel is None:
            raise ValueError("provide path, session_id, and/or panel")
        if path is not None and session_id is not None:
            raise ValueError("provide either path or session_id, not both")
        body: dict[str, Any] = {}
        if path is not None:
            body["path"] = path
        if session_id is not None:
            body["sessionId"] = session_id
        if panel is not None:
            body["panel"] = panel
        res = await client.post(
            f"{self.api_url}/e2e-control/ui/navigate",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()

    async def ui_elements(
        self,
        client: httpx.AsyncClient,
        *,
        selector: str | None = None,
        timeout_ms: int | None = None,
    ) -> dict[str, Any]:
        """List interactable elements (role, label, testId, bounds) (#2902)."""
        body: dict[str, Any] = {}
        if selector is not None:
            body["selector"] = selector
        if timeout_ms is not None:
            body["timeoutMs"] = timeout_ms
        res = await client.post(
            f"{self.api_url}/e2e-control/ui/elements",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()

    async def ui_input(
        self,
        client: httpx.AsyncClient,
        action: str,
        *,
        selector: str | None = None,
        test_id: str | None = None,
        x: int | None = None,
        y: int | None = None,
        text: str | None = None,
        key: str | None = None,
        modifiers: list[str] | None = None,
        scroll_amount: int | None = None,
        axis: str | None = None,
        button: str | None = None,
    ) -> dict[str, Any]:
        """Deliver window-scoped input to the isolated instance (#2902)."""
        if not action or not action.strip():
            raise ValueError("action must not be empty")
        if (x is None) != (y is None):
            raise ValueError("x and y must be given together")
        body: dict[str, Any] = {"action": action}
        if selector is not None:
            body["selector"] = selector
        if test_id is not None:
            body["testId"] = test_id
        if x is not None and y is not None:
            body["x"] = x
            body["y"] = y
        if text is not None:
            body["text"] = text
        if key is not None:
            body["key"] = key
        if modifiers is not None:
            body["modifiers"] = modifiers
        if scroll_amount is not None:
            body["scrollAmount"] = scroll_amount
        if axis is not None:
            body["axis"] = axis
        if button is not None:
            body["button"] = button
        res = await client.post(
            f"{self.api_url}/e2e-control/ui/input",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()

    async def ui_restart(self, client: httpx.AsyncClient) -> dict[str, Any]:
        """Restart the isolated instance in place over HTTP (#2902)."""
        res = await client.post(
            f"{self.api_url}/e2e-control/ui/restart",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def ui_wait(
        self,
        client: httpx.AsyncClient,
        condition: str,
        *,
        session_id: str | None = None,
        timeout_ms: int | None = None,
        poll_ms: int | None = None,
    ) -> dict[str, Any]:
        """Wait for a session or UI condition with a timeout (#2902)."""
        if not condition or not condition.strip():
            raise ValueError("condition must not be empty")
        norm = condition.strip().lower()
        if norm != "dialog-open" and not (session_id and session_id.strip()):
            raise ValueError(f"condition '{condition}' requires session_id")
        body: dict[str, Any] = {"condition": condition}
        if session_id is not None:
            body["sessionId"] = session_id
        if timeout_ms is not None:
            body["timeoutMs"] = timeout_ms
        if poll_ms is not None:
            body["pollMs"] = poll_ms
        res = await client.post(
            f"{self.api_url}/e2e-control/ui/wait",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()
        return res.json()
