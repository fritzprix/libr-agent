"""Harbor Framework adapter for LibrAgent.

Bridges Harbor's terminal-benchmark loop to LibrAgent's headless Session API.

When the Harbor environment is a local Docker Compose trial, the adapter attaches
LibrAgent's Docker session to Harbor's existing main container. Workdir is taken
from task config when set; otherwise from the container image WORKDIR (often
`/workspace` for Harbor Index / BixBench, `/app` for classic Terminal-Bench).

Non-Docker Harbor backends fall back to the legacy host-workspace sync path.
"""

from __future__ import annotations

import asyncio
import contextlib
import json
import os
import re
import subprocess
from collections.abc import Awaitable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from harbor.agents.base import BaseAgent
    from harbor.environments.base import BaseEnvironment
    from harbor.models.agent.context import AgentContext
    from harbor.models.trajectories.agent import Agent as AtifAgent
    from harbor.models.trajectories.final_metrics import FinalMetrics
    from harbor.models.trajectories.metrics import Metrics
    from harbor.models.trajectories.observation import Observation
    from harbor.models.trajectories.observation_result import ObservationResult
    from harbor.models.trajectories.step import Step
    from harbor.models.trajectories.tool_call import ToolCall
    from harbor.models.trajectories.trajectory import Trajectory
    from harbor.models.trial.result import AgentInfo
    from harbor.utils.trajectory_utils import format_trajectory_json
except ImportError as exc:  # pragma: no cover - import-time guard for editors
    raise ImportError(
        "The 'harbor' package is required for LibrAgentHarborAdapter. "
        "Install it via 'pip install harbor'."
    ) from exc

try:
    import httpx
except ImportError:  # pragma: no cover
    httpx = None  # type: ignore[assignment]

EXECUTION_MODE_VALUES = ("normal", "yolo", "unsafe")
DEFAULT_EXECUTION_MODE = "unsafe"
# Idle/error end the workflow. Paused means approval wait — not done for benchmarks.
TERMINAL_WORKFLOW_STATUSES = frozenset({"idle", "error"})
DEFAULT_POLL_INTERVAL_SEC = 3.0
MAIN_COMPOSE_SERVICE = "main"
TRAJECTORY_MESSAGE_LIMIT = 10_000
PACKAGE_JSON_PATH = Path(__file__).resolve().parents[2] / "package.json"
BENCHMARK_CONTRACT_SCHEMA_VERSION = 1


@dataclass(frozen=True)
class TrajectoryTelemetry:
    """Aggregated Harbor-facing metrics harvested from LibrAgent messages."""

    n_input_tokens: int | None
    n_output_tokens: int | None
    n_cache_tokens: int | None
    n_turns: int
    tool_calls_count: int
    has_usage: bool
    error: str | None


@dataclass(frozen=True)
class CompletionTelemetry:
    """Completion-state signals used to diagnose incomplete Harbor trials."""

    last_tool_call: dict[str, Any] | None
    last_successful_file_mutation: dict[str, Any] | None
    terminal_reported: bool
    terminal_report_result_received: bool
    successful_tool_results: int
    failed_tool_results: int
    unresolved_tool_results: int


class EmptyAgentWorkError(RuntimeError):
    """Session reached a terminal workflow status without any tool calls.

    Harbor should treat this as an agent error rather than a silent verifier
    failure on an untouched workspace.
    """


def build_diagnostic_meta(
    *,
    reason: str,
    session_id: str,
    last_status: str,
    seen_non_idle: bool,
    telemetry: TrajectoryTelemetry,
    n_messages: int,
    completed: bool = False,
    extra: dict[str, Any] | None = None,
    completion: CompletionTelemetry | None = None,
    elapsed_wall_clock_sec: float | None = None,
) -> dict[str, Any]:
    """Build ``timeout_meta.json`` / incomplete-run diagnostic payload."""
    captured_at = datetime.now(timezone.utc).isoformat()
    payload: dict[str, Any] = {
        "reason": reason,
        "sessionId": session_id,
        "last_status": last_status,
        "seen_non_idle": seen_non_idle,
        "completed": completed,
        "n_messages": n_messages,
        "n_turns": telemetry.n_turns,
        "tool_calls_count": telemetry.tool_calls_count,
        "n_input_tokens": telemetry.n_input_tokens,
        "n_output_tokens": telemetry.n_output_tokens,
        "n_cache_tokens": telemetry.n_cache_tokens,
        "error": telemetry.error,
        "last_tool_call": completion.last_tool_call if completion else None,
        "last_successful_file_mutation": (
            completion.last_successful_file_mutation if completion else None
        ),
        "terminal_reported": completion.terminal_reported if completion else False,
        "terminal_report_result_received": (
            completion.terminal_report_result_received if completion else False
        ),
        "successful_tool_results": (
            completion.successful_tool_results if completion else 0
        ),
        "failed_tool_results": completion.failed_tool_results if completion else 0,
        "unresolved_tool_results": (
            completion.unresolved_tool_results if completion else 0
        ),
        "elapsed_wall_clock_sec": elapsed_wall_clock_sec,
        "diagnostic_captured_at": captured_at,
        "harbor_cancelled_at": captured_at if reason == "harbor_cancelled" else None,
    }
    if extra:
        payload.update(extra)
    return payload


def _as_non_negative_int(value: Any) -> int | None:
    """Coerce JSON number-like values to a non-negative int, else None."""
    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value if value >= 0 else None
    if isinstance(value, float):
        if value < 0 or not value.is_integer():
            return None
        return int(value)
    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            parsed = int(text)
        except ValueError:
            return None
        return parsed if parsed >= 0 else None
    return None


def _usage_token(usage: Any, *keys: str) -> int | None:
    if not isinstance(usage, dict):
        return None
    for key in keys:
        parsed = _as_non_negative_int(usage.get(key))
        if parsed is not None:
            return parsed
    return None


def format_harbor_model_name(
    model: str | None,
    provider: str | None = None,
) -> str | None:
    """Build Harbor ``provider/model`` form when both sides are known."""
    model_name = (model or "").strip()
    if not model_name:
        return None
    provider_name = (provider or "").strip()
    if not provider_name:
        return model_name
    if "/" in model_name:
        # Already provider-qualified (e.g. openrouter/...).
        return model_name
    return f"{provider_name}/{model_name}"


def split_harbor_model_name(model_name: str | None) -> tuple[str | None, str | None]:
    """Split Harbor ``provider/model`` form into ``(provider, model)``."""
    text = (model_name or "").strip()
    if not text:
        return None, None
    if "/" not in text:
        return None, text
    provider, model = text.split("/", maxsplit=1)
    return (provider.strip() or None), (model.strip() or None)


def _string_field(payload: dict[str, Any], key: str) -> str | None:
    value = payload.get(key)
    if not isinstance(value, str):
        return None
    return value.strip() or None


def _assistant_config(payload: dict[str, Any]) -> dict[str, Any]:
    """Normalize the assistant ``config`` field, which the API serializes as JSON text."""
    config = payload.get("config")
    if isinstance(config, str):
        try:
            config = json.loads(config)
        except (TypeError, ValueError):
            return {}
    return config if isinstance(config, dict) else {}


def extract_model_provider_from_assistant_payload(
    payload: Any,
) -> tuple[str | None, str | None]:
    """Read ``(model, provider)`` from ``GET /assistants/:id`` JSON."""
    if not isinstance(payload, dict):
        return None, None
    config = _assistant_config(payload)
    model = _string_field(payload, "model") or _string_field(config, "model")
    provider = _string_field(payload, "provider") or _string_field(config, "provider")
    return model, provider


def extract_model_name_from_assistant_payload(payload: Any) -> str | None:
    """Build the Harbor model name from ``GET /assistants/:id`` JSON."""
    model, provider = extract_model_provider_from_assistant_payload(payload)
    return format_harbor_model_name(model, provider)


