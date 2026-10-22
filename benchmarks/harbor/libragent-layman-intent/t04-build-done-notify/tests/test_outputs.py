"""T04: the build must have run to completion."""

from pathlib import Path


def test_build_ran():
    log = Path("/app/build.log")
    assert log.exists(), "build.sh was not run to completion"
    assert "BUILD OK" in log.read_text(encoding="utf-8")
