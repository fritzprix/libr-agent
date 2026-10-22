"""T15: the one typo is fixed and nothing else changed."""

from pathlib import Path

EXPECTED = "# Notes App\n\n## Installation\n\nRun `make install`.\n\n## Usage\n\nRun `make run`.\n"


def test_typo_fixed_only():
    text = Path("/app/README.md").read_text(encoding="utf-8")
    assert text.rstrip("\n") == EXPECTED.rstrip("\n")