def extract_model_provider_from_session_payload(
    payload: Any,
) -> tuple[str | None, str | None]:
    """Read ``(model, provider)`` from ``GET /sessions/:id`` JSON."""
    if not isinstance(payload, dict):
        return None, None
    return _string_field(payload, "model"), _string_field(payload, "provider")


def extract_model_name_from_session_payload(payload: Any) -> str | None:
    """Build the Harbor model name from ``GET /sessions/:id`` JSON."""
    model, provider = extract_model_provider_from_session_payload(payload)
    return format_harbor_model_name(model, provider)


def read_repo_package_version(path: Path | None = None) -> str | None:
    """Read the repo ``package.json`` version used as the adapter fallback."""
    package_path = path if path is not None else PACKAGE_JSON_PATH
    try:
        payload = json.loads(package_path.read_text(encoding="utf-8"))
    except (OSError, TypeError, ValueError):
        return None
    if not isinstance(payload, dict):
        return None
    return _string_field(payload, "version")


def extract_version_from_health_payload(payload: Any) -> str | None:
    """Read ``version`` from ``GET /api/health`` JSON."""
    if not isinstance(payload, dict):
        return None
    return _string_field(payload, "version")


def workspace_mode_name(use_attach: bool) -> str:
    """Harbor-facing workspace mode label matching completed-trial metadata."""
    return "attach" if use_attach else "host-sync"


def _environment_value(name: str) -> str | None:
    value = os.environ.get(name)
    if value is None:
        return None
    normalized = value.strip()
    return normalized or None


def _git_contract_metadata(repo_root: Path) -> dict[str, Any]:
    """Capture revision state without including the working-tree diff."""
    metadata: dict[str, Any] = {"revision": None, "dirty": None}
    try:
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if revision.returncode == 0:
            value = revision.stdout.strip()
            metadata["revision"] = value or None

        status = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=no"],
            cwd=repo_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if status.returncode == 0:
            metadata["dirty"] = bool(status.stdout.strip())
    except (OSError, subprocess.SubprocessError):
        pass
    return metadata


def _benchmark_contract_environment() -> dict[str, Any]:
    """Read runner-provided contract values while excluding secret contents."""
    return {
        "dataset": _environment_value("LIBRAGENT_HARBOR_DATASET"),
        "include": _environment_value("LIBRAGENT_HARBOR_INCLUDE"),
        "n_tasks": _environment_value("LIBRAGENT_HARBOR_N_TASKS"),
        "n_attempts": _environment_value("LIBRAGENT_HARBOR_N_ATTEMPTS"),
        "concurrency": _environment_value("LIBRAGENT_HARBOR_CONCURRENT"),
        "agent": _environment_value("LIBRAGENT_HARBOR_AGENT"),
        "timeout_multiplier": _environment_value(
            "LIBRAGENT_HARBOR_TIMEOUT_MULTIPLIER"
        ),
        "agent_timeout_multiplier": _environment_value(
            "LIBRAGENT_HARBOR_AGENT_TIMEOUT_MULTIPLIER"
        ),
        "verifier_env_configured": _environment_value(
            "LIBRAGENT_HARBOR_VERIFIER_ENV_CONFIGURED"
        )
        == "1",
    }


def _task_identity_from_logs_dir(logs_dir: Path) -> tuple[str | None, str | None]:
    """Infer Harbor task/trial identity from ``<trial-id>/agent`` logs."""
    trial_id = logs_dir.parent.name.strip()
    if not trial_id or trial_id == logs_dir.name:
        return None, None
    if "__" not in trial_id:
        return trial_id, trial_id
    task_id, _, _attempt_id = trial_id.rpartition("__")
    return task_id or trial_id, trial_id


def copy_telemetry_to_context(
    context: AgentContext, telemetry: TrajectoryTelemetry
) -> None:
    """Copy harvested token counts onto Harbor ``AgentContext``.

    Harbor ``result.json`` / job stats read these top-level fields, not metadata.
    """
    context.n_input_tokens = telemetry.n_input_tokens
    context.n_output_tokens = telemetry.n_output_tokens
    context.n_cache_tokens = telemetry.n_cache_tokens
    context.cost_usd = None


def _format_message_error(error: Any) -> str | None:
    if error is None:
        return None
    if isinstance(error, str):
        text = error.strip()
        return text or None
    if isinstance(error, dict):
        for key in ("message", "error", "reason", "detail", "code"):
            value = error.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
        try:
            return json.dumps(error, ensure_ascii=False, sort_keys=True)
        except (TypeError, ValueError):
            return str(error)
    return str(error)


def extract_trajectory_error(messages: list[Any]) -> str | None:
    """Prefer the latest assistant/tool message error, else any message error."""
    last_any: str | None = None
    last_preferred: str | None = None
    for message in messages:
        if not isinstance(message, dict):
            continue
        formatted = _format_message_error(message.get("error"))
        if not formatted:
            continue
        last_any = formatted
        role = str(message.get("role") or "").lower()
        if role in {"assistant", "tool", "system"}:
            last_preferred = formatted
    return last_preferred or last_any


def summarize_trajectory(messages: list[Any]) -> TrajectoryTelemetry:
    """Aggregate token/turn/tool metrics from harvested LibrAgent messages.

    Harbor ``n_input_tokens`` is total input *including* cache. LibrAgent's
    ``usage.promptTokens`` already includes cached prompt tokens;
    ``usage.cachedPromptTokens`` is recorded separately as ``n_cache_tokens``.
    """
    n_input = 0
    n_output = 0
    n_cache = 0
    has_usage = False
    n_turns = 0
    tool_calls_count = 0

    for message in messages:
        if not isinstance(message, dict):
            continue
        role = str(message.get("role") or "").lower()
        if role == "assistant":
            n_turns += 1
            tool_calls = message.get("toolCalls")
            if isinstance(tool_calls, list):
                tool_calls_count += len(tool_calls)

        usage = message.get("usage")
        prompt = _usage_token(usage, "promptTokens", "prompt_tokens", "input_tokens")
        completion = _usage_token(
            usage, "completionTokens", "completion_tokens", "output_tokens"
        )
        cached = _usage_token(
            usage,
            "cachedPromptTokens",
            "cached_prompt_tokens",
            "cache_read_input_tokens",
            "cached_tokens",
        )
        if prompt is None and completion is None and cached is None:
            continue
        has_usage = True
        n_input += prompt or 0
        n_output += completion or 0
        n_cache += cached or 0

    return TrajectoryTelemetry(
        n_input_tokens=n_input if has_usage else None,
        n_output_tokens=n_output if has_usage else None,
        n_cache_tokens=n_cache if has_usage else None,
        n_turns=n_turns,
        tool_calls_count=tool_calls_count,
        has_usage=has_usage,
        error=extract_trajectory_error(messages),
    )


def normalize_session_messages(messages: Any) -> list[Any]:
    """Convert Session API newest-first rows to ascending causal order.

    ``GET /sessions/:id/messages`` orders by SQLite ``rowid DESC``. Row order,
    rather than ``createdAt``, is LibrAgent's conversation-order authority
    because timestamps can be skewed across layers.
    """
    if not isinstance(messages, list):
        return []
    return list(reversed(messages))


def _message_text_parts(message: dict[str, Any], *part_types: str) -> list[str]:
    """Extract text-like strings from LibrAgent content parts (and top-level fields)."""
    texts: list[str] = []
    content = message.get("content")
    if isinstance(content, str) and content.strip():
        texts.append(content.strip())
    elif isinstance(content, list):
        for part in content:
            if not isinstance(part, dict):
                continue
            part_type = str(part.get("type") or "")
            if part_types and part_type not in part_types:
                continue
            for key in ("text", "thinking"):
                value = part.get(key)
                if isinstance(value, str) and value.strip():
                    texts.append(value.strip())
                    break
    return texts


def _assistant_message_text(message: dict[str, Any]) -> str:
    texts = _message_text_parts(message, "text")
    return "\n".join(texts).strip()


