"""Resolve pending UI-backed interactions via the E2E control plane."""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import httpx

from benchmarks.harbor.session_e2e.control import E2eControlClient, e2e_control_configured


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
    spec: SessionE2eAutomationSpec | None,
) -> bool:
    """Return True when any control action was taken."""
    if not e2e_control_configured():
        return False
    control = E2eControlClient(api_url)
    snapshot = await control.interactions(client, session_id)
    acted = False
    synthetic = ""
    if spec is not None:
        synthetic = os.environ.get(spec.synthetic_secret_env, "").strip()
    for prompt in snapshot.get("interactiveShellPrompts") or []:
        if spec is not None and not spec.auto_answer_interactive:
            continue
        execution_id = str(prompt.get("executionId", ""))
        if not execution_id:
            continue
        answer = synthetic or "e2e-synthetic-answer"
        await control.submit_interactive(
            client,
            session_id,
            execution_id,
            action="submit",
            value=answer,
        )
        acted = True
    for approval in snapshot.get("pendingApprovals") or []:
        tool_call_id = str(approval.get("toolCallId", ""))
        if not tool_call_id:
            continue
        kind = str(approval.get("approvalKind", ""))
        approved = True
        if spec is not None:
            if spec.reject_hard_approvals and "Hard" in kind:
                approved = False
            elif not spec.approve_standard_tools and "Standard" in kind:
                approved = False
        await control.submit_approval(
            client, session_id, tool_call_id, approved=approved
        )
        acted = True
    return acted
