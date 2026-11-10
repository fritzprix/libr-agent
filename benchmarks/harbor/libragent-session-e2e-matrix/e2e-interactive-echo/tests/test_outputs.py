"""requireUserInput (secret user prompt) path completes via E2E control without leaking literals."""

import json
from pathlib import Path

from verifier_lib import load_trajectory, observation_result_texts, trajectory_steps

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")

# The instruction asks the agent to run
# ``echo E2E_MARKER_$((60+43))``, so the literal only ever exists as the
# computed stdout of a real command. Echoing the command line itself cannot
# satisfy this check.
MARKER = "E2E_MARKER_103"
LITERAL = "synthetic-e2e-fixture-only"


def test_evidence_present():
    assert EVIDENCE.is_file(), "harness must write session_e2e_evidence.json"


def test_interactive_prompt_was_resolved():
    """The harness must have answered at least one real prompt via the control plane.

    ``resolvedInteractions`` records every action the harness took while the
    session ran, so an empty list means no prompt was ever opened and answered
    — which is exactly the false-pass this test used to allow. An answer the
    control plane did not deliver (``delivered: false``) does not count.
    """
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    assert data.get("sessionId"), "evidence must record the resolved session id"
    assert data.get("controlPlaneConfigured"), (
        "the trial did not run against a debug binary with the E2E control "
        "plane enabled (see run logs for the control-plane warning)"
    )
    answered = [
        action
        for action in data.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt"
        and action.get("action") == "answer"
        and action.get("delivered") is True
    ]
    assert answered, (
        "the harness never delivered an answer to a user prompt; the "
        f"requireUserInput path was not exercised. Actions: {data.get('resolvedInteractions')!r}"
    )
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


def test_marker_in_tool_results_not_literal_secret():
    """The computed marker must appear in a tool result, not in the agent's prose."""
    trajectory = load_trajectory()
    texts = observation_result_texts(trajectory_steps(trajectory))
    assert any(MARKER in t for t in texts), f"{MARKER} never appeared in any tool result"
    blob = json.dumps(trajectory)
    assert LITERAL not in blob


def test_negative_literal_must_not_appear_even_if_marker_present():
    blob = TRAJECTORY.read_text(encoding="utf-8")
    assert LITERAL not in blob