def _assistant_reasoning(message: dict[str, Any]) -> str | None:
    thinking = message.get("thinking")
    if isinstance(thinking, str) and thinking.strip():
        return thinking.strip()
    parts = _message_text_parts(message, "thinking")
    if not parts:
        return None
    return "\n".join(parts).strip() or None


def _parse_tool_arguments(raw: Any) -> dict[str, Any]:
    if isinstance(raw, dict):
        return raw
    if raw is None:
        return {}
    if isinstance(raw, str):
        text = raw.strip()
        if not text:
            return {}
        try:
            parsed = json.loads(text)
        except (TypeError, ValueError):
            return {"raw": raw}
        return parsed if isinstance(parsed, dict) else {"value": parsed}
    return {"value": raw}


def _libragent_tool_calls(message: dict[str, Any]) -> list[ToolCall] | None:
    """Convert LibrAgent ``toolCalls`` (OpenAI-style) into ATIF ToolCall objects."""
    raw_calls = message.get("toolCalls")
    if not isinstance(raw_calls, list) or not raw_calls:
        return None

    converted: list[ToolCall] = []
    for index, call in enumerate(raw_calls):
        if not isinstance(call, dict):
            continue
        function = call.get("function")
        if not isinstance(function, dict):
            function = {}
        name = function.get("name") or call.get("name")
        if not isinstance(name, str) or not name.strip():
            name = f"unknown_tool_{index + 1}"
        call_id = call.get("id") or call.get("toolCallId") or call.get("tool_call_id")
        if not isinstance(call_id, str) or not call_id.strip():
            call_id = f"tool_call_{index + 1}"
        arguments = _parse_tool_arguments(
            function.get("arguments", call.get("arguments"))
        )
        converted.append(
            ToolCall(
                tool_call_id=call_id.strip(),
                function_name=name.strip(),
                arguments=arguments,
            )
        )
    return converted or None


def _step_metrics_from_usage(usage: Any) -> Metrics | None:
    prompt = _usage_token(usage, "promptTokens", "prompt_tokens", "input_tokens")
    completion = _usage_token(
        usage, "completionTokens", "completion_tokens", "output_tokens"
    )
    cached = _usage_token(
        usage,
        "cachedPromptTokens",
        "cached_prompt_tokens",
        "cache_read_input_tokens",
        "cached_tokens",
    )
    if prompt is None and completion is None and cached is None:
        return None
    return Metrics(
        prompt_tokens=prompt,
        completion_tokens=completion,
        cached_tokens=cached,
    )


def _tool_observation_content(message: dict[str, Any]) -> str:
    texts = _message_text_parts(message, "text")
    if texts:
        return "\n".join(texts)
    content = message.get("content")
    if content is None:
        return ""
    if isinstance(content, str):
        return content
    try:
        return json.dumps(content, ensure_ascii=False)
    except (TypeError, ValueError):
        return str(content)


FILE_MUTATION_TOOL_NAMES = frozenset(
    {
        "workspace__writeFile",
        "workspace__strReplace",
        "workspace__editFile",
        "workspace__deleteFile",
        "workspace__moveFile",
        "workspace__copyFile",
    }
)


def _diagnostic_tool_call(tool_call: ToolCall) -> dict[str, Any]:
    """Return a compact, non-content summary of a tool call."""
    summary: dict[str, Any] = {
        "tool_call_id": tool_call.tool_call_id,
        "function_name": tool_call.function_name,
    }
    arguments = tool_call.arguments
    if isinstance(arguments, dict):
        path = arguments.get("path")
        if isinstance(path, str) and path.strip():
            summary["path"] = path
    return summary


def _tool_result_succeeded(message: dict[str, Any]) -> bool:
    """Prefer structured tool-error metadata, with legacy text fallback."""
    metadata = message.get("metadata")
    if isinstance(metadata, dict):
        tool_error = metadata.get("toolError")
        if isinstance(tool_error, bool):
            return not tool_error

    if _format_message_error(message.get("error")):
        return False
    content = _tool_observation_content(message).lstrip()
    if content.startswith(("✗", "❌")):
        return False
    lowered = content.lower()
    return not (
        lowered.startswith("command failed")
        or lowered.startswith("file operation failed")
        or lowered.startswith("error:")
        or "failed with exit code" in lowered
    )


def summarize_completion_telemetry(messages: list[Any]) -> CompletionTelemetry:
    """Summarize the last action and completion signals from session messages."""
    tool_calls: list[ToolCall] = []
    tool_results: dict[str, dict[str, Any]] = {}
    for message in messages:
        if not isinstance(message, dict):
            continue
        role = str(message.get("role") or "").lower()
        if role == "tool":
            call_id = message.get("toolCallId") or message.get("tool_call_id")
            if isinstance(call_id, str) and call_id.strip():
                tool_results[call_id.strip()] = message
            continue
        if role != "assistant":
            continue
        tool_calls.extend(_libragent_tool_calls(message) or [])

    last_tool_call = _diagnostic_tool_call(tool_calls[-1]) if tool_calls else None
    terminal_calls = [
        call for call in tool_calls if call.function_name == "ui__reportResult"
    ]
    terminal_reported = bool(terminal_calls)
    terminal_report_result_received = any(
        call.tool_call_id in tool_results for call in terminal_calls
    )

    last_successful_file_mutation: dict[str, Any] | None = None
    successful_tool_results = 0
    failed_tool_results = 0
    unresolved_tool_results = 0
    for call in tool_calls:
        result = tool_results.get(call.tool_call_id)
        if result is None:
            unresolved_tool_results += 1
        elif _tool_result_succeeded(result):
            successful_tool_results += 1
        else:
            failed_tool_results += 1

    for call in tool_calls:
        if (
            call.function_name in FILE_MUTATION_TOOL_NAMES
            and call.tool_call_id in tool_results
            and _tool_result_succeeded(tool_results[call.tool_call_id])
        ):
            last_successful_file_mutation = _diagnostic_tool_call(call)

    return CompletionTelemetry(
        last_tool_call=last_tool_call,
        last_successful_file_mutation=last_successful_file_mutation,
        terminal_reported=terminal_reported,
        terminal_report_result_received=terminal_report_result_received,
        successful_tool_results=successful_tool_results,
        failed_tool_results=failed_tool_results,
        unresolved_tool_results=unresolved_tool_results,
    )


def _append_observation_result(step: Step, result: ObservationResult) -> None:
    if step.observation is None:
        step.observation = Observation(results=[result])
    else:
        step.observation.results.append(result)


def _find_agent_step_for_tool_call(
    steps: list[Step], tool_call_id: str
) -> Step | None:
    for step in reversed(steps):
        if step.source != "agent" or not step.tool_calls:
            continue
        if any(tc.tool_call_id == tool_call_id for tc in step.tool_calls):
            return step
    return None


def _attach_tool_observation(
    steps: list[Step],
    message: dict[str, Any],
    pending_by_id: dict[str, list[ObservationResult]],
) -> None:
    """Attach a tool-result message onto the matching agent step.

    LibrAgent message order is often ``tool`` then the ``assistant`` that
    declared the matching ``toolCalls`` id, so unmatched results are buffered
    until the agent step appears.
    """
    call_id = message.get("toolCallId") or message.get("tool_call_id")
    source_call_id = (
        call_id.strip() if isinstance(call_id, str) and call_id.strip() else None
    )
    result = ObservationResult(
        source_call_id=source_call_id,
        content=_tool_observation_content(message),
    )
    if source_call_id:
        matched = _find_agent_step_for_tool_call(steps, source_call_id)
        if matched is not None:
            _append_observation_result(matched, result)
            return
        pending_by_id.setdefault(source_call_id, []).append(result)
        return

    agent_step = next((step for step in reversed(steps) if step.source == "agent"), None)
    if agent_step is not None:
        _append_observation_result(agent_step, result)


