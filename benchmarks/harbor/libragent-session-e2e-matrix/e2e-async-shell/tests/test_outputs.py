"""Session API E2E check for async shell execution (#2907).

Verifies from the exported ATIF transcript that:
1. Session evidence recorded a real sessionId.
2. The agent invoked ``spawnProcess``.
3. The computed marker ``ASYNC_E2E_MARKER_42`` appears in tool observations.
4. The dummy secret literal never leaks into the transcript.
"""

from __future__ import annotations

import json

from verifier_lib import (
    TRAJECTORY,
    assert_no_synthetic_literal_in_transcript,
    load_trajectory,
    observation_result_texts,
    require_session_evidence,
    trajectory_steps,
)

MARKER = "ASYNC_E2E_MARKER_42"
DUMMY_SECRET = "synthetic-e2e-fixture-only"


def test_session_evidence_present():
    require_session_evidence()


def test_spawn_process_called_and_marker_in_transcript():
    trajectory = load_trajectory()
    steps = trajectory_steps(trajectory)

    called_spawn = any(
        "spawnProcess" in str(call.get("function_name") or call.get("name") or call.get("tool_name") or "")
        for step in steps
        for call in (step.get("tool_calls") or [])
    )
    assert called_spawn, "expected workspace__spawnProcess tool call in transcript"

    texts = observation_result_texts(steps)
    assert any(MARKER in t for t in texts), (
        f"{MARKER} never appeared in any tool result observation"
    )


def test_dummy_secret_not_leaked():
    blob = TRAJECTORY.read_text(encoding="utf-8") if TRAJECTORY.is_file() else json.dumps(load_trajectory())
    assert_no_synthetic_literal_in_transcript(
        blob,
        literal=DUMMY_SECRET,
        allow_name="LIBRAGENT_E2E_SYNTHETIC_SECRET",
    )
