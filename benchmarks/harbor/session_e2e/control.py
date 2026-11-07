"""Loopback E2E control HTTP client (debug builds + LIBRAGENT_E2E_CONTROL=1)."""

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
