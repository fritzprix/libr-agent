"""requireUserInput (secret user prompt) path completes via E2E control without leaking literals."""

import json
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")
LITERAL = "synthetic-e2e-fixture-only"


def test_evidence_present():
    assert EVIDENCE.is_file(), "harness must write session_e2e_evidence.json"


def test_interactive_prompt_was_resolved():
    """The harvested snapshot must show the session id and nothing still pending.

    Evidence is written after the workflow completes, so the control-plane
    snapshot must carry a resolved session with no leftover blocking prompt.
    """
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    assert data.get("sessionId"), "evidence must record the resolved session id"
    interactions = data.get("interactions")
    assert isinstance(interactions, dict), f"missing interactions snapshot: {interactions!r}"
    assert "interactionsError" not in data, f"control call failed: {data['interactionsError']}"
    assert interactions.get("pendingUserPrompt") is None, (
        "a user prompt was still pending at completion: "
        f"{interactions['pendingUserPrompt']!r}"
    )
    assert interactions.get("pendingApprovals") == [], (
        f"approvals still pending at completion: {interactions['pendingApprovals']!r}"
    )


def test_marker_in_trajectory_not_literal_secret():
    traj = json.loads(TRAJECTORY.read_text(encoding="utf-8"))
    blob = json.dumps(traj)
    assert "E2E_MARKER_OK" in blob
    assert LITERAL not in blob


def test_negative_literal_must_not_appear_even_if_marker_present():
    blob = TRAJECTORY.read_text(encoding="utf-8")
    assert LITERAL not in blob
