"""T04: the build must have run to completion."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_build_ran():
    log = Path("/app/build.log")
    assert log.exists(), "build.sh was not run to completion"
    assert "BUILD OK" in log.read_text(encoding="utf-8")


def test_critical_path():
    """`__spawnProcess` → `__waitForProcess` (last call succeeds)"""
    failures = grade_trajectory("T04", TRAJECTORY)
    assert not failures, failures
