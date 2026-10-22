"""T15: the one typo is fixed and nothing else changed."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


EXPECTED = "# Notes App\n\n## Installation\n\nRun `make install`.\n\n## Usage\n\nRun `make run`.\n"


def test_typo_fixed_only():
    text = Path("/app/README.md").read_text(encoding="utf-8")
    assert text.rstrip("\n") == EXPECTED.rstrip("\n")


def test_critical_path():
    """do it directly; no `__spawnSession`"""
    failures = grade_trajectory("T15", TRAJECTORY)
    assert not failures, failures
