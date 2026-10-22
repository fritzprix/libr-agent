"""T14: one merged summary that covers every document."""

from pathlib import Path


def test_summary_covers_all_docs():
    summary = Path("/app/summary.md")
    assert summary.exists(), "missing /app/summary.md"
    text = summary.read_text(encoding="utf-8")
    missing = [f"doc{i:02d}" for i in range(1, 51) if f"doc{i:02d}" not in text]
    assert not missing, f"summary does not mention: {missing}"
