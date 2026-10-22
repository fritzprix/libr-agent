"""T17: graded from the agent trajectory by the skill's critical path."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_critical_path():
    """`__createAgent` (last call succeeds)"""
    failures = grade_trajectory("T17", TRAJECTORY)
    assert not failures, failures
