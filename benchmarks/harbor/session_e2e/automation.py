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
    resume_circuit_break: bool = True
    cancel_interactive: bool = False
    answer_value: str | None = None
    execution_mode: str | None = None
    resolve_child_interactions: bool = False


def _clean_execution_mode(raw: object) -> str | None:
    """Task-level executionMode override (``normal`` | ``yolo`` | ``unsafe``).

    Empty or absent means the task inherits the run's ``LIBRAGENT_EXECUTION_MODE``
    — existing tasks are unaffected. Validation is the adapter's job
    (``resolve_execution_mode``), so this stays a single trivial helper.
    """
    if not isinstance(raw, str):
        return None
    mode = raw.strip().lower()
    return mode or None


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
        resume_circuit_break=bool(spec.get("resume_circuit_break", True)),
        cancel_interactive=bool(spec.get("cancel_interactive", False)),
        answer_value=(
            str(spec["answer_value"])
            if isinstance(spec.get("answer_value"), (str, int, float))
            else None
        ),
        execution_mode=_clean_execution_mode(spec.get("execution_mode")),
        resolve_child_interactions=bool(
            spec.get("resolve_child_interactions", False)
        ),
    )


async def resolve_pending_interactions(
    client: httpx.AsyncClient,
    *,
    api_url: str,
    session_id: str,
    data_dir: Path,
    spec: SessionE2eAutomationSpec | None,
    child_session_ids: list[str] | None = None,
) -> list[dict[str, Any]]:
    """Answer/cancel waiting prompts and approve/deny pending tool approvals.

    ``data_dir`` is the trial instance's own data directory; the capability
    token is read only from there. Returns one record per action taken, so
    callers can persist the control activity as evidence.

    When ``resolve_child_interactions`` is on, the poller passes the session's
    child sessions (2565 X-2): every child is resolved **before** the primary
    session, so a waiting prompt in session B is answered ahead of session A,
    and child answers carry a ``-child`` suffix on the injected value. The
    suffix is a harness marker for the evidence records — it does not prove
    product answer-channel isolation.
    """
    if not e2e_control_configured(data_dir):
        return []
    token = read_capability_token(data_dir)
    if token is None:  # e2e_control_configured already checked this
        return []
    control = E2eControlClient(api_url, token=token)

    synthetic = ""
    if spec is not None:
        synthetic = os.environ.get(spec.synthetic_secret_env, "").strip()

    actions: list[dict[str, Any]] = []
    child_ids = list(child_session_ids or []) if spec and spec.resolve_child_interactions else []
    for sid, is_child in [(child, True) for child in child_ids] + [(session_id, False)]:
        actions.extend(
            await _resolve_one_session(
                client, control, sid, spec, synthetic, is_child=is_child
            )
        )
    return actions


async def _resolve_one_session(
    client: httpx.AsyncClient,
    control: E2eControlClient,
    session_id: str,
    spec: SessionE2eAutomationSpec | None,
    synthetic: str,
    *,
    is_child: bool,
) -> list[dict[str, Any]]:
    snapshot = await control.interactions(client, session_id)
    actions: list[dict[str, Any]] = []

    prompt = snapshot.get("pendingUserPrompt")
    if isinstance(prompt, dict) and (spec is None or spec.auto_answer_interactive):
        request_id = str(prompt.get("requestId", ""))
        if request_id:
            cancel = spec is not None and spec.cancel_interactive
            # A task's ``answer_value`` (task.toml metadata) overrides the
            # synthetic secret — e.g. the JSON-object answer a multi-question
            # ui__askUser expects (2565 A-1/A-2 wire contract).
            override = spec.answer_value if spec is not None else None
            injected = override or synthetic or "e2e-synthetic-answer"
            if is_child and override is None:
                # The suffix distinguishes synthetic child answers in the
                # evidence only. An answer_value override (e.g. a JSON object
                # for a multi-question ui__askUser) must be injected verbatim,
                # so it never gets the suffix.
                injected = f"{injected}-child"
            response = await control.respond_user_prompt(
                client,
                session_id,
                request_id,
                action="cancel" if cancel else "answer",
                value=None if cancel else injected,
            )
            # ``delivered: false`` means the answer never reached the session
            # (e.g. the prompt was already gone). Record it verbatim so the
            # task verifier can reject an undelivered answer instead of
            # counting the attempt as resolved. ``injectedValue`` pins the
            # exact (synthetic) value the harness injected: the verifier
            # runs inside the Harbor container without the host's
            # ``LIBRAGENT_E2E_SYNTHETIC_SECRET``, so the evidence record —
            # not a container env guess — is what makes leak checks like
            # e2e-storeas-masked non-vacuous.
            record = {
                "kind": "userPrompt",
                "sessionId": session_id,
                "requestId": request_id,
                "action": "cancel" if cancel else "answer",
                "delivered": bool(response.get("delivered")),
            }
            if not cancel:
                record["injectedValue"] = injected
            actions.append(record)

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
                "sessionId": session_id,
                "toolCallId": tool_call_id,
                "approvalKind": kind,
                "approved": approved,
            }
        )

    break_pending = snapshot.get("pendingCircuitBreak")
    if isinstance(break_pending, dict) and (spec is None or spec.resume_circuit_break):
        response = await control.resume_circuit_break(client, session_id)
        actions.append(
            {
                "kind": "circuitBreakResume",
                "sessionId": session_id,
                "resumedTool": response.get("resumedTool"),
                "repetitionCount": response.get("repetitionCount"),
            }
        )
    return actions
