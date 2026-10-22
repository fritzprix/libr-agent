"""T12: graded from the agent trajectory by the skill's critical path."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_critical_path():
    """`__runShell` with `validate_skill.py` (last call succeeds)"""
    failures = grade_trajectory("T12", TRAJECTORY)
    assert not failures, failures
