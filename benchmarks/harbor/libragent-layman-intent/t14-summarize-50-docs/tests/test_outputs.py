"""T14: one merged summary that covers every document."""

from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_summary_covers_all_docs():
    summary = Path("/app/summary.md")
    assert summary.exists(), "missing /app/summary.md"
    text = summary.read_text(encoding="utf-8")
    missing = [f"doc{i:02d}" for i in range(1, 51) if f"doc{i:02d}" not in text]
    assert not missing, f"summary does not mention: {missing}"


def test_critical_path():
    """`__spawnSession` (last call succeeds)"""
    failures = grade_trajectory("T14", TRAJECTORY)
    assert not failures, failures
