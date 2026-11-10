"""B-6: distinguish masked tool output vs logging contract (#2631)."""

import json
import os
from pathlib import Path

TRAJECTORY = Path("/logs/agent/session_e2e_evidence.json")
SYNTH = os.environ.get("LIBRAGENT_E2E_SYNTHETIC_SECRET", "synthetic-e2e-fixture-only")


def test_trajectory_masks_value_in_tool_results():
    traj_path = Path("/logs/agent/trajectory.json")
    blob = traj_path.read_text(encoding="utf-8")
    assert SYNTH not in blob, "synthetic literal must not appear in harvested trajectory"
    assert "***" in blob or "E2E_DEMO_TOKEN" in blob


def test_evidence_harvested():
    assert TRAJECTORY.is_file()
