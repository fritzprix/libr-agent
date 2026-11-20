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
    wait_for_overlapping_child_prompt: bool = False
    distinct_secret_per_prompt: bool = False
    terminate_child_on_prompt: bool = False
    # G1.2: task refuses to run unless the bench uses this workspace mode
    # ("managed" for the privileged tasks, whose fakes live in the task image).
    requires_workspace_mode: str | None = None
    # G1.4: per-task mock keychain mode ("succeed" | "fail-write" |
    # "fail-read" | "unavailable"); applied to the instance env before the
    # per-trial relaunch and cleared for every other task.
    mock_keychain_mode: str | None = None
    # Restart-dependent rows (#2903): after the session completes, the harness
    # restarts the *same* bench instance in place and records a post-restart
    # snapshot so the verifier can assert persistence without a second session.
    restart_after_completion: bool = False
    # Probes run against the restarted instance, recorded verbatim into
    # session_e2e_restart_evidence.json. Each http probe is
    # {name, path} (GET <api_url><path>); each sql probe is {name, query}
    # (read-only SELECT against the instance DB).
    post_restart_http_probes: tuple[dict[str, str], ...] = ()
    post_restart_sql_probes: tuple[dict[str, str], ...] = ()
    # Optional HTTP PUT requests executed right BEFORE restarting the instance
    # (each {name, path, body}), recorded in session_e2e_restart_evidence.json
    # under "preRestart" so verifiers can assert a written value survives restart.
    pre_restart_http_puts: tuple[dict[str, Any], ...] = ()
    # 2565 P-5 / X-5: observe pending prompt on poll 1, re-fetch session
    # metadata without answering, then answer on poll 2 when the same
    # requestId is still waiting.
    re_poll_prompt_before_answer: bool = False
    # 2565 P-8 / 2672 A-4: when a user prompt is waiting, restart the isolated
    # instance in place instead of answering it, then snapshot post-restart
    # messages to verify the restart tombstone.
    restart_while_prompt_waiting: bool = False
    # #2903 AC1: probe POST /api/e2e-control/picker (path + cancel) against
    # the live isolated debug instance and record responses in evidence.
    probe_picker_control: bool = False


MOCK_KEYCHAIN_MODES = ("succeed", "fail-write", "fail-read", "unavailable")
MOCK_KEYCHAIN_MODE_ENV = "LIBRAGENT_E2E_MOCK_KEYCHAIN"


def _clean_mock_keychain_mode(raw: object) -> str | None:
    """Validate the task-declared mock keychain mode, failing fast on typos."""
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise RuntimeError(
            f"mock_keychain_mode must be one of {', '.join(MOCK_KEYCHAIN_MODES)} (got {raw!r})."
        )
    mode = raw.strip().lower()
    if mode not in MOCK_KEYCHAIN_MODES:
        raise RuntimeError(
            f"mock_keychain_mode must be one of {', '.join(MOCK_KEYCHAIN_MODES)} (got {raw!r})."
        )
    return mode


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
        wait_for_overlapping_child_prompt=bool(
            spec.get("wait_for_overlapping_child_prompt", False)
        ),
        distinct_secret_per_prompt=bool(
            spec.get("distinct_secret_per_prompt", False)
        ),
        terminate_child_on_prompt=bool(
            spec.get("terminate_child_on_prompt", False)
        ),
        requires_workspace_mode=(
            str(spec["requires_workspace_mode"]).strip().lower()
            if isinstance(spec.get("requires_workspace_mode"), str)
            and str(spec["requires_workspace_mode"]).strip()
            else None
        ),
        mock_keychain_mode=_clean_mock_keychain_mode(spec.get("mock_keychain_mode")),
        restart_after_completion=bool(spec.get("restart_after_completion", False)),
        post_restart_http_probes=_probe_list(spec.get("post_restart_http_probes"), ("name", "path")),
        post_restart_sql_probes=_probe_list(spec.get("post_restart_sql_probes"), ("name", "query")),
        pre_restart_http_puts=_http_put_list(spec.get("pre_restart_http_puts")),
        re_poll_prompt_before_answer=bool(spec.get("re_poll_prompt_before_answer", False)),
        restart_while_prompt_waiting=bool(spec.get("restart_while_prompt_waiting", False)),
        probe_picker_control=bool(spec.get("probe_picker_control", False)),
    )


def _probe_list(raw: Any, required: tuple[str, ...]) -> tuple[dict[str, str], ...]:
    """Validate declared restart probes; entries missing required string
    fields are dropped rather than crashing the trial."""
    if not isinstance(raw, list):
        return ()
    probes: list[dict[str, str]] = []
    for entry in raw:
        if not isinstance(entry, dict) or not all(
            isinstance(entry.get(key), str) and entry.get(key).strip() for key in required
        ):
            continue
        probes.append({key: str(entry[key]).strip() for key in required})
    return tuple(probes)


def _http_put_list(raw: Any) -> tuple[dict[str, Any], ...]:
    """Validate pre-restart HTTP PUT specs ({name, path, body})."""
    if not isinstance(raw, list):
        return ()
    puts: list[dict[str, Any]] = []
    for entry in raw:
        if (
            not isinstance(entry, dict)
            or not isinstance(entry.get("name"), str)
            or not entry["name"].strip()
            or not isinstance(entry.get("path"), str)
            or not entry["path"].strip()
            or "body" not in entry
        ):
            continue
        puts.append(
            {
                "name": entry["name"].strip(),
                "path": entry["path"].strip(),
                "body": entry["body"],
            }
        )
    return tuple(puts)


def _has_pending_prompt(snapshot: dict[str, Any]) -> bool:
    prompt = snapshot.get("pendingUserPrompt")
    return isinstance(prompt, dict) and bool(str(prompt.get("requestId", "")))


