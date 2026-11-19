"""Session API E2E check for async shell execution (#2907).

Verifies from the exported ATIF transcript that:
1. Session evidence recorded a real sessionId.
2. The agent invoked ``spawnProcess``.
3. The computed marker ``ASYNC_E2E_MARKER_42`` appears in a ``waitForProcess``
   or ``readProcessOutput`` tool observation (not merely echoed in ``spawnProcess``).
4. The dummy secret literal never leaks into the transcript.
"""

from __future__ import annotations

import json
import os

from verifier_lib import (
    TRAJECTORY,
    assert_no_synthetic_literal_in_transcript,
    load_trajectory,
    require_session_evidence,
    trajectory_steps,
)

MARKER = "ASYNC_E2E_MARKER_42"
DUMMY_SECRET = os.environ.get(
    "LIBRAGENT_E2E_SYNTHETIC_SECRET", "synthetic-e2e-fixture-only"
)
OUTPUT_TOOLS = ("waitForProcess", "readProcessOutput")


def _call_name(call: dict) -> str:
    return str(call.get("function_name") or call.get("name") or call.get("tool_name") or "")


def _observation_text(result: dict) -> str:
    content = result.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(
            str(item["text"])
            for item in content
            if isinstance(item, dict) and item.get("text")
        )
    return ""


def _process_output_observations(steps: list[dict]) -> list[str]:
    """Return observation texts produced by waitForProcess or readProcessOutput calls."""
    texts: list[str] = []
    for step in steps:
        calls = step.get("tool_calls") or []
        by_id = {
            call.get("tool_call_id") or call.get("id"): _call_name(call)
            for call in calls
            if call.get("tool_call_id") or call.get("id")
        }
        step_has_output_tool = any(
            any(tool in _call_name(call) for tool in OUTPUT_TOOLS) for call in calls
        )
        observation = step.get("observation") or {}
        for result in observation.get("results") or []:
            source_id = result.get("source_call_id")
            if source_id and source_id in by_id:
                if not any(tool in by_id[source_id] for tool in OUTPUT_TOOLS):
                    continue
            elif not step_has_output_tool:
                continue
            text = _observation_text(result)
            if text:
                texts.append(text)
    return texts


def test_session_evidence_present():
    require_session_evidence()


def test_spawn_process_called_and_marker_in_process_output():
    trajectory = load_trajectory()
    steps = trajectory_steps(trajectory)

    called_spawn = any(
        "spawnProcess" in _call_name(call)
        for step in steps
        for call in (step.get("tool_calls") or [])
    )
    assert called_spawn, "expected workspace__spawnProcess tool call in transcript"

    texts = _process_output_observations(steps)
    assert any(MARKER in t for t in texts), (
        f"{MARKER} never appeared in a waitForProcess/readProcessOutput observation"
    )


def test_dummy_secret_not_leaked():
    blob = (
        TRAJECTORY.read_text(encoding="utf-8")
        if TRAJECTORY.is_file()
        else json.dumps(load_trajectory())
    )
    assert_no_synthetic_literal_in_transcript(
        blob,
        literal=DUMMY_SECRET,
        allow_name="LIBRAGENT_E2E_SYNTHETIC_SECRET",
    )
