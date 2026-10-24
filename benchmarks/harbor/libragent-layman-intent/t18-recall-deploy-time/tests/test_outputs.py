"""T18: graded from the agent trajectory by the skill's critical path."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_critical_path():
    """`__globFiles` with `pre_compaction_epoch_` → `__readFile` with `pre_compaction_epoch_2` (last call succeeds)"""
    failures = grade_trajectory("T18", TRAJECTORY)
    assert not failures, failures
