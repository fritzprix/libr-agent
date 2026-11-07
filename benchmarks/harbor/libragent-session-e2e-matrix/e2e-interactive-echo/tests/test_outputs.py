"""requireUserInput (secret user prompt) path completes via E2E control without leaking literals."""

import json
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")
LITERAL = "synthetic-e2e-fixture-only"


def test_evidence_present():
    assert EVIDENCE.is_file(), "harness must write session_e2e_evidence.json"


def test_interactive_prompt_was_resolved():
    """The harness must have answered at least one real prompt via the control plane.

    ``resolvedInteractions`` records every action the harness took while the
    session ran, so an empty list means no prompt was ever opened and answered
    — which is exactly the false-pass this test used to allow.
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
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]
    assert answered, (
        "the harness never answered a user prompt; the requireUserInput path "
        f"was not exercised. Actions: {data.get('resolvedInteractions')!r}"
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
    """The marker must appear in a tool result, not in the agent's own prose."""
    traj = json.loads(TRAJECTORY.read_text(encoding="utf-8"))
    texts: list[str] = []
    for step in traj.get("steps") or []:
        # ATIF stores tool results in the step's ``observation`` blocks, not
        # on the tool call itself — reading ``tool_calls[].result`` yields
        # nothing and makes this check silently vacuous.
        observation = step.get("observation") or {}
        for result in observation.get("results") or []:
            content = result.get("content")
            if isinstance(content, str) and content:
                texts.append(content)
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("text"):
                        texts.append(str(item["text"]))
    assert any("E2E_MARKER_OK" in t for t in texts), (
        "E2E_MARKER_OK never appeared in any tool result"
    )
    blob = json.dumps(traj)
    assert LITERAL not in blob


def test_negative_literal_must_not_appear_even_if_marker_present():
    blob = TRAJECTORY.read_text(encoding="utf-8")
    assert LITERAL not in blob