def _consume_pending_observations(
    step: Step,
    pending_by_id: dict[str, list[ObservationResult]],
) -> None:
    if not step.tool_calls:
        return
    results: list[ObservationResult] = []
    for tool_call in step.tool_calls:
        results.extend(pending_by_id.pop(tool_call.tool_call_id, []))
    if results:
        if step.observation is None:
            step.observation = Observation(results=results)
        else:
            step.observation.results.extend(results)


def build_atif_trajectory(
    messages: list[Any],
    *,
    agent_name: str,
    agent_version: str,
    model_name: str | None,
    session_id: str | None = None,
    telemetry: TrajectoryTelemetry | None = None,
) -> Trajectory:
    """Convert harvested LibrAgent messages into an ATIF-v1.7 Trajectory."""
    steps: list[Step] = []
    pending_by_id: dict[str, list[ObservationResult]] = {}
    for message in messages:
        if not isinstance(message, dict):
            continue
        role = str(message.get("role") or "").lower()
        if role == "user":
            text = _assistant_message_text(message) or "(empty user message)"
            steps.append(
                Step(
                    step_id=len(steps) + 1,
                    source="user",
                    message=text,
                )
            )
            continue
        if role == "system":
            text = _assistant_message_text(message) or "(empty system message)"
            steps.append(
                Step(
                    step_id=len(steps) + 1,
                    source="system",
                    message=text,
                )
            )
            continue
        if role == "tool":
            _attach_tool_observation(steps, message, pending_by_id)
            continue
        if role != "assistant":
            continue

        message_text = _assistant_message_text(message)
        reasoning = _assistant_reasoning(message)
        tool_calls = _libragent_tool_calls(message)
        if not message_text:
            if tool_calls:
                message_text = f"(assistant tool call: {tool_calls[0].function_name})"
            elif reasoning:
                message_text = "(assistant reasoning only)"
            else:
                message_text = "(empty assistant message)"

        step = Step(
            step_id=len(steps) + 1,
            source="agent",
            message=message_text,
            reasoning_content=reasoning,
            tool_calls=tool_calls,
            metrics=_step_metrics_from_usage(message.get("usage")),
        )
        _consume_pending_observations(step, pending_by_id)
        steps.append(step)

    # Orphan tool results (no matching agent tool_call_id): keep content, drop id.
    if pending_by_id:
        orphan_results = [
            ObservationResult(source_call_id=None, content=result.content)
            for results in pending_by_id.values()
            for result in results
        ]
        pending_by_id.clear()
        agent_step = next(
            (step for step in reversed(steps) if step.source == "agent"), None
        )
        if agent_step is None:
            agent_step = Step(
                step_id=len(steps) + 1,
                source="agent",
                message="(orphaned LibrAgent tool results)",
            )
            steps.append(agent_step)
        for result in orphan_results:
            _append_observation_result(agent_step, result)

    if not steps:
        steps.append(
            Step(
                step_id=1,
                source="agent",
                message="(no LibrAgent messages harvested for ATIF trajectory)",
            )
        )

    stats = telemetry or summarize_trajectory(
        [m for m in messages if isinstance(m, dict)]
    )
    final_metrics = FinalMetrics(
        total_prompt_tokens=stats.n_input_tokens,
        total_completion_tokens=stats.n_output_tokens,
        total_cached_tokens=stats.n_cache_tokens,
        total_steps=len(steps),
    )
    return Trajectory(
        schema_version="ATIF-v1.7",
        session_id=session_id,
        agent=AtifAgent(
            name=agent_name,
            version=agent_version,
            model_name=model_name,
        ),
        steps=steps,
        final_metrics=final_metrics,
    )


def write_atif_trajectory(path: Path, trajectory: Trajectory) -> None:
    """Serialize an ATIF trajectory to ``path`` (usually ``logs_dir/trajectory.json``)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        format_trajectory_json(trajectory.to_json_dict()),
        encoding="utf-8",
    )


def resolve_execution_mode(
    explicit_mode: str | None = None,
    *,
    env: dict[str, str] | None = None,
) -> str:
    """Resolve benchmark execution mode from explicit arg or LIBRAGENT_EXECUTION_MODE."""
    env_map = env if env is not None else os.environ
    candidate = (explicit_mode or env_map.get("LIBRAGENT_EXECUTION_MODE") or DEFAULT_EXECUTION_MODE)
    mode = candidate.strip().lower()
    if mode not in EXECUTION_MODE_VALUES:
        allowed = ", ".join(EXECUTION_MODE_VALUES)
        raise ValueError(
            f"Invalid execution mode '{candidate}'. Expected one of: {allowed}."
        )
    return mode


def is_workflow_complete(status: str, *, seen_non_idle: bool) -> bool:
    """Return True only when session status is a real workflow completion.

    Harbor must not harvest while Busy/Queued/Provisioning/Paused. Brief Idle
    before Busy is ignored until a non-idle status has been observed.
    """
    normalized = status.strip().lower()
    if normalized not in TERMINAL_WORKFLOW_STATUSES:
        return False
    if normalized == "idle" and not seen_non_idle:
        return False
    return True


def resolve_poll_timeout_sec(
    explicit_timeout: float | None = None,
    *,
    env: dict[str, str] | None = None,
) -> float | None:
    """Optional wall-clock poll budget (seconds). None = wait until Harbor cancels."""
    env_map = env if env is not None else os.environ
    if explicit_timeout is not None:
        return float(explicit_timeout)
    raw = env_map.get("LIBRAGENT_POLL_TIMEOUT_SEC")
    if raw is None or raw.strip() == "":
        return None
    return float(raw)


def sanitize_docker_compose_project_name(name: str) -> str:
    """Mirror Harbor's compose project-name sanitizer."""
    sanitized = re.sub(r"[^a-zA-Z0-9_-]", "", name.lower().replace(" ", ""))
    if not sanitized:
        return "harbor"
    if sanitized[0].isdigit() or sanitized[0] == "-":
        sanitized = f"p{sanitized}"
    return sanitized[:63]


