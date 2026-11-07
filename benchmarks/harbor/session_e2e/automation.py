"""Resolve pending blocking interactions via the E2E control plane."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx

from benchmarks.harbor.session_e2e.control import (
    E2eControlClient,
    e2e_control_configured,
    read_capability_token,
)


@dataclass(frozen=True)
class SessionE2eAutomationSpec:
    """Task metadata under [metadata.libragent_session_e2e] in task.toml."""

    synthetic_secret_env: str = "LIBRAGENT_E2E_SYNTHETIC_SECRET"
    approve_standard_tools: bool = True
    reject_hard_approvals: bool = False
    auto_answer_interactive: bool = True


def load_session_e2e_spec(logs_dir: Any) -> SessionE2eAutomationSpec | None:
    import json
    import tomllib
    from pathlib import Path

    logs = Path(logs_dir)
    try:
        config = json.loads((logs.parent / "config.json").read_text(encoding="utf-8"))
        task_dir = Path(config["task"]["path"])
        task = tomllib.loads((task_dir / "task.toml").read_text(encoding="utf-8"))
    except (OSError, ValueError, KeyError, TypeError):
        return None
    spec = task.get("metadata", {}).get("libragent_session_e2e")
    if not isinstance(spec, dict):
        return None
    return SessionE2eAutomationSpec(
        synthetic_secret_env=str(
            spec.get("synthetic_secret_env", "LIBRAGENT_E2E_SYNTHETIC_SECRET")
        ),
        approve_standard_tools=bool(spec.get("approve_standard_tools", True)),
        reject_hard_approvals=bool(spec.get("reject_hard_approvals", False)),
        auto_answer_interactive=bool(spec.get("auto_answer_interactive", True)),
    )


async def resolve_pending_interactions(
    client: httpx.AsyncClient,
    *,
    api_url: str,
    session_id: str,
    data_dir: Path,
    spec: SessionE2eAutomationSpec | None,
) -> list[dict[str, Any]]:
    """Answer/cancel waiting prompts and approve/deny pending tool approvals.

    ``data_dir`` is the trial instance's own data directory; the capability
    token is read only from there. Returns one record per action taken, so
    callers can persist the control activity as evidence.
    """
    if not e2e_control_configured(data_dir):
        return []
    token = read_capability_token(data_dir)
    if token is None:  # e2e_control_configured already checked this
        return []
    control = E2eControlClient(api_url, token=token)
    snapshot = await control.interactions(client, session_id)
    actions: list[dict[str, Any]] = []

    synthetic = ""
    if spec is not None:
        synthetic = os.environ.get(spec.synthetic_secret_env, "").strip()

    prompt = snapshot.get("pendingUserPrompt")
    if isinstance(prompt, dict) and (spec is None or spec.auto_answer_interactive):
        request_id = str(prompt.get("requestId", ""))
        if request_id:
            await control.respond_user_prompt(
                client,
                session_id,
                request_id,
                action="answer",
                value=synthetic or "e2e-synthetic-answer",
            )
            actions.append(
                {"kind": "userPrompt", "requestId": request_id, "action": "answer"}
            )

    for approval in snapshot.get("pendingApprovals") or []:
        tool_call_id = str(approval.get("toolCallId", ""))
        if not tool_call_id:
            continue
        kind = str(approval.get("approvalKind", ""))
        approved = True
        if spec is not None:
            if spec.reject_hard_approvals and kind == "hard":
                approved = False
            elif not spec.approve_standard_tools and kind == "standard":
                approved = False
        await control.respond_tool_approval(
            client, session_id, tool_call_id, approved=approved
        )
        actions.append(
            {
                "kind": "toolApproval",
                "toolCallId": tool_call_id,
                "approvalKind": kind,
                "approved": approved,
            }
        )
    return actions
