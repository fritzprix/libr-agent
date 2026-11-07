"""Loopback E2E control HTTP client (debug builds + LIBRAGENT_E2E_CONTROL=1)."""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

import httpx

E2E_HEADER = "x-libragent-e2e-control"


def read_capability_token() -> str | None:
    env = os.environ.get("LIBRAGENT_E2E_CONTROL_TOKEN", "").strip()
    if env:
        return env
    data_dir = os.environ.get("LIBRAGENT_DATA_DIR", "").strip()
    bases: list[Path] = []
    if data_dir:
        bases.append(Path(data_dir))
    home = os.environ.get("HOME") or os.environ.get("USERPROFILE")
    if home:
        bases.append(Path(home) / "AppData" / "Local" / "com.fritzprix.libragent")
        bases.append(Path(home) / ".local" / "share" / "com.fritzprix.libragent")
    for base in bases:
        cap = base / "e2e_control.cap"
        if cap.is_file():
            return cap.read_text(encoding="utf-8").strip()
    return None


def e2e_control_configured() -> bool:
    return os.environ.get("LIBRAGENT_E2E_CONTROL") == "1" and bool(read_capability_token())


class E2eControlClient:
    def __init__(self, api_url: str, *, token: str | None = None) -> None:
        self.api_url = api_url.rstrip("/")
        self.token = token or read_capability_token()
        if not self.token:
            raise RuntimeError("E2E control capability token missing")

    def _headers(self) -> dict[str, str]:
        return {E2E_HEADER: self.token or ""}

    async def status(self, client: httpx.AsyncClient) -> dict[str, Any]:
        res = await client.get(
            f"{self.api_url}/e2e-control/status",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def interactions(self, client: httpx.AsyncClient, session_id: str) -> dict[str, Any]:
        res = await client.get(
            f"{self.api_url}/e2e-control/sessions/{session_id}/interactions",
            headers=self._headers(),
        )
        res.raise_for_status()
        return res.json()

    async def submit_interactive(
        self,
        client: httpx.AsyncClient,
        session_id: str,
        execution_id: str,
        *,
        action: str,
        value: str | None = None,
    ) -> None:
        body: dict[str, Any] = {"executionId": execution_id, "action": action}
        if value is not None:
            body["input"] = value
        res = await client.post(
            f"{self.api_url}/e2e-control/sessions/{session_id}/interactive-shell",
            headers=self._headers(),
            json=body,
        )
        res.raise_for_status()

    async def submit_approval(
        self,
        client: httpx.AsyncClient,
        session_id: str,
        tool_call_id: str,
        *,
        approved: bool,
    ) -> None:
        res = await client.post(
            f"{self.api_url}/e2e-control/sessions/{session_id}/tool-approvals",
            headers=self._headers(),
            json={"toolCallId": tool_call_id, "approved": approved},
        )
        res.raise_for_status()
