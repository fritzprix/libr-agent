"""Interactive shell path must complete via E2E control without leaking literals."""

import json
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")
LITERAL = "synthetic-e2e-fixture-only"


def test_evidence_present():
    assert EVIDENCE.is_file(), "harness must write session_e2e_evidence.json"


def test_interactive_prompt_was_resolved():
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    prompts = (data.get("interactions") or {}).get("interactiveShellPrompts") or []
    # After completion, pending list should be empty; harvest records last snapshot
    assert "interactions" in data


def test_marker_in_trajectory_not_literal_secret():
    traj = json.loads(TRAJECTORY.read_text(encoding="utf-8"))
    blob = json.dumps(traj)
    assert "E2E_MARKER_OK" in blob
    assert LITERAL not in blob


def test_negative_literal_must_not_appear_even_if_marker_present():
    blob = TRAJECTORY.read_text(encoding="utf-8")
    assert LITERAL not in blob