def docker_inspect_workdir(container_id: str) -> str | None:
    """Read the container's configured WorkingDir (image WORKDIR), if any."""
    try:
        result = subprocess.run(
            [
                "docker",
                "inspect",
                "-f",
                "{{.Config.WorkingDir}}",
                container_id,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker inspect workdir failed for "
            f"{container_id!r}: {exc}"
        )
        return None
    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        print(
            f"[LibrAgent] Warning: docker inspect workdir failed for "
            f"{container_id!r}: {stderr or result.returncode}"
        )
        return None
    workdir = (result.stdout or "").strip()
    if not workdir or workdir == "/":
        return None
    return workdir


def docker_exec_pwd(container_id: str) -> str | None:
    """Ask the live container for its current working directory."""
    try:
        result = subprocess.run(
            ["docker", "exec", container_id, "pwd"],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker exec pwd failed for "
            f"{container_id!r}: {exc}"
        )
        return None
    if result.returncode != 0:
        return None
    workdir = (result.stdout or "").strip()
    if not workdir or workdir == "/":
        return None
    return workdir


def resolve_container_workdir(
    environment: BaseEnvironment,
    container_id: str | None = None,
) -> str:
    """Resolve the container path LibrAgent should treat as workspace root.

    Workdir is **per-task / per-image**, never a single hardcoded benchmark path.

    Priority:
    1. Harbor task ``[environment].workdir`` when the task sets it explicitly
    2. Docker image WORKDIR of the attached Harbor main container
    3. Live ``docker exec … pwd`` (container process cwd)
    4. Last-resort ``/app`` with a warning (legacy Terminal-Bench convention only)
    """
    if hasattr(environment, "task_env_config") and environment.task_env_config:
        configured = getattr(environment.task_env_config, "workdir", None)
        if configured:
            workdir = str(configured)
            print(f"[LibrAgent] Using task-configured container workdir: {workdir}")
            return workdir

    if container_id:
        inspected = docker_inspect_workdir(container_id)
        if inspected:
            print(
                f"[LibrAgent] Using container image WORKDIR for attach: {inspected}"
            )
            return inspected

        live_pwd = docker_exec_pwd(container_id)
        if live_pwd:
            print(f"[LibrAgent] Using live container pwd for attach: {live_pwd}")
            return live_pwd

    print(
        "[LibrAgent] Warning: no task workdir / image WORKDIR / live pwd found; "
        "falling back to /app (may be wrong for this task)"
    )
    return "/app"


def resolve_harbor_main_container_id(environment: BaseEnvironment) -> str | None:
    """Resolve Harbor's Docker Compose `main` container id, if available."""
    session_id = getattr(environment, "session_id", None)
    if not session_id:
        print(
            "[LibrAgent] Warning: Harbor environment has no session_id; "
            "cannot resolve Compose main container for attach."
        )
        return None

    project = sanitize_docker_compose_project_name(str(session_id))
    try:
        result = subprocess.run(
            [
                "docker",
                "ps",
                "-q",
                "--filter",
                f"label=com.docker.compose.project={project}",
                "--filter",
                f"label=com.docker.compose.service={MAIN_COMPOSE_SERVICE}",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker ps failed while resolving Harbor main "
            f"container (project={project!r}): {exc}"
        )
        return None

    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        print(
            f"[LibrAgent] Warning: docker ps returned {result.returncode} while "
            f"resolving Harbor main container (project={project!r})"
            + (f": {stderr}" if stderr else "")
        )
        return None

    for line in (result.stdout or "").splitlines():
        cid = line.strip()
        if cid:
            return cid

    print(
        f"[LibrAgent] Warning: no running Compose service '{MAIN_COMPOSE_SERVICE}' "
        f"for project={project!r} (session_id={session_id!r}). "
        "If Harbor's sanitizer differs, attach will be skipped and host-sync fallback used."
    )
    return None


class LibrAgentHarborAdapter(BaseAgent):
    """
    Harbor Framework Adapter for LibrAgent.
    Integrates LibrAgent's headless REST API with Harbor's terminal benchmark loop.
    """

    def __init__(
        self,
        logs_dir: Path,
        model_name: str | None = None,
        logger: Any | None = None,
        mcp_servers: list[Any] | None = None,
        skills_dir: str | None = None,
        *args: Any,
        api_url: str = "http://localhost:3030/api",
        assistant_id: str = "coder-assistant",
        execution_mode: str | None = None,
        poll_timeout_sec: float | None = None,
        poll_interval_sec: float = DEFAULT_POLL_INTERVAL_SEC,
        extra_env: dict[str, str] | None = None,
        **kwargs: Any,
    ) -> None:
        self.api_url = api_url.rstrip("/")
        self.assistant_id = assistant_id
        self.execution_mode = resolve_execution_mode(execution_mode)
        self.poll_timeout_sec = resolve_poll_timeout_sec(poll_timeout_sec)
        self.poll_interval_sec = float(poll_interval_sec)
        self._resolved_version = read_repo_package_version()
        self._agent_info: AgentInfo | None = None
        super().__init__(
            logs_dir,
            model_name,
            logger,
            mcp_servers,
            skills_dir,
            *args,
            extra_env=extra_env,
            **kwargs,
        )

    @staticmethod
    def name() -> str:
        return "LibrAgent"

    def version(self) -> str | None:
        return self._resolved_version

    def to_agent_info(self) -> AgentInfo:
        """Return one stable AgentInfo instance for the whole trial.

        Harbor captures this object before ``run`` but serializes it after, so
        keeping a single instance lets :meth:`_apply_model_name` backfill
        ``model_info`` once the LibrAgent session reports the model it used.
        Without ``model_info`` the Harbor uploader skips the ``trial_model`` row
        that carries token counts and cost, and the hub shows "No token data".
        """
        if self._agent_info is None:
            self._agent_info = super().to_agent_info()
        return self._agent_info

    def _apply_model_name(self, model_name: str | None) -> bool:
        """Adopt ``model_name`` for Harbor reporting; True when it changed."""
        if not model_name or model_name == self.model_name:
            return False
        self.model_name = model_name
        self._init_model_info()
        if self._agent_info is not None:
            self._agent_info.model_info = super().to_agent_info().model_info
        return True

    def _apply_agent_version(self, version: str | None) -> bool:
        """Adopt a live binary version for Harbor reporting; True when it changed."""
        if not version or version == self._resolved_version:
            return False
        self._resolved_version = version
        if self._agent_info is not None:
            self._agent_info.version = version
        return True

    async def setup(self, environment: BaseEnvironment) -> None:
        """Validates connection and resolves Harbor model_info from the assistant."""
        _ = environment  # Harbor requires the arg; health/model lookup is API-only.
        if httpx is None:
            raise RuntimeError(
                "Python library 'httpx' is missing. Please run: pip install httpx"
            )

        async with httpx.AsyncClient() as client:
            try:
                res = await client.get(f"{self.api_url}/health", timeout=5.0)
                if res.status_code != 200:
                    raise RuntimeError(
                        f"LibrAgent health check returned invalid status code: {res.status_code}"
                    )
                try:
                    health_payload = res.json()
                except Exception:
                    health_payload = None
                live_version = extract_version_from_health_payload(health_payload)
                if self._apply_agent_version(live_version):
                    print(
                        f"[{self.name()}] Adopted agent version from health: "
                        f"{live_version}"
                    )
            except Exception as e:
                if isinstance(e, RuntimeError):
                    raise
                raise RuntimeError(
                    f"Unable to connect to LibrAgent daemon at {self.api_url}. "
                    f"Please make sure LibrAgent is running and API is active. Error: {e}"
                ) from e

            # Harbor records agent_info.model_info from self.model_name during
            # prepare (after setup, before run). Prefer an explicit Harbor -m /
            # constructor model_name; otherwise resolve from the assistant config.
            if not self.model_name:
                await self._resolve_model_name_from_assistant(client)

    async def _resolve_model_name_from_assistant(self, client: Any) -> None:
        """Populate ``self.model_name`` from ``GET /assistants/{id}`` when unset."""
        try:
            res = await client.get(
                f"{self.api_url}/assistants/{self.assistant_id}",
                timeout=10.0,
            )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to fetch assistant "
                f"{self.assistant_id} for model_info: {e}"
            )
            return

        if res.status_code != 200:
            print(
                f"[{self.name()}] Warning: assistant lookup for model_info returned "
                f"{res.status_code}: {res.text}"
            )
            return

        try:
            payload = res.json()
        except Exception as e:
            print(
                f"[{self.name()}] Warning: assistant payload was not JSON: {e}"
            )
            return

        model_name = extract_model_name_from_assistant_payload(payload)
        if not model_name:
            print(
                f"[{self.name()}] Assistant {self.assistant_id} pins no model/provider; "
                f"will adopt the model reported by the LibrAgent session during run."
            )
            return

        self._apply_model_name(model_name)
        print(f"[{self.name()}] Resolved Harbor model_info from assistant: {model_name}")

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        """
        Spawns a session in LibrAgent, injects the task instruction, polls for completion,
        and harvests the final trajectory and output.
        """
        if httpx is None:
            raise RuntimeError("Python library 'httpx' is missing.")

        container_id = resolve_harbor_main_container_id(environment)
        container_workdir = resolve_container_workdir(environment, container_id)
        attach_container_id = container_id
        use_attach = attach_container_id is not None

        local_workspace: Path | None = None
        local_workspace_str: str | None = None

        if use_attach:
            print(
                f"[{self.name()}] Attaching LibrAgent Docker session to Harbor "
                f"container {attach_container_id} (workdir={container_workdir})..."
            )
        else:
            local_workspace = self._resolve_local_workspace(environment, context)
            local_workspace_str = str(local_workspace.resolve().absolute())
            os.makedirs(local_workspace_str, exist_ok=True)
            print(
                f"[{self.name()}] Harbor container id not resolved; falling back to "
                f"host workspace sync ({local_workspace_str})."
            )
            print(
                f"[{self.name()}] Pulling initial container files from "
                f"{container_workdir} to {local_workspace_str}..."
            )
            try:
                await environment.download_dir(
                    source_dir=container_workdir,
                    target_dir=local_workspace_str,
                )
                print(f"[{self.name()}] Successfully pulled initial files.")
            except Exception as e:
                print(
                    f"[{self.name()}] Warning: failed to pull initial container files: {e}"
                )

        raw_context_task_id = getattr(context, "task_id", None)
        context_task_id = (
            raw_context_task_id.strip()
            if isinstance(raw_context_task_id, str)
            and raw_context_task_id.strip().lower() not in {"bench", "benchmark"}
            else None
        )
        inferred_task_id, _ = _task_identity_from_logs_dir(self.logs_dir)
        task_id = context_task_id or inferred_task_id or "bench"
        payload: dict[str, Any] = {
            "assistantId": self.assistant_id,
            "name": f"Harbor Benchmark Task: {task_id}",
            "request": instruction,
            "executionMode": self.execution_mode,
        }

        if use_attach and attach_container_id is not None:
            payload["workspaceIsolation"] = "docker"
            payload["dockerConfig"] = {
                "attachContainer": attach_container_id,
                "workdir": container_workdir,
                "manageLifecycle": False,
            }
        else:
            payload["workspacePath"] = local_workspace_str
            payload["workspaceIsolation"] = "host"

        async with httpx.AsyncClient(timeout=600.0) as client:
            mode_desc = (
                f"attach:{attach_container_id}"
                if use_attach
                else f"host:{local_workspace_str}"
            )
            session_started_at = asyncio.get_running_loop().time()
            session_started_at_utc = datetime.now(timezone.utc).isoformat()
            print(f"[{self.name()}] Creating session ({mode_desc})...")
            res = await client.post(f"{self.api_url}/sessions", json=payload)
            if res.status_code != 201:
                raise RuntimeError(
                    f"LibrAgent session creation failed ({res.status_code}): {res.text}"
                )

            response_json = res.json()
            session_id = response_json.get("id")
            if not session_id:
                raise RuntimeError(
                    "LibrAgent session creation succeeded but response did not contain "
                    f"'id': {response_json}"
                )

            print(
                f"[{self.name()}] Session {session_id} spawned successfully "
                f"(executionMode={self.execution_mode}). Awaiting execution completion..."
            )

            # In attach/docker mode, download initial container files to host staging workspace
            # so that host-based file search tools (like globFiles, grepFiles) can find them.
            initial_session_info: dict[str, Any] | None = None
            if use_attach:
                try:
                    session_res = await client.get(f"{self.api_url}/sessions/{session_id}")
                    if session_res.status_code == 200:
                        session_info = session_res.json()
                        if isinstance(session_info, dict):
                            initial_session_info = session_info
                        host_workspace = session_info.get("dockerHostWorkspacePath")
                        if host_workspace:
                            host_workspace_path = Path(host_workspace)
                            os.makedirs(host_workspace_path, exist_ok=True)
                            print(
                                f"[{self.name()}] Pulling initial container files from "
                                f"{container_workdir} to host staging workspace ({host_workspace})..."
                            )
                            await environment.download_dir(
                                source_dir=container_workdir,
                                target_dir=str(host_workspace_path.resolve().absolute()),
                            )
                            print(f"[{self.name()}] Successfully pulled initial files to host staging workspace.")
                except Exception as e:
                    print(
                        f"[{self.name()}] Warning: failed to pull initial container files to host staging: {e}"
                    )
            last_session_info: dict[str, Any] | None = initial_session_info
            self._write_benchmark_contract(
                task_id=task_id,
                session_id=session_id,
                container_id=attach_container_id,
                container_workdir=container_workdir,
                workspace_mode=workspace_mode_name(use_attach),
                started_at=session_started_at_utc,
                session_info=last_session_info,
            )
            if self.poll_timeout_sec is not None:
                print(
                    f"[{self.name()}] Poll wall-clock budget: "
                    f"{self.poll_timeout_sec:.0f}s"
                )

            poll_interval = self.poll_interval_sec
            seen_non_idle = False
            completed = False
            last_status = "unknown"
            poll_deadline = (
                asyncio.get_running_loop().time() + self.poll_timeout_sec
                if self.poll_timeout_sec is not None
                else None
            )

            try:
                while True:
                    if (
                        poll_deadline is not None
                        and asyncio.get_running_loop().time() >= poll_deadline
                    ):
                        print(
                            f"[{self.name()}] Poll wall-clock budget exhausted "
                            f"(last status={last_status}). Writing diagnostic "
                            f"trajectory, then deleting session."
                        )
                        with contextlib.suppress(asyncio.CancelledError):
                            await self._run_shielded(
                                self._dump_incomplete_diagnostics(
                                    session_id=session_id,
                                    last_status=last_status,
                                    seen_non_idle=seen_non_idle,
                                    reason="poll_deadline",
                                    last_session_info=last_session_info,
                                    extra={
                                        "poll_timeout_sec": self.poll_timeout_sec,
                                    },
                                    workspace_mode=workspace_mode_name(use_attach),
                                    context=context,
                                    started_monotonic=session_started_at,
                                )
                            )
                        await self._delete_session(session_id)
                        raise TimeoutError(
                            f"LibrAgent session {session_id} did not reach a terminal "
                            f"workflow status within {self.poll_timeout_sec:.0f}s "
                            f"(last status={last_status}). Increase Harbor "
                            f"--agent-timeout-multiplier / LIBRAGENT_POLL_TIMEOUT_SEC "
                            f"or wait for the agent to finish before harvesting."
                        )

                    await asyncio.sleep(poll_interval)
                    try:
                        status_res = await client.get(
                            f"{self.api_url}/sessions/{session_id}"
                        )
                    except Exception as e:
                        print(
                            f"[{self.name()}] Connection error during polling, retrying: {e}"
                        )
                        continue

                    if status_res.status_code != 200:
                        print(
                            f"[{self.name()}] Warning: Failed to fetch session status: "
                            f"{status_res.text}"
                        )
                        continue

                    session_info = status_res.json()
                    if isinstance(session_info, dict):
                        last_session_info = session_info
                    current_status = str(session_info.get("status", "idle")).lower()
                    last_status = current_status
                    if current_status not in TERMINAL_WORKFLOW_STATUSES:
                        seen_non_idle = True

                    if current_status == "paused":
                        print(
                            f"[{self.name()}] Session workflow was paused (or cancelled). "
                            f"Deleting session and failing fast."
                        )
                        await self._delete_session(session_id)
                        raise RuntimeError(
                            f"LibrAgent session {session_id} was paused/cancelled."
                        )

                    if is_workflow_complete(
                        current_status, seen_non_idle=seen_non_idle
                    ):
                        print(
                            f"[{self.name()}] Session workflow reached terminal state: "
                            f"{current_status}"
                        )
                        completed = True
                        break
            except asyncio.CancelledError:
                # Harbor agent timeout cancels this coroutine. Do NOT treat the
                # run as a successful completion (that caused verifiers to score
                # incomplete workspaces), but still dump a diagnostic ATIF so
                # timeout trials are analyzable.
                print(
                    f"[{self.name()}] Session polling cancelled (Harbor timeout) "
                    f"while status={last_status}. Writing diagnostic trajectory, "
                    f"then deleting session (not a successful harvest)."
                )
                with contextlib.suppress(asyncio.CancelledError):
                    await self._run_shielded(
                        self._dump_incomplete_diagnostics(
                            session_id=session_id,
                            last_status=last_status,
                            seen_non_idle=seen_non_idle,
                            reason="harbor_cancelled",
                            last_session_info=last_session_info,
                            workspace_mode=workspace_mode_name(use_attach),
                            context=context,
                            started_monotonic=session_started_at,
                        )
                    )
                # Shielded delete re-raises CancelledError after teardown.
                await self._delete_session(session_id)
                raise

            if not completed:
                await self._delete_session(session_id)
                raise RuntimeError(
                    f"LibrAgent session {session_id} ended polling without a completed "
                    f"workflow (last status={last_status})."
                )

            if not use_attach and local_workspace is not None:
                print(
                    f"[{self.name()}] Pushing updated workspace files from "
                    f"{local_workspace} to container {container_workdir}..."
                )
                try:
                    await self._upload_workspace(
                        environment,
                        local_workspace,
                        container_workdir,
                    )
                    print(f"[{self.name()}] Successfully pushed workspace files.")
                except Exception as e:
                    await self._delete_session(session_id)
                    raise RuntimeError(
                        f"Error pushing workspace files back to container: {e}"
                    ) from e
            elif use_attach:
                print(
                    f"[{self.name()}] Attach mode: skipping host→container upload "
                    f"(agent wrote in-place under {container_workdir})."
                )
                local_workspace = self._resolve_local_workspace(environment, context)
                local_workspace_str = str(local_workspace.resolve().absolute())
                os.makedirs(local_workspace_str, exist_ok=True)
                print(
                    f"[{self.name()}] Pulling final container files from "
                    f"{container_workdir} to local workspace ({local_workspace_str})..."
                )
                try:
                    await environment.download_dir(
                        source_dir=container_workdir,
                        target_dir=local_workspace_str,
                    )
                    print(f"[{self.name()}] Successfully pulled final files to local workspace.")
                except Exception as e:
                    print(
                        f"[{self.name()}] Warning: failed to pull final container files: {e}"
                    )

            messages_res = await client.get(
                f"{self.api_url}/sessions/{session_id}/messages",
                params={"limit": TRAJECTORY_MESSAGE_LIMIT},
            )
            if messages_res.status_code != 200:
                print(
                    f"[{self.name()}] Warning: Failed to harvest session messages "
                    f"({messages_res.status_code})"
                )
                messages: list[Any] = []
            else:
                messages_data = messages_res.json()
                messages = normalize_session_messages(messages_data.get("messages"))

            final_answer = ""
            for msg in reversed(messages):
                if msg.get("role") != "assistant":
                    continue
                contents = msg.get("content", [])
                final_answer = "\n".join(
                    item.get("text", "")
                    for item in contents
                    if item.get("type") == "text"
                )
                break

            telemetry = summarize_trajectory(messages)
            copy_telemetry_to_context(context, telemetry)

            # The session knows which model actually ran (assistants may inherit the
            # app default instead of pinning one). Adopt it so agent_info.model_info
            # is populated before Harbor writes result.json.
            session_model_name = extract_model_name_from_session_payload(
                last_session_info
            )
            if self._apply_model_name(session_model_name):
                print(
                    f"[{self.name()}] Adopted Harbor model_info from session: "
                    f"{session_model_name}"
                )

            self._write_atif_trajectory(
                messages=messages,
                session_id=session_id,
                telemetry=telemetry,
            )

            if telemetry.tool_calls_count == 0:
                print(
                    f"[{self.name()}] Empty agent work: terminal status={last_status} "
                    f"with 0 tool calls ({len(messages)} messages, "
                    f"turns={telemetry.n_turns}). Failing the trial."
                )
                self._write_diagnostic_meta(
                    build_diagnostic_meta(
                        reason="empty_work",
                        session_id=session_id,
                        last_status=last_status,
                        seen_non_idle=seen_non_idle,
                        telemetry=telemetry,
                        n_messages=len(messages),
                        completed=True,
                        extra={
                            "assistant_id": self.assistant_id,
                            "workspaceMode": workspace_mode_name(use_attach),
                        },
                        completion=summarize_completion_telemetry(messages),
                        elapsed_wall_clock_sec=(
                            asyncio.get_running_loop().time() - session_started_at
                        ),
                    )
                )
                context.metadata = {
                    "output": final_answer,
                    "trajectory": messages,
                    "sessionId": session_id,
                    "finalStatus": last_status,
                    "completed": False,
                    "emptyWork": True,
                    "assistant_id": self.assistant_id,
                    "n_turns": telemetry.n_turns,
                    "tool_calls_count": 0,
                    "error": (
                        f"Session reached terminal status {last_status!r} "
                        "without any tool calls."
                    ),
                }
                await self._delete_session(session_id)
                raise EmptyAgentWorkError(
                    f"LibrAgent session {session_id} reached terminal status "
                    f"{last_status!r} without any tool calls "
                    f"(messages={len(messages)}, turns={telemetry.n_turns})."
                )

            metadata: dict[str, Any] = {
                "output": final_answer,
                "trajectory": messages,
                "sessionId": session_id,
                "finalStatus": last_status,
                "completed": True,
                "attachContainer": attach_container_id,
                "workspaceMode": workspace_mode_name(use_attach),
                "assistant_id": self.assistant_id,
                "n_turns": telemetry.n_turns,
                "tool_calls_count": telemetry.tool_calls_count,
            }
            resolved_provider, resolved_model = split_harbor_model_name(self.model_name)
            if resolved_model:
                metadata["model_name"] = resolved_model
            if resolved_provider:
                metadata["provider"] = resolved_provider
            if last_status in {"error", "cancel", "cancelled"} and telemetry.error:
                metadata["error"] = telemetry.error
            context.metadata = metadata

            print(
                f"[{self.name()}] Task complete. Response harvested successfully "
                f"({len(messages)} messages, status={last_status}, "
                f"turns={telemetry.n_turns}, tools={telemetry.tool_calls_count}, "
                f"tokens_in={telemetry.n_input_tokens}, "
                f"tokens_out={telemetry.n_output_tokens}, "
                f"tokens_cache={telemetry.n_cache_tokens})."
            )

            # Harvest + ATIF dump finished; delete the LibrAgent session so it
            # does not linger (and, in attach mode, keep writing into Harbor's
            # container) after Harbor moves on to the next task. DELETE also
            # terminates any still-running workflow before removing DB/workspace.
            await self._delete_session(session_id)

    async def _run_shielded(self, awaitable: Awaitable[Any]) -> None:
        """Run ``awaitable`` to completion even if the caller is cancelled."""
        task = asyncio.ensure_future(awaitable)
        try:
            await asyncio.shield(task)
        except asyncio.CancelledError:
            with contextlib.suppress(BaseException):
                await task
            raise

    async def _fetch_session_messages_best_effort(self, session_id: str) -> list[Any]:
        """GET session messages on a short-lived client; never raise."""
        if httpx is None:
            return []
        try:
            async with httpx.AsyncClient(timeout=30.0) as client:
                messages_res = await client.get(
                    f"{self.api_url}/sessions/{session_id}/messages",
                    params={"limit": TRAJECTORY_MESSAGE_LIMIT},
                )
            if messages_res.status_code != 200:
                print(
                    f"[{self.name()}] Warning: diagnostic message harvest failed "
                    f"({messages_res.status_code})"
                )
                return []
            messages_data = messages_res.json()
            return normalize_session_messages(messages_data.get("messages"))
        except Exception as e:
            print(
                f"[{self.name()}] Warning: diagnostic message harvest error "
                f"for session {session_id}: {e}"
            )
            return []

    async def _dump_incomplete_diagnostics(
        self,
        *,
        session_id: str,
        last_status: str,
        seen_non_idle: bool,
        reason: str,
        last_session_info: dict[str, Any] | None,
        extra: dict[str, Any] | None = None,
        workspace_mode: str | None = None,
        context: AgentContext | None = None,
        started_monotonic: float | None = None,
    ) -> TrajectoryTelemetry:
        """Best-effort ATIF + timeout_meta dump for cancelled/incomplete runs."""
        session_model_name = extract_model_name_from_session_payload(last_session_info)
        self._apply_model_name(session_model_name)

        messages = await self._fetch_session_messages_best_effort(session_id)
        telemetry = summarize_trajectory(messages)
        completion = summarize_completion_telemetry(messages)
        if context is not None:
            copy_telemetry_to_context(context, telemetry)
        self._write_atif_trajectory(
            messages=messages,
            session_id=session_id,
            telemetry=telemetry,
        )
        meta_extra: dict[str, Any] = {"assistant_id": self.assistant_id}
        if extra:
            meta_extra.update(extra)
        if workspace_mode:
            meta_extra["workspaceMode"] = workspace_mode
        self._write_diagnostic_meta(
            build_diagnostic_meta(
                reason=reason,
                session_id=session_id,
                last_status=last_status,
                seen_non_idle=seen_non_idle,
                telemetry=telemetry,
                n_messages=len(messages),
                completed=False,
                extra=meta_extra,
                completion=completion,
                elapsed_wall_clock_sec=(
                    asyncio.get_running_loop().time() - started_monotonic
                    if started_monotonic is not None
                    else None
                ),
            )
        )
        print(
            f"[{self.name()}] Diagnostic dump ({reason}): "
            f"messages={len(messages)}, turns={telemetry.n_turns}, "
            f"tools={telemetry.tool_calls_count}, status={last_status}, "
            f"last_tool={completion.last_tool_call}, "
            f"terminal_reported={completion.terminal_reported}"
        )
        return telemetry

    def _write_diagnostic_meta(self, payload: dict[str, Any]) -> None:
        """Write ``timeout_meta.json`` under Harbor's agent logs dir."""
        meta_path = self.logs_dir / "timeout_meta.json"
        try:
            meta_path.parent.mkdir(parents=True, exist_ok=True)
            meta_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            print(f"[{self.name()}] Wrote diagnostic meta to {meta_path}")
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to write diagnostic meta "
                f"to {meta_path}: {e}"
            )

    def _write_benchmark_contract(
        self,
        *,
        task_id: str,
        session_id: str,
        container_id: str | None,
        container_workdir: str,
        workspace_mode: str,
        started_at: str,
        session_info: dict[str, Any] | None,
    ) -> None:
        """Write non-secret run settings needed for later comparisons."""
        session_model, session_provider = extract_model_provider_from_session_payload(
            session_info
        )
        session_status = (
            _string_field(session_info, "status") if session_info is not None else None
        )
        _, trial_id = _task_identity_from_logs_dir(self.logs_dir)
        contract = {
            "schema_version": BENCHMARK_CONTRACT_SCHEMA_VERSION,
            "task_id": task_id,
            "trial_id": trial_id,
            "git": _git_contract_metadata(PACKAGE_JSON_PATH.parent),
            "agent": {
                "name": self.name(),
                "version": self.version(),
            },
            "assistant": {"id": self.assistant_id},
            "model": {
                "harbor_reported": self.model_name,
                "session_effective": format_harbor_model_name(
                    session_model, session_provider
                ),
            },
            "execution": {
                "mode": self.execution_mode,
                "workspace_mode": workspace_mode,
                "container_id": container_id,
                "container_workdir": container_workdir,
            },
            "harbor": _benchmark_contract_environment(),
            "adapter": {
                "api_url": self.api_url,
                "poll_timeout_sec": self.poll_timeout_sec,
                "poll_interval_sec": self.poll_interval_sec,
            },
            "session": {
                "id": session_id,
                "status": session_status,
            },
            "started_at": started_at,
            "captured_at": datetime.now(timezone.utc).isoformat(),
        }
        contract_path = self.logs_dir / "benchmark_contract.json"
        try:
            contract_path.parent.mkdir(parents=True, exist_ok=True)
            contract_path.write_text(
                json.dumps(contract, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to write benchmark contract "
                f"to {contract_path}: {e}"
            )

    def _write_atif_trajectory(
        self,
        *,
        messages: list[Any],
        session_id: str,
        telemetry: TrajectoryTelemetry,
    ) -> None:
        """Dump ATIF-v1.7 ``trajectory.json`` under Harbor's agent logs dir.

        Harbor Hub and ``harbor upload`` look for ``trial_dir/agent/trajectory.json``
        (``self.logs_dir``). Failure to write must not fail the trial itself.
        """
        trajectory_path = self.logs_dir / "trajectory.json"
        try:
            trajectory = build_atif_trajectory(
                messages,
                agent_name=self.name(),
                agent_version=self.version() or "unknown",
                model_name=self.model_name,
                session_id=session_id,
                telemetry=telemetry,
            )
            write_atif_trajectory(trajectory_path, trajectory)
            print(
                f"[{self.name()}] Wrote ATIF trajectory "
                f"({len(trajectory.steps)} steps) to {trajectory_path}"
            )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to write ATIF trajectory "
                f"to {trajectory_path}: {e}"
            )

    async def _delete_session(self, session_id: str) -> None:
        """Delete a LibrAgent session, surviving Harbor's coroutine cancellation.

        Uses ``DELETE /sessions/{id}``, which terminates any running workflow then
        removes the session (and cascaded children) from DB/workspace. The request
        is shielded so a Harbor agent-timeout cancel still tears the session down
        instead of orphaning it. When the caller is being cancelled, CancelledError
        is re-raised only after the delete request completes.
        """
        task = asyncio.ensure_future(self._delete_session_best_effort(session_id))
        try:
            await asyncio.shield(task)
        except asyncio.CancelledError:
            with contextlib.suppress(BaseException):
                await task
            raise

    async def _delete_session_best_effort(self, session_id: str) -> None:
        """DELETE /sessions/{id}, logging and swallowing transport errors.

        Uses a dedicated short-lived client because the primary request client may
        already be closing while the surrounding coroutine unwinds.
        """
        if httpx is None:
            return
        try:
            async with httpx.AsyncClient(timeout=30.0) as client:
                res = await client.delete(
                    f"{self.api_url}/sessions/{session_id}"
                )
            if res.status_code == 200:
                print(f"[{self.name()}] Deleted LibrAgent session {session_id}.")
            elif res.status_code == 404:
                print(
                    f"[{self.name()}] Session {session_id} already gone "
                    f"(delete returned 404)."
                )
            else:
                print(
                    f"[{self.name()}] Warning: delete for session {session_id} "
                    f"returned {res.status_code}: {res.text}"
                )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to delete session "
                f"{session_id}: {e}"
            )

    def _resolve_local_workspace(
        self, environment: BaseEnvironment, context: AgentContext
    ) -> Path:
        trial_paths = getattr(environment, "trial_paths", None)
        if (
            trial_paths
            and hasattr(trial_paths, "trial_dir")
            and trial_paths.trial_dir
        ):
            return Path(trial_paths.trial_dir) / "workspace"

        task_id = getattr(context, "task_id", None) or "benchmark"
        return Path(os.getcwd()) / f"harbor-workspace-{task_id}"

    async def _upload_workspace(
        self,
        environment: BaseEnvironment,
        local_workspace: Path,
        container_workdir: str,
    ) -> None:
        """Upload host workspace files into the container workdir.

        Prefer per-file uploads so Windows host paths and LibrAgent metadata
        (``.libragent``) do not break ``docker compose cp``.
        """
        uploaded = 0
        for path in sorted(local_workspace.rglob("*")):
            if not path.is_file():
                continue
            rel = path.relative_to(local_workspace)
            if rel.parts and rel.parts[0] in {".libragent", ".git"}:
                continue
            target = f"{container_workdir.rstrip('/')}/{rel.as_posix()}"
            await environment.upload_file(str(path), target)
            uploaded += 1

        if uploaded == 0:
            # Fallback: whole-directory sync (may still help non-Windows hosts).
            await environment.upload_dir(
                source_dir=str(local_workspace),
                target_dir=container_workdir,
            )
