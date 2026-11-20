"""Trajectory helpers for the B-11 verifier.

Slim on purpose: only what ``test_outputs.py`` here needs. The fuller
verifier library lives in the dataset-level ``shared/`` helpers and is not
duplicated in this task.

B-11 checks *tool calls and their observations*, not a lowercased blob of
the whole trajectory: the instruction itself names ``workspace__runShell``
and the agent may quote it while explaining the fallback, which blob
matching miscounts (see issue #2655).
"""

from __future__ import annotations

import json
from pathlib import Path

TRAJECTORY = Path("/logs/agent/trajectory.json")

#: The forbidden synchronous bash tool (any builtin-server prefix allowed).
RUN_SHELL_SUFFIX = "runshell"

#: Interactive shell submit must not exist at all (regression guard).
INTERACTIVE_SUBMIT_SUFFIX = "submitinteractiveshellinput"

#: The Windows tool profile the task expects the agent to fall back to.
POWERSHELL_SUFFIXES = frozenset(
    {
        "runpowershell",
        "runinpersistentpowershell",
    }
)


def load_trajectory() -> dict:
    if not TRAJECTORY.is_file():
        return {}
    return json.loads(TRAJECTORY.read_text(encoding="utf-8"))


def trajectory_steps(trajectory: dict) -> list[dict]:
    return list(trajectory.get("steps") or [])


def tool_calls(steps: list[dict]) -> list[dict]:
    """Every declared tool call, from ``step["tool_calls"]``."""
    calls: list[dict] = []
    for step in steps:
        for call in step.get("tool_calls") or []:
            if isinstance(call, dict):
                calls.append(call)
    return calls


def call_function_name(call: dict) -> str:
    return str(call.get("function_name") or "")


def call_matches(call: dict, suffixes: frozenset[str] | str) -> bool:
    name = call_function_name(call).lower()
    if isinstance(suffixes, str):
        suffixes = frozenset({suffixes})
    return any(name.endswith(suffix) for suffix in suffixes)


def observation_results_by_call_id(steps: list[dict]) -> dict[str, dict]:
    """Map ``tool_call_id`` -> first observation result object.

    Tool results live in ``step["observation"]["results"][i]`` with
    ``source_call_id`` pointing back at the declaring tool call — not on
    the call itself.
    """
    by_id: dict[str, dict] = {}
    for step in steps:
        observation = step.get("observation") or {}
        for result in observation.get("results") or []:
            if not isinstance(result, dict):
                continue
            call_id = result.get("source_call_id")
            if isinstance(call_id, str) and call_id and call_id not in by_id:
                by_id[call_id] = result
    return by_id


def result_content_text(result: dict) -> str:
    content = result.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict) and item.get("text"):
                parts.append(str(item["text"]))
        return "\n".join(parts)
    return ""


def result_succeeded(result: dict) -> bool:
    """True unless the trajectory marks the result as a tool error.

    The harbor adapter writes ``extra.tool_error`` (bool) per observation
    result; when that field is absent (e.g. a Rust-side export without it),
    fall back to the same content heuristics the adapter uses. A call with
    no observation at all is unresolved, never "successful".
    """
    extra = result.get("extra")
    if isinstance(extra, dict) and isinstance(extra.get("tool_error"), bool):
        return not extra["tool_error"]
    text = result_content_text(result).lstrip()
    if text.startswith(("✗", "❌")):
        return False
    lowered = text.lower()
    return not (
        lowered.startswith("command failed")
        or lowered.startswith("file operation failed")
        or lowered.startswith("error:")
        or "failed with exit code" in lowered
    )