async def resolve_pending_interactions(
    client: httpx.AsyncClient,
    *,
    api_url: str,
    session_id: str,
    data_dir: Path,
    spec: SessionE2eAutomationSpec | None,
    child_session_ids: list[str] | None = None,
    prior_actions: list[dict[str, Any]] | None = None,
) -> list[dict[str, Any]]:
    """Answer/cancel waiting prompts and approve/deny pending tool approvals.

    ``data_dir`` is the trial instance's own data directory; the capability
    token is read only from there. Returns one record per action taken, so
    callers can persist the control activity as evidence.

    When ``resolve_child_interactions`` is on, the poller passes the session's
    child sessions (2565 X-2): every child is resolved **before** the primary
    session, so a waiting prompt in session B is answered ahead of session A,
    and child answers carry a ``-child`` suffix on the injected value. When
    ``wait_for_overlapping_child_prompt`` is also on, prompt answers are held
    until at least one child and the primary session both have a pending prompt
    in the same poll cycle, and each resulting answer record carries
    ``simultaneousPending: true``.
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
    targets = [(child, True) for child in child_ids] + [(session_id, False)]

    snapshots: dict[str, dict[str, Any]] = {}
    allow_prompt_resolution = True
    simultaneous_pending = False
    if spec is not None and spec.wait_for_overlapping_child_prompt:
        for sid, _ in targets:
            snapshots[sid] = await control.interactions(client, sid)
        primary_pending = _has_pending_prompt(snapshots.get(session_id, {}))
        child_pending = any(_has_pending_prompt(snapshots.get(cid, {})) for cid in child_ids)
        simultaneous_pending = primary_pending and child_pending
        allow_prompt_resolution = simultaneous_pending

    for sid, is_child in targets:
        actions.extend(
            await _resolve_one_session(
                client,
                control,
                sid,
                spec,
                synthetic,
                is_child=is_child,
                snapshot=snapshots.get(sid),
                prior_actions=(list(prior_actions or []) + actions),
                allow_prompt_resolution=allow_prompt_resolution,
                simultaneous_pending=simultaneous_pending,
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
    snapshot: dict[str, Any] | None = None,
    prior_actions: list[dict[str, Any]] | None = None,
    allow_prompt_resolution: bool = True,
    simultaneous_pending: bool = False,
) -> list[dict[str, Any]]:
    if snapshot is None:
        snapshot = await control.interactions(client, session_id)
    actions: list[dict[str, Any]] = []

    prompt = snapshot.get("pendingUserPrompt")
    if isinstance(prompt, dict) and allow_prompt_resolution:
        request_id = str(prompt.get("requestId", ""))
        if request_id and not is_child and spec is not None and spec.restart_while_prompt_waiting:
            actions.append(
                {
                    "kind": "restartOnPrompt",
                    "sessionId": session_id,
                    "requestId": request_id,
                }
            )
            return actions
        if request_id and is_child and spec is not None and spec.terminate_child_on_prompt:
            term_res = await client.post(f"{control.api_url}/sessions/{session_id}/terminate")
            term_res.raise_for_status()
            prompt_cleared = True
            try:
                after = await control.interactions(client, session_id)
                prompt_cleared = not _has_pending_prompt(after)
            except httpx.HTTPStatusError as exc:
                if exc.response.status_code != 404:
                    raise
            actions.append(
                {
                    "kind": "terminateOnPrompt",
                    "sessionId": session_id,
                    "requestId": request_id,
                    "terminated": True,
                    "promptCleared": prompt_cleared,
                }
            )
        elif request_id and (spec is None or spec.auto_answer_interactive):
            survived_repoll = False
            if spec is not None and spec.re_poll_prompt_before_answer:
                already_observed = any(
                    a.get("kind") == "userPromptObserved"
                    and a.get("sessionId") == session_id
                    and a.get("requestId") == request_id
                    for a in (prior_actions or [])
                )
                if not already_observed:
                    refetched = False
                    try:
                        sess_res = await client.get(f"{control.api_url}/sessions/{session_id}")
                        sess_res.raise_for_status()
                        refetched = True
                    except (httpx.HTTPError, OSError):
                        refetched = False
                    actions.append(
                        {
                            "kind": "userPromptObserved",
                            "sessionId": session_id,
                            "requestId": request_id,
                            "sessionRefetched": refetched,
                        }
                    )
                    return actions
                survived_repoll = True
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
            if (
                not cancel
                and override is None
                and spec is not None
                and spec.distinct_secret_per_prompt
            ):
                prior_count = sum(
                    1
                    for a in (prior_actions or [])
                    if a.get("kind") == "userPrompt"
                    and a.get("sessionId") == session_id
                    and a.get("action") == "answer"
                    and a.get("delivered") is True
                )
                injected = f"{injected}-v{prior_count + 1}"
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
            record: dict[str, Any] = {
                "kind": "userPrompt",
                "sessionId": session_id,
                "requestId": request_id,
                "action": "cancel" if cancel else "answer",
                "delivered": bool(response.get("delivered")),
            }
            if not cancel:
                record["injectedValue"] = injected
            if simultaneous_pending:
                record["simultaneousPending"] = True
            if survived_repoll:
                record["survivedRepoll"] = True
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
        approval_record: dict[str, Any] = {
            "kind": "toolApproval",
            "sessionId": session_id,
            "toolCallId": tool_call_id,
            "approvalKind": kind,
            "approved": approved,
        }
        if approval.get("toolName"):
            approval_record["toolName"] = str(approval["toolName"])
        if approval.get("commandPreview"):
            approval_record["commandPreview"] = str(approval["commandPreview"])
        actions.append(approval_record)

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
